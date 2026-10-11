//! The place of MPRIS on systems that do not have it.
//!
//! MPRIS is how a Linux desktop controls a media player: see the `mpris`
//! module. Elsewhere, this module offers the same handle, which does
//! nothing: the rest of the application does not need to know.

use std::time::Duration;

use tauri::AppHandle;
use unisson_playback::Snapshot;

/// A handle that publishes nothing.
#[derive(Debug, Clone)]
pub struct Mpris;

impl Mpris {
    /// Returns the handle. Nothing is started.
    pub fn start(_app: AppHandle) -> Self {
        Self
    }

    /// Does nothing.
    pub fn state_changed(&self, _snapshot: &Snapshot, _has_next: bool, _has_previous: bool) {}

    /// Does nothing.
    pub fn position_changed(&self, _position: Duration) {}
}
