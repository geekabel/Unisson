//! Reading and writing a [`Media`] in the database.
//!
//! Several tables store media (history, favorites, playlists). They all use
//! the same eight columns, in the same order, listed in [`MEDIA_COLUMNS`].

use std::time::Duration;

use rusqlite::Row;
use rusqlite::types::Value;
use unisson_core::{Media, MediaId, MediaKind, SourceKind};

/// The columns that describe a media, in the order used everywhere.
pub(crate) const MEDIA_COLUMNS: &str =
    "source, key, kind, title, artist, album, duration_ms, artwork";

/// As many placeholders as [`MEDIA_COLUMNS`] has columns.
pub(crate) const MEDIA_PLACEHOLDERS: &str = "?, ?, ?, ?, ?, ?, ?, ?";

/// Returns the values of a media, in the order of [`MEDIA_COLUMNS`].
pub(crate) fn media_values(media: &Media) -> [Value; 8] {
    // Milliseconds fit in 64 bits for any duration a media can have.
    let duration_ms = media.duration.map(|duration| duration.as_millis() as i64);
    [
        Value::from(media.id.source.id().to_owned()),
        Value::from(media.id.key.clone()),
        Value::from(media.kind.id().to_owned()),
        Value::from(media.title.clone()),
        Value::from(media.artist.clone()),
        Value::from(media.album.clone()),
        Value::from(duration_ms),
        Value::from(media.artwork.clone()),
    ]
}

/// Reads a media from a row whose columns start at `first`.
///
/// Returns `None` for a media written by a newer version of the
/// application, with a source or a kind this version does not know.
pub(crate) fn media_from_row(row: &Row<'_>, first: usize) -> rusqlite::Result<Option<Media>> {
    let source: String = row.get(first)?;
    let kind: String = row.get(first + 2)?;
    let (Some(source), Some(kind)) = (SourceKind::from_id(&source), MediaKind::from_id(&kind))
    else {
        return Ok(None);
    };
    let duration_ms: Option<i64> = row.get(first + 6)?;
    Ok(Some(Media {
        id: MediaId::new(source, row.get::<_, String>(first + 1)?),
        kind,
        title: row.get(first + 3)?,
        artist: row.get(first + 4)?,
        album: row.get(first + 5)?,
        duration: duration_ms
            .and_then(|milliseconds| u64::try_from(milliseconds).ok())
            .map(Duration::from_millis),
        artwork: row.get(first + 7)?,
    }))
}

/// Collects the media of a query, leaving out the ones this version cannot
/// understand.
pub(crate) fn collect_media(
    rows: impl Iterator<Item = rusqlite::Result<Option<Media>>>,
) -> rusqlite::Result<Vec<Media>> {
    let mut media = Vec::new();
    for row in rows {
        media.extend(row?);
    }
    Ok(media)
}

#[cfg(test)]
pub(crate) mod samples {
    //! Media used by the tests of several modules.

    use super::*;

    /// A track with every optional field filled in.
    pub(crate) fn track(title: &str) -> Media {
        Media::track(
            MediaId::new(SourceKind::YouTube, format!("id-{title}")),
            title,
        )
        .with_artist("Miles Davis")
        .with_album("Kind of Blue")
        .with_duration(Duration::from_millis(562_500))
        .with_artwork("https://example.org/cover.jpg")
    }

    /// A live station, with only the mandatory fields.
    pub(crate) fn station(name: &str) -> Media {
        Media::station(
            MediaId::new(SourceKind::Radio, format!("uuid-{name}")),
            name,
        )
    }
}
