//! Errors a source can report.

use std::error::Error;
use std::fmt;

/// What can go wrong when querying a source.
///
/// The [`Display`](fmt::Display) output is a short lowercase message without
/// trailing punctuation, as is customary for Rust errors. The user interface
/// is expected to match on the variant to show a localized message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// The network or the remote server did not answer properly.
    Network(String),
    /// The requested media does not exist, or no longer exists, in the source.
    NotFound,
    /// The source cannot answer: missing tool, source not registered, crash.
    Unavailable(String),
    /// The request or the response is malformed.
    Invalid(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(detail) => write!(f, "network error: {detail}"),
            Self::NotFound => write!(f, "media not found"),
            Self::Unavailable(detail) => write!(f, "source unavailable: {detail}"),
            Self::Invalid(detail) => write!(f, "invalid data: {detail}"),
        }
    }
}

impl Error for SourceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_readable() {
        let error = SourceError::Network("timed out".into());
        assert_eq!(error.to_string(), "network error: timed out");
        assert_eq!(SourceError::NotFound.to_string(), "media not found");
    }

    #[test]
    fn works_as_a_standard_error() {
        let boxed: Box<dyn Error> = Box::new(SourceError::NotFound);
        assert_eq!(boxed.to_string(), "media not found");
    }
}
