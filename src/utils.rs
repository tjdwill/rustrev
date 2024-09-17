// :;Modules::
mod tests;
// ::Imports::
use std::{
    convert::From, // Needed for seemless error conversions
    fs::{self, File, OpenOptions},
    io::{self, BufReader, Error as IOError, ErrorKind, Read, Seek, SeekFrom, Write},
    os::unix::fs::PermissionsExt, // Needed for Unix permissions polling
    path::Path,                   // File pathing things
    process::Command,             // needed for concatenation
    str::{from_utf8, Utf8Error},  // enables raw bytes -> &str conversion
    string::FromUtf8Error,        // bytes -> String conversion error (if needed)
};

// ::Constants::
const BUF_SIZE: usize = 1024;
const FILE_EXT: &str = ".txt";
const LINE_FILE_PREFIX: &str = "line";
const REV_FILE_PREFIX: &str = "revline";
const TMP_DIR: &str = ".revtmp";
const WORD_DELIM: &str = " ";
const WORKSPACE: &str = "workspace.txt";

// ::Type_aliases::
pub type RevResult<T> = Result<T, RevError>;

// ::Structs/Traits::
// ::Helper Funcs::
pub fn reverse_data<S: Read, D: Write>(src: &mut S, mut dest: D) -> RevResult<()> {
    let num_lines = segment_file(src)?;
    for i in 0..num_lines {
        let mut line_file = make_line_file(i, LINE_FILE_PREFIX, FILE_EXT)?;
        reverse_word_order(&mut line_file, i)?;
        // Add newline
        if i > 0 {
            let mut rev_f = make_line_file(i, REV_FILE_PREFIX, FILE_EXT)?;
            rev_f.seek(SeekFrom::End(0))?;
            rev_f.write_all("\n".as_bytes())?;
            rev_f.flush()?;
        }
    }
    // == Concatenate lines ==
    // How do I collect all of the file paths?
    let file_paths = (0..num_lines)
        .rev()
        .map(|x| format!("{REV_FILE_PREFIX}_{:05}{}", x, FILE_EXT))
        .collect::<Vec<_>>();
    /*
     * println!("Cat Args: {:?}", file_paths);
    for p in &file_paths {
        println!("{}", p);
    }
    */
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
    fs::remove_dir_all(TMP_DIR)?;
    // Return
    Ok(())
}

/// Prepends data to the front of the specified file.
pub fn insert(f: &mut File, data: &[u8]) -> io::Result<()> {
    insert_at(f, data, 0)
}

/// Inserts data to the specified file at the given byte offset.
pub fn insert_at(f: &mut File, data: &[u8], offset: u64) -> io::Result<()> {
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
    const TMP_NAME: &str = "__file_swap.tmp";
    // Permission validation
    const USR_RW: u32 = 0o600;
    let permissions = f.metadata()?.permissions();
    if permissions.mode() < USR_RW {
        return Err(IOError::new(
            ErrorKind::PermissionDenied,
            "Must have user-level R/W permissions for this operation",
        ));
    }

    // Set-up
    assert_eq!(0, f.seek(SeekFrom::Start(0))?); // set the cursor to the head of the file
    const BUF_SIZE: usize = 4096; // a page?
    let mut wbuf: [u8; BUF_SIZE] = [0; BUF_SIZE];
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
    while let Ok(cnt) = tmp_file.read(&mut wbuf[..]) {
        // println!("<insert_at>: BytesRead from tmpfile ({}, {:?})", cnt, &wbuf[..cnt]);
        if cnt == 0 {
            success = true;
            break;
        }
        f.write_all(&wbuf[..cnt])?;
    }
    if !success {
        Err(IOError::new(
            ErrorKind::Other,
            format!(
                "Could not read from tmp file: {}",
                tmpfile_path.canonicalize()?.to_str().expect(
                    "<insert_at>: Canonicalize -> &str should always succeed for tmp file."
                )
            ),
        ))
    } else {
        std::fs::remove_file(tmpfile_path)?;
        Ok(())
    }
}

// Creates a read-write file representing a line
pub fn make_line_file(line_num: u32, prefix: &str, ext: &str) -> io::Result<File> {
    let dir: &Path = Path::new(TMP_DIR);
    OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(dir.join(format!("{}_{:05}{}", prefix, line_num, ext)))
}

