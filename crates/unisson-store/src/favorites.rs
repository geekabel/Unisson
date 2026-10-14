//! The favorite media: what the user marked with the heart button.

use rusqlite::params_from_iter;
use unisson_core::{Media, MediaId};

use crate::error::StoreError;
use crate::media::{
    MEDIA_COLUMNS, MEDIA_PLACEHOLDERS, collect_media, media_from_row, media_values,
};
use crate::store::Store;

impl Store {
    /// Marks a media as a favorite.
    ///
    /// Returns `false` if it already was one.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the favorite cannot be written.
    pub fn add_favorite(&self, media: &Media) -> Result<bool, StoreError> {
        let added = self.connection().execute(
            &format!(
                "INSERT OR IGNORE INTO favorite_media ({MEDIA_COLUMNS})
                 VALUES ({MEDIA_PLACEHOLDERS})"
            ),
            params_from_iter(media_values(media)),
        )?;
        Ok(added > 0)
    }

    /// Removes a media from the favorites.
    ///
    /// Returns `false` if it was not a favorite.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the favorite cannot be removed.
    pub fn remove_favorite(&self, id: &MediaId) -> Result<bool, StoreError> {
        let removed = self.connection().execute(
            "DELETE FROM favorite_media WHERE source = ?1 AND key = ?2",
            (id.source.id(), &id.key),
        )?;
        Ok(removed > 0)
    }

    /// Returns `true` if the media is a favorite.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the favorites cannot be read.
    pub fn is_favorite(&self, id: &MediaId) -> Result<bool, StoreError> {
        Ok(self.connection().query_row(
            "SELECT EXISTS (SELECT 1 FROM favorite_media WHERE source = ?1 AND key = ?2)",
            (id.source.id(), &id.key),
            |row| row.get(0),
        )?)
    }

    /// Returns the favorite media, the most recently added first.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the favorites cannot be read.
    pub fn favorites(&self) -> Result<Vec<Media>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {MEDIA_COLUMNS} FROM favorite_media ORDER BY id DESC"
        ))?;
        let rows = statement.query_map((), |row| media_from_row(row, 0))?;
        Ok(collect_media(rows)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::samples::{station, track};

    #[test]
    fn favorites_come_back_complete_and_latest_first() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.add_favorite(&track("So What")).unwrap());
        assert!(store.add_favorite(&station("FIP")).unwrap());

        assert_eq!(
            store.favorites().unwrap(),
            [station("FIP"), track("So What")]
        );
    }

    #[test]
    fn a_media_is_a_favorite_only_once() {
        let store = Store::open_in_memory().unwrap();
        assert!(store.add_favorite(&track("a")).unwrap());
        assert!(!store.add_favorite(&track("a")).unwrap());
        assert_eq!(store.favorites().unwrap().len(), 1);
    }

    #[test]
    fn remove_and_is_favorite_follow_the_list() {
        let store = Store::open_in_memory().unwrap();
        let media = track("a");
        assert!(!store.is_favorite(&media.id).unwrap());

        store.add_favorite(&media).unwrap();
        assert!(store.is_favorite(&media.id).unwrap());
        // The same key in another source is another media.
        assert!(!store.is_favorite(&station("a").id).unwrap());

        assert!(store.remove_favorite(&media.id).unwrap());
        assert!(!store.remove_favorite(&media.id).unwrap());
        assert!(!store.is_favorite(&media.id).unwrap());
    }
}
