//! Watching a folder for audio files that appear, change or disappear.
//!
//! # Design
//!
//! The operating system reports raw events: a file was created, written to,
//! renamed... Copying one song produces dozens of them, and the first ones
//! arrive while the file is still incomplete.
//!
//! The watcher therefore does not trust what an event says. An event only
//! marks a path as "to look at". Once nothing has happened to that path for
//! a short while, the watcher looks at what is really on disk and reports
//! one [`LibraryChange`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::event::{AccessKind, AccessMode};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::error::LocalError;
use crate::scan::{is_audio_file, scan};
use crate::track::LocalTrack;

/// How often the watcher checks whether some paths have settled.
const TICK: Duration = Duration::from_millis(100);

/// A change to apply to the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryChange {
    /// An audio file is new, or its content changed. Its tags were read.
    Updated(LocalTrack),
    /// A path is gone: a single file, or a folder with everything in it.
    ///
    /// The watcher cannot tell which, nor whether the path held any audio.
    /// The library should forget the track at this path and every track
    /// below it.
    Removed(PathBuf),
}

/// Watches a folder and its sub-folders.
///
/// Dropping the watcher stops the watch.
///
/// # Examples
///
/// ```no_run
/// use unisson_local::{LibraryChange, LibraryWatcher};
///
/// let (_watcher, changes) = LibraryWatcher::start("/home/me/Music")?;
/// for change in changes {
///     match change {
///         LibraryChange::Updated(track) => println!("new or changed: {}", track.title),
///         LibraryChange::Removed(path) => println!("gone: {}", path.display()),
///     }
/// }
/// # Ok::<(), unisson_local::LocalError>(())
/// ```
#[derive(Debug)]
pub struct LibraryWatcher {
    /// Kept alive for as long as the watch must go on.
    watcher: Option<RecommendedWatcher>,
    thread: Option<JoinHandle<()>>,
}

impl LibraryWatcher {
    /// How long a path must stay quiet before it is looked at, by default.
    pub const DEFAULT_SETTLE_DELAY: Duration = Duration::from_secs(1);

    /// Starts watching `root`, and returns the receiver of the changes.
    ///
    /// Hidden files and folders (whose name starts with a dot) are ignored,
    /// like in [`scan`](crate::scan). Changes made before the watch started
    /// are not reported: scan the folder for those.
    ///
    /// Reported paths start with `root` as it was given, like the paths
    /// found by [`scan`](crate::scan), even when `root` is a symbolic link.
    ///
    /// # Errors
    ///
    /// Returns [`LocalError::Io`] if the folder cannot be watched, for
    /// instance because it does not exist.
    pub fn start(root: impl AsRef<Path>) -> Result<(Self, Receiver<LibraryChange>), LocalError> {
        Self::start_with_delay(root, Self::DEFAULT_SETTLE_DELAY)
    }

    /// Same as [`start`](Self::start), with a custom settle delay.
    ///
    /// A longer delay copes better with slow copies. A shorter one reports
    /// changes sooner.
    ///
    /// # Errors
    ///
    /// Same as [`start`](Self::start).
    pub fn start_with_delay(
        root: impl AsRef<Path>,
        settle_delay: Duration,
    ) -> Result<(Self, Receiver<LibraryChange>), LocalError> {
        let root = root.as_ref().to_owned();
        let watch_error =
            |error: notify::Error| LocalError::Io(format!("{}: {error}", root.display()));
        // Events carry real paths, with symbolic links followed.
        let real_root = root
            .canonicalize()
            .map_err(|error| LocalError::Io(format!("{}: {error}", root.display())))?;

        let (events, inbox) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(events).map_err(watch_error)?;
        watcher
            .watch(&real_root, RecursiveMode::Recursive)
            .map_err(watch_error)?;

        let (changes, outbox) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("unisson-library-watcher".into())
            .spawn(move || {
                Worker {
                    root,
                    real_root,
                    settle_delay,
                    changes,
                    pending: HashMap::new(),
                }
                .run(&inbox)
            })
            .map_err(|error| LocalError::Io(error.to_string()))?;

        Ok((
            Self {
                watcher: Some(watcher),
                thread: Some(thread),
            },
            outbox,
        ))
    }
}

impl Drop for LibraryWatcher {
    fn drop(&mut self) {
        // Dropping the system watcher closes the event channel, which makes
        // the worker thread finish.
        self.watcher.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The state kept by the watcher thread.
struct Worker {
    /// The watched folder, as the caller named it.
    root: PathBuf,
    /// The same folder, with symbolic links followed.
    real_root: PathBuf,
    settle_delay: Duration,
    changes: Sender<LibraryChange>,
    /// Paths to look at, with the time of their last event.
    pending: HashMap<PathBuf, Instant>,
}

impl Worker {
    /// Runs until the system watcher is dropped.
    fn run(mut self, inbox: &Receiver<notify::Result<Event>>) {
        loop {
            match inbox.recv_timeout(TICK) {
                Ok(Ok(event)) => self.note(event),
                // A failed event is lost, but the watch itself goes on.
                Ok(Err(_)) | Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
            self.report_settled();
        }
    }

    /// Remembers that the paths of an event must be looked at.
    fn note(&mut self, event: Event) {
        // Reading a file to get its tags is itself reported as an access.
        // Only the end of a write matters among accesses.
        let relevant = match event.kind {
            EventKind::Access(kind) => kind == AccessKind::Close(AccessMode::Write),
            _ => true,
        };
        if !relevant {
            return;
        }
        let now = Instant::now();
        for path in event.paths {
            // The root itself is not a track, and nothing outside it is
            // expected.
            let Ok(inside) = path.strip_prefix(&self.real_root) else {
                continue;
            };
            if inside.as_os_str().is_empty() || is_hidden(inside) {
                continue;
            }
            self.pending.insert(self.root.join(inside), now);
        }
    }

    /// Looks at the paths that have been quiet long enough.
    fn report_settled(&mut self) {
        let settle_delay = self.settle_delay;
        let settled: Vec<PathBuf> = self
            .pending
            .iter()
            .filter(|(_, last_event)| last_event.elapsed() >= settle_delay)
            .map(|(path, _)| path.clone())
            .collect();
        for path in settled {
            self.pending.remove(&path);
            self.report(&path);
        }
    }

    /// Reports what is really at `path` now.
    fn report(&self, path: &Path) {
        match std::fs::metadata(path) {
            // A folder that just arrived, for instance moved in: its files
            // did not get events of their own.
            Ok(metadata) if metadata.is_dir() => {
                if let Ok(report) = scan(path) {
                    for track in report.tracks {
                        self.emit(LibraryChange::Updated(track));
                    }
                }
            }
            Ok(metadata) if metadata.is_file() && is_audio_file(path) => {
                // An unreadable file is probably still being written: its
                // next event will bring it back here.
                if let Ok(track) = LocalTrack::read(path) {
                    self.emit(LibraryChange::Updated(track));
                }
            }
            Ok(_) => {}
            Err(_) => self.emit(LibraryChange::Removed(path.to_owned())),
        }
    }

    fn emit(&self, change: LibraryChange) {
        // Nobody listening is not an error.
        let _ = self.changes.send(change);
    }
}

/// Returns `true` if a path relative to the root is, or is inside, a hidden
/// entry.
fn is_hidden(inside: &Path) -> bool {
    inside
        .components()
        .any(|component| component.as_os_str().to_string_lossy().starts_with('.'))
}
