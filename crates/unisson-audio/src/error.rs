//! Errors reported by the audio engine.

use std::error::Error;
use std::fmt;

/// What can go wrong when driving the [`Player`](crate::Player).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    /// GStreamer could not be initialized.
    Init(String),
    /// A required GStreamer element is not installed.
    MissingElement(&'static str),
    /// A playback command was sent before any address was loaded.
    NothingLoaded,
    /// GStreamer refused to change the playback state.
    StateChange(String),
    /// The file path cannot be turned into a playable address.
    InvalidPath(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Init(detail) => write!(f, "cannot initialize GStreamer: {detail}"),
            Self::MissingElement(name) => write!(f, "missing GStreamer element: {name}"),
            Self::NothingLoaded => write!(f, "nothing is loaded"),
            Self::StateChange(detail) => write!(f, "cannot change playback state: {detail}"),
            Self::InvalidPath(detail) => write!(f, "invalid file path: {detail}"),
        }
    }
}

impl Error for AudioError {}
