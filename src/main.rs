#![allow(unused_imports, dead_code)]

use rev::utils::*;
/// Reverse File
/// Given an input source, read in the data and reverse the word order
/// (but not the words themselves).
///
/// Usage: rev [<src> [<dest>]]
use std::{
    convert::From, // Needed for seemless error conversions
    env,           // Command-line argument handling
    fs::{self, read, File, OpenOptions},
    io::{self, prelude::*, Error as IOError, ErrorKind},
    path::{Path, PathBuf}, // File pathing things
};

const BUF_SIZE: usize = 1024;
const LINE_FILE_PREFIX: &str = "line";
const LINE_FILE_EXT: &str = ".txt";
const TMP_DIR: &str = ".revtmp";
const WORD_DELIM: &str = " ";
const WORD_DELIM_CHAR: char = ' ';
const WORD_STORE: &str = "word.txt";

fn main() -> RevResult<()> {
    // command line argument parsing
    let mut stdin_mode = false;
    if env::args().count() < 3 {
        stdin_mode = true;
    }
    let ret = {
        match env::args().count() {
            1 => {
                let mut src = io::stdin();
                reverse_data(&mut src, io::stdout())
            }
            2 => {
                let src = env::args().nth(1).unwrap();
                let src = Path::new(&src).canonicalize()?;
                let mut src_file = File::open(src)?;
                reverse_data(&mut src_file, io::stdout())
            }
            3 => {
                let mut args = env::args();
                args.next();
                let src = args.next().unwrap();
                let dest = args.next().unwrap();

                if dest == src {
                    eprintln!("Source and Destination cannot be the same.");
                    return Err(RevError::ArgumentError);
                }
                let src = Path::new(&src).canonicalize()?;
                let mut src = File::open(src)?;
                let dest = File::create(dest)?;
                reverse_data(&mut src, dest)
            }
            _ => {
                eprintln!("Too many arguments. Usage: rev [<src> [<dest>]]");
                Err(RevError::ArgumentError)
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
        Err(err) => Err(err),
    }
}
