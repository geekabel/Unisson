//! The [`Store`]: the SQLite database of the application.

use std::fs::DirBuilder;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use rusqlite::Connection;
use unisson_radio::Station;

use crate::error::StoreError;
use crate::paths::default_database_path;

/// The successive versions of the database layout.
///
/// Each entry upgrades the database by one version. To change the layout,
/// append an entry: never edit one that was released, since existing
/// databases have already applied it.
const MIGRATIONS: [&str; 2] = [
    "
    CREATE TABLE favorite_stations (
        position     INTEGER NOT NULL,
        uuid         TEXT    NOT NULL PRIMARY KEY,
        name         TEXT    NOT NULL,
        stream_url   TEXT    NOT NULL,
        homepage     TEXT,
        favicon      TEXT,
        tags         TEXT    NOT NULL,
        country_code TEXT,
        codec        TEXT,
        bitrate      INTEGER,
        hls          INTEGER NOT NULL
    );
",
    // The eight columns from `source` to `artwork` describe a media. They are
    // the same in every table that stores one: see the `media` module.
    "
    CREATE TABLE tracks (
        path         BLOB    NOT NULL PRIMARY KEY,
        title        TEXT    NOT NULL,
        artist       TEXT,
        album        TEXT,
        track_number INTEGER,
        duration_ms  INTEGER,
        has_cover    INTEGER NOT NULL,
        search_key   TEXT    NOT NULL
    );

    CREATE TABLE history (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        played_at   INTEGER NOT NULL,
        source      TEXT    NOT NULL,
        key         TEXT    NOT NULL,
        kind        TEXT    NOT NULL,
        title       TEXT    NOT NULL,
        artist      TEXT,
        album       TEXT,
        duration_ms INTEGER,
        artwork     TEXT
    );
    CREATE INDEX history_by_media ON history (source, key);

    CREATE TABLE favorite_media (
        id          INTEGER PRIMARY KEY AUTOINCREMENT,
        source      TEXT    NOT NULL,
        key         TEXT    NOT NULL,
        kind        TEXT    NOT NULL,
        title       TEXT    NOT NULL,
        artist      TEXT,
        album       TEXT,
        duration_ms INTEGER,
        artwork     TEXT,
        UNIQUE (source, key)
    );

    CREATE TABLE playlists (
        id   INTEGER PRIMARY KEY AUTOINCREMENT,
        name TEXT    NOT NULL
    );

    CREATE TABLE playlist_items (
        playlist_id INTEGER NOT NULL REFERENCES playlists (id) ON DELETE CASCADE,
        position    INTEGER NOT NULL,
        source      TEXT    NOT NULL,
        key         TEXT    NOT NULL,
        kind        TEXT    NOT NULL,
        title       TEXT    NOT NULL,
        artist      TEXT,
        album       TEXT,
        duration_ms INTEGER,
        artwork     TEXT,
        PRIMARY KEY (playlist_id, position)
    );
",
];

/// The local database of the application.
///
/// A store can be shared between threads: its methods take `&self`, and
/// calls are run one at a time.
#[derive(Debug)]
pub struct Store {
    connection: Mutex<Connection>,
}

impl Store {
    /// Opens the database at its default location, creating it if needed.
    ///
    /// See [`default_database_path`] for the location.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Location`] if the location cannot be found or
    /// created, or [`StoreError::Database`] if the file cannot be opened as
    /// a database.
    pub fn open_default() -> Result<Self, StoreError> {
        Self::open(default_database_path()?)
    }

