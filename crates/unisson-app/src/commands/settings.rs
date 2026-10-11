//! Commands of the "Sources and settings" screen.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use unisson_core::SourceKind;

use crate::dto::CommandError;
use crate::events::SETTINGS_CHANGED;
use crate::library::LibrarySync;
use crate::state::AppState;

/// One source, as the settings screen shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceSettingDto {
    /// The source, as [`SourceKind::id`].
    pub id: String,
    /// Whether the source is switched on.
    pub enabled: bool,
}

/// The settings, as the interface sees them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    /// The sources of this version of the application, from the most to
    /// the least preferred.
    pub sources: Vec<SourceSettingDto>,
    /// The folder of the local library, when one is known.
    pub music_folder: Option<String>,
    /// Whether that folder is the music folder of the desktop, as opposed
    /// to one the user chose.
    pub music_folder_is_default: bool,
    /// Whether that folder exists.
    pub music_folder_exists: bool,
    /// The number of tracks in the library.
    pub library_total: usize,
    /// Whether the last radio station takes over when the queue runs out.
    pub resume_radio: bool,
    /// Where the settings are saved.
    pub config_path: Option<String>,
}

fn describe(state: &AppState) -> Result<SettingsDto, CommandError> {
    let settings = state.settings();
    let registered = state.sources.kinds();
    let folder = state.music_folder();
    Ok(SettingsDto {
        sources: settings
            .priority
            .iter()
            .filter(|source| registered.contains(source))
            .map(|source| SourceSettingDto {
                id: source.id().to_owned(),
                enabled: settings.is_enabled(*source),
            })
            .collect(),
        music_folder_exists: folder.as_ref().is_some_and(|folder| folder.is_dir()),
        music_folder: folder.map(|folder| folder.to_string_lossy().into_owned()),
        music_folder_is_default: settings.music_folder.is_none(),
        library_total: state.store.library_len()?,
        resume_radio: settings.resume_radio,
        config_path: state
            .settings_path()
            .map(|path| path.to_string_lossy().into_owned()),
    })
}

fn source(id: &str) -> Result<SourceKind, CommandError> {
    SourceKind::from_id(id).ok_or_else(|| CommandError::invalid(format!("unknown source: {id}")))
}

/// Tells the interface that the settings changed, and returns them.
fn changed(app: &AppHandle, state: &AppState) -> Result<SettingsDto, CommandError> {
    // A failed emit means the window is closing: nothing to do.
    let _ = app.emit(SETTINGS_CHANGED, ());
    describe(state)
}

/// Returns the settings.
#[tauri::command(async)]
pub fn settings_get(state: State<'_, AppState>) -> Result<SettingsDto, CommandError> {
    describe(&state)
}

/// Switches a source on or off. A source that is off is not searched.
#[tauri::command(async)]
pub fn settings_set_source_enabled(
    app: AppHandle,
    state: State<'_, AppState>,
    source_id: String,
    enabled: bool,
) -> Result<SettingsDto, CommandError> {
    let source = source(&source_id)?;
    state.update_settings(|settings| settings.set_enabled(source, enabled))?;
    changed(&app, &state)
}

/// Sets the preferred order of the sources: when a track exists in several
/// sources, the first one of the list is used.
#[tauri::command(async)]
pub fn settings_set_priority(
    app: AppHandle,
    state: State<'_, AppState>,
    priority: Vec<String>,
) -> Result<SettingsDto, CommandError> {
    let priority = priority
        .iter()
        .map(|id| source(id))
        .collect::<Result<Vec<_>, _>>()?;
    state.update_settings(|settings| settings.set_priority(&priority))?;
    changed(&app, &state)
}

/// Lets the user choose the music folder in a system dialog, then reads it.
///
/// Returns the settings, unchanged if the dialog was closed without a
/// choice.
#[tauri::command(async)]
pub fn settings_pick_music_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    library: State<'_, LibrarySync>,
) -> Result<SettingsDto, CommandError> {
    let mut dialog = app
        .dialog()
        .file()
        .set_title("Choisir le dossier de musique");
    if let Some(current) = state.music_folder().filter(|folder| folder.is_dir()) {
        dialog = dialog.set_directory(current);
    }
    let Some(chosen) = dialog.blocking_pick_folder() else {
        return describe(&state);
    };
    let folder = chosen
        .into_path()
        .map_err(|error| CommandError::invalid(error.to_string()))?;
    state.update_settings(|settings| settings.music_folder = Some(folder.clone()))?;
    library.follow(folder);
    changed(&app, &state)
}

/// Goes back to the music folder of the desktop.
#[tauri::command(async)]
pub fn settings_reset_music_folder(
    app: AppHandle,
    state: State<'_, AppState>,
    library: State<'_, LibrarySync>,
) -> Result<SettingsDto, CommandError> {
    state.update_settings(|settings| settings.music_folder = None)?;
    if let Some(folder) = state.music_folder() {
        library.follow(folder);
    }
    changed(&app, &state)
}

/// Reads the music folder again, from scratch.
#[tauri::command(async)]
pub fn settings_rescan_library(
    state: State<'_, AppState>,
    library: State<'_, LibrarySync>,
) -> Result<(), CommandError> {
    if let Some(folder) = state.music_folder() {
        library.follow(folder);
    }
    Ok(())
}
