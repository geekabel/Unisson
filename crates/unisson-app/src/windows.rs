//! The windows of the application, and how they are shown.

use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::tray::Tray;

/// The label of the main window.
pub const MAIN: &str = "main";

/// Tells whether closing the main window should only hide it.
///
/// That needs a tray icon, or the user would have no way back to a window
/// they cannot see.
pub fn keeps_running_when_closed(app: &AppHandle) -> bool {
    app.try_state::<Tray>().is_some() && app.state::<AppState>().settings().keep_in_tray
}

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
