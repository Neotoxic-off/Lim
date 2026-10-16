//! User-facing errors, worded like Vim's.

use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub enum Error {
    NoFileName,
    FileExists(PathBuf),
    NotUtf8(PathBuf),
    NotAFile(PathBuf),
    Io(PathBuf, io::Error),
    Unsaved(String),
    NotACommand(String),
    PatternNotFound(String),
    NoPattern,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoFileName => write!(f, "E32: No file name"),
            Self::FileExists(p) => write!(f, "E13: File exists (add ! to override): {}", p.display()),
            Self::NotAFile(p) => write!(f, "\"{}\" is not a regular file", p.display()),
            Self::NotUtf8(p) => write!(f, "\"{}\" is not valid UTF-8, not opened", p.display()),
            Self::Io(p, e) => write!(f, "\"{}\": {e}", p.display()),
            Self::Unsaved(name) => {
                write!(f, "E37: No write since last change for buffer \"{name}\" (add ! to override)")
            }
            Self::NotACommand(c) => write!(f, "E492: Not an editor command: {c}"),
            Self::PatternNotFound(p) => write!(f, "E486: Pattern not found: {p}"),
            Self::NoPattern => write!(f, "E35: No previous regular expression"),
        }
    }
}

impl std::error::Error for Error {}
