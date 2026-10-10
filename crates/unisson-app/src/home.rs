//! What the home screen shows.
//!
//! # Design
//!
//! The home screen exists to start something with one click. It is built
//! from what the user already did: the listening history and the favorite
//! stations. Nothing here needs the network.
//!
//! The "resume" tiles are the last few things listened to, seen as a whole
//! rather than track by track: listening to three tracks of an album gives
//! one tile for the album, which plays it from the last track heard.

use serde::Serialize;
use unisson_core::{Media, MediaKind, SourceKind};
use unisson_radio::{Favorites, Station};
use unisson_store::{Store, StoreError};

use crate::dto::{MediaDto, StationDto};

/// Number of "resume" tiles.
const TILE_LIMIT: usize = 4;
/// Number of lines in "recently played".
const RECENT_LIMIT: usize = 12;
/// How far back in the history the tiles are looked for.
const HISTORY_DEPTH: usize = 60;
/// Number of library tracks listed.
const LIBRARY_LIMIT: usize = 100;

/// One "resume" tile: a thing to play, with the list it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TileDto {
    /// `station`, `album` or `track`.
    pub kind: &'static str,
    /// The name of the station, of the album or of the track.
    pub title: String,
    /// The artist, when it is known.
    pub artist: Option<String>,
    /// The source, as [`SourceKind::id`].
    pub source: String,
    /// What a click plays: a single media, or the tracks of the album.
    pub items: Vec<MediaDto>,
    /// The index in `items` where playback starts.
    pub start: usize,
}

/// Everything the home screen shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HomeDto {
    /// The "resume" tiles, the most recent first.
    pub resume: Vec<TileDto>,
    /// The favorite stations, in the order chosen by the user.
    pub stations: Vec<StationDto>,
    /// The media played most recently, the latest first.
    pub recent: Vec<MediaDto>,
    /// The first tracks of the local library.
    pub library: Vec<MediaDto>,
    /// The number of tracks in the whole library.
    pub library_total: usize,
}

/// Gathers what the home screen shows.
///
/// # Errors
///
/// Returns [`StoreError::Database`] if the database cannot be read.
pub fn build(store: &Store, favorites: &Favorites) -> Result<HomeDto, StoreError> {
    let history: Vec<Media> = store
        .recently_played(HISTORY_DEPTH)?
        .into_iter()
        .filter(|media| is_playable(media, favorites))
        .collect();

    let mut resume: Vec<TileDto> = Vec::new();
    for media in &history {
        if resume.len() == TILE_LIMIT {
            break;
        }
        let tile = tile_for(store, media)?;
        // Several tracks of one album give the same tile: keep the first.
        let known = resume
            .iter()
            .any(|other| other.kind == tile.kind && other.title == tile.title);
        if !known {
            resume.push(tile);
        }
    }

    Ok(HomeDto {
        resume,
        stations: favorites.stations().iter().map(StationDto::from).collect(),
        recent: history
            .iter()
            .take(RECENT_LIMIT)
            .map(MediaDto::from)
            .collect(),
        library: store
            .library_page(0, LIBRARY_LIMIT)?
            .iter()
            .map(|track| MediaDto::from(&track.to_media()))
            .collect(),
        library_total: store.library_len()?,
    })
}

/// Tells whether a media of the history can still be played.
///
/// A station added by address is unknown to the directory: once it has left
/// the favorites, its address is lost.
fn is_playable(media: &Media, favorites: &Favorites) -> bool {
    let lost = media.id.source == SourceKind::Radio
        && media.id.key.starts_with(Station::CUSTOM_PREFIX)
        && !favorites.contains(&media.id.key);
    !lost
}

