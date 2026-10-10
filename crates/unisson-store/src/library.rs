//! The library: the tags of the local audio files.
//!
//! # Design
//!
//! Reading the tags of every file takes time on a large collection. The
//! library keeps them in the database, so the application starts and
//! searches at once, without touching the files.
//!
//! Each track also stores a search key: its title, artist and album in
//! lower case and without accents (see [`unisson_core::search_key`]). A
//! search simplifies the typed words the same way and looks for them in
//! that key. SQLite's own text matching only ignores case for unaccented
//! letters, which is not enough for names such as "Céline".

use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use rusqlite::types::Value;
use rusqlite::{Connection, Row, params_from_iter};
use unisson_core::search_key;
use unisson_local::{LibraryChange, LocalTrack};

use crate::error::StoreError;
use crate::store::Store;

const TRACK_COLUMNS: &str = "path, title, artist, album, track_number, duration_ms, has_cover";

fn insert_track(connection: &Connection, track: &LocalTrack) -> rusqlite::Result<()> {
    let searchable = [
        Some(&track.title),
        track.artist.as_ref(),
        track.album.as_ref(),
    ]
    .into_iter()
    .flatten()
    .map(String::as_str)
    .collect::<Vec<_>>()
    .join(" ");
    connection.execute(
        &format!(
            "INSERT OR REPLACE INTO tracks ({TRACK_COLUMNS}, search_key)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        ),
        (
            // Paths are stored as raw bytes: they are not always valid text.
            track.path.as_os_str().as_bytes(),
            &track.title,
            &track.artist,
            &track.album,
            track.track_number,
            track.duration.map(|duration| duration.as_millis() as i64),
            track.has_cover,
            search_key(&searchable),
        ),
    )?;
    Ok(())
}

fn track_from_row(row: &Row<'_>) -> rusqlite::Result<LocalTrack> {
    let path: Vec<u8> = row.get(0)?;
    let duration_ms: Option<i64> = row.get(5)?;
    Ok(LocalTrack {
        path: PathBuf::from(OsString::from_vec(path)),
        title: row.get(1)?,
        artist: row.get(2)?,
        album: row.get(3)?,
        track_number: row.get(4)?,
        duration: duration_ms
            .and_then(|milliseconds| u64::try_from(milliseconds).ok())
            .map(Duration::from_millis),
        has_cover: row.get(6)?,
    })
}

/// Turns a word into a pattern that finds it anywhere in a text.
fn contains_pattern(word: &str) -> String {
    // `%` and `_` are wildcards for SQLite: escape them to search them as
    // ordinary characters.
    let mut pattern = String::from("%");
    for letter in word.chars() {
        if matches!(letter, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(letter);
    }
    pattern.push('%');
    pattern
}

impl Store {
    /// Replaces the whole library with `tracks`.
    ///
    /// Call it after a full scan of the music folder. The change is all or
    /// nothing: if it fails, the previous library is kept.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the tracks cannot be written.
    pub fn replace_library(&self, tracks: &[LocalTrack]) -> Result<(), StoreError> {
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM tracks", ())?;
        for track in tracks {
            insert_track(&transaction, track)?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Adds a track to the library, or refreshes it if its file is known.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the track cannot be written.
    pub fn save_track(&self, track: &LocalTrack) -> Result<(), StoreError> {
        Ok(insert_track(&self.connection(), track)?)
    }

    /// Removes the track of a file from the library.
    ///
    /// Returns `false` if the file was not in the library.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the track cannot be removed.
    pub fn remove_track(&self, path: impl AsRef<Path>) -> Result<bool, StoreError> {
        let removed = self.connection().execute(
            "DELETE FROM tracks WHERE path = ?1",
            [path.as_ref().as_os_str().as_bytes()],
        )?;
        Ok(removed > 0)
    }

    /// Removes a path from the library: a file, or a folder with every
    /// track below it.
    ///
    /// Returns the number of tracks removed.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the tracks cannot be removed.
    pub fn remove_tracks_under(&self, path: impl AsRef<Path>) -> Result<usize, StoreError> {
        let path = path.as_ref().as_os_str().as_bytes();
        // Paths are compared byte by byte. Everything inside the folder
        // starts with "<path>/", so it sorts from "<path>/" up to, but not
        // including, "<path>0": `0` is the character right after `/`.
        // A neighbor such as "<path> (live)" is outside that range.
        let first_inside = [path, b"/"].concat();
        let after_inside = [path, b"0"].concat();
        let removed = self.connection().execute(
            "DELETE FROM tracks WHERE path = ?1 OR (path >= ?2 AND path < ?3)",
            (path, first_inside, after_inside),
        )?;
        Ok(removed)
    }

    /// Applies a change reported by a
    /// [`LibraryWatcher`](unisson_local::LibraryWatcher).
    ///
    /// Returns the number of tracks added, refreshed or removed. It is zero
    /// when a removed path held no known track.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the library cannot be changed.
    pub fn apply_library_change(&self, change: &LibraryChange) -> Result<usize, StoreError> {
        match change {
            LibraryChange::Updated(track) => self.save_track(track).map(|()| 1),
            LibraryChange::Removed(path) => self.remove_tracks_under(path),
        }
    }

    /// Returns the number of tracks in the library.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the library cannot be read.
    pub fn library_len(&self) -> Result<usize, StoreError> {
        let count: i64 = self
            .connection()
            .query_row("SELECT COUNT(*) FROM tracks", (), |row| row.get(0))?;
        Ok(count as usize)
    }

    /// Returns every track, sorted by artist, album, track number and title.
    ///
    /// Tracks without an artist or an album come last.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the library cannot be read.
    pub fn library_tracks(&self) -> Result<Vec<LocalTrack>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {TRACK_COLUMNS} FROM tracks
             ORDER BY artist IS NULL, artist COLLATE NOCASE,
                      album IS NULL, album COLLATE NOCASE,
                      track_number IS NULL, track_number,
                      title COLLATE NOCASE, path"
        ))?;
        let tracks = statement.query_map((), track_from_row)?;
        Ok(tracks.collect::<Result<_, _>>()?)
    }

    /// Searches the library.
    ///
    /// A track matches when its title, artist and album, taken together,
    /// contain every word of `text`. Case and accents are ignored, and the
    /// words may be incomplete: `mil blu` finds "Miles Davis - Kind of Blue".
    /// At most `limit` tracks are returned, sorted by title. An empty text
    /// finds nothing.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the library cannot be read.
    pub fn search_library(&self, text: &str, limit: usize) -> Result<Vec<LocalTrack>, StoreError> {
        let key = search_key(text);
        if key.is_empty() {
            return Ok(Vec::new());
        }
        let words: Vec<&str> = key.split(' ').collect();
        let conditions = vec!["search_key LIKE ? ESCAPE '\\'"; words.len()].join(" AND ");
        let values = words
            .iter()
            .map(|word| Value::from(contains_pattern(word)))
            .chain([Value::from(limit as i64)]);

        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {TRACK_COLUMNS} FROM tracks
             WHERE {conditions}
             ORDER BY title COLLATE NOCASE, path
             LIMIT ?"
        ))?;
        let tracks = statement.query_map(params_from_iter(values), track_from_row)?;
        Ok(tracks.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    fn track(path: &str, title: &str, artist: Option<&str>, album: Option<&str>) -> LocalTrack {
        LocalTrack {
            path: PathBuf::from(path),
            title: title.into(),
            artist: artist.map(str::to_owned),
            album: album.map(str::to_owned),
            track_number: None,
            duration: None,
            has_cover: false,
        }
    }

    fn sample_library() -> Store {
        let store = Store::open_in_memory().unwrap();
        store
            .replace_library(&[
                track(
                    "/m/so what.flac",
                    "So What",
                    Some("Miles Davis"),
                    Some("Kind of Blue"),
                ),
                track(
                    "/m/blue.flac",
                    "Blue in Green",
                    Some("Miles Davis"),
                    Some("Kind of Blue"),
                ),
                track(
                    "/m/take five.mp3",
                    "Take Five",
                    Some("Dave Brubeck"),
                    Some("Time Out"),
                ),
                track(
                    "/m/pour.mp3",
                    "Pour que tu m'aimes encore",
                    Some("Céline Dion"),
                    None,
                ),
                track("/m/100.mp3", "100% Pure_Love", None, None),
            ])
            .unwrap();
        store
    }

    fn titles(tracks: &[LocalTrack]) -> Vec<&str> {
        tracks.iter().map(|track| track.title.as_str()).collect()
    }

    fn found(store: &Store, text: &str) -> Vec<String> {
        store
            .search_library(text, 10)
            .unwrap()
            .into_iter()
            .map(|track| track.title)
            .collect()
    }

    #[test]
    fn tracks_come_back_complete() {
        let store = Store::open_in_memory().unwrap();
        let full = LocalTrack {
            path: PathBuf::from("/music/été 2010/a.flac"),
            title: "So What".into(),
            artist: Some("Miles Davis".into()),
            album: Some("Kind of Blue".into()),
            track_number: Some(1),
            duration: Some(Duration::from_millis(562_500)),
            has_cover: true,
        };
        let bare = track("/music/b.wav", "b", None, None);

        store
            .replace_library(&[full.clone(), bare.clone()])
            .unwrap();

        assert_eq!(store.library_len().unwrap(), 2);
        assert_eq!(store.library_tracks().unwrap(), [full, bare]);
    }

    #[test]
    fn a_path_that_is_not_valid_text_is_kept_exactly() {
        let store = Store::open_in_memory().unwrap();
        let odd_path = PathBuf::from(OsString::from_vec(b"/music/\xFF\xFE.mp3".to_vec()));
        let odd = LocalTrack {
            path: odd_path.clone(),
            ..track("", "odd", None, None)
        };

        store.save_track(&odd).unwrap();

        assert_eq!(store.library_tracks().unwrap()[0].path, odd_path);
        assert!(store.remove_track(&odd_path).unwrap());
    }

    #[test]
    fn the_library_is_sorted_by_artist_album_then_track_number() {
        let store = Store::open_in_memory().unwrap();
        let numbered = |path: &str, title: &str, number| LocalTrack {
            track_number: Some(number),
            ..track(path, title, Some("miles davis"), Some("Kind of Blue"))
        };
        store
            .replace_library(&[
                track("/1", "Untagged", None, None),
                numbered("/2", "Freddie Freeloader", 2),
                track("/3", "Take Five", Some("Dave Brubeck"), Some("Time Out")),
                numbered("/4", "So What", 1),
            ])
            .unwrap();
        assert_eq!(
            titles(&store.library_tracks().unwrap()),
            ["Take Five", "So What", "Freddie Freeloader", "Untagged"]
        );
    }

    #[test]
    fn replace_library_forgets_the_previous_tracks() {
        let store = sample_library();
        store
            .replace_library(&[track("/new.mp3", "New", None, None)])
            .unwrap();
        assert_eq!(titles(&store.library_tracks().unwrap()), ["New"]);
    }

    #[test]
    fn save_track_adds_or_refreshes_and_remove_track_removes() {
        let store = sample_library();
        let before = store.library_len().unwrap();

        store
            .save_track(&track("/m/new.mp3", "New", None, None))
            .unwrap();
        assert_eq!(store.library_len().unwrap(), before + 1);

        // Same file, new tags: the track is refreshed, not duplicated.
        store
            .save_track(&track("/m/new.mp3", "Renamed", Some("Someone"), None))
            .unwrap();
        assert_eq!(store.library_len().unwrap(), before + 1);
        assert_eq!(found(&store, "renamed someone"), ["Renamed"]);
        assert!(found(&store, "new").is_empty());

        assert!(store.remove_track("/m/new.mp3").unwrap());
        assert!(!store.remove_track("/m/new.mp3").unwrap());
        assert_eq!(store.library_len().unwrap(), before);
    }

    #[test]
    fn removing_a_folder_removes_what_is_below_it_and_nothing_else() {
        let store = Store::open_in_memory().unwrap();
        store
            .replace_library(&[
                track("/m/Album/1.mp3", "Inside", None, None),
                track("/m/Album/CD 2/2.mp3", "Deep inside", None, None),
                track("/m/Album (live)/3.mp3", "Neighbor folder", None, None),
                track("/m/Album.mp3", "Neighbor file", None, None),
                track("/m/Albu/4.mp3", "Shorter name", None, None),
            ])
            .unwrap();

        assert_eq!(store.remove_tracks_under("/m/Album").unwrap(), 2);
        assert_eq!(
            titles(&store.library_tracks().unwrap()),
            ["Neighbor file", "Neighbor folder", "Shorter name"]
        );

        // A single file is removed the same way.
        assert_eq!(store.remove_tracks_under("/m/Album.mp3").unwrap(), 1);
        assert_eq!(store.remove_tracks_under("/m/Album.mp3").unwrap(), 0);
    }

    #[test]
    fn watcher_changes_are_applied() {
        let store = sample_library();
        let before = store.library_len().unwrap();
        let new = track("/m/new/song.mp3", "Brand New", None, None);

        let updated = LibraryChange::Updated(new.clone());
        assert_eq!(store.apply_library_change(&updated).unwrap(), 1);
        // The same change twice does not duplicate the track.
        assert_eq!(store.apply_library_change(&updated).unwrap(), 1);
        assert_eq!(store.library_len().unwrap(), before + 1);
        assert_eq!(found(&store, "brand"), ["Brand New"]);

        let removed = LibraryChange::Removed(PathBuf::from("/m/new"));
        assert_eq!(store.apply_library_change(&removed).unwrap(), 1);
        assert_eq!(store.apply_library_change(&removed).unwrap(), 0);
        assert_eq!(store.library_len().unwrap(), before);
    }

    #[test]
    fn search_looks_in_title_artist_and_album() {
        let store = sample_library();
        assert_eq!(found(&store, "take"), ["Take Five"]);
        assert_eq!(found(&store, "brubeck"), ["Take Five"]);
        assert_eq!(found(&store, "time out"), ["Take Five"]);
        assert_eq!(
            found(&store, "miles"),
            ["Blue in Green", "So What"],
            "sorted by title"
        );
    }

    #[test]
    fn every_word_must_match_and_words_may_be_incomplete() {
        let store = sample_library();
        assert_eq!(found(&store, "mil blu gre"), ["Blue in Green"]);
        assert_eq!(found(&store, "davis what"), ["So What"]);
        assert!(found(&store, "miles brubeck").is_empty());
    }

    #[test]
    fn search_ignores_case_and_accents() {
        let store = sample_library();
        let expected = ["Pour que tu m'aimes encore"];
        assert_eq!(found(&store, "celine"), expected);
        assert_eq!(found(&store, "CÉLINE dion"), expected);
        assert_eq!(found(&store, "  Dion   céline "), expected);
    }

    #[test]
    fn wildcard_characters_are_searched_as_plain_text() {
        let store = sample_library();
        assert_eq!(found(&store, "100%"), ["100% Pure_Love"]);
        assert_eq!(found(&store, "pure_love"), ["100% Pure_Love"]);
        // `_` must not stand for "any character", nor `%` for "anything".
        assert!(found(&store, "s_ what").is_empty());
        assert!(found(&store, "%").len() == 1);
    }

    #[test]
    fn an_empty_search_finds_nothing_and_the_limit_is_respected() {
        let store = sample_library();
        assert!(found(&store, "   ").is_empty());
        assert_eq!(store.search_library("e", 2).unwrap().len(), 2);
        assert!(store.search_library("e", 0).unwrap().is_empty());
    }

    #[test]
    fn search_is_fast_on_a_large_library() {
        let store = Store::open_in_memory().unwrap();
        let tracks: Vec<LocalTrack> = (0..20_000)
            .map(|index| {
                let artist = format!("Artist {}", index % 400);
                let album = format!("Album {}", index % 2_000);
                track(
                    &format!("/m/{index}.mp3"),
                    &format!("Song {index}"),
                    Some(&artist),
                    Some(&album),
                )
            })
            .collect();
        store.replace_library(&tracks).unwrap();

        let start = Instant::now();
        let result = store.search_library("song 19990 artist", 20).unwrap();
        let elapsed = start.elapsed();

        assert_eq!(titles(&result), ["Song 19990"]);
        // A generous bound: the point is to catch a search that became slow.
        assert!(
            elapsed < Duration::from_millis(500),
            "the search took {elapsed:?}"
        );
        println!("search over 20000 tracks: {elapsed:?}");
    }
}
