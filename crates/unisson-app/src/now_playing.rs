//! What plays, in the words shown outside the window.
//!
//! The desktop shows what plays in several places: the media controls of
//! GNOME, the tooltip and the menu of the tray icon, notifications. They all
//! need the same short description, built here once.

use std::time::Duration;

use unisson_playback::{Snapshot, Status};

/// A short description of the selected media.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NowPlaying {
    /// The title of the track, or what a radio station has on air.
    pub title: String,
    /// The artist of the track, or the name of the radio station.
    pub artist: Option<String>,
    /// The album of the track.
    pub album: Option<String>,
    /// The address of a picture on the web.
    pub artwork: Option<String>,
    /// The duration of the track. Live streams have none.
    pub length: Option<Duration>,
    /// Whether this is a live stream.
    pub live: bool,
    /// Whether sound is coming out, or about to.
    pub playing: bool,
}

impl NowPlaying {
    /// Describes the selected media of a snapshot, if there is one.
    ///
    /// For a radio station that announces what is on air, the announcement
    /// becomes the title and the station takes the place of the artist:
    /// that is what a listener wants to read first.
    pub fn of(snapshot: &Snapshot) -> Option<Self> {
        let media = snapshot.current.as_ref()?;
        let on_air = snapshot
            .stream_title
            .as_deref()
            .map(str::trim)
            .filter(|title| !title.is_empty());
        let (title, artist) = match on_air {
            Some(on_air) if media.is_live() => (on_air.to_owned(), Some(media.title.clone())),
            _ => (media.title.clone(), media.artist.clone()),
        };
        Some(Self {
            title,
            artist,
            album: media.album.clone(),
            artwork: media.artwork.clone().filter(|address| {
                address.starts_with("http://") || address.starts_with("https://")
            }),
            length: snapshot.duration.filter(|_| !media.is_live()),
            live: media.is_live(),
            playing: !matches!(snapshot.status, Status::Paused | Status::Stopped),
        })
    }

    /// Returns one line such as `So What, Miles Davis`.
    pub fn line(&self) -> String {
        match &self.artist {
            Some(artist) => format!("{}, {artist}", self.title),
            None => self.title.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use unisson_core::{Media, MediaId, SourceKind};

    use super::*;

    fn snapshot(media: Media) -> Snapshot {
        Snapshot {
            status: Status::Playing,
            duration: media.duration,
            current: Some(media),
            ..Snapshot::default()
        }
    }

    #[test]
    fn nothing_selected_gives_nothing() {
        assert_eq!(NowPlaying::of(&Snapshot::default()), None);
    }

    #[test]
    fn a_track_is_described_by_its_tags() {
        let track = Media::track(MediaId::new(SourceKind::Local, "/m/a.flac"), "So What")
            .with_artist("Miles Davis")
            .with_album("Kind of Blue")
            .with_duration(Duration::from_secs(562))
            .with_artwork("/covers/a.jpg");

        let now = NowPlaying::of(&snapshot(track)).unwrap();

        assert_eq!(now.line(), "So What, Miles Davis");
        assert_eq!(now.album.as_deref(), Some("Kind of Blue"));
        assert_eq!(now.length, Some(Duration::from_secs(562)));
        assert_eq!(now.artwork, None, "only pictures from the web are kept");
        assert!(!now.live);
        assert!(now.playing);
    }

    #[test]
    fn a_station_shows_what_is_on_air_first() {
        let station = Media::station(MediaId::new(SourceKind::Radio, "fip"), "FIP")
            .with_artwork("https://example.org/fip.png");
        let mut state = snapshot(station);

        let silent = NowPlaying::of(&state).unwrap();
        assert_eq!(silent.line(), "FIP");
        assert!(silent.live);
        assert_eq!(
            silent.artwork.as_deref(),
            Some("https://example.org/fip.png")
        );

        state.stream_title = Some("  Nina Simone - Sinnerman ".into());
        let announced = NowPlaying::of(&state).unwrap();
        assert_eq!(announced.title, "Nina Simone - Sinnerman");
        assert_eq!(announced.artist.as_deref(), Some("FIP"));
        assert_eq!(announced.length, None);

        state.stream_title = Some("   ".into());
        assert_eq!(NowPlaying::of(&state).unwrap().line(), "FIP");
    }

    #[test]
    fn paused_and_stopped_are_not_playing() {
        let track = Media::track(MediaId::new(SourceKind::Local, "/m/a.flac"), "So What");
        for (status, playing) in [
            (Status::Playing, true),
            (Status::Loading, true),
            (Status::Paused, false),
            (Status::Stopped, false),
        ] {
            let state = Snapshot {
                status,
                ..snapshot(track.clone())
            };
            assert_eq!(
                NowPlaying::of(&state).unwrap().playing,
                playing,
                "{status:?}"
            );
        }
    }
}
