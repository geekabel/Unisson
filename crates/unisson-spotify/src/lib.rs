//! Spotify source of Unisson.
//!
//! # Design
//!
//! Spotify plays in two ways:
//!
//! - **inside Unisson**, by default: [`builtin`] signs in like the Spotify
//!   desktop application and plays the audio itself, through librespot.
//!   One sign-in, nothing else to install. Spotify does not endorse it,
//!   and refuses it on some accounts or for a while;
//! - **in the official Spotify application**, as a backup: Unisson drives
//!   it through the [Web API](https://developer.spotify.com/documentation/web-api),
//!   with an application the user registers on the Spotify developer site.
//!   An application in development mode only serves the few accounts its
//!   owner lists, so each user registers their own: see [`auth`].
//!
//! [`SpotifyPlayer`] is the remote player the playback session hands
//! `spotify:track:...` addresses to. It plays inside Unisson, and moves to
//! the official application when the built-in playback fails for a known
//! reason, if the backup is set up. Both ways need Spotify Premium.
//!
//! [`SpotifySource`] searches the catalog like the other sources, and turns
//! a track into a `spotify:track:...` address. [`Spotify`] holds the
//! connection, shared by all, and the [`PlaybackStatus`] the interface
//! shows.

pub mod api;
pub mod auth;
pub mod builtin;
mod connection;
mod credentials;
mod player;
mod remote;
mod source;
mod status;
#[cfg(test)]
mod test_server;

pub use api::{Device, PlaybackState, SpotifyApi, Track};
pub use connection::Spotify;
pub use credentials::Credentials;
pub use player::{
    Decision, RETRY_WITH_BACKUP, RETRY_WITHOUT_BACKUP, Situation, SpotifyPlayer, after_failure,
    choose_route,
};
pub use remote::SpotifyRemote;
pub use source::SpotifySource;
pub use status::{PlaybackStatus, Problem, Route};

/// The scheme of the addresses handed to the remote player.
pub const SCHEME: &str = "spotify:";

/// Tells whether a text is a Spotify identifier: 22 letters and digits.
///
/// Identifiers become parts of addresses: nothing else is accepted.
pub fn is_spotify_id(text: &str) -> bool {
    text.len() == 22 && text.chars().all(|letter| letter.is_ascii_alphanumeric())
}

/// Returns the identifier of a track from its address, such as
/// `spotify:track:6rqhFgbbKwnb9MLmUQDhG6`, or from its web link, such as
/// `https://open.spotify.com/track/6rqhFgbbKwnb9MLmUQDhG6?si=...`.
pub fn track_id(text: &str) -> Option<&str> {
    let text = text.trim();
    let id = if let Some(id) = text.strip_prefix("spotify:track:") {
        id
    } else {
        let path = text
            .strip_prefix("https://open.spotify.com/")
            .or_else(|| text.strip_prefix("http://open.spotify.com/"))?;
        // Links may carry a language before the type: `intl-fr/track/...`.
        let path = path.split(['?', '#']).next().unwrap_or(path);
        let mut parts = path.split('/').filter(|part| !part.is_empty());
        let mut kind = parts.next()?;
        if kind.starts_with("intl-") {
            kind = parts.next()?;
        }
        if kind != "track" {
            return None;
        }
        parts.next()?
    };
    is_spotify_id(id).then_some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_letters_and_digits_only() {
        assert!(is_spotify_id("6rqhFgbbKwnb9MLmUQDhG6"));
        assert!(!is_spotify_id("6rqhFgbbKwnb9MLmUQDhG"));
        assert!(!is_spotify_id("6rqhFgbbKwnb9MLmUQDh/6"));
        assert!(!is_spotify_id(""));
    }

    #[test]
    fn a_track_is_found_in_an_address_or_a_link() {
        let id = Some("6rqhFgbbKwnb9MLmUQDhG6");
        assert_eq!(track_id("spotify:track:6rqhFgbbKwnb9MLmUQDhG6"), id);
        assert_eq!(
            track_id("https://open.spotify.com/track/6rqhFgbbKwnb9MLmUQDhG6?si=abc"),
            id
        );
        assert_eq!(
            track_id(" https://open.spotify.com/intl-fr/track/6rqhFgbbKwnb9MLmUQDhG6 "),
            id
        );
        assert_eq!(
            track_id("https://open.spotify.com/album/6rqhFgbbKwnb9MLmUQDhG6"),
            None
        );
        assert_eq!(
            track_id("https://example.org/track/6rqhFgbbKwnb9MLmUQDhG6"),
            None
        );
        assert_eq!(track_id("spotify:track:not-an-id"), None);
    }
}
