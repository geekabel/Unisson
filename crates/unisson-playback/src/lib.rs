//! Playback session of Unisson.
//!
//! The [`Session`] is what the user interface talks to. It ties together
//! three parts that know nothing about each other:
//!
//! - the [`PlayQueue`](unisson_core::PlayQueue), which orders the media;
//! - the sources, which turn a media into an address to play;
//! - the audio [`Engine`](unisson_audio::Engine), which plays an address,
//!   or a [`RemotePlayer`] for media only another application can play.
//!
//! The interface sends short orders ("play this list", "next", "pause") and
//! receives [`SessionEvent`]s. It never waits: every order returns at once.
//!
//! This crate has no user interface code, so it can be tested without a
//! window.

mod output;
mod session;
mod snapshot;

pub use output::{EventSink, Output, RemotePlayer, Router};

pub use session::{Resolver, Session, SessionEvent};
pub use snapshot::{ErrorKind, PlaybackError, QueueView, Snapshot, Status};
