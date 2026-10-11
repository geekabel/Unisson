//! The playlists: named lists that can mix media from every source.

use rusqlite::types::Value;
use rusqlite::{Connection, params_from_iter};
use unisson_core::Media;

use crate::error::StoreError;
use crate::media::{
    MEDIA_COLUMNS, MEDIA_PLACEHOLDERS, collect_media, media_from_row, media_values,
};
use crate::store::Store;

/// A playlist, without its items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Playlist {
    /// Identifier of the playlist in the database.
    pub id: i64,
    /// Name given by the user.
    pub name: String,
    /// Number of media in the playlist.
    pub len: usize,
}

/// Trims a playlist name, and refuses an empty one.
fn checked_name(name: &str) -> Result<&str, StoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StoreError::Invalid("a playlist needs a name".into()));
    }
    Ok(name)
}

fn exists(connection: &Connection, id: i64) -> rusqlite::Result<bool> {
    connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM playlists WHERE id = ?1)",
        [id],
        |row| row.get(0),
    )
}

fn insert_item(
    connection: &Connection,
    playlist_id: i64,
    position: i64,
    media: &Media,
) -> rusqlite::Result<()> {
    let values = [Value::from(playlist_id), Value::from(position)]
        .into_iter()
        .chain(media_values(media));
    connection.execute(
        &format!(
            "INSERT INTO playlist_items (playlist_id, position, {MEDIA_COLUMNS})
             VALUES (?, ?, {MEDIA_PLACEHOLDERS})"
        ),
        params_from_iter(values),
    )?;
    Ok(())
}

impl Store {
    /// Creates an empty playlist.
    ///
    /// Two playlists may have the same name.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Invalid`] if the name is empty, or
    /// [`StoreError::Database`] if the playlist cannot be written.
    pub fn create_playlist(&self, name: &str) -> Result<Playlist, StoreError> {
        let name = checked_name(name)?;
        let connection = self.connection();
        connection.execute("INSERT INTO playlists (name) VALUES (?1)", [name])?;
        Ok(Playlist {
            id: connection.last_insert_rowid(),
            name: name.to_owned(),
            len: 0,
        })
    }

    /// Renames a playlist.
    ///
    /// Returns `false` if there is no playlist with this identifier.
    ///
    /// # Errors
    ///
    /// Same as [`create_playlist`](Self::create_playlist).
    pub fn rename_playlist(&self, id: i64, name: &str) -> Result<bool, StoreError> {
        let name = checked_name(name)?;
        let renamed = self
            .connection()
            .execute("UPDATE playlists SET name = ?1 WHERE id = ?2", (name, id))?;
        Ok(renamed > 0)
    }

    /// Deletes a playlist and its items.
    ///
    /// Returns `false` if there is no playlist with this identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the playlist cannot be deleted.
    pub fn delete_playlist(&self, id: i64) -> Result<bool, StoreError> {
        // The items go with it: the tables are linked with ON DELETE CASCADE.
        let deleted = self
            .connection()
            .execute("DELETE FROM playlists WHERE id = ?1", [id])?;
        Ok(deleted > 0)
    }

