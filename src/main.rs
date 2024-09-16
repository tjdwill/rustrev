#![allow(unused_imports, dead_code)]

/// Reverse File
/// Given an input source, read in the data and reverse the word order
/// (but not the words themselves).
///
/// Usage: rev [<src> [<dest>]]
use std::{
    convert::From,                  // Needed for seemless error conversions
    env,                            // Command-line argument handling 
    fs::{self, read, File, OpenOptions},
    io::{
        self, Error as IOError, ErrorKind, prelude::*
    },
    path::{Path, PathBuf},          // File pathing things
};
use rev::utils::*;

const BUF_SIZE: usize = 1024;
const LINE_FILE_PREFIX: &str = "line";
const LINE_FILE_EXT: &str = ".txt";
const TMP_DIR: &str = ".revtmp";
const WORD_DELIM: &str = " ";
const WORD_DELIM_CHAR: char = ' ';
const WORD_STORE: &str = "word.txt";

// TODO: Rewrite to call different function based on number of args.
fn main() -> RevResult<()> {
    // command line argument parsing
    let mut stdin_mode = false;
    if env::args().count() < 3 {
        stdin_mode = true;
    }
    let ret = { 
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
                let src = File::open(src)?;
                let dest = args.next().unwrap();
                let dest = File::create(dest)?;
                reverse_data(src, dest)
            }
            _ => {
                eprintln!("Too many arguments. Usage: rev [<src> [<dest>]]");
                Err(RevError::ExcessArguments)
            }
        }
    };
    match ret {
        Ok(()) => {
            if stdin_mode {
                io::stdout().write("\n".as_bytes())?;
            }
            Ok(())
        }
        Err(err) => Err(err)
    }
}

