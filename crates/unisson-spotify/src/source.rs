//! The Spotify catalog, as a source of Unisson.

use std::sync::Arc;

use unisson_core::{Media, MediaId, SearchQuery, Source, SourceError, SourceKind, StreamUri};

use crate::api::{Device, NO_DEVICE, Track};
use crate::connection::Spotify;
use crate::is_spotify_id;

/// Searches the Spotify catalog, and gives the address of a track.
///
/// Without a connection, the source simply finds nothing: a user who does
/// not use Spotify sees no complaint about it.
#[derive(Debug, Clone)]
pub struct SpotifySource {
    spotify: Arc<Spotify>,
}

impl SpotifySource {
    /// Creates the source on top of a connection.
    pub fn new(spotify: Arc<Spotify>) -> Self {
        Self { spotify }
    }
}

/// Turns a track of the catalog into a media of Unisson.
pub fn track_media(track: Track) -> Media {
    let mut media = Media::track(MediaId::new(SourceKind::Spotify, track.id), track.name);
    if !track.artists.is_empty() {
        media = media.with_artist(track.artists.join(", "));
    }
    if !track.album.trim().is_empty() {
        media = media.with_album(track.album);
    }
    if !track.duration.is_zero() {
        media = media.with_duration(track.duration);
    }
    if let Some(artwork) = track.artwork {
        media = media.with_artwork(artwork);
    }
    media
}

/// Picks the Spotify application to play on: the one that already plays,
/// else a computer, else any. Applications that refuse to be driven are
/// left out.
pub fn choose_device(devices: &[Device]) -> Option<&Device> {
    let usable = || {
        devices
            .iter()
            .filter(|device| device.id.is_some() && !device.is_restricted)
    };
    usable()
        .find(|device| device.is_active)
        .or_else(|| usable().find(|device| device.kind.eq_ignore_ascii_case("computer")))
        .or_else(|| usable().next())
}

impl Source for SpotifySource {
    fn kind(&self) -> SourceKind {
        SourceKind::Spotify
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError> {
        let Some(api) = self.spotify.api() else {
            return Ok(Vec::new());
        };
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let tracks = api.search_tracks(query.text(), query.limit())?;
        Ok(tracks.into_iter().map(track_media).collect())
    }

    /// Gives the `spotify:track:...` address of a track, after checking
    /// that a Spotify application is there to play it.
    fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
        if id.source != SourceKind::Spotify {
            return Err(SourceError::Invalid(format!(
                "not a Spotify track: {}",
                id.source
            )));
        }
        if !is_spotify_id(&id.key) {
            return Err(SourceError::NotFound);
        }
        let api = self
            .spotify
            .api()
            .ok_or_else(|| SourceError::Unavailable("not connected to Spotify".into()))?;
        let devices = api.devices()?;
        if choose_device(&devices).is_none() {
            return Err(SourceError::Unavailable(NO_DEVICE.into()));
        }
        StreamUri::new(format!("spotify:track:{}", id.key))
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn device(id: &str, kind: &str, active: bool, restricted: bool) -> Device {
        Device {
            id: Some(id.into()),
            name: id.into(),
            kind: kind.into(),
            is_active: active,
            is_restricted: restricted,
            supports_volume: true,
        }
    }

    #[test]
    fn the_device_that_plays_is_preferred_then_a_computer() {
        let phone = device("phone", "Smartphone", false, false);
        let computer = device("computer", "Computer", false, false);
        let speaker = device("speaker", "Speaker", true, false);
        let locked = device("locked", "Computer", true, true);

        let pick = |devices: &[Device]| choose_device(devices).map(|device| device.name.clone());
        assert_eq!(
            pick(&[phone.clone(), computer.clone(), speaker]).as_deref(),
            Some("speaker")
        );
        assert_eq!(
            pick(&[phone.clone(), computer, locked.clone()]).as_deref(),
            Some("computer")
        );
        assert_eq!(pick(&[phone]).as_deref(), Some("phone"));
        assert_eq!(pick(&[locked]), None);
        assert_eq!(pick(&[]), None);
    }

    #[test]
    fn a_track_becomes_a_media() {
        let media = track_media(Track {
            id: "6rqhFgbbKwnb9MLmUQDhG6".into(),
            name: "So What".into(),
            artists: vec!["Miles Davis".into(), "John Coltrane".into()],
            album: "Kind of Blue".into(),
            duration: Duration::from_secs(562),
            artwork: Some("https://i.scdn.co/300".into()),
        });
        assert_eq!(
            media.id,
            MediaId::new(SourceKind::Spotify, "6rqhFgbbKwnb9MLmUQDhG6")
        );
        assert_eq!(media.artist.as_deref(), Some("Miles Davis, John Coltrane"));
        assert_eq!(media.album.as_deref(), Some("Kind of Blue"));
        assert_eq!(media.duration, Some(Duration::from_secs(562)));
        assert!(!media.is_live());
    }

    #[test]
    fn without_a_connection_nothing_is_found_and_nothing_resolves() {
        let source = SpotifySource::new(Arc::new(Spotify::open(None)));
        assert_eq!(source.search(&SearchQuery::new("so what")), Ok(Vec::new()));
        assert!(matches!(
            source.resolve(&MediaId::new(SourceKind::Spotify, "6rqhFgbbKwnb9MLmUQDhG6")),
            Err(SourceError::Unavailable(_))
        ));
        assert_eq!(
            source.resolve(&MediaId::new(SourceKind::Spotify, "../../etc")),
            Err(SourceError::NotFound)
        );
        assert!(matches!(
            source.resolve(&MediaId::new(SourceKind::Local, "6rqhFgbbKwnb9MLmUQDhG6")),
            Err(SourceError::Invalid(_))
        ));
    }
}
