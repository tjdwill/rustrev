#![cfg(test)]
use super::{make_line_file, reverse_word_order, segment_file, RevResult};
use std::{fs::File, path::PathBuf};

#[test]
/// Because this is run as a test, the current working directory is set to the
/// project's root directory
fn segment_file_test() -> RevResult<()> {
    let this_file = PathBuf::from(file!()).canonicalize()?;
    println!("{:?}", this_file.parent());
    let in_file = this_file.parent().unwrap().join("resources/segtest.txt");
    println!("FILE: {:?}", in_file);
    let mut f = File::open(in_file)?;
    segment_file(&mut f)?;
    Ok(())
}

#[test]
fn test_reverse_word_order() -> RevResult<()> {
    use super::{FILE_EXT, LINE_FILE_PREFIX, TMP_DIR};
    // File setup
    let this_file = PathBuf::from(file!()).canonicalize()?;
    // the crate root directory is 3 levels up from this file.
    let tmp_dir = this_file
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(TMP_DIR);
    let mut src_file = File::open(this_file.parent().unwrap().join("resources/segtest.txt"))?;
    for i in 0..segment_file(&mut src_file)? {
        let mut line_file = make_line_file(i, LINE_FILE_PREFIX, FILE_EXT)?;
        reverse_word_order(&mut line_file, i)?;
    }
    Ok(())
}
