//! Spotify source of Unisson.
//!
//! # Design
//!
//! Spotify does not let a third-party program decode its audio. What it
//! offers is the [Web API](https://developer.spotify.com/documentation/web-api):
//! search the catalog, and tell a Spotify application (a "device") what to
//! play. So:
//!
//! - [`SpotifySource`] searches the catalog, like the other sources, and
//!   turns a track into a `spotify:track:...` address;
//! - [`SpotifyRemote`] is the remote player the playback session hands those
//!   addresses to. It asks the official Spotify application, open on the
//!   computer, to play them, and follows the playback to report the
//!   position and the end of each track.
//!
//! Playing through the Web API needs a Spotify Premium account. Each user
//! registers their own application on the Spotify developer dashboard and
//! gives its client ID to Unisson: an application in development mode only
//! serves the few accounts its owner lists, so a client ID shared by every
//! user of Unisson could not work. The user then connects with the
//! "authorization code with PKCE" flow, made for programs that cannot keep
//! a secret: see [`auth`].
//!
//! [`Spotify`] holds the connection, shared by the source and the remote
//! player.

pub mod api;
pub mod auth;
mod connection;
mod credentials;
mod remote;
mod source;
#[cfg(test)]
mod test_server;

pub use api::{Device, PlaybackState, SpotifyApi, Track};
pub use connection::Spotify;
pub use credentials::Credentials;
pub use remote::SpotifyRemote;
pub use source::SpotifySource;

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
