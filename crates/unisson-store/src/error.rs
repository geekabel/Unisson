//! Errors reported by the local storage.

use std::error::Error;
use std::fmt;

/// What can go wrong when reading or writing the local data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreError {
    /// The place where the data must live cannot be found or created.
    Location(String),
    /// The database refused an operation, or its content is damaged.
    Database(String),
    /// A value given by the caller cannot be stored.
    Invalid(String),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Location(detail) => write!(f, "cannot use the data location: {detail}"),
            Self::Database(detail) => write!(f, "database error: {detail}"),
            Self::Invalid(detail) => write!(f, "invalid value: {detail}"),
        }
    }
}

impl Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error.to_string())
    }
}
