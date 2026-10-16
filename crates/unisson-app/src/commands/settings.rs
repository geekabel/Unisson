//! Commands of the "Sources and settings" screen.

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_dialog::DialogExt;
use unisson_core::SourceKind;

use crate::dto::CommandError;
use crate::events::SETTINGS_CHANGED;
use crate::library::LibrarySync;
use crate::settings::ThemeChoice;
use crate::shortcut;
use crate::state::AppState;
use crate::texts::LanguageChoice;
use crate::windows;

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
    /// Whether closing the window keeps the application running.
    pub keep_in_tray: bool,
    /// Whether a desktop notification tells each new title.
    pub notifications: bool,
    /// The colors of the interface: `system`, `dark` or `light`.
    pub theme: &'static str,
    /// The language of the interface: `system`, `fr` or `en`.
    pub language: &'static str,
    /// Where the settings are saved.
    pub config_path: Option<String>,
    /// The version of the yt-dlp program, which YouTube needs. `None` when
    /// it is not installed.
    pub yt_dlp_version: Option<String>,
    /// The keyboard shortcut of the quick launcher.
    pub shortcut: ShortcutDto,
    /// The connection to Spotify.
    pub spotify: crate::commands::spotify::SpotifyDto,
}

/// The keyboard shortcut of the quick launcher, as the interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutDto {
    /// Whether this desktop lets Unisson register the shortcut itself.
    pub supported: bool,
    /// The key combination in place, as GNOME writes it.
    pub binding: Option<String>,
    /// The key combinations offered: how GNOME writes each, and its name.
    pub choices: Vec<(String, String)>,
    /// The command to bind by hand on a desktop that is not supported.
    pub command: String,
}

impl ShortcutDto {
    fn read() -> Self {
        let status = shortcut::status();
        Self {
            supported: status.supported,
            binding: status.binding,
            choices: shortcut::BINDINGS
                .iter()
                .map(|(binding, name)| ((*binding).to_owned(), (*name).to_owned()))
                .collect(),
            command: format!(
                "{} {}",
                std::env::current_exe()
                    .map(|path| path.to_string_lossy().into_owned())
                    .unwrap_or_else(|_| "unisson".into()),
                shortcut::LAUNCHER_ARGUMENT
            ),
        }
    }
}

/// Describes the settings.
///
/// # Errors
///
/// Returns a [`CommandError`] if the library cannot be counted.
pub fn describe(state: &AppState) -> Result<SettingsDto, CommandError> {
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
        keep_in_tray: settings.keep_in_tray,
        notifications: settings.notifications,
        theme: settings.theme.id(),
        language: settings.language.id(),
        config_path: state
            .settings_path()
            .map(|path| path.to_string_lossy().into_owned()),
        yt_dlp_version: state.yt_dlp_version(false),
        shortcut: ShortcutDto::read(),
        spotify: crate::commands::spotify::SpotifyDto::read(&state.spotify),
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

/// Chooses whether closing the window keeps the application running, with
/// its icon in the top bar.
#[tauri::command(async)]
pub fn settings_set_keep_in_tray(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<SettingsDto, CommandError> {
    state.update_settings(|settings| settings.keep_in_tray = enabled)?;
    changed(&app, &state)
}

/// Returns the language the interface must use: `fr` or `en`.
///
/// "Like the desktop" is already worked out: every window asks this when
/// it opens, before it writes its first word.
#[tauri::command]
pub fn settings_language(state: State<'_, AppState>) -> &'static str {
    state.language().id()
}

/// Chooses the language of the interface: `system`, `fr` or `en`.
#[tauri::command(async)]
pub fn settings_set_language(
    app: AppHandle,
    state: State<'_, AppState>,
    language: String,
) -> Result<SettingsDto, CommandError> {
    let choice = LanguageChoice::from_id(&language)
        .ok_or_else(|| CommandError::invalid(format!("unknown language: {language}")))?;
    state.update_settings(|settings| settings.language = choice)?;
    windows::apply_language(&app);
    changed(&app, &state)
}

/// Returns the colors of the interface: `system`, `dark` or `light`.
///
/// Every window asks this when it opens. It is much cheaper than
/// [`settings_get`], which looks at the desktop and at yt-dlp.
#[tauri::command]
pub fn settings_theme(state: State<'_, AppState>) -> &'static str {
    state.settings().theme.id()
}

/// Chooses the colors of the interface: `system`, `dark` or `light`.
#[tauri::command(async)]
pub fn settings_set_theme(
    app: AppHandle,
    state: State<'_, AppState>,
    theme: String,
) -> Result<SettingsDto, CommandError> {
    let choice = ThemeChoice::from_id(&theme)
        .ok_or_else(|| CommandError::invalid(format!("unknown theme: {theme}")))?;
    state.update_settings(|settings| settings.theme = choice)?;
    windows::apply_theme(&app);
    changed(&app, &state)
}

/// Chooses whether a desktop notification tells each new title.
#[tauri::command(async)]
pub fn settings_set_notifications(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<SettingsDto, CommandError> {
    state.update_settings(|settings| settings.notifications = enabled)?;
    changed(&app, &state)
}

/// Registers the keyboard shortcut of the quick launcher in the desktop,
/// or changes its key combination.
#[tauri::command(async)]
pub fn settings_install_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
    binding: String,
) -> Result<SettingsDto, CommandError> {
    // The text ends up in the settings of the desktop: accept nothing but
    // the combinations that were offered.
    if !shortcut::is_offered(&binding) {
        return Err(CommandError::invalid("this key combination is not offered"));
    }
    let program = std::env::current_exe()
        .map_err(|error| CommandError::new("unavailable", error.to_string()))?;
    shortcut::install(&program, &binding, state.language().texts().launcher_title)
        .map_err(|error| CommandError::new("unavailable", error.to_string()))?;
    changed(&app, &state)
}

/// Removes the keyboard shortcut of the quick launcher from the desktop.
#[tauri::command(async)]
pub fn settings_remove_shortcut(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsDto, CommandError> {
    shortcut::remove().map_err(|error| CommandError::new("unavailable", error.to_string()))?;
    changed(&app, &state)
}

/// Looks again for the yt-dlp program, after the user installed it.
#[tauri::command(async)]
pub fn settings_check_yt_dlp(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<SettingsDto, CommandError> {
    state.yt_dlp_version(true);
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
