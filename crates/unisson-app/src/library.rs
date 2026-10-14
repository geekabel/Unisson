//! Keeping the library in step with the music folder.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use unisson_local::{LibraryWatcher, scan};
use unisson_store::Store;

use crate::events::LIBRARY_CHANGED;

/// How often a watch checks whether it was replaced by a newer one.
const CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Reads the music folder into the library, then follows its changes.
///
/// Only one folder is followed at a time: each call to
/// [`follow`](Self::follow) replaces the previous watch.
pub struct LibrarySync {
    app: AppHandle,
    store: Arc<Store>,
    /// Number of the current watch. An older watch stops when it sees that
    /// this number moved on.
    generation: Arc<AtomicU64>,
}

impl LibrarySync {
    /// Prepares the synchronization. Nothing is read until
    /// [`follow`](Self::follow) is called.
    pub fn new(app: AppHandle, store: Arc<Store>) -> Self {
        Self {
            app,
            store,
            generation: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Makes `folder` the music folder: its audio files replace the library,
    /// which then follows what appears, changes or disappears in it.
    ///
    /// The work happens on another thread, and the interface is told with
    /// the `library:changed` event. A folder that does not exist leaves the
    /// library as it is: a removable disk may simply be unplugged.
    pub fn follow(&self, folder: PathBuf) {
        let number = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let generation = Arc::clone(&self.generation);
        let app = self.app.clone();
        let store = Arc::clone(&self.store);
        let is_current = move || generation.load(Ordering::SeqCst) == number;

        let spawned = thread::Builder::new()
            .name("unisson-library".into())
            .spawn(move || {
                if !folder.is_dir() {
                    return;
                }
                // Watch first, scan second: a file that arrives during the
                // scan is then reported by the watcher instead of being
                // missed.
                let watch = LibraryWatcher::start(&folder);
                if let Err(error) = &watch {
                    eprintln!("unisson: the music folder is not watched: {error}");
                }

                let scanned = scan(&folder);
                // The user may have chosen another folder during the scan.
                if !is_current() {
                    return;
                }
                match scanned.map(|report| store.replace_library(&report.tracks)) {
                    Ok(Ok(())) => {
                        let _ = app.emit(LIBRARY_CHANGED, ());
                    }
                    Ok(Err(error)) => eprintln!("unisson: cannot save the library: {error}"),
                    Err(error) => eprintln!("unisson: cannot read the music folder: {error}"),
                }

                let Ok((_watcher, changes)) = watch else {
                    return;
                };
                while is_current() {
                    let change = match changes.recv_timeout(CHECK_INTERVAL) {
                        Ok(change) => change,
                        Err(RecvTimeoutError::Timeout) => continue,
                        Err(RecvTimeoutError::Disconnected) => break,
                    };
                    match store.apply_library_change(&change) {
                        Ok(0) => {}
                        Ok(_) => {
                            let _ = app.emit(LIBRARY_CHANGED, ());
                        }
                        Err(error) => eprintln!("unisson: cannot update the library: {error}"),
                    }
                }
            });
        if let Err(error) = spawned {
            eprintln!("unisson: cannot start the library thread: {error}");
        }
    }
}
