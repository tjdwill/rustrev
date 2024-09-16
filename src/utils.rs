// Modules
mod tests;
// IMPORTS
use std::{
    convert::From,                  // Needed for seemless error conversions
    env,                            // Command-line argument handling 
    fs::{self, read, File, OpenOptions},
    io::{
        self, BufReader, Error as IOError, ErrorKind, 
        Read, Seek, Write, SeekFrom
    },
    os::unix::fs::{FileExt, PermissionsExt},          // Needed for offset-based I/O operatiosn
    path::{Path, PathBuf},          // File pathing things
    process::Command,               // needed for concatenation 
    str::{from_utf8, Utf8Error},    // enables raw bytes -> &str conversion
    string::FromUtf8Error,          // bytes -> String conversion error (if needed)
};

// CONSTANTS
const BUF_SIZE: usize = 1024;
const LINE_FILE_PREFIX: &str = "line";
const LINE_FILE_EXT: &str = ".txt";
const TMP_DIR: &str = ".revtmp";
const WORD_DELIM: &str = " ";
const WORD_DELIM_CHAR: char = ' ';
const WORKSPACE: &str = "workspace.txt";

// TYPE_ALIASES
pub type RevResult<T> = Result<T, RevError>;

// Structs/Traits
// Helper Funcs
pub fn reverse_data<S: Read, D: Write>(src: S, mut dest: D) -> RevResult<()> {
    let mut src = BufReader::with_capacity( BUF_SIZE, src,);
    let mut read_buf: [u8; BUF_SIZE] = [0; BUF_SIZE];
    let mut write_buf: [u8; BUF_SIZE] = [0; BUF_SIZE];
    // Create the temp directory, the word store, and the first tmp line.
    let tmp_dir = Path::new(TMP_DIR);
    match fs::create_dir(tmp_dir) {
        Err(err) => {
            eprintln!("Temp Directory already exists. Delete `{}`", tmp_dir.canonicalize()?.to_str().unwrap());
            return Err(RevError::IOError(err));
        }
        _ => ()
    }
    let mut word_store = OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(tmp_dir.join(WORKSPACE))?;
    let mut line_num = 0;
    let mut curr_line: File = make_line_file(line_num)?;
    // Process until the end of File
    loop {
        let bytes_read = src.read(&mut read_buf[..])?;
        // println!("READBUF: {:?}", &read_buf[..bytes_read]);
        if bytes_read == 0 {
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
         * the WORKSPACE file. Assuming this now completes the word, write the contents of this
         * file to the front of the current line (include a space). From here, we can clear the
         * word store, write the last word to the
         *
         */
        let word = from_utf8(&read_buf[..bytes_read])?;
        let mut word_iter = word.split(WORD_DELIM);
        // println!("SPLIT_str: {:?}", word_iter.clone().collect::<Vec<_>>());
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
        while let Some(mut lines) = word_iter.next().map(|s| s.split_inclusive("\n")) {
            // Iterate over lines (a given word may have newlines in them based on the previous
            // split operation)
            // println!("SPLIT_str: {:?}", lines.clone().collect::<Vec<_>>());
            while let Some(ln) = lines.next() {
                // If a newline is present, it is the last character of the string slice
                // println!("Payload: {:?}", ln);
                let mut ub = ln.len();
                let mut nl_present = false;
                if ln.contains("\n") {
                    nl_present = true;
                    // For reversed line ordering purposes, we don't want to include the newline; we add it later.
                    ub -= 1;
                }
                let data_to_write = &ln[..ub];
                // NOTE: Assume writing an empty buffer to file is not an error.
                // println!("Data to write: {:?}", data_to_write); 
                let _x = word_store.write(data_to_write.as_bytes())?;
                // println!("Bytes Written: {_x}");
                word_store.write(WORD_DELIM.as_bytes())?;
                word_store.flush()?;
                
                /*if last_line && last_word {
                    // Don't write to the current line
                    // Cause the loops to terminate
                    continue;
                }*/
                // Transfer data from word store to the current line file.
                // Since we don't know how large the word store is, keep track
                // of the number of bytes written and use it as the write offset.
                // write_buf.clear();
                word_store.rewind()?;
                let mut byte_offset = 0;
                while let Ok(bytes) = word_store.read(&mut write_buf[..]) {
                    if bytes == 0 {break;}
                    // println!("WriteBuf: {:?}", &write_buf[..bytes]);
                    insert_at(&mut curr_line, &write_buf[..bytes], byte_offset)?;
                    byte_offset += bytes as u64;
                }
                // println!("Byte OFFSET: {}", byte_offset);
                curr_line.flush()?;
                
                // Perform clean-up action(s)
                word_store.set_len(0)?;
                word_store.rewind()?;
                if nl_present {
                    // Close current line and create a new line file
                    // I am assuming that replacing the value in `curr_line` will cause the previous
                    // value to be dropped, thereby closing the associated file
                    if line_num > 0 {
                        curr_line.write("\n".as_bytes())?;
                    }
                    curr_line.flush()?;
                    line_num += 1;
                    curr_line = make_line_file(line_num)?;
                }
            }
            // NOTE: There's no work done past this point in the outer while-let loop.
            // The loop will terminate naturally once `last_line` and `last_word` are both true
        }
    }
    // Write the last newline character in order to have proper line ordering.
    //curr_line.write("\n".as_bytes())?;
    //curr_line.flush()?;
    { 
        // Perform a move in order to ensure the resources are closed
        // before deleting the entire temp directory
        let _ln = curr_line;
        let _x = word_store;
    }

    // == Concatenate lines ==
    // How do I collect all of the file paths?
    let file_paths = (0..=line_num).rev()
        .map(|x| format!("{}_{:05}{}", LINE_FILE_PREFIX, x, LINE_FILE_EXT))
        .collect::<Vec<_>>();

    // I don't know how to pipe the child process output into a file, so this 
    // reads the total data into memory anyway...
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

// Creates a read-write file representing a line
pub fn make_line_file(line_num: usize) -> io::Result<File> {
    let dir: &Path = Path::new(TMP_DIR);
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(dir.join(format!("{}_{:05}{}", LINE_FILE_PREFIX, line_num, LINE_FILE_EXT)))
}

fn buf_end(buf: &[u8]) -> &[u8] {
    let mut idx = 0;
    for x in buf {
        if *x == 0 {
            break;
        } else {
            idx += 1;
        }
    }
    if idx == 0 {
        &[]
    } else {
        &buf[..idx]
    }
}

/// Prepends data to the front of the specified file.
pub fn insert(f: &mut File, data: &[u8],) -> io::Result<()> {
    insert_at(f, data, 0) 
}
/// Inserts data to the specified file at the given byte offset.
pub fn insert_at(f: &mut File, data: &[u8], offset: u64,) -> io::Result<()> {
    /*
     * Since files can't really be appended to, we need to use a copy.
     * 
     * 1. Create a copy Y of the file X
     * 2. Determine *where* you want to insert the new data (byte offset).
     * 3. From index range 0..byte_offset, leave the file alone. The data is already in the
     *    proper position. 
     * 4. Write the data to file X (the original). Ensure the cursor is immediately after the last
     *    written byte.
     * 5. Write the rest of file Y (the copy of the unmodified original) to file X.
     * 6. Delete file Y
    */
    // println!("<insert_at>: Data to Write: {:?}", data);
    const TMP_NAME: &str  = "__file_swap.tmp";
    // Permission validation
    const USR_RW: u32 = 0o600;
    let permissions = f.metadata()?.permissions();
    if permissions.mode() < USR_RW {
        return Err(IOError::new(ErrorKind::PermissionDenied, "Must have user-level R/W permissions for this operation"))
    }

    // Set-up
    assert_eq!(0, f.seek(SeekFrom::Start(0))?);  // set the cursor to the head of the file
    const BUF_SIZE: usize = 4096;  // a page?
    let mut wbuf: [u8; BUF_SIZE] = [0; BUF_SIZE] ;
    let tmpfile_path: &Path = Path::new(TMP_NAME);
    let mut tmp_file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(true)
        .open(tmpfile_path)?;
    let _bytes_copied = io::copy(f, &mut tmp_file)?; 
    tmp_file.flush()?;
    // println!("<insert_at> NumBytesCopied: {}", bytes_copied);
    // Now, we have an identical copy of the original file `f`.
    // Overwrite the original file (we write to the original because we have no information on the
    // file's path)
    // All bytes before the insertion point remain in the same position, so we can skip them.

    // Transfer data payload
    let tail_begin = offset;
    assert_eq!(offset, tmp_file.seek(SeekFrom::Start(tail_begin))?);
    assert_eq!(offset, f.seek(SeekFrom::Start(tail_begin))?);
    // println!("<insert_at> Cursor: {}", offset);
    f.write_all(data)?; 
    // write the tail to `f`
    let mut success = false;
    while let Ok(cnt) = tmp_file.read(&mut wbuf[..], ) {
        // println!("<insert_at>: BytesRead from tmpfile ({}, {:?})", cnt, &wbuf[..cnt]);
        if cnt == 0 {
            success = true;
            break;
        }
        f.write_all(&wbuf[..cnt])?;
    }     
    if !success {
        Err(
            IOError::new(ErrorKind::Other, format!("Could not read from tmp file: {}", tmpfile_path.canonicalize()?.to_str().expect("<insert_at>: Canonicalize -> &str should always succeed for tmp file.")))
        )
    } else { 
        std::fs::remove_file(tmpfile_path)?;
        Ok(())
    }
}

/// Given a printable text file, this function partitions the file based on newline delimiting.
/// Each line is its own file within the temporary directory made with the value of TMP_DIR.
fn segment_file(f: &mut File) -> RevResult<()> {
    // Validation
    const USR_RW: u32 = 0o600;
    const NEWLINE: char = '\n';
    let permissions = f.metadata()?.permissions();
    if permissions.mode() < USR_RW {
        return Err(
            RevError::IOError(
                IOError::new(ErrorKind::PermissionDenied, "Must have user-level R/W permissions for this operation")
            )
        )
    }
    // Initialize buffers, workspace, and line_file
    let mut read_buf = [0_u8; BUF_SIZE];
    let mut line_num = 0;
    let tmp_dir = Path::new(TMP_DIR);
    match fs::create_dir(tmp_dir) {
        Err(err) => {
            eprintln!("Temp Directory already exists. Delete `{}`", tmp_dir.canonicalize()?.to_str().unwrap());
            return Err(RevError::IOError(err));
        }
        _ => ()
    }
    let mut curr_line = make_line_file(line_num)?; 
    f.rewind()?;
    // Read through the file, segmenting the data by newline.
    loop {
        let bytes_read = f.read(&mut read_buf[..])?;
        if bytes_read == 0 {
            break;
        }
        let buf_str = from_utf8(&read_buf[..bytes_read])?;
        let mut line_ended = false;
        for line in buf_str.split_inclusive(NEWLINE) {
            if line_ended {
                line_num += 1;
                curr_line = make_line_file(line_num)?;
                line_ended = false;
            }
            let mut ub = line.len();
            if line.contains(NEWLINE) {
                // split_inclusive guarantees that *if* a `&str` has a newline, the newline is the
                // last byte
                // Don't include newline in file.
                ub -= 1;
                line_ended = true;
            }
            let payload = &line[..ub]; 
            curr_line.write_all(payload.as_bytes())?;
        }
    }
    Ok(())
}

#[derive(Debug)]
pub enum RevError {
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
