/// A function that allows me to insert data at the beginning of a file.
use std::{
    fs::{File, OpenOptions},
    io::{self, prelude::*, Error, ErrorKind, Result, SeekFrom},
    os::unix::fs::PermissionsExt,
    path::Path,
};

fn main() -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .read(true)
        .open("insert_src.txt")?;
    insert("<HeadInsertion>".as_bytes(), &mut file)?;
    insert_at("|INSERTED|".as_bytes(), 7, &mut file)?;
    Ok(())
}


fn insert(data: &[u8], f: &mut File) -> Result<()> {
    insert_at(data, 0, f)
}
fn insert_at(data: &[u8], offset: u64, f: &mut File) -> Result<()> {
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
    const TMP_NAME: &str  = "__file_swap.tmp";
    // Permission validation
    const USR_RW: u32 = 0o600;
    let permissions = f.metadata()?.permissions();
    if permissions.mode() < USR_RW {
        return Err(Error::new(ErrorKind::PermissionDenied, "Must have user-level R/W permissions for this operation"))
    }

    // Set-up
    f.seek(SeekFrom::Start(0))?;  // set the cursor to the head of the file
    const BUF_SIZE: usize = 4096;  // a page?
    let mut wbuf: [u8; BUF_SIZE] = [0; BUF_SIZE] ;
    let tmpfile_path: &Path = Path::new(TMP_NAME);
    let mut tmp_file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .truncate(true)
        .open(tmpfile_path)?;
    io::copy(f, &mut tmp_file)?; 
    // Now, we have an identical copy of the original file `f`.
    // Overwrite the original file (we write to the original because we have no information on the
    // file's path)
    // All bytes before the insertion point remain in the same position, so we can skip them.
    let tail_begin = offset;
    assert_eq!(offset, tmp_file.seek(SeekFrom::Start(tail_begin))?);
    assert_eq!(offset, f.seek(SeekFrom::Start(tail_begin))?);
    f.write_all(data)?; 
    // write the tail to `f`
    let mut success = false;
    while let Ok(cnt) = tmp_file.read(&mut wbuf[..], ) {
        if cnt == 0 {
            success = true;
            break;
        }
        f.write_all(&wbuf[..cnt])?;
    }     
    if !success {
        Err(
            Error::new(ErrorKind::Other, format!("Could not read from tmp file: {}", tmpfile_path.canonicalize()?.to_str().expect("<insert_at>: Canonicalize -> &str should always succeed for tmp file.")))
        )
    } else { 
        std::fs::remove_file(tmpfile_path)?;
        Ok(())
    }
}
