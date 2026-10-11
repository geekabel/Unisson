//! Entry point of the Unisson desktop application.

// On Windows, a release build is a window program: without this, a
// console opens behind the window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    unisson_app::run();
}