    /// Returns the playlists, in the order they were created.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the playlists cannot be read.
    pub fn playlists(&self) -> Result<Vec<Playlist>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(
            "SELECT id, name,
                    (SELECT COUNT(*) FROM playlist_items WHERE playlist_id = playlists.id)
             FROM playlists
             ORDER BY id",
        )?;
        let playlists = statement.query_map((), |row| {
            let len: i64 = row.get(2)?;
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                len: len as usize,
            })
        })?;
        Ok(playlists.collect::<Result<_, _>>()?)
    }

    /// Returns the media of a playlist, in order.
    ///
    /// The list is empty if there is no playlist with this identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the playlist cannot be read.
    pub fn playlist_items(&self, id: i64) -> Result<Vec<Media>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {MEDIA_COLUMNS} FROM playlist_items
             WHERE playlist_id = ?1
             ORDER BY position"
        ))?;
        let rows = statement.query_map([id], |row| media_from_row(row, 0))?;
        Ok(collect_media(rows)?)
    }

    /// Adds a media at the end of a playlist.
    ///
    /// The same media can be added several times. Returns `false` if there
    /// is no playlist with this identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the media cannot be written.
    pub fn add_to_playlist(&self, id: i64, media: &Media) -> Result<bool, StoreError> {
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        if !exists(&transaction, id)? {
            return Ok(false);
        }
        let position: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM playlist_items WHERE playlist_id = ?1",
            [id],
            |row| row.get(0),
        )?;
        insert_item(&transaction, id, position, media)?;
        transaction.commit()?;
        Ok(true)
    }

    /// Replaces the media of a playlist with `items`, in that order.
    ///
    /// This is how the interface saves a playlist after the user reordered
    /// or removed items. The change is all or nothing. Returns `false` if
    /// there is no playlist with this identifier.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the media cannot be written.
    pub fn set_playlist_items(&self, id: i64, items: &[Media]) -> Result<bool, StoreError> {
        let mut connection = self.connection();
        let transaction = connection.transaction()?;
        if !exists(&transaction, id)? {
            return Ok(false);
        }
        transaction.execute("DELETE FROM playlist_items WHERE playlist_id = ?1", [id])?;
        for (position, media) in (0_i64..).zip(items) {
            insert_item(&transaction, id, position, media)?;
        }
        transaction.commit()?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::samples::{station, track};

    #[test]
    fn playlists_are_listed_in_creation_order_with_their_size() {
        let store = Store::open_in_memory().unwrap();
        let focus = store.create_playlist("  Concentration ").unwrap();
        let writing = store.create_playlist("Rédaction").unwrap();
        store.add_to_playlist(writing.id, &track("a")).unwrap();

        assert_eq!(focus.name, "Concentration");
        assert_eq!(
            store.playlists().unwrap(),
            [
                Playlist {
                    id: focus.id,
                    name: "Concentration".into(),
                    len: 0
                },
                Playlist {
                    id: writing.id,
                    name: "Rédaction".into(),
                    len: 1
                },
            ]
        );
    }

    #[test]
    fn a_playlist_needs_a_name() {
        let store = Store::open_in_memory().unwrap();
        assert!(matches!(
            store.create_playlist("  "),
            Err(StoreError::Invalid(_))
        ));
        let playlist = store.create_playlist("a").unwrap();
        assert!(matches!(
            store.rename_playlist(playlist.id, ""),
            Err(StoreError::Invalid(_))
        ));
    }

    #[test]
    fn a_playlist_mixes_sources_keeps_its_order_and_allows_repeats() {
        let store = Store::open_in_memory().unwrap();
        let playlist = store.create_playlist("Mix").unwrap();
        let items = [track("So What"), station("FIP"), track("So What")];
        for item in &items {
            assert!(store.add_to_playlist(playlist.id, item).unwrap());
        }
        assert_eq!(store.playlist_items(playlist.id).unwrap(), items);
    }

    #[test]
    fn set_playlist_items_replaces_the_content() {
        let store = Store::open_in_memory().unwrap();
        let playlist = store.create_playlist("Mix").unwrap();
        store.add_to_playlist(playlist.id, &track("a")).unwrap();

        let reordered = [track("c"), track("b")];
        assert!(store.set_playlist_items(playlist.id, &reordered).unwrap());
        assert_eq!(store.playlist_items(playlist.id).unwrap(), reordered);

        // Adding afterwards still goes to the end.
        store.add_to_playlist(playlist.id, &track("d")).unwrap();
        assert_eq!(
            store.playlist_items(playlist.id).unwrap().last(),
            Some(&track("d"))
        );
    }

    #[test]
    fn rename_changes_only_the_name() {
        let store = Store::open_in_memory().unwrap();
        let playlist = store.create_playlist("Old").unwrap();
        store.add_to_playlist(playlist.id, &track("a")).unwrap();

        assert!(store.rename_playlist(playlist.id, " New ").unwrap());

        assert_eq!(store.playlists().unwrap()[0].name, "New");
        assert_eq!(store.playlist_items(playlist.id).unwrap(), [track("a")]);
    }

    #[test]
    fn deleting_a_playlist_deletes_its_items_only() {
        let store = Store::open_in_memory().unwrap();
        let doomed = store.create_playlist("Doomed").unwrap();
        let kept = store.create_playlist("Kept").unwrap();
        store.add_to_playlist(doomed.id, &track("a")).unwrap();
        store.add_to_playlist(kept.id, &track("b")).unwrap();

        assert!(store.delete_playlist(doomed.id).unwrap());

        assert_eq!(store.playlists().unwrap().len(), 1);
        assert_eq!(store.playlist_items(kept.id).unwrap(), [track("b")]);
        let orphans: i64 = store
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM playlist_items WHERE playlist_id = ?1",
                [doomed.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(orphans, 0);
    }

    #[test]
    fn an_unknown_playlist_is_reported_not_created() {
        let store = Store::open_in_memory().unwrap();
        assert!(!store.add_to_playlist(42, &track("a")).unwrap());
        assert!(!store.set_playlist_items(42, &[track("a")]).unwrap());
        assert!(!store.rename_playlist(42, "name").unwrap());
        assert!(!store.delete_playlist(42).unwrap());
        assert!(store.playlist_items(42).unwrap().is_empty());
        assert!(store.playlists().unwrap().is_empty());
    }
}
