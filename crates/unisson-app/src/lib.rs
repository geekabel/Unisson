//! Unisson desktop application.
//!
//! This crate is the window: a [Tauri](https://tauri.app) application whose
//! interface is a web page, kept in the `ui` folder and embedded in the
//! program at build time. Everything else lives in the other crates.
//!
//! The two sides talk in two ways:
//!
//! - the interface calls the [`commands`], to ask or to order something;
//! - the application sends [`events`], to tell that something changed.
//!
//! The [`dto`] module holds the shapes they exchange, and [`home`] builds
//! what the home screen shows.

pub mod commands;
pub mod dto;
pub mod events;
pub mod home;
pub mod state;

use std::sync::Arc;

use tauri::Manager;

use crate::state::AppState;

/// Runs the application until its window is closed.
///
/// # Panics
///
/// Panics if the application cannot start: the database cannot be opened,
/// the audio engine is missing, or no window can be created.
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let (state, session_events) = AppState::start()?;
            let store = Arc::clone(&state.store);
            app.manage(state);

            let handle = app.handle();
            events::forward_session_events(handle.clone(), Arc::clone(&store), session_events);
            events::keep_library_in_sync(handle.clone(), store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::player_state,
            commands::player_play_list,
            commands::player_open,
            commands::player_toggle,
            commands::player_next,
            commands::player_previous,
            commands::player_seek,
            commands::player_set_volume,
            commands::home,
        ])
        .run(tauri::generate_context!())
        .expect("cannot run Unisson");
}
