//! Commands of the Spotify card of the "Sources" screen.
//!
//! Signing in, setting up the backup application, and following where
//! Spotify plays. The interface learns each change of the playback status
//! from the [`SPOTIFY_CHANGED`] event.

use std::time::UNIX_EPOCH;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_opener::OpenerExt;
use unisson_spotify::{PlaybackStatus, Problem, Route, Spotify, auth};

use crate::commands::player::source_error;
use crate::commands::settings::SettingsDto;
use crate::dto::CommandError;
use crate::events::{SETTINGS_CHANGED, SPOTIFY_CHANGED, SPOTIFY_SIGN_IN_LINK};
use crate::state::AppState;

/// The backup application, as the interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupDto {
    /// Whether it is set up and signed in.
    pub ready: bool,
    /// Its client ID, current or last, to fill the form again.
    pub client_id: Option<String>,
    /// Whether playback moves to it on its own.
    pub auto_switch: bool,
    /// The redirect address to register for it.
    pub redirect_uri: &'static str,
}

/// Where Spotify plays now, as the interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusDto {
    /// `builtin`, `app`, or nothing when Spotify cannot play.
    pub route: Option<&'static str>,
    /// `limited`, `accountRefused`, `premiumRequired`, `offline`, or
    /// nothing.
    pub problem: Option<&'static str>,
    /// When the built-in playback is tried again, in milliseconds since
    /// 1970.
    pub retry_at_ms: Option<u64>,
}

impl From<PlaybackStatus> for StatusDto {
    fn from(status: PlaybackStatus) -> Self {
        Self {
            route: status.route.map(|route| match route {
                Route::Builtin => "builtin",
                Route::App => "app",
            }),
            problem: status.problem.map(|problem| match problem {
                Problem::Limited => "limited",
                Problem::AccountRefused => "accountRefused",
                Problem::PremiumRequired => "premiumRequired",
                Problem::Offline => "offline",
            }),
            retry_at_ms: status.retry_at.and_then(|at| {
                at.duration_since(UNIX_EPOCH)
                    .ok()
                    .and_then(|since| u64::try_from(since.as_millis()).ok())
            }),
        }
    }
}

/// The Spotify card, as the interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotifyDto {
    /// Whether the built-in playback is signed in.
    pub signed_in: bool,
    /// The backup application.
    pub backup: BackupDto,
    /// Where Spotify plays now.
    pub status: StatusDto,
}

impl SpotifyDto {
    /// Describes the connection.
    pub fn read(spotify: &Spotify) -> Self {
        Self {
            signed_in: spotify.has_account(),
            backup: BackupDto {
                ready: spotify.has_app(),
                client_id: spotify.app_client_id(),
                auto_switch: spotify.auto_switch(),
                redirect_uri: auth::REDIRECT_TO_REGISTER,
            },
            status: spotify.status().into(),
        }
    }
}

/// Starts telling the interface each change of where Spotify plays.
pub fn watch(app: AppHandle, spotify: &std::sync::Arc<Spotify>) {
    let weak = std::sync::Arc::downgrade(spotify);
    spotify.watch(move |_| {
        if let Some(spotify) = weak.upgrade() {
            // A failed emit means the window is closing: nothing to do.
            let _ = app.emit(SPOTIFY_CHANGED, SpotifyDto::read(&spotify));
        }
    });
}

/// Tells the interface that the settings changed, and returns them.
fn changed(app: &AppHandle, state: &AppState) -> Result<SettingsDto, CommandError> {
    let _ = app.emit(SETTINGS_CHANGED, ());
    let _ = app.emit(SPOTIFY_CHANGED, SpotifyDto::read(&state.spotify));
    crate::commands::settings::describe(state)
}

/// Opens the consent page of Spotify in the browser, and tells the
/// interface its address, so the user can copy it if no browser appears.
fn open_browser(app: &AppHandle, address: &str) -> Result<(), String> {
    let _ = app.emit(SPOTIFY_SIGN_IN_LINK, address);
    app.opener()
        .open_url(address, None::<&str>)
        .map_err(|error| format!("cannot open the browser: {error}"))
}

/// Signs in to Spotify: opens its consent page in the browser, and waits
/// for the user to accept, at most five minutes.
#[tauri::command(async)]
pub fn spotify_sign_in(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsDto, CommandError> {
    let page = state.language().texts().spotify_page();
    state
        .spotify
        .sign_in(|address| open_browser(&app, address), &page)
        .map_err(source_error)?;
    changed(&app, &state)
}

/// Stops a sign-in that waits for the browser.
#[tauri::command]
pub fn spotify_cancel_sign_in(state: State<'_, AppState>) {
    state.spotify.cancel_sign_in();
}

/// Signs out of Spotify. The client ID of the backup application is kept.
#[tauri::command(async)]
pub fn spotify_sign_out(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsDto, CommandError> {
    state.spotify.sign_out().map_err(source_error)?;
    changed(&app, &state)
}

/// Sets up the backup application, with the client ID of the application
/// the user registered on the Spotify developer site.
#[tauri::command(async)]
pub fn spotify_set_up_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    client_id: String,
) -> Result<SettingsDto, CommandError> {
    let page = state.language().texts().spotify_page();
    state
        .spotify
        .set_up_app(&client_id, |address| open_browser(&app, address), &page)
        .map_err(source_error)?;
    changed(&app, &state)
}

/// Removes the backup application.
#[tauri::command(async)]
pub fn spotify_remove_backup(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsDto, CommandError> {
    state.spotify.remove_app().map_err(source_error)?;
    changed(&app, &state)
}

/// Chooses whether playback moves to the backup application on its own.
#[tauri::command(async)]
pub fn spotify_set_auto_switch(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<SettingsDto, CommandError> {
    state
        .spotify
        .set_auto_switch(enabled)
        .map_err(source_error)?;
    changed(&app, &state)
}

/// Where the backup application is registered.
const DASHBOARD: &str = "https://developer.spotify.com/dashboard";

/// Opens the Spotify developer site in the browser. The address is fixed:
/// the interface cannot open anything else through this.
#[tauri::command]
pub fn spotify_open_dashboard(app: AppHandle) -> Result<(), CommandError> {
    app.opener()
        .open_url(DASHBOARD, None::<&str>)
        .map_err(|error| CommandError::new("unavailable", error.to_string()))
}

/// Tries the built-in playback again, from the next track, and forgets
/// that Spotify refused it.
#[tauri::command(async)]
pub fn spotify_retry(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsDto, CommandError> {
    state.spotify.set_builtin_refused(false);
    state.spotify.reset_status();
    changed(&app, &state)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn the_status_is_told_in_words_and_milliseconds() {
        let dto = StatusDto::from(PlaybackStatus {
            route: Some(Route::App),
            problem: Some(Problem::AccountRefused),
            retry_at: Some(UNIX_EPOCH + Duration::from_millis(1_234)),
        });
        assert_eq!(
            dto,
            StatusDto {
                route: Some("app"),
                problem: Some("accountRefused"),
                retry_at_ms: Some(1_234),
            }
        );
        assert_eq!(
            StatusDto::from(PlaybackStatus::default()),
            StatusDto {
                route: None,
                problem: None,
                retry_at_ms: None,
            }
        );
    }

    #[test]
    fn a_new_connection_is_described_as_not_signed_in() {
        let dto = SpotifyDto::read(&Spotify::open(None));
        assert!(!dto.signed_in);
        assert!(!dto.backup.ready);
        assert!(dto.backup.auto_switch);
        assert_eq!(dto.backup.redirect_uri, "http://127.0.0.1/callback");
        assert_eq!(dto.status.route, None);
    }
}