/// Given a line file created by `segment_file`, reverses the word order of the file
fn reverse_word_order(line_f: &mut File, line_num: u32) -> RevResult<()> {
    let mut read_buf = [0_u8; BUF_SIZE];
    let mut write_buf = [0_u8; BUF_SIZE];
    let tmp_dir = Path::new(TMP_DIR);
    if !tmp_dir.is_dir() {
        return Err(RevError::IOError(IOError::new(
            ErrorKind::NotFound,
            "Temp Directory not found.",
        )));
    }
    let mut workspace = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .open(tmp_dir.join(WORKSPACE))?;
    let mut rev_line = make_line_file(line_num, REV_FILE_PREFIX, FILE_EXT)?;
    line_f.rewind()?;
    let mut first_word = true;
    loop {
        let bytes_read = line_f.read(&mut read_buf[..])?;
        if bytes_read == 0 {
            // Write the word store to the line file and exit loop
            workspace.rewind()?;
            let mut bytes_offset = 0;
            while let Ok(bytes) = workspace.read(&mut write_buf[..]) {
                if bytes == 0 {
                    break;
                }
                insert_at(&mut rev_line, &write_buf[..bytes], bytes_offset)?;
                bytes_offset += bytes as u64;
            }
            // Insert spacing as needed
            if !first_word {
                insert_at(&mut rev_line, WORD_DELIM.as_bytes(), bytes_offset)?;
            }
            // Only reset the workspace so we don't keep recreating it on subsequent calls.
            // The entire tmp directory is removed at a higher function call.
            workspace.rewind()?;
            workspace.set_len(0)?;
            break;
        }

        let mut had_space = false;
        let buf_str = from_utf8(&read_buf[..bytes_read])?;
        for word in buf_str.split_inclusive(WORD_DELIM) {
            /*
             * println!(
                "<reverse_word_order>: Words: {:?}\tCurrent: {}",
                buf_str.split_inclusive(WORD_DELIM).collect::<Vec<_>>(),
                word
            );
            */
            if had_space {
                // insert word at head of revline file
                workspace.rewind()?;
                let mut bytes_offset = 0;
                while let Ok(bytes) = workspace.read(&mut write_buf[..]) {
                    if bytes == 0 {
                        break;
                    }
                    //let _payload = &write_buf[..bytes];
                    //println!("<reverse_word_order> To REV_LINE: {}", from_utf8(_payload)?);
                    insert_at(&mut rev_line, &write_buf[..bytes], bytes_offset)?;
                    bytes_offset += bytes as u64;
                }
                // Insert spacing as needed
                if !first_word {
                    insert_at(&mut rev_line, " ".as_bytes(), bytes_offset)?;
                } else {
                    first_word = false;
                }
                // Clean-up
                had_space = false;
                workspace.flush()?;
                workspace.rewind()?;
                workspace.set_len(0)?;
            }
            let mut ub = word.len();
            if word.contains(" ") {
                ub -= 1;
                had_space = true;
            }
            let payload = &word[..ub];
            //println!("<reverse_word_order> PAYLOAD: {}", payload);
            workspace.write_all(payload.as_bytes())?;
        }
    }
    rev_line.flush()?;
    Ok(())
}

/// Given a printable text file, this function partitions the file based on newline delimiting.
/// Each line is its own file within the temporary directory made with the value of TMP_DIR.
/// Returns the number of line files created
fn segment_file<R: Read>(src: &mut R) -> RevResult<u32> {
    // Validation
    /*
    const USR_RW: u32 = 0o600;
    let permissions = f.metadata()?.permissions();
    if permissions.mode() < USR_RW {
        return Err(RevError::IOError(IOError::new(
            ErrorKind::PermissionDenied,
            "Must have user-level R/W permissions for this operation",
        )));
    }*/
    const NEWLINE: char = '\n';
    let mut f = BufReader::with_capacity(BUF_SIZE, src);
    // Initialize buffers, workspace, and line_file
    let mut read_buf = [0_u8; BUF_SIZE];
    let mut line_num = 0;
    let tmp_dir = Path::new(TMP_DIR);
    match fs::create_dir(tmp_dir) {
        Err(err) => {
            eprintln!(
                "Temp Directory already exists. Delete `{}`",
                tmp_dir.canonicalize()?.to_str().unwrap()
            );
            return Err(RevError::IOError(err));
        }
        _ => (),
    }
    let mut curr_line = make_line_file(line_num, LINE_FILE_PREFIX, FILE_EXT)?;
    // f.rewind()?;
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
                curr_line = make_line_file(line_num, LINE_FILE_PREFIX, FILE_EXT)?;
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
    Ok(line_num + 1)
}

#[derive(Debug)]
pub enum RevError {
    ArgumentError,
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
