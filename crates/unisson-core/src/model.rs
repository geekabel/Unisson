//! Common model shared by every source.
//!
//! A [`Media`] describes something that can be listened to: a track or a
//! radio station. It does not hold the stream address. That address
//! ([`StreamUri`]) is requested from the source right before playback,
//! because it may expire.

use std::fmt;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::time::Duration;

use crate::error::SourceError;

/// The sources Unisson knows about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourceKind {
    /// Internet radio stations.
    Radio,
    /// Audio files stored on disk.
    Local,
    /// YouTube.
    YouTube,
}

impl SourceKind {
    /// Returns the short name shown in the interface (the source badge).
    pub fn label(self) -> &'static str {
        match self {
            Self::Radio => "Radio",
            Self::Local => "Local",
            Self::YouTube => "YouTube",
        }
    }
}

impl SourceKind {
    /// Returns the stable identifier of the source, such as `radio`.
    ///
    /// Unlike [`label`](Self::label), it never changes: it is what gets
    /// written to disk.
    pub fn id(self) -> &'static str {
        match self {
            Self::Radio => "radio",
            Self::Local => "local",
            Self::YouTube => "youtube",
        }
    }

    /// Returns the source with the given [`id`](Self::id), if there is one.
    pub fn from_id(id: &str) -> Option<Self> {
        [Self::Radio, Self::Local, Self::YouTube]
            .into_iter()
            .find(|kind| kind.id() == id)
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Identifies a media: its source, plus a key that is unique in that source.
///
/// The key is, for instance, a station identifier, a file path or a video
/// identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaId {
    /// The source that knows this media.
    pub source: SourceKind,
    /// The key of the media within that source.
    pub key: String,
}

impl MediaId {
    /// Creates an identifier.
    pub fn new(source: SourceKind, key: impl Into<String>) -> Self {
        Self {
            source,
            key: key.into(),
        }
    }
}

/// The nature of a media.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// A track with a beginning and an end.
    Track,
    /// A live station, with no end.
    Station,
}

impl MediaKind {
    /// Returns the stable identifier of the kind, such as `track`.
    ///
    /// It never changes: it is what gets written to disk.
    pub fn id(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Station => "station",
        }
    }

    /// Returns the kind with the given [`id`](Self::id), if there is one.
    pub fn from_id(id: &str) -> Option<Self> {
        [Self::Track, Self::Station]
            .into_iter()
            .find(|kind| kind.id() == id)
    }
}

/// Something that can be put in the play queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    /// Identifier of the media.
    pub id: MediaId,
    /// Whether this is a track or a live station.
    pub kind: MediaKind,
    /// Track title, or station name.
    pub title: String,
    /// Artist, when the source knows it.
    pub artist: Option<String>,
    /// Album, when the source knows it.
    pub album: Option<String>,
    /// Duration. Always `None` for a live station.
    pub duration: Option<Duration>,
    /// URL or path of the picture (cover art, station logo).
    pub artwork: Option<String>,
}

impl Media {
    /// Creates a track.
    ///
    /// The optional fields are set with the `with_*` methods.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::time::Duration;
    /// use unisson_core::{Media, MediaId, SourceKind};
    ///
    /// let media = Media::track(MediaId::new(SourceKind::Local, "/music/a.flac"), "So What")
    ///     .with_artist("Miles Davis")
    ///     .with_duration(Duration::from_secs(562));
    /// assert!(!media.is_live());
    /// ```
    pub fn track(id: MediaId, title: impl Into<String>) -> Self {
        Self::new(id, MediaKind::Track, title)
    }

    /// Creates a live radio station.
    pub fn station(id: MediaId, title: impl Into<String>) -> Self {
        Self::new(id, MediaKind::Station, title)
    }

    fn new(id: MediaId, kind: MediaKind, title: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            title: title.into(),
            artist: None,
            album: None,
            duration: None,
            artwork: None,
        }
    }

    /// Sets the artist.
    pub fn with_artist(mut self, artist: impl Into<String>) -> Self {
        self.artist = Some(artist.into());
        self
    }

    /// Sets the album.
    pub fn with_album(mut self, album: impl Into<String>) -> Self {
        self.album = Some(album.into());
        self
    }

    /// Sets the duration. Has no effect on a live station.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        if self.kind == MediaKind::Track {
            self.duration = Some(duration);
        }
        self
    }

    /// Sets the picture.
    pub fn with_artwork(mut self, artwork: impl Into<String>) -> Self {
        self.artwork = Some(artwork.into());
        self
    }

    /// Returns `true` for a live stream: no duration and no progress bar.
    pub fn is_live(&self) -> bool {
        self.kind == MediaKind::Station
    }
}

/// An absolute address that an audio engine can play.
///
/// Only the `file://`, `http://` and `https://` schemes are accepted.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamUri(String);