fn tile_for(store: &Store, media: &Media) -> Result<TileDto, StoreError> {
    let single = |kind| TileDto {
        kind,
        title: media.title.clone(),
        artist: media.artist.clone(),
        source: media.id.source.id().to_owned(),
        items: vec![MediaDto::from(media)],
        start: 0,
    };
    if media.kind == MediaKind::Station {
        return Ok(single("station"));
    }
    // Only the local library knows the other tracks of an album.
    let album = match &media.album {
        Some(album) if media.id.source == SourceKind::Local => album,
        _ => return Ok(single("track")),
    };
    let tracks: Vec<Media> = store
        .album_tracks(album)?
        .iter()
        .map(|track| track.to_media())
        .collect();
    // The file may have left the library since it was played.
    let Some(start) = tracks.iter().position(|track| track.id == media.id) else {
        return Ok(single("track"));
    };
    if tracks.len() == 1 {
        return Ok(single("track"));
    }
    Ok(TileDto {
        kind: "album",
        title: album.clone(),
        artist: media.artist.clone(),
        source: media.id.source.id().to_owned(),
        items: tracks.iter().map(MediaDto::from).collect(),
        start,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime};

    use unisson_core::MediaId;
    use unisson_local::LocalTrack;

    use super::*;

    fn track(path: &str, title: &str, album: Option<&str>, number: Option<u32>) -> LocalTrack {
        LocalTrack {
            path: PathBuf::from(path),
            title: title.into(),
            artist: Some("Miles Davis".into()),
            album: album.map(str::to_owned),
            track_number: number,
            duration: None,
            has_cover: false,
        }
    }

    /// A store with one album of three tracks and one loose track.
    fn library() -> (Store, Vec<LocalTrack>) {
        let store = Store::open_in_memory().unwrap();
        let tracks = vec![
            track("/m/1.flac", "So What", Some("Kind of Blue"), Some(1)),
            track(
                "/m/2.flac",
                "Freddie Freeloader",
                Some("Kind of Blue"),
                Some(2),
            ),
            track("/m/3.flac", "Blue in Green", Some("Kind of Blue"), Some(3)),
            track("/m/loose.mp3", "Loose", None, None),
        ];
        store.replace_library(&tracks).unwrap();
        (store, tracks)
    }

    /// Records plays, the first one being the oldest.
    fn play(store: &Store, media: &[Media]) {
        for (index, media) in media.iter().enumerate() {
            let at = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000 + index as u64);
            store.record_play(media, at).unwrap();
        }
    }

    fn station(key: &str, name: &str) -> Media {
        Media::station(MediaId::new(SourceKind::Radio, key), name)
    }

    #[test]
    fn an_empty_history_gives_an_empty_home_with_the_library() {
        let (store, tracks) = library();
        let home = build(&store, &Favorites::new()).unwrap();
        assert!(home.resume.is_empty());
        assert!(home.recent.is_empty());
        assert!(home.stations.is_empty());
        assert_eq!(home.library.len(), tracks.len());
        assert_eq!(home.library_total, tracks.len());
    }

    #[test]
    fn tracks_of_one_album_give_one_tile_that_resumes_the_album() {
        let (store, tracks) = library();
        play(&store, &[tracks[0].to_media(), tracks[1].to_media()]);

        let home = build(&store, &Favorites::new()).unwrap();

        assert_eq!(home.resume.len(), 1);
        let tile = &home.resume[0];
        assert_eq!(tile.kind, "album");
        assert_eq!(tile.title, "Kind of Blue");
        assert_eq!(tile.artist.as_deref(), Some("Miles Davis"));
        assert_eq!(tile.items.len(), 3, "the whole album is queued");
        assert_eq!(tile.items[tile.start].title, "Freddie Freeloader");
        // The history itself stays track by track, the latest first.
        let recent: Vec<&str> = home.recent.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(recent, ["Freddie Freeloader", "So What"]);
    }

    #[test]
    fn tiles_mix_stations_albums_and_tracks_the_latest_first() {
        let (store, tracks) = library();
        play(
            &store,
            &[
                tracks[3].to_media(),
                station("fip", "FIP"),
                tracks[2].to_media(),
            ],
        );

        let home = build(&store, &Favorites::new()).unwrap();

        let tiles: Vec<(&str, &str)> = home
            .resume
            .iter()
            .map(|tile| (tile.kind, tile.title.as_str()))
            .collect();
        assert_eq!(
            tiles,
            [
                ("album", "Kind of Blue"),
                ("station", "FIP"),
                ("track", "Loose")
            ]
        );
        assert_eq!(home.resume[1].source, "radio");
        assert_eq!(home.resume[1].items.len(), 1);
    }

    #[test]
    fn there_are_at_most_four_tiles() {
        let (store, _) = library();
        let stations: Vec<Media> = (0..6)
            .map(|index| station(&format!("s{index}"), &format!("Station {index}")))
            .collect();
        play(&store, &stations);

        let home = build(&store, &Favorites::new()).unwrap();

        assert_eq!(home.resume.len(), TILE_LIMIT);
        assert_eq!(home.resume[0].title, "Station 5");
        assert_eq!(home.recent.len(), 6);
    }

    #[test]
    fn a_track_that_left_the_library_gives_a_tile_for_itself() {
        let (store, tracks) = library();
        play(&store, &[tracks[0].to_media()]);
        store.remove_track("/m/1.flac").unwrap();

        let home = build(&store, &Favorites::new()).unwrap();

        assert_eq!(home.resume[0].kind, "track");
        assert_eq!(home.resume[0].title, "So What");
    }

    #[test]
    fn a_station_added_by_address_is_shown_only_while_it_is_a_favorite() {
        let (store, _) = library();
        let custom = Station::custom("My radio", "https://example.org/live.mp3").unwrap();
        play(&store, &[custom.to_media(), station("fip", "FIP")]);

        let without = build(&store, &Favorites::new()).unwrap();
        let titles: Vec<&str> = without.recent.iter().map(|m| m.title.as_str()).collect();
        assert_eq!(titles, ["FIP"]);

        let favorites = Favorites::from_stations([custom]);
        let with = build(&store, &favorites).unwrap();
        assert_eq!(with.recent.len(), 2);
        assert_eq!(with.stations[0].name, "My radio");
        assert_eq!(with.resume.len(), 2);
    }
}
