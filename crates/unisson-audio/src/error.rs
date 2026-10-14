//! Errors reported by the audio engine.

use std::error::Error;
use std::fmt;

/// Why a playback failed, in broad terms.
///
/// GStreamer explains a failure with a sentence meant for developers. An
/// interface needs less and more: which of a few situations this is, to
/// pick the right words and the right way out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FailureKind {
    /// The address leads nowhere: a deleted file, a page that is gone.
    NotFound,
    /// The server could not be reached, or the connection was lost.
    Network,
    /// The data arrived, but it is not audio this engine can decode.
    Unsupported,
    /// Anything else.
    Other,
}

/// A playback failure: its broad kind, and the technical message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The broad reason.
    pub kind: FailureKind,
    /// The technical message, in English or in the language of the
    /// desktop. It usually comes from GStreamer.
    pub message: String,
}

impl Failure {
    /// Creates a failure of the [`FailureKind::Other`] kind.
    pub fn other(message: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Other,
            message: message.into(),
        }
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for Failure {}

/// What can go wrong when driving the [`Player`](crate::Player).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioError {
    /// GStreamer could not be initialized.
    Init(String),
    /// A required GStreamer element is not installed.
    MissingElement(&'static str),
    /// A playback command was sent before any address was loaded.
    NothingLoaded,
    /// GStreamer refused to change the playback state. The payload says
    /// why.
    StateChange(Failure),
    /// The file path cannot be turned into a playable address.
    InvalidPath(String),
    /// The playback position cannot be changed. The payload says why.
    Seek(String),
    /// The audio thread of the [`Engine`](crate::Engine) is no longer running.
    EngineStopped,
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Init(detail) => write!(f, "cannot initialize GStreamer: {detail}"),
            Self::MissingElement(name) => write!(f, "missing GStreamer element: {name}"),
            Self::NothingLoaded => write!(f, "nothing is loaded"),
            Self::StateChange(detail) => write!(f, "cannot change playback state: {detail}"),
            Self::InvalidPath(detail) => write!(f, "invalid file path: {detail}"),
            Self::Seek(detail) => write!(f, "cannot change the playback position: {detail}"),
            Self::EngineStopped => write!(f, "the audio engine has stopped"),
        }
    }
}

impl Error for AudioError {}

impl AudioError {
    /// Describes this error as a playback failure, with its broad kind.
    pub fn failure(&self) -> Failure {
        let kind = match self {
            Self::StateChange(failure) => failure.kind,
            Self::InvalidPath(_) => FailureKind::NotFound,
            _ => FailureKind::Other,
        };
        Failure {
            kind,
            message: self.to_string(),
        }
    }
}
