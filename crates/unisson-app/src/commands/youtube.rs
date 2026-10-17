//! Commands of the YouTube card of the "Sources" screen.
//!
//! The component YouTube needs is downloaded the first time YouTube is
//! used: see [`unisson_youtube::Component`]. The interface follows the
//! download through the [`YOUTUBE_CHANGED`] event.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use unisson_youtube::Status;

use crate::events::YOUTUBE_CHANGED;
use crate::state::AppState;

/// The YouTube component, as the interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct YoutubeDto {
    /// `absent`, `installing`, `ready`, `failed`, or `manual` when Unisson
    /// cannot download the component for this system: then the user
    /// installs it by hand.
    pub state: &'static str,
    /// How far the download is, from 0 to 100, when known.
    pub percent: Option<u8>,
    /// The version of the program that answers now, if any. It may be the
    /// one of the `PATH` while the component is downloaded.
    pub version: Option<String>,
    /// Why the download failed, in English.
    pub detail: Option<String>,
}

impl YoutubeDto {
    /// Describes the component. Asking the version may start the program:
    /// pass the version when it is already known.
    pub fn read(state: &AppState, version: Option<String>) -> Self {
        let (name, percent, detail) = match state.youtube.component().map(|c| c.status()) {
            None => ("manual", None, None),
            Some(Status::Absent) => ("absent", None, None),
            Some(Status::Installing { percent }) => ("installing", percent, None),
            Some(Status::Ready) => ("ready", None, None),
            Some(Status::Failed(reason)) => ("failed", None, Some(reason)),
        };
        Self {
            state: name,
            percent,
            version,
            detail,
        }
    }
}

/// Starts telling the interface each change of the component.
pub fn watch(app: AppHandle, state: &AppState) {
    let Some(component) = state.youtube.component() else {
        return;
    };
    component.watch(move |status| {
        let Some(state) = app.try_state::<AppState>() else {
            return;
        };
        // A new program is in place: its version is asked again. During the
        // download, the version known so far is enough.
        let refresh = matches!(status, Status::Ready | Status::Failed(_));
        let version = state.yt_dlp_version(refresh);
        // A failed emit means the window is closing: nothing to do.
        let _ = app.emit(YOUTUBE_CHANGED, YoutubeDto::read(&state, version));
    });
}

/// Downloads the component now, also right after a failure.
#[tauri::command]
pub fn youtube_install(state: State<'_, AppState>) {
    if let Some(component) = state.youtube.component() {
        component.install_now();
    }
}
