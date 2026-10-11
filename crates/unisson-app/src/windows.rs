//! The windows of the application, and how they are shown.
//!
//! The main window exists from the start. The small ones are created the
//! first time they are needed, then only shown and hidden: opening them
//! again is instant.
//!
//! # Giving the keyboard to a window
//!
//! A desktop does not let a program take the keyboard whenever it wants:
//! that would let any window steal what the user types. Under X11, the
//! window manager compares two moments: the last time the user did
//! something in the program, and the last time they did something
//! anywhere. A window whose program was idle since then is shown, but
//! left without the keyboard.
//!
//! The launcher is exactly that case: Unisson was idle, the user worked
//! elsewhere, then pressed the shortcut. The shortcut is the action of the
//! user that justifies the keyboard, but it reached GNOME, not Unisson. So
//! this module asks the X server for the current time and gives it as the
//! moment of that action. Under Wayland there is no such time: a window
//! that appears gets the keyboard if the desktop agrees.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gtk::prelude::{Cast, GtkWindowExt, WidgetExt};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::events::LAUNCHER_OPENED;
use crate::shortcut::LAUNCHER_ARGUMENT;

use crate::state::AppState;
use crate::tray::Tray;

/// The label of the main window.
pub const MAIN: &str = "main";
/// The label of the mini-player: a small bar that stays above the other
/// windows.
pub const MINI: &str = "mini";

/// The label of the quick launcher: a search box that opens above the
/// current work.
pub const LAUNCHER: &str = "launcher";

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

/// How long the launcher ignores the loss of the keyboard after it opens.
/// While a window appears, the desktop may move the keyboard back and
/// forth: those moves say nothing about what the user wants.
const LAUNCHER_SETTLE_TIME: Duration = Duration::from_millis(400);

/// What is known about the keyboard and the quick launcher.
///
/// The launcher goes away when the user clicks elsewhere, like any pop-up.
/// "Elsewhere" only means something once the launcher really had the
/// keyboard, which the desktop may refuse or delay.
#[derive(Debug, Default)]
pub struct LauncherFocus {
    /// When the launcher was last opened.
    opened_at: Mutex<Option<Instant>>,
    /// Whether it got the keyboard since then.
    had_keyboard: AtomicBool,
}

impl LauncherFocus {
    fn opened(&self) {
        self.had_keyboard.store(false, Ordering::Relaxed);
        if let Ok(mut opened_at) = self.opened_at.lock() {
            *opened_at = Some(Instant::now());
        }
    }

    /// Notes that the launcher got or lost the keyboard, and tells whether
    /// it should now be put away.
    fn keyboard_changed(&self, focused: bool) -> bool {
        if focused {
            self.had_keyboard.store(true, Ordering::Relaxed);
            return false;
        }
        let settled = self
            .opened_at
            .lock()
            .ok()
            .and_then(|opened_at| *opened_at)
            .is_some_and(|opened_at| opened_at.elapsed() >= LAUNCHER_SETTLE_TIME);
        settled && self.had_keyboard.load(Ordering::Relaxed)
    }
}

/// Reacts to the launcher getting or losing the keyboard: once it had it,
/// losing it puts the launcher away.
pub fn launcher_focus_changed(app: &AppHandle, focused: bool) {
    if app.state::<LauncherFocus>().keyboard_changed(focused) {
        hide_launcher(app);
    }
}

/// Opens the quick launcher above the other windows.
pub fn show_launcher(app: &AppHandle) {
    let window = app.get_webview_window(LAUNCHER).map_or_else(
        || {
            WebviewWindowBuilder::new(app, LAUNCHER, WebviewUrl::App("launcher.html".into()))
                .title("Unisson, lanceur rapide")
                .inner_size(640.0, 460.0)
                .resizable(false)
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .center()
                // Shown by `present`, which knows how to ask for the
                // keyboard.
                .visible(false)
                .build()
        },
        Ok,
    );
    match window {
        Ok(window) => {
            app.state::<LauncherFocus>().opened();
            // Each time: the screen may have changed since the last one.
            // Wayland ignores it and places the window itself.
            let _ = window.center();
            present(&window);
            // The page empties its box and takes the keyboard.
            let _ = app.emit_to(LAUNCHER, LAUNCHER_OPENED, ());
        }
        Err(error) => eprintln!("unisson: cannot open the launcher: {error}"),
    }
}

/// Puts the quick launcher away.
pub fn hide_launcher(app: &AppHandle) {
    hide(app, LAUNCHER);
}

/// Reacts to Unisson being started: with the launcher argument, the
/// launcher opens. Otherwise the main window comes back.
///
/// It serves the first start, and every later one: a second copy hands its
/// arguments over to the first and exits.
pub fn handle_launch(app: &AppHandle, arguments: &[String]) {
    // A copy that could not start only shows why: it has no window to
    // offer. See the `startup` module.
    if app.try_state::<AppState>().is_none() {
        return;
    }
    if arguments
        .iter()
        .any(|argument| argument == LAUNCHER_ARGUMENT)
    {
        show_launcher(app);
    } else {
        show_main(app);
    }
}

/// Shows a window and gives it the keyboard.
fn present(window: &WebviewWindow) {
    // Each step may be refused by the desktop: do what is allowed.
    let _ = window.unminimize();
    // GTK may only be used from the main thread.
    let target = window.clone();
    let asked = window.run_on_main_thread(move || {
        if present_with_gtk(&target).is_none() {
            show_plainly(&target);
        }
    });
    if asked.is_err() {
        show_plainly(window);
    }
}

/// Shows a window the simple way, which leaves the keyboard to the good
/// will of the desktop.
fn show_plainly(window: &WebviewWindow) {
    let _ = window.show();
    let _ = window.set_focus();
}

/// Shows a window through GTK, with the current time of the X server as
/// the moment of the user action. See the notes at the top of this file.
///
/// Returns `None` when the window cannot be reached through GTK.
fn present_with_gtk(window: &WebviewWindow) -> Option<()> {
    let gtk_window = window.gtk_window().ok()?;
    if !gtk_window.is_realized() {
        gtk_window.realize();
    }
    let x11_window = gtk_window
        .window()
        .and_then(|window| window.downcast::<gdkx11::X11Window>().ok());
    match x11_window {
        Some(x11_window) => {
            let now = gdkx11::functions::x11_get_server_time(&x11_window);
            // Before the window appears: the window manager decides at
            // that moment whether it gets the keyboard.
            x11_window.set_user_time(now);
            gtk_window.show_all();
            gtk_window.present_with_time(now);
        }
        None => {
            gtk_window.show_all();
            gtk_window.present();
        }
    }
    Some(())
}

fn hide(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
}
