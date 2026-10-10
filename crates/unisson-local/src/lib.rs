//! Local audio files for Unisson.
//!
//! The crate finds the audio files of a folder and reads what they say about
//! themselves: title, artist, album, duration and cover picture.
//!
//! - [`scan`] walks a folder and returns a [`LocalTrack`] per audio file.
//! - [`LocalTrack::read`] reads the tags of a single file.
//! - [`read_cover`] extracts the cover picture of a file, on demand.
//! - [`music_directory`] finds the user's music folder.
//! - [`LibraryWatcher`] reports the files that appear, change or disappear
//!   in a folder while the application runs.
//!
//! # Examples
//!
//! ```no_run
//! use unisson_local::{music_directory, scan};
//!
//! let folder = music_directory().ok_or("no music folder")?;
//! let report = scan(&folder)?;
//! for track in &report.tracks {
//!     println!("{} ({})", track.title, track.path.display());
//! }
//! println!("{} file(s) could not be read", report.unreadable.len());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod dirs;
mod error;
mod scan;
mod track;
mod watch;

pub use dirs::music_directory;
pub use error::LocalError;
pub use scan::{ScanReport, is_audio_file, scan};
pub use track::{Cover, LocalTrack, read_cover};
pub use watch::{LibraryChange, LibraryWatcher};
