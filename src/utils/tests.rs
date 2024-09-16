#![cfg(test)]
use std::{
    fs::File,
    path::{PathBuf,},
};
use super::{
    segment_file, RevResult
};

#[test]
fn segment_file_test() -> RevResult<()> {
    let this_file = PathBuf::from(file!()).canonicalize()?;
    println!("{:?}", this_file.parent());
    let in_file = this_file.parent().unwrap().join("resources/segtest.txt");
    println!("FILE: {:?}", in_file);
    let mut f = File::open(in_file)?;
    segment_file(&mut f)
}
