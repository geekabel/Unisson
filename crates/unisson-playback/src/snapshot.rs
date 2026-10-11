//! What the session shows to the outside: its state at one moment.

use std::time::Duration;

use unisson_core::{Media, SourceError};

/// What the session is doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Nothing plays. A media may still be selected, ready to start again.
    Stopped,
    /// A media was chosen, and its sound has not started yet.
    Loading,
    /// Sound is coming out.
    Playing,
    /// Playback is suspended and keeps its position.
    Paused,
    /// A live stream was lost, and the engine is connecting again.
    Reconnecting {
        /// Number of this attempt, starting at 1.
        attempt: usize,
        /// Number of attempts made before giving up.
        max_attempts: usize,
    },
}

/// Why a playback failed, in broad terms.
///
/// The interface picks its message from the kind. The detail of a
/// [`PlaybackError`] is technical and meant for a "more information" area.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// The media no longer exists: a deleted file, a closed station.
    NotFound,
    /// The network or the remote service did not answer.
    Network,
    /// The source cannot be used right now.
    Unavailable,
    /// The address was found, but its sound cannot be played.
    Playback,
}

/// A playback failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackError {
    /// The broad reason.
    pub kind: ErrorKind,
    /// The technical message, in English.
    pub detail: String,
}

impl From<SourceError> for PlaybackError {
    fn from(error: SourceError) -> Self {
        let kind = match &error {
            SourceError::NotFound => ErrorKind::NotFound,
            SourceError::Network(_) => ErrorKind::Network,
            _ => ErrorKind::Unavailable,
        };
        Self {
            kind,
            detail: error.to_string(),
        }
    }
}

/// The state of the session at one moment.
#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    /// What the session is doing.
    pub status: Status,
    /// The selected media: the one playing, or the last one played.
    pub current: Option<Media>,
    /// The title a radio station announces for what is on air.
    pub stream_title: Option<String>,
    /// Time elapsed since the beginning of the media.
    pub position: Duration,
    /// Total duration, when it is known. Live streams have none.
    pub duration: Option<Duration>,
    /// The volume, from `0.0` to `1.0`.
    pub volume: f64,
    /// Whether a media was played before the current one.
    pub has_previous: bool,
    /// Whether a media is waiting after the current one.
    pub has_next: bool,
    /// Why the last playback failed. Cleared when a media starts.
    pub error: Option<PlaybackError>,
}

/// The play queue at one moment, as the outside sees it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QueueView {
    /// The selected media.
    pub current: Option<Media>,
    /// The media waiting to be played, the next one first.
    pub upcoming: Vec<Media>,
    /// What plays when the queue runs out, if anything. See
    /// [`Session::set_fallback`](crate::Session::set_fallback).
    pub fallback: Option<Media>,
}

impl Default for Snapshot {
    fn default() -> Self {
        Self {
            status: Status::Stopped,
            current: None,
            stream_title: None,
            position: Duration::ZERO,
            duration: None,
            volume: 1.0,
            has_previous: false,
            has_next: false,
            error: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_errors_map_to_a_kind() {
        let kind = |error: SourceError| PlaybackError::from(error).kind;
        assert_eq!(kind(SourceError::NotFound), ErrorKind::NotFound);
        assert_eq!(
            kind(SourceError::Network("down".into())),
            ErrorKind::Network
        );
        assert_eq!(
            kind(SourceError::Unavailable("off".into())),
            ErrorKind::Unavailable
        );
        assert_eq!(
            kind(SourceError::Invalid("x".into())),
            ErrorKind::Unavailable
        );
    }
}
