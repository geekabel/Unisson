//! One audio file and its tags.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::file::TaggedFile;
use lofty::picture::PictureType;
use lofty::prelude::*;
use lofty::tag::Tag;
use unisson_core::{Media, MediaId, SourceKind};

use crate::error::LocalError;

/// An audio file of the disk, described by its tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalTrack {
    /// Where the file is.
    pub path: PathBuf,
    /// Title from the tags, or the file name without its extension.
    pub title: String,
    /// Artist, when the tags give one.
    pub artist: Option<String>,
    /// Album, when the tags give one.
    pub album: Option<String>,
    /// Position of the track in its album, when the tags give one.
    pub track_number: Option<u32>,
    /// Duration of the audio, when it can be worked out.
    pub duration: Option<Duration>,
    /// Whether the file embeds a cover picture. See [`read_cover`].
    pub has_cover: bool,
}

impl LocalTrack {
    /// Reads the tags of an audio file.
    ///
    /// A file without tags is fine: its title is then its file name.
    ///
    /// # Errors
    ///
    /// Returns [`LocalError::Io`] if the file cannot be opened, or
    /// [`LocalError::Unreadable`] if it is not an audio file the tag reader
    /// understands.
    pub fn read(path: impl AsRef<Path>) -> Result<Self, LocalError> {
        let path = path.as_ref();
        let file = open(path)?;
        let tag = main_tag(&file);
        let text = |value: Option<std::borrow::Cow<'_, str>>| {
            value
                .map(|value| value.trim().to_owned())
                .filter(|value| !value.is_empty())
        };
        let file_name = || {
            path.file_stem()
                .map_or_else(String::new, |stem| stem.to_string_lossy().into_owned())
        };
        let duration = file.properties().duration();
        Ok(Self {
            path: path.to_owned(),
            title: text(tag.and_then(Accessor::title)).unwrap_or_else(file_name),
            artist: text(tag.and_then(Accessor::artist)),
            album: text(tag.and_then(Accessor::album)),
            track_number: tag.and_then(Accessor::track),
            // The reader gives zero when it cannot tell.
            duration: (!duration.is_zero()).then_some(duration),
            has_cover: tag.is_some_and(|tag| !tag.pictures().is_empty()),
        })
    }

    /// Converts the track into the common [`Media`] model.
    ///
    /// The media belongs to the local source, and its key is the file path.
    pub fn to_media(&self) -> Media {
        let id = MediaId::new(SourceKind::Local, self.path.to_string_lossy());
        let mut media = Media::track(id, &self.title);
        if let Some(artist) = &self.artist {
            media = media.with_artist(artist);
        }
        if let Some(album) = &self.album {
            media = media.with_album(album);
        }
        if let Some(duration) = self.duration {
            media = media.with_duration(duration);
        }
        media
    }
}

/// A cover picture embedded in an audio file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    /// Media type of the picture, such as `image/jpeg`, when it is known.
    pub mime_type: Option<String>,
    /// The picture file itself.
    pub data: Vec<u8>,
}

/// Extracts the cover picture embedded in an audio file.
///
/// The front cover is preferred. Without one, the first picture is used.
/// Returns `None` if the file has no picture.
///
/// Covers are read on demand, and not during a [`scan`](crate::scan), so a
/// large library does not fill the memory with pictures.
///
/// # Errors
///
/// Same as [`LocalTrack::read`].
pub fn read_cover(path: impl AsRef<Path>) -> Result<Option<Cover>, LocalError> {
    let file = open(path.as_ref())?;
    let Some(pictures) = main_tag(&file).map(Tag::pictures) else {
        return Ok(None);
    };
    let picture = pictures
        .iter()
        .find(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures.first());
    Ok(picture.map(|picture| Cover {
        mime_type: picture
            .mime_type()
            .map(|mime_type| mime_type.as_str().to_owned()),
        data: picture.data().to_vec(),
    }))
}

/// Opens an audio file with the tag reader.
fn open(path: &Path) -> Result<TaggedFile, LocalError> {
    // Checked first, so a missing file is not reported as a damaged one.
    std::fs::metadata(path)
        .map_err(|error| LocalError::Io(format!("{}: {error}", path.display())))?;
    lofty::read_from_path(path)
        .map_err(|error| LocalError::Unreadable(format!("{}: {error}", path.display())))
}

/// Returns the tag to trust: the usual one for the format, or else any tag.
fn main_tag(file: &TaggedFile) -> Option<&Tag> {
    file.primary_tag().or_else(|| file.first_tag())
}
