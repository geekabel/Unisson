//! Audio engine of Unisson, built on GStreamer.
//!
//! The crate exposes two levels:
//!
//! - [`Player`] plays one address at a time, a local file or a network
//!   stream. It hides GStreamer behind a few plain methods (load, play,
//!   pause, stop, volume). Its methods are called directly, on your thread.
//! - [`Engine`] runs a [`Player`] on its own thread. Commands return at
//!   once, and what happens comes back as [`EngineEvent`]s. This is what the
//!   application uses, so the interface never waits for audio work.
//!
//! # System requirements
//!
//! GStreamer 1.14 or newer and its plugins must be installed. On Ubuntu:
//!
//! ```sh
//! sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
//!     gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
//!     gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav
//! ```
//!
//! # Examples
//!
//! Playing a file until it ends:
//!
//! ```no_run
//! use std::time::Duration;
//! use unisson_audio::{Player, PlayerEvent, file_uri};
//!
//! let player = Player::new()?;
//! player.load(&file_uri("/home/me/Music/song.flac")?)?;
//! player.play()?;
//! loop {
//!     match player.poll_event(Duration::from_millis(200)) {
//!         Some(PlayerEvent::EndOfStream) => break,
//!         Some(PlayerEvent::Error(message)) => return Err(message.into()),
//!         None => {}
//!     }
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod engine;
mod error;
mod player;

pub use engine::{Engine, EngineConfig, EngineEvent};
pub use error::AudioError;
pub use player::{PlaybackState, Player, PlayerEvent, file_uri};

/// Re-export of the GStreamer bindings used by this crate.
///
/// Only needed for advanced uses such as [`Player::with_audio_sink`].
pub use gst;
