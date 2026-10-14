//! Finding the audio files of a folder.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::error::LocalError;
use crate::track::LocalTrack;

/// File extensions treated as audio, in lower case.
const AUDIO_EXTENSIONS: [&str; 12] = [
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "m4b", "aac", "wav", "aiff", "aif", "wv",
];

/// The result of a [`scan`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// The audio files found, sorted by path.
    pub tracks: Vec<LocalTrack>,
    /// The audio files and folders that could not be read, with the reason.
    pub unreadable: Vec<(PathBuf, LocalError)>,
}

/// Returns `true` if the path has the extension of an audio file.
///
/// The check is on the name only: the file is not opened.
///
/// # Examples
///
/// ```
/// use unisson_local::is_audio_file;
///
/// assert!(is_audio_file("album/01 So What.FLAC"));
/// assert!(!is_audio_file("album/cover.jpg"));
/// ```
pub fn is_audio_file(path: impl AsRef<Path>) -> bool {
    path.as_ref()
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| AUDIO_EXTENSIONS.contains(&extension.to_lowercase().as_str()))
}

/// Finds every audio file under `root`, and reads its tags.
///
/// Sub-folders are visited too. Hidden files and folders (whose name starts
/// with a dot) are skipped. A file or a sub-folder that cannot be read does
/// not stop the scan: it is listed in [`ScanReport::unreadable`].
///
/// # Errors
///
/// Returns [`LocalError::Io`] if `root` itself cannot be listed, for
/// instance because it does not exist.
pub fn scan(root: impl AsRef<Path>) -> Result<ScanReport, LocalError> {
    let root = root.as_ref();
    let io_error =
        |path: &Path, error: std::io::Error| LocalError::Io(format!("{}: {error}", path.display()));
    // Fail early if the starting folder is unusable.
    std::fs::read_dir(root).map_err(|error| io_error(root, error))?;

    let mut report = ScanReport::default();
    let mut pending = vec![root.to_owned()];
    // A link can point back to a parent folder: remember the real location
    // of each visited folder so none is visited twice.
    let mut visited = HashSet::new();

    while let Some(folder) = pending.pop() {
        let real_folder = folder.canonicalize().unwrap_or_else(|_| folder.clone());
        if !visited.insert(real_folder) {
            continue;
        }
        let entries = match std::fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) => {
                report
                    .unreadable
                    .push((folder.clone(), io_error(&folder, error)));
                continue;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            // `metadata` follows links, unlike the entry's own file type.
            match std::fs::metadata(&path) {
                Ok(metadata) if metadata.is_dir() => pending.push(path),
                Ok(metadata) if metadata.is_file() && is_audio_file(&path) => {
                    match LocalTrack::read(&path) {
                        Ok(track) => report.tracks.push(track),
                        Err(error) => report.unreadable.push((path, error)),
                    }
                }
                // Not audio, or a broken link: nothing to report.
                _ => {}
            }
        }
    }

    report.tracks.sort_by(|a, b| a.path.cmp(&b.path));
    report.unreadable.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(report)
}
