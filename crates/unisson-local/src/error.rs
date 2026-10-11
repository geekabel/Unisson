//! Errors reported when reading local files.

use std::error::Error;
use std::fmt;

/// What can go wrong when reading local audio files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocalError {
    /// A file or a folder cannot be opened or listed.
    Io(String),
    /// A file is not a readable audio file: unknown format or damaged content.
    Unreadable(String),
}

impl fmt::Display for LocalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(detail) => write!(f, "cannot access the file system: {detail}"),
            Self::Unreadable(detail) => write!(f, "not a readable audio file: {detail}"),
        }
    }
}

impl Error for LocalError {}
