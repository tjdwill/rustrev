#![allow(unused_imports, dead_code)]
/// Reverse File
/// Given an input source, read in the data and reverse the word order
/// (but not the words themselves).
///
/// Usage: rev [<src> [<dest>]]
use std::{
    convert::From,                  // Needed for seemless error conversions
    env,                            // Command-line argument handling 
    fs::{self, File, OpenOptions},
    io::{self, Error as IOError, Read, Write},
    os::unix::fs::FileExt,          // Needed for offset-based I/O operatiosn
    path::{Path, PathBuf},          // File pathing things
    process::Command,               // needed for concatenation 
    str::{from_utf8, Utf8Error},    // enables raw bytes -> &str conversion
    string::FromUtf8Error,          // bytes -> String conversion error (if needed)
};

const BUF_SIZE: usize = 1024;
const LINE_FILE_PREFIX: &str = "line";
const LINE_FILE_EXT: &str = ".txt";
const TMP_DIR: &str = ".revtmp";
const WORD_DELIM: &str = " ";
const WORD_DELIM_CHAR: char = ' ';
const WORD_STORE: &str = "word.txt";

type RevType<T> = Result<T, RevError>;


fn main() -> RevType<()> {
    // command line argument parsing
    match env::args().count() {
        1 => reverse_data(io::stdin(), io::stdout()),
        2 => {
            let src = env::args().nth(1).unwrap();
            let src = Path::new(&src).canonicalize()?;
            let src_file = File::open(src)?;
            reverse_data(src_file, io::stdout())
        }
        3 => {
            let mut args = env::args();
            args.next();
            let src = args.next().unwrap();
            let src = Path::new(&src).canonicalize()?;
            println!("{:?}", src);
            let src = File::open(src)?;
            let dest = args.next().unwrap();
            println!("{:?}", dest);
            let dest = File::create(dest)?;
            reverse_data(src, dest)
        }
        _ => {
            eprintln!("Too many arguments. Usage: rev [<src> [<dest>]]");
            Err(RevError::ExcessArguments)
        }
    }
}

