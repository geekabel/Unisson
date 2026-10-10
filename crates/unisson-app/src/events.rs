//! Events sent to the web interface.
//!
//! The interface never asks "what changed?": the application tells it. Each
//! background activity gets a thread here, which turns what happens into a
//! Tauri event.

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::SystemTime;

use tauri::{AppHandle, Emitter};
use unisson_local::{LibraryWatcher, music_directory, scan};
use unisson_playback::SessionEvent;
use unisson_store::Store;

use crate::dto::{PositionDto, SnapshotDto};

/// The playback state changed. Payload: [`SnapshotDto`].
pub const PLAYER_CHANGED: &str = "player:changed";
/// Playback moved on. Payload: [`PositionDto`].
pub const PLAYER_POSITION: &str = "player:position";
/// The library of local files changed. No payload.
pub const LIBRARY_CHANGED: &str = "library:changed";
/// A media was added to the listening history. No payload.
pub const HISTORY_CHANGED: &str = "history:changed";

/// Forwards the session events to the interface, until the session ends.
///
/// It also adds each media that starts to the listening history.
pub fn forward_session_events(app: AppHandle, store: Arc<Store>, events: Receiver<SessionEvent>) {
    spawn("unisson-player-events", move || {
        for event in events {
            // A failed emit means the window is closing: nothing to do.
            let _ = match event {
                SessionEvent::Changed(snapshot) => {
                    app.emit(PLAYER_CHANGED, SnapshotDto::from(&snapshot))
                }
                SessionEvent::Position { position, duration } => {
                    app.emit(PLAYER_POSITION, PositionDto::new(position, duration))
                }
                SessionEvent::Started(media) => {
                    match store.record_play(&media, SystemTime::now()) {
                        Ok(()) => app.emit(HISTORY_CHANGED, ()),
                        Err(error) => {
                            eprintln!("unisson: cannot write the history: {error}");
                            Ok(())
                        }
                    }
                }
            };
        }
    });
}

/// Keeps the library in step with the music folder, for as long as the
/// application runs.
///
/// The folder is read once at startup, then watched for changes.
pub fn keep_library_in_sync(app: AppHandle, store: Arc<Store>) {
    spawn("unisson-library", move || {
        let Some(folder) = music_directory().filter(|folder| folder.is_dir()) else {
            return;
        };
        // Watch first, scan second: a file that arrives during the scan is
        // then reported by the watcher instead of being missed.
        let watch = LibraryWatcher::start(&folder);
        if let Err(error) = &watch {
            eprintln!("unisson: the music folder is not watched: {error}");
        }

        match scan(&folder).map(|report| store.replace_library(&report.tracks)) {
            Ok(Ok(())) => {
                let _ = app.emit(LIBRARY_CHANGED, ());
            }
            Ok(Err(error)) => eprintln!("unisson: cannot save the library: {error}"),
            Err(error) => eprintln!("unisson: cannot read the music folder: {error}"),
        }

        let Ok((_watcher, changes)) = watch else {
            return;
        };
        for change in changes {
            match store.apply_library_change(&change) {
                Ok(0) => {}
                Ok(_) => {
                    let _ = app.emit(LIBRARY_CHANGED, ());
                }
                Err(error) => eprintln!("unisson: cannot update the library: {error}"),
            }
        }
    });
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(error) = thread::Builder::new().name(name.into()).spawn(work) {
        eprintln!("unisson: cannot start the {name} thread: {error}");
    }
}
