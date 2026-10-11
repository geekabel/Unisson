//! The windows of the application, and how they are shown.
//!
//! The main window exists from the start. The small ones are created the
//! first time they are needed, then only shown and hidden: opening them
//! again is instant.

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::state::AppState;
use crate::tray::Tray;

/// The label of the main window.
pub const MAIN: &str = "main";
/// The label of the mini-player: a small bar that stays above the other
/// windows.
pub const MINI: &str = "mini";

/// Tells whether closing the main window should only hide it.
///
/// That needs a tray icon, or the user would have no way back to a window
/// they cannot see.
pub fn keeps_running_when_closed(app: &AppHandle) -> bool {
    app.try_state::<Tray>().is_some() && app.state::<AppState>().settings().keep_in_tray
}

/// Brings the main window back in front of the user, and puts the
/// mini-player away.
pub fn show_main(app: &AppHandle) {
    hide(app, MINI);
    if let Some(window) = app.get_webview_window(MAIN) {
        present(&window);
    }
}

/// Replaces the main window with the mini-player.
pub fn show_mini(app: &AppHandle) {
    let window = app.get_webview_window(MINI).map_or_else(
        || {
            WebviewWindowBuilder::new(app, MINI, WebviewUrl::App("mini.html".into()))
                .title("Unisson, mini-lecteur")
                .inner_size(520.0, 96.0)
                .resizable(false)
                .decorations(false)
                // The desktop may ignore these two: Wayland leaves the
                // choice to the user.
                .always_on_top(true)
                .skip_taskbar(true)
                .build()
        },
        Ok,
    );
    match window {
        Ok(window) => {
            present(&window);
            hide(app, MAIN);
        }
        // Keep the main window: a missing mini-player must not leave the
        // user without any window.
        Err(error) => eprintln!("unisson: cannot open the mini-player: {error}"),
    }
}

/// Shows a window and gives it the keyboard.
fn present(window: &WebviewWindow) {
    // Each step may be refused by the desktop: do what is allowed.
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

fn hide(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
}