fn reverse_data<S: Read, D: Write>(mut src: S, mut dest: D) -> RevType<()> {
    let mut read_buf: Vec<u8> = Vec::with_capacity(BUF_SIZE);
    let mut write_buf: Vec<u8> = Vec::with_capacity(BUF_SIZE);
    // Create the temp directory, the word store, and the first tmp line.
    let tmp_dir = Path::new(TMP_DIR);
    fs::create_dir(tmp_dir)?;
    let mut word_store = OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(tmp_dir.join(WORD_STORE))?;
    let mut line_num = 0;
    let mut curr_line: File = make_line_file(line_num)?;
    // Process until the end of File
    println!("Entering loop");
    loop {
        let bytes_read = src.read(&mut read_buf)?;
        println!("BytesRead: {}", bytes_read);
        if bytes_read == 0 && read_buf.len() == 0 {
            // EOF
            break;
        }
        /* 
         * The whitespace character 0x20 is considered a word boundary (delimiter).
         * In any sequence of form `<BOUND><CHARS><BOUND>`, CHARS is guaranteed to be a complete word.
         * However, we cannot say the same for forms `<CHARS><BOUND>` and `<BOUND><CHARS>`. In the
         * latter case, CHARS may be only part of a complete word (the rest couldn't fit in the
         * buffer). For the former, CHARS may be a continuation of a previous word—the remainder of
         * the `<BOUND><CHARS>` form.
         *
         * To combat this, the idea is to write the first "word" found via the split operation to
         * the WORD_STORE file. Assuming this now completes the word, write the contents of this
         * file to the front of the current line (include a space). From here, we can clear the
         * word store, write the last word to the
         *
         */
        let mut word_iter = from_utf8(&read_buf[..])?.split(WORD_DELIM).peekable();
        /* 
         * Since we know our buffer is non-zero in capacity, we can assume that if the code reaches
         * here, the read was successful. Otherwise, the EoF error would have triggered.
         * Under this assumption, we are guaranteed to have at least one word.
        */

        // Iterate over the words, writing each word to the front of the tmp file that represents the current line.
        // When we reach the last word in the iteration, DO NOT write it to the line file. We don't
        // know if this word is complete.
        // We iterate over "words" in the outer loop and lines in the inner loop in order to get
        // the correct newline behavior. The more intuitive order doesn't actually work in this
        // case.
        let mut last_word = false;
        let mut last_line = false;
        while let Some(mut lines) = word_iter.next().map(|s| s.split_inclusive("\n").peekable()) {
            if let None = word_iter.peek() {
                last_word = true;
            }
            while let Some(ln) = lines.next() {
                if let None = lines.peek() {
                    last_line = true;
                }
                // If a newline is present, it is the last character of the string slice
                let mut ub = ln.len();
                let mut nl_present = false;
                if ln.contains("\n") {
                    nl_present = true;
                    // For reversed line ordering purposes, we don't want to include the newline; we add it later.
                    ub -= 1;
                }
                let data_to_write = &ln[..ub];
                // NOTE: Assume writing an empty buffer to file is not an error.
                word_store.write(data_to_write.as_bytes())?;
                word_store.flush()?;
                
                if last_line && last_word {
                    // Don't write to the current line
                    // Cause the loops to terminate
                    continue;
                }
                // Transfer data from word store to the current line file.
                // Since we don't know how large the word store is, keep track
                // of the number of bytes written and use it as the write offset.
                write_buf.clear();
                let mut write_offset = 0;
                while let Ok(bytes) = word_store.read(&mut write_buf) {
                    if bytes == 0 {break;}
                    curr_line.write_all_at(&write_buf[..], write_offset)?;
                    write_offset += bytes as u64;
                    write_buf.clear();
                }
                curr_line.write_at(WORD_DELIM.as_bytes(), write_offset)?;
                
                // Perform clean-up action(s)
                word_store.set_len(0)?;
                if nl_present {
                    // Close current line and create a new line file
                    // I am assuming that replacing the value in `curr_line` will cause the previous
                    // value to be dropped, thereby closing the associated file
                    curr_line.flush()?;
                    line_num += 1;
                    curr_line = make_line_file(line_num)?;
                }
            }
            // NOTE: There's no work done past this point in the outer while-let loop.
            // The loop will terminate naturally once `last_line` and `last_word` are both true
        }
        read_buf.clear();
    }

    { 
        // Perform a move in order to ensure the resources are closed
        // before deleting the entire temp directory
        println!("Dropping Resources");
        let _ln = curr_line;
        let _x = word_store;
    }

    // == Concatenate lines ==
    // How do I collect all of the file paths?
    let file_paths = (0..=line_num).rev()
        .map(|x| format!("{}_{:02}{}", LINE_FILE_PREFIX, x, LINE_FILE_EXT))
        .collect::<Vec<_>>();

    // I don't know how to pipe the child process output into a file, so this 
    // reads the total data into memory anyway...
    println!("LinesRead: {}", line_num);
    {
        let cat_output = Command::new("cat")
            .current_dir(Path::new(TMP_DIR).canonicalize()?)
            .args(file_paths)
            .output()
            .expect("failed to execute cat");
        if cat_output.status.success() {
            // Write to destination
           dest.write_all(&cat_output.stdout)?;
           dest.flush()?;
        } else {
           return Err(RevError::ChildProcessError);
        }
    }
    // Clean-up
    //fs::remove_dir_all(TMP_DIR)?;
    // Return
    Ok(())
}

fn make_line_file(line_num: usize) -> io::Result<File> {
    let dir: &Path = Path::new(TMP_DIR);
    File::create(dir.join(format!("{}_{:02}{}", LINE_FILE_PREFIX, line_num, LINE_FILE_EXT)))
}

#[derive(Debug)]
enum RevError {
    ExcessArguments,
    ChildProcessError,
    IOError(IOError),
    EncodingError(Utf8Error),
}
impl From<IOError> for RevError {
    fn from(err: IOError) -> Self {
        Self::IOError(err)
    }
}
impl From<FromUtf8Error> for RevError {
    fn from(err: FromUtf8Error) -> Self {
        Self::EncodingError(err.utf8_error())
    }
}
impl From<Utf8Error> for RevError {
    fn from(err: Utf8Error) -> Self {
        Self::EncodingError(err)
    }
}
