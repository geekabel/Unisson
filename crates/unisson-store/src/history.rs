//! The listening history.

use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::params_from_iter;
use rusqlite::types::Value;
use unisson_core::Media;

use crate::error::StoreError;
use crate::media::{
    MEDIA_COLUMNS, MEDIA_PLACEHOLDERS, collect_media, media_from_row, media_values,
};
use crate::store::Store;

impl Store {
    /// Most plays kept in the history. Older ones are forgotten.
    pub const HISTORY_LIMIT: usize = 5000;

    /// Adds a play to the history.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the play cannot be written.
    pub fn record_play(&self, media: &Media, played_at: SystemTime) -> Result<(), StoreError> {
        // Seconds since 1970. A clock set before that date counts as 1970.
        let played_at = played_at.duration_since(UNIX_EPOCH).map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        });
        let values = std::iter::once(Value::from(played_at)).chain(media_values(media));

        let connection = self.connection();
        connection.execute(
            &format!(
                "INSERT INTO history (played_at, {MEDIA_COLUMNS}) VALUES (?, {MEDIA_PLACEHOLDERS})"
            ),
            params_from_iter(values),
        )?;
        connection.execute(
            "DELETE FROM history WHERE id <= (SELECT MAX(id) FROM history) - ?1",
            [Self::HISTORY_LIMIT as i64],
        )?;
        Ok(())
    }

    /// Returns the media played most recently, the latest first.
    ///
    /// Each media appears once, at the place of its latest play.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the history cannot be read.
    pub fn recently_played(&self, limit: usize) -> Result<Vec<Media>, StoreError> {
        let connection = self.connection();
        let mut statement = connection.prepare(&format!(
            "SELECT {MEDIA_COLUMNS} FROM history
             WHERE id IN (SELECT MAX(id) FROM history GROUP BY source, key)
             ORDER BY id DESC
             LIMIT ?1"
        ))?;
        let rows = statement.query_map([limit as i64], |row| media_from_row(row, 0))?;
        Ok(collect_media(rows)?)
    }

    /// Forgets the whole history.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the history cannot be erased.
    pub fn clear_history(&self) -> Result<(), StoreError> {
        self.connection().execute("DELETE FROM history", ())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;
    use crate::media::samples::{station, track};

    fn at(seconds: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn titles(media: &[Media]) -> Vec<&str> {
        media.iter().map(|media| media.title.as_str()).collect()
    }

    #[test]
    fn plays_come_back_complete_and_latest_first() {
        let store = Store::open_in_memory().unwrap();
        store.record_play(&track("So What"), at(100)).unwrap();
        store.record_play(&station("FIP"), at(200)).unwrap();

        let recent = store.recently_played(10).unwrap();

        assert_eq!(recent, [station("FIP"), track("So What")]);
    }

    #[test]
    fn a_media_played_twice_appears_once_at_its_latest_place() {
        let store = Store::open_in_memory().unwrap();
        for (title, time) in [("a", 1), ("b", 2), ("a", 3), ("c", 4)] {
            store.record_play(&track(title), at(time)).unwrap();
        }
        assert_eq!(titles(&store.recently_played(10).unwrap()), ["c", "a", "b"]);
    }

    #[test]
    fn the_limit_is_respected() {
        let store = Store::open_in_memory().unwrap();
        for title in ["a", "b", "c"] {
            store.record_play(&track(title), at(1)).unwrap();
        }
        assert_eq!(titles(&store.recently_played(2).unwrap()), ["c", "b"]);
        assert!(store.recently_played(0).unwrap().is_empty());
    }

    #[test]
    fn old_plays_are_forgotten() {
        let store = Store::open_in_memory().unwrap();
        for index in 0..Store::HISTORY_LIMIT + 3 {
            store
                .record_play(&track(&index.to_string()), at(index as u64))
                .unwrap();
        }
        let count: i64 = store
            .connection()
            .query_row("SELECT COUNT(*) FROM history", (), |row| row.get(0))
            .unwrap();
        assert_eq!(count as usize, Store::HISTORY_LIMIT);
        let recent = store.recently_played(1).unwrap();
        assert_eq!(recent[0].title, (Store::HISTORY_LIMIT + 2).to_string());
    }

    #[test]
    fn clear_history_forgets_everything() {
        let store = Store::open_in_memory().unwrap();
        store.record_play(&track("a"), at(1)).unwrap();
        store.clear_history().unwrap();
        assert!(store.recently_played(10).unwrap().is_empty());
    }

    #[test]
    fn a_media_from_an_unknown_source_is_left_out() {
        let store = Store::open_in_memory().unwrap();
        store.record_play(&track("known"), at(1)).unwrap();
        // As if a newer version of the application had written this play.
        store
            .connection()
            .execute(
                "INSERT INTO history (played_at, source, key, kind, title)
                 VALUES (2, 'deezer', 'x', 'track', 'unknown')",
                (),
            )
            .unwrap();
        assert_eq!(titles(&store.recently_played(10).unwrap()), ["known"]);
    }
}
