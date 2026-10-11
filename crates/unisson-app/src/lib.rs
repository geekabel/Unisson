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
//! The [`dto`] module holds the shapes they exchange. [`home`] builds what
//! the home screen shows, and [`search`] what the unified search reports.

pub mod commands;
pub mod dto;
pub mod events;
pub mod home;
pub mod library;
pub mod search;
pub mod settings;
pub mod state;

use std::sync::Arc;

use tauri::Manager;

use crate::library::LibrarySync;
use crate::state::AppState;

/// Runs the application until its window is closed.
///
/// # Panics
///
/// Panics if the application cannot start: the database cannot be opened,
/// the audio engine is missing, or no window can be created.
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let (state, session_events) = AppState::start()?;
            let store = Arc::clone(&state.store);
            let music_folder = state.music_folder();
            app.manage(state);

            let handle = app.handle();
            events::forward_session_events(handle.clone(), Arc::clone(&store), session_events);
            events::save_settings_when_idle(handle.clone());

            let library = LibrarySync::new(handle.clone(), store);
            if let Some(folder) = music_folder {
                library.follow(folder);
            }
            app.manage(library);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::collection::favorites_list,
            commands::collection::favorite_set,
            commands::collection::history_list,
            commands::collection::history_clear,
            commands::collection::playlists_list,
            commands::collection::playlist_create,
            commands::collection::playlist_rename,
            commands::collection::playlist_delete,
            commands::collection::playlist_items,
            commands::collection::playlist_add,
            commands::collection::playlist_set_items,
            commands::home::home,
            commands::player::player_state,
            commands::player::player_play_list,
            commands::player::player_play_now,
            commands::player::player_play_next,
            commands::player::player_enqueue,
            commands::player::player_open,
            commands::player::player_toggle,
            commands::player::player_next,
            commands::player::player_previous,
            commands::player::player_seek,
            commands::player::player_set_volume,
            commands::queue::queue_state,
            commands::queue::queue_jump,
            commands::queue::queue_remove,
            commands::queue::queue_move,
            commands::queue::queue_clear,
            commands::queue::queue_set_resume_radio,
            commands::radio::stations_favorites,
            commands::radio::stations_search,
            commands::radio::stations_play,
            commands::radio::station_set_favorite,
            commands::radio::station_add_custom,
            commands::search::search,
            commands::settings::settings_get,
            commands::settings::settings_set_source_enabled,
            commands::settings::settings_set_priority,
            commands::settings::settings_pick_music_folder,
            commands::settings::settings_reset_music_folder,
            commands::settings::settings_rescan_library,
        ])
        .run(tauri::generate_context!())
        .expect("cannot run Unisson");
}
