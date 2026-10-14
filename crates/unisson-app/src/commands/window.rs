//! Commands that switch between the windows of the application.

use tauri::{AppHandle, Emitter};

use crate::events::FOCUS_SEARCH;
use crate::windows;

/// Shows the main window, and puts the mini-player away.
#[tauri::command]
pub fn window_show_main(app: AppHandle) {
    windows::show_main(&app);
}

/// Shows the main window with the keyboard in its search box.
///
/// With a `text`, the box is filled with it and the search starts.
#[tauri::command]
pub fn window_show_search(app: AppHandle, text: Option<String>) {
    windows::hide_launcher(&app);
    windows::show_main(&app);
    // A failed emit means the window is closing: nothing to do.
    let _ = app.emit_to(windows::MAIN, FOCUS_SEARCH, text);
}

/// Opens the quick launcher above the other windows.
#[tauri::command]
pub fn window_show_launcher(app: AppHandle) {
    windows::show_launcher(&app);
}

/// Puts the quick launcher away.
#[tauri::command]
pub fn window_hide_launcher(app: AppHandle) {
    windows::hide_launcher(&app);
}

/// Replaces the main window with the mini-player.
#[tauri::command]
pub fn window_show_mini(app: AppHandle) {
    windows::show_mini(&app);
}
