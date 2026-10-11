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
pub mod mpris;
pub mod notify;
pub mod now_playing;
pub mod search;
pub mod settings;
pub mod shortcut;
pub mod startup;
pub mod state;
pub mod tray;
pub mod windows;

use std::sync::Arc;

use tauri::Manager;

use crate::library::LibrarySync;
use crate::mpris::Mpris;
use crate::notify::Notifier;
use crate::state::AppState;
use crate::tray::Tray;

/// Runs the application until its window is closed.
///
/// # Panics
///
/// Panics if the application cannot start: the database cannot be opened,
/// the audio engine is missing, or no window can be created.
pub fn run() {
    tauri::Builder::default()
        // This plugin must come first. When Unisson is already open, the
        // new copy hands its arguments over here and exits.
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            windows::handle_launch(app, &arguments);
        }))
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let (state, session_events) = match AppState::start() {
                Ok(started) => started,
                Err(error) => {
                    // Without its state, the application offers nothing:
                    // say why, then end.
                    startup::report_failure(app.handle(), error.as_ref());
                    return Ok(());
                }
            };
            let store = Arc::clone(&state.store);
            let music_folder = state.music_folder();
            app.manage(state);
            windows::apply_theme(app.handle());
            windows::set_main_icon(app.handle());

            let handle = app.handle();
            app.manage(Mpris::start(handle.clone()));
            app.manage(Notifier::start());
            app.manage(windows::LauncherFocus::default());
            events::forward_session_events(handle.clone(), Arc::clone(&store), session_events);
            events::save_settings_when_idle(handle.clone());

            let library = LibrarySync::new(handle.clone(), store);
            if let Some(folder) = music_folder {
                library.follow(folder);
            }
            app.manage(library);

            // Without a tray icon, the application simply has none.
            match Tray::create(handle) {
                Ok(tray) => {
                    app.manage(tray);
                }
                Err(error) => eprintln!("unisson: no tray icon: {error}"),
            }

            // Started by the keyboard shortcut while it was closed: the
            // launcher is what was asked for.
            let arguments: Vec<String> = std::env::args().collect();
            if arguments
                .iter()
                .any(|argument| argument == shortcut::LAUNCHER_ARGUMENT)
            {
                windows::show_launcher(handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            if let tauri::WindowEvent::Focused(focused) = event {
                if window.label() == windows::LAUNCHER {
                    windows::launcher_focus_changed(app, *focused);
                }
                return;
            }
            let tauri::WindowEvent::CloseRequested { api, .. } = event else {
                return;
            };
            match window.label() {
                windows::MAIN if windows::keeps_running_when_closed(app) => {
                    // Keep playing: only hide the window. The tray icon,
                    // the media keys, or starting Unisson again bring it
                    // back.
                    api.prevent_close();
                    let _ = window.hide();
                }
                // Closing the main window for good ends the application,
                // small windows included.
                windows::MAIN => app.exit(0),
                // The launcher is only put away: it opens again at once.
                windows::LAUNCHER => {
                    api.prevent_close();
                    windows::hide_launcher(app);
                }
                // Closing the mini-player goes back to the main window.
                windows::MINI => {
                    api.prevent_close();
                    windows::show_main(app);
                }
                _ => {}
            }
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
            commands::settings::settings_check_yt_dlp,
            commands::settings::settings_set_keep_in_tray,
            commands::settings::settings_set_notifications,
            commands::settings::settings_theme,
            commands::settings::settings_set_theme,
            commands::settings::settings_install_shortcut,
            commands::settings::settings_remove_shortcut,
            commands::window::window_show_main,
            commands::window::window_show_mini,
            commands::window::window_show_search,
            commands::window::window_show_launcher,
            commands::window::window_hide_launcher,
        ])
        .run(tauri::generate_context!())
        .expect("cannot run Unisson");
}