    /// Opens the database stored in `path`, creating it if needed.
    ///
    /// Missing parent directories are created, readable by the user only.
    ///
    /// # Errors
    ///
    /// Same as [`open_default`](Self::open_default).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            let mut builder = DirBuilder::new();
            builder.recursive(true);
            // 0700 is the mode the XDG specification asks for. On Windows
            // the profile of the user is already private.
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder
                .create(parent)
                .map_err(|error| StoreError::Location(format!("{}: {error}", parent.display())))?;
        }
        Self::prepare(Connection::open(path)?)
    }

    /// Opens a temporary database that lives in memory only.
    ///
    /// Useful for tests: nothing is written to disk.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the database cannot be created.
    pub fn open_in_memory() -> Result<Self, StoreError> {
        Self::prepare(Connection::open_in_memory()?)
    }

    /// Brings the layout of a database up to date, and wraps it.
    fn prepare(mut connection: Connection) -> Result<Self, StoreError> {
        // SQLite only enforces links between tables when asked, and the
        // setting is per connection. Deleting a playlist relies on it.
        connection.pragma_update(None, "foreign_keys", true)?;
        // SQLite keeps this number in the file for exactly that purpose.
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        let latest = MIGRATIONS.len() as i64;
        if !(0..=latest).contains(&version) {
            return Err(StoreError::Database(format!(
                "the database was written by a newer version of Unisson (layout {version})"
            )));
        }
        for (migration, new_version) in MIGRATIONS.iter().zip(1..=latest) {
            if new_version <= version {
                continue;
            }
            // Either the whole step applies, version number included, or none of it.
            let transaction = connection.transaction()?;
            transaction.execute_batch(migration)?;
            transaction.pragma_update(None, "user_version", new_version)?;
            transaction.commit()?;
        }
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Returns the favorite stations, in the user's order.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the stations cannot be read.
    pub fn favorite_stations(&self) -> Result<Vec<Station>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT uuid, name, stream_url, homepage, favicon, tags,
                    country_code, codec, bitrate, hls
             FROM favorite_stations
             ORDER BY position",
        )?;
        let stations = statement.query_map((), |row| {
            let tags: String = row.get(5)?;
            Ok(Station {
                uuid: row.get(0)?,
                name: row.get(1)?,
                stream_url: row.get(2)?,
                homepage: row.get(3)?,
                favicon: row.get(4)?,
                tags: tags
                    .split(',')
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_owned)
                    .collect(),
                country_code: row.get(6)?,
                codec: row.get(7)?,
                bitrate: row.get(8)?,
                hls: row.get(9)?,
            })
        })?;
        Ok(stations.collect::<Result<_, _>>()?)
    }

    /// Replaces the saved favorite stations with `stations`, in that order.
    ///
    /// The change is all or nothing: if it fails, the previous list is kept.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the stations cannot be written,
    /// for instance when two of them share an identifier.
    pub fn save_favorite_stations(&self, stations: &[Station]) -> Result<(), StoreError> {
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        transaction.execute("DELETE FROM favorite_stations", ())?;
        {
            let mut insert = transaction.prepare(
                "INSERT INTO favorite_stations
                     (position, uuid, name, stream_url, homepage, favicon, tags,
                      country_code, codec, bitrate, hls)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            )?;
            for (position, station) in (0_i64..).zip(stations) {
                insert.execute((
                    position,
                    &station.uuid,
                    &station.name,
                    &station.stream_url,
                    &station.homepage,
                    &station.favicon,
                    station.tags.join(","),
                    &station.country_code,
                    &station.codec,
                    station.bitrate,
                    station.hls,
                ))?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn connection(&self) -> MutexGuard<'_, Connection> {
        // A panic in another thread cannot leave the connection half-used:
        // an unfinished transaction is rolled back when it is dropped.
        self.connection
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn directory_station() -> Station {
        Station {
            uuid: "aaaa-1111".into(),
            name: "FIP".into(),
            stream_url: "https://stream.example.org/fip.mp3".into(),
            homepage: Some("https://example.org".into()),
            favicon: Some("https://example.org/fip.png".into()),
            tags: vec!["eclectic".into(), "jazz".into()],
            country_code: Some("FR".into()),
            codec: Some("MP3".into()),
            bitrate: Some(128),
            hls: true,
        }
    }

    fn custom_station(name: &str) -> Station {
        Station::custom(name, &format!("https://example.org/{name}")).unwrap()
    }

    /// Returns a fresh path under the temporary directory.
    fn temporary_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("unisson-store-tests-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(dir.join(name));
        dir.join(name).join("nested").join("unisson.db")
    }

    #[test]
    fn a_new_store_has_no_favorite() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.favorite_stations().unwrap().is_empty());
    }

    #[test]
    fn favorites_come_back_complete_and_in_order() {
        let store = Store::open_in_memory().unwrap();
        let stations = [
            custom_station("nova"),
            directory_station(),
            custom_station("tsf"),
        ];

        store.save_favorite_stations(&stations).unwrap();

        assert_eq!(store.favorite_stations().unwrap(), stations);
    }

    #[test]
    fn saving_replaces_the_previous_list() {
        let store = Store::open_in_memory().unwrap();
        store
            .save_favorite_stations(&[custom_station("a"), custom_station("b")])
            .unwrap();

        let reordered = [custom_station("b"), custom_station("c")];
        store.save_favorite_stations(&reordered).unwrap();
        assert_eq!(store.favorite_stations().unwrap(), reordered);

        store.save_favorite_stations(&[]).unwrap();
        assert!(store.favorite_stations().unwrap().is_empty());
    }

    #[test]
    fn a_failed_save_keeps_the_previous_list() {
        let store = Store::open_in_memory().unwrap();
        let saved = [custom_station("a")];
        store.save_favorite_stations(&saved).unwrap();

        // Two stations with the same identifier cannot be stored.
        let duplicate = [custom_station("b"), custom_station("b")];
        let error = store.save_favorite_stations(&duplicate).unwrap_err();

        assert!(matches!(error, StoreError::Database(_)), "{error}");
        assert_eq!(store.favorite_stations().unwrap(), saved);
    }

    #[test]
    fn favorites_survive_a_restart() {
        let path = temporary_path("restart");
        let stations = [directory_station(), custom_station("nova")];
        {
            let store = Store::open(&path).unwrap();
            store.save_favorite_stations(&stations).unwrap();
        }
        // Opening again also checks that the layout is not created twice.
        let store = Store::open(&path).unwrap();
        assert_eq!(store.favorite_stations().unwrap(), stations);
    }

    #[test]
    #[cfg(unix)]
    fn the_data_directory_is_private() {
        use std::os::unix::fs::PermissionsExt;

        let path = temporary_path("private");
        Store::open(&path).unwrap();
        let mode = std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn a_database_from_a_newer_version_is_refused() {
        let path = temporary_path("newer");
        Store::open(&path).unwrap();
        {
            let connection = Connection::open(&path).unwrap();
            connection
                .pragma_update(None, "user_version", MIGRATIONS.len() as i64 + 1)
                .unwrap();
        }
        let error = Store::open(&path).unwrap_err();
        assert!(matches!(error, StoreError::Database(_)), "{error}");
    }

    #[test]
    fn a_database_from_an_older_version_is_upgraded_and_keeps_its_data() {
        let path = temporary_path("older");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let station = custom_station("nova");
        {
            // Build a database as the first released layout made it.
            let connection = Connection::open(&path).unwrap();
            connection.execute_batch(MIGRATIONS[0]).unwrap();
            connection.pragma_update(None, "user_version", 1).unwrap();
            connection
                .execute(
                    "INSERT INTO favorite_stations (position, uuid, name, stream_url, tags, hls)
                     VALUES (0, ?1, ?2, ?3, '', 0)",
                    (&station.uuid, &station.name, &station.stream_url),
                )
                .unwrap();
        }

        let store = Store::open(&path).unwrap();

        assert_eq!(store.favorite_stations().unwrap(), [station]);
        // The tables added since then are there and usable.
        assert_eq!(store.library_len().unwrap(), 0);
        assert!(store.playlists().unwrap().is_empty());
        let version: i64 = store
            .connection()
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, MIGRATIONS.len() as i64);
    }

    #[test]
    fn a_file_that_is_not_a_database_is_refused() {
        let path = temporary_path("garbage");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "this is not an SQLite database, just some text").unwrap();
        assert!(matches!(Store::open(&path), Err(StoreError::Database(_))));
    }

    #[test]
    fn a_store_can_be_shared_between_threads() {
        let store = Store::open_in_memory().unwrap();
        std::thread::scope(|scope| {
            for name in ["a", "b", "c", "d"] {
                let store = &store;
                scope.spawn(move || {
                    store
                        .save_favorite_stations(&[custom_station(name)])
                        .unwrap();
                    assert_eq!(store.favorite_stations().unwrap().len(), 1);
                });
            }
        });
    }
}
