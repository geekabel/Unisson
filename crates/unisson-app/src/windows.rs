//! The windows of the application, and how they are shown.

use tauri::{AppHandle, Manager};

/// The label of the main window.
pub const MAIN: &str = "main";

/// Brings the main window back in front of the user.
pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        return;
    };
    // Each step may be refused by the desktop: do what is allowed.
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}
