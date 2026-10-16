//! The built-in playback: Spotify played inside Unisson, through
//! [librespot](https://github.com/librespot-org/librespot).
//!
//! librespot is an open-source Spotify client. It signs in like the
//! Spotify desktop application, fetches the audio, decodes it, and hands
//! the samples to a small GStreamer pipeline of Unisson. Spotify
//! does not endorse it: it may stop working for some accounts, or for a
//! while. The [`SpotifyPlayer`](crate::SpotifyPlayer) then falls back on
//! the official Spotify application.

mod player;
mod sink;

pub use player::{BuiltinPlayer, Failed, Issue};
pub use sink::Volume;