impl StreamUri {
    const SCHEMES: [&'static str; 3] = ["file://", "http://", "https://"];

    /// Validates then wraps an address.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Invalid`] if the address does not start with a
    /// supported scheme, or has nothing after the scheme.
    ///
    /// # Examples
    ///
    /// ```
    /// use unisson_core::StreamUri;
    ///
    /// let uri = StreamUri::new("https://example.org/stream.mp3")?;
    /// assert_eq!(uri.as_str(), "https://example.org/stream.mp3");
    /// assert!(StreamUri::new("stream.mp3").is_err());
    /// # Ok::<(), unisson_core::SourceError>(())
    /// ```
    pub fn new(uri: impl Into<String>) -> Result<Self, SourceError> {
        let uri = uri.into();
        let known = Self::SCHEMES
            .iter()
            .any(|scheme| uri.len() > scheme.len() && uri.starts_with(scheme));
        if known {
            Ok(Self(uri))
        } else {
            Err(SourceError::Invalid(format!(
                "unsupported stream URI: {uri}"
            )))
        }
    }

    /// Builds the `file://` address of a local file.
    ///
    /// The path is not checked on disk. Characters that are not allowed in
    /// an address, such as spaces and accents, are percent-encoded.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Invalid`] if the path is not absolute.
    ///
    /// # Examples
    ///
    /// ```
    /// use unisson_core::StreamUri;
    ///
    /// let uri = StreamUri::from_file_path("/home/me/Music/été 2010.mp3")?;
    /// assert_eq!(uri.as_str(), "file:///home/me/Music/%C3%A9t%C3%A9%202010.mp3");
    /// # Ok::<(), unisson_core::SourceError>(())
    /// ```
    pub fn from_file_path(path: impl AsRef<Path>) -> Result<Self, SourceError> {
        let path = path.as_ref();
        if !path.is_absolute() {
            return Err(SourceError::Invalid(format!(
                "a file address needs an absolute path: {}",
                path.display()
            )));
        }
        let mut uri = String::from("file://");
        for byte in path.as_os_str().as_bytes() {
            // RFC 3986 allows these characters as they are. `/` is kept
            // too, since it separates the folders.
            if byte.is_ascii_alphanumeric() || b"-._~/".contains(byte) {
                uri.push(char::from(*byte));
            } else {
                uri.push_str(&format!("%{byte:02X}"));
            }
        }
        Ok(Self(uri))
    }

    /// Returns the address as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StreamUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_station_is_live_and_has_no_duration() {
        let id = MediaId::new(SourceKind::Radio, "fip");
        let station = Media::station(id, "FIP").with_duration(Duration::from_secs(60));
        assert!(station.is_live());
        assert_eq!(station.duration, None);
    }

    #[test]
    fn a_track_keeps_its_details() {
        let id = MediaId::new(SourceKind::YouTube, "abc123");
        let track = Media::track(id.clone(), "Take Five")
            .with_artist("The Dave Brubeck Quartet")
            .with_album("Time Out")
            .with_duration(Duration::from_secs(324))
            .with_artwork("https://example.org/cover.jpg");
        assert_eq!(track.id, id);
        assert!(!track.is_live());
        assert_eq!(track.artist.as_deref(), Some("The Dave Brubeck Quartet"));
        assert_eq!(track.album.as_deref(), Some("Time Out"));
        assert_eq!(track.duration, Some(Duration::from_secs(324)));
        assert!(track.artwork.is_some());
    }

    #[test]
    fn stream_uri_accepts_only_known_schemes() {
        for good in ["file:///home/a.mp3", "http://a.fr/x", "https://a.fr/x"] {
            assert_eq!(StreamUri::new(good).unwrap().as_str(), good);
        }
        for bad in ["", "a.mp3", "/home/a.mp3", "ftp://a.fr/x", "https://"] {
            assert!(
                matches!(StreamUri::new(bad), Err(SourceError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_file_path_becomes_an_escaped_address() {
        let uri = StreamUri::from_file_path("/music/AC DC/100% rock & roll #1?.mp3").unwrap();
        assert_eq!(
            uri.as_str(),
            "file:///music/AC%20DC/100%25%20rock%20%26%20roll%20%231%3F.mp3"
        );
        assert_eq!(
            StreamUri::from_file_path("/a-b_c.d~e/f").unwrap().as_str(),
            "file:///a-b_c.d~e/f"
        );
        assert!(matches!(
            StreamUri::from_file_path("music/a.mp3"),
            Err(SourceError::Invalid(_))
        ));
    }

    #[test]
    fn identifiers_are_stable_and_reversible() {
        for kind in [SourceKind::Radio, SourceKind::Local, SourceKind::YouTube] {
            assert_eq!(SourceKind::from_id(kind.id()), Some(kind));
        }
        for kind in [MediaKind::Track, MediaKind::Station] {
            assert_eq!(MediaKind::from_id(kind.id()), Some(kind));
        }
        // These values are stored on disk: they must never change.
        assert_eq!(SourceKind::YouTube.id(), "youtube");
        assert_eq!(MediaKind::Station.id(), "station");
        assert_eq!(SourceKind::from_id("spotify"), None);
        assert_eq!(MediaKind::from_id("Track"), None);
    }

    #[test]
    fn source_labels_match_the_interface() {
        assert_eq!(SourceKind::Radio.to_string(), "Radio");
        assert_eq!(SourceKind::Local.to_string(), "Local");
        assert_eq!(SourceKind::YouTube.to_string(), "YouTube");
    }
}
