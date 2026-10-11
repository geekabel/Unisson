//! Events sent to the web interface.
//!
//! The interface never asks "what changed?": the application tells it. Each
//! background activity gets a thread here, which turns what happens into a
//! Tauri event.

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, SystemTime};

use tauri::{AppHandle, Emitter, Manager};
use unisson_playback::SessionEvent;
use unisson_store::Store;

use crate::dto::PositionDto;
use crate::mpris::Mpris;
use crate::state::AppState;
use crate::tray;

/// The playback state changed. Payload: [`SnapshotDto`](crate::dto::SnapshotDto).
pub const PLAYER_CHANGED: &str = "player:changed";
/// Playback moved on. Payload: [`PositionDto`].
pub const PLAYER_POSITION: &str = "player:position";
/// The settings changed. No payload.
pub const SETTINGS_CHANGED: &str = "settings:changed";
/// How long a changed setting may wait before it is written to the file.
const SAVE_INTERVAL: Duration = Duration::from_secs(1);
/// The library of local files changed. No payload.
pub const LIBRARY_CHANGED: &str = "library:changed";
/// The play queue changed. No payload.
pub const QUEUE_CHANGED: &str = "queue:changed";
/// The favorite media changed. No payload.
pub const FAVORITES_CHANGED: &str = "favorites:changed";
/// The playlists changed: their list, a name, or what one holds. No payload.
pub const PLAYLISTS_CHANGED: &str = "playlists:changed";
/// The favorite stations changed. No payload.
pub const STATIONS_CHANGED: &str = "stations:changed";
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
                    let state = app.state::<AppState>();
                    state.note_volume(snapshot.volume);
                    let dto = state.snapshot_dto(&snapshot);
                    app.state::<Mpris>()
                        .state_changed(&snapshot, dto.has_next, dto.has_previous);
                    tray::refresh(&app);
                    app.emit(PLAYER_CHANGED, dto)
                }
                SessionEvent::Position { position, duration } => {
                    app.state::<Mpris>().position_changed(position);
                    app.emit(PLAYER_POSITION, PositionDto::new(position, duration))
                }
                SessionEvent::QueueChanged => app.emit(QUEUE_CHANGED, ()),
                SessionEvent::Started(media) => {
                    app.state::<AppState>().note_started(&media);
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

/// Writes the settings to their file a moment after they changed.
///
/// Some settings change many times in a row, such as the volume while its
/// slider moves. Waiting for a pause avoids writing the file each time.
pub fn save_settings_when_idle(app: AppHandle) {
    spawn("unisson-settings", move || {
        loop {
            thread::sleep(SAVE_INTERVAL);
            app.state::<AppState>().save_settings_if_changed();
        }
    });
}

fn spawn(name: &str, work: impl FnOnce() + Send + 'static) {
    if let Err(error) = thread::Builder::new().name(name.into()).spawn(work) {
        eprintln!("unisson: cannot start the {name} thread: {error}");
    }
}
