//! The settings of the user, kept in a text file.
//!
//! # Design
//!
//! The settings live in `unisson/config.toml` under `$XDG_CONFIG_HOME`,
//! which defaults to `~/.config`. The file is plain TOML, so it can be read
//! and changed with a text editor:
//!
//! ```toml
//! volume = 0.8
//! resume_radio = true
//! keep_in_tray = true
//! notifications = true
//! theme = "system"
//! language = "system"
//! music_folder = "/home/me/Music"
//!
//! [sources]
//! priority = ["local", "radio", "youtube"]
//! disabled = ["youtube"]
//! ```
//!
//! A hand-edited file can hold anything. Reading it therefore never fails
//! the application: a missing value takes its default, an unknown source is
//! dropped, and a file that is not valid TOML is set aside with a `.broken`
//! extension so the user can look at it, then replaced by the defaults.

use std::ffi::OsString;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use unisson_core::SourceKind;

use crate::texts::LanguageChoice;

/// The settings as the application uses them: always complete and valid.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// The volume, from `0.0` to `1.0`.
    pub volume: f64,
    /// Whether the last radio station takes over when the queue runs out.
    pub resume_radio: bool,
    /// Whether closing the window keeps the application running, with its
    /// icon in the top bar.
    pub keep_in_tray: bool,
    /// Whether a desktop notification tells each new title.
    pub notifications: bool,
    /// The colors of the interface.
    pub theme: ThemeChoice,
    /// The language of the interface.
    pub language: LanguageChoice,
    /// The folder of the local library. `None` means the music folder of
    /// the desktop.
    pub music_folder: Option<PathBuf>,
    /// Every source, from the most to the least preferred. When a track
    /// exists in several sources, the first one of this list is used.
    pub priority: Vec<SourceKind>,
    /// The sources that are switched off: they are not searched.
    pub disabled: Vec<SourceKind>,
}

/// The colors of the interface: those of the desktop, or a fixed choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeChoice {
    /// Dark or light, like the desktop.
    #[default]
    System,
    /// Always dark.
    Dark,
    /// Always light.
    Light,
}

impl ThemeChoice {
    /// Every choice, in the order the settings screen lists them.
    pub const ALL: [Self; 3] = [Self::System, Self::Dark, Self::Light];

    /// Returns the name used in the settings file and by the interface.
    pub fn id(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }

    /// Finds a choice from its [`id`](Self::id).
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|choice| choice.id() == id)
    }
}

/// The preferred order of the sources when the user chose none.
///
/// A local file needs no network and no third-party service, so it wins.
pub const DEFAULT_PRIORITY: [SourceKind; 3] =
    [SourceKind::Local, SourceKind::Radio, SourceKind::YouTube];

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 1.0,
            resume_radio: true,
            keep_in_tray: true,
            notifications: true,
            theme: ThemeChoice::System,
            language: LanguageChoice::System,
            music_folder: None,
            priority: DEFAULT_PRIORITY.to_vec(),
            disabled: Vec::new(),
        }
    }
}

/// Why the settings could not be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError(String);

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot save the settings: {}", self.0)
    }
}

impl std::error::Error for SettingsError {}

/// The shape of the file. Everything is optional: see [`Settings`] for the
/// meaning of each value.
#[derive(Debug, Default, Serialize, Deserialize)]
struct SettingsFile {
    volume: Option<f64>,
    resume_radio: Option<bool>,
    keep_in_tray: Option<bool>,
    notifications: Option<bool>,
    theme: Option<String>,
    language: Option<String>,
    music_folder: Option<PathBuf>,
    #[serde(default)]
    sources: SourcesFile,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SourcesFile {
    #[serde(default)]
    priority: Vec<String>,
    #[serde(default)]
    disabled: Vec<String>,
}

impl Settings {
    /// Returns the place of the settings file, or `None` when no home
    /// directory is known.
    pub fn default_path() -> Option<PathBuf> {
        user_config_dir().map(|dir| dir.join("unisson").join("config.toml"))
    }

    /// Reads the settings from a file.
    ///
    /// A missing file gives the defaults. So does a file that is not valid
    /// TOML: it is first renamed with a `.broken` extension.
    pub fn load(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        match toml::from_str::<SettingsFile>(&text) {
            Ok(file) => Self::from_file(file),
            Err(error) => {
                eprintln!("unisson: the settings file is not valid, using defaults: {error}");
                let _ = std::fs::rename(path, path.with_extension("toml.broken"));
                Self::default()
            }
        }
    }

    /// Writes the settings to a file, creating its folder if needed.
    ///
    /// The file is written beside its final place, then moved there, so a
    /// crash in the middle never leaves half a file.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] if the folder or the file cannot be
    /// written.
    pub fn save(&self, path: &Path) -> Result<(), SettingsError> {
        let error = |error: std::io::Error| SettingsError(format!("{}: {error}", path.display()));
        let text =
            toml::to_string(&self.to_file()).map_err(|error| SettingsError(error.to_string()))?;
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(error)?;
        }
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, text).map_err(error)?;
        std::fs::rename(&temporary, path).map_err(error)
    }

    /// Tells whether a source is switched on.
    pub fn is_enabled(&self, source: SourceKind) -> bool {
        !self.disabled.contains(&source)
    }

    /// Returns the sources that are switched on, the preferred one first.
    pub fn enabled_sources(&self) -> Vec<SourceKind> {
        self.priority
            .iter()
            .copied()
            .filter(|source| self.is_enabled(*source))
            .collect()
    }

    /// Switches a source on or off.
    pub fn set_enabled(&mut self, source: SourceKind, enabled: bool) {
        self.disabled.retain(|other| *other != source);
        if !enabled {
            self.disabled.push(source);
        }
    }

    /// Sets the preferred order of the sources.
    ///
    /// The list may be incomplete or repeat a source: see
    /// [`complete_priority`].
    pub fn set_priority(&mut self, priority: &[SourceKind]) {
        self.priority = complete_priority(priority);
    }

    /// Sets the volume, kept between `0.0` and `1.0`.
    pub fn set_volume(&mut self, volume: f64) {
        self.volume = clamp_volume(volume);
    }

    /// Turns what a file holds into complete and valid settings.
    fn from_file(file: SettingsFile) -> Self {
        let defaults = Self::default();
        let known = |ids: &[String]| -> Vec<SourceKind> {
            ids.iter()
                .filter_map(|id| SourceKind::from_id(id.trim()))
                .collect()
        };
        let mut disabled = known(&file.sources.disabled);
        disabled.dedup();
        Self {
            volume: file.volume.map_or(defaults.volume, clamp_volume),
            resume_radio: file.resume_radio.unwrap_or(defaults.resume_radio),
            keep_in_tray: file.keep_in_tray.unwrap_or(defaults.keep_in_tray),
            notifications: file.notifications.unwrap_or(defaults.notifications),
            theme: file
                .theme
                .as_deref()
                .and_then(|id| ThemeChoice::from_id(id.trim()))
                .unwrap_or(defaults.theme),
            language: file
                .language
                .as_deref()
                .and_then(|id| LanguageChoice::from_id(id.trim()))
                .unwrap_or(defaults.language),
            // A relative folder would depend on where the program starts.
            music_folder: file.music_folder.filter(|folder| folder.is_absolute()),
            priority: complete_priority(&known(&file.sources.priority)),
            disabled,
        }
    }

    fn to_file(&self) -> SettingsFile {
        let ids = |sources: &[SourceKind]| sources.iter().map(|s| s.id().to_owned()).collect();
        SettingsFile {
            volume: Some(self.volume),
            resume_radio: Some(self.resume_radio),
            keep_in_tray: Some(self.keep_in_tray),
            notifications: Some(self.notifications),
            theme: Some(self.theme.id().to_owned()),
            language: Some(self.language.id().to_owned()),
            music_folder: self.music_folder.clone(),
            sources: SourcesFile {
                priority: ids(&self.priority),
                disabled: ids(&self.disabled),
            },
        }
    }
}

/// Makes a priority list complete: each source once.
///
/// Repeated sources keep their first place. Sources missing from `chosen`
/// are added at the end, in the order of [`DEFAULT_PRIORITY`].
pub fn complete_priority(chosen: &[SourceKind]) -> Vec<SourceKind> {
    let mut priority = Vec::with_capacity(DEFAULT_PRIORITY.len());
    for source in chosen.iter().chain(&DEFAULT_PRIORITY) {
        if !priority.contains(source) {
            priority.push(*source);
        }
    }
    priority
}

fn clamp_volume(volume: f64) -> f64 {
    if volume.is_nan() {
        1.0
    } else {
        volume.clamp(0.0, 1.0)
    }
}

/// Returns the folder where the programs of the user keep their settings:
/// the XDG one on Linux, `%APPDATA%` on Windows.
#[cfg(not(windows))]
fn user_config_dir() -> Option<PathBuf> {
    config_dir(
        std::env::var_os("XDG_CONFIG_HOME"),
        std::env::var_os("HOME"),
    )
}

#[cfg(windows)]
fn user_config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
}

#[cfg_attr(windows, allow(dead_code))]
fn config_dir(xdg_config_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    // The specification says a relative path is invalid and must be ignored.
    let absolute =
        |value: Option<OsString>| value.map(PathBuf::from).filter(|path| path.is_absolute());
    absolute(xdg_config_home).or_else(|| absolute(home).map(|home| home.join(".config")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::texts::Language;

    /// Returns the path of a settings file in a new empty folder.
    fn test_path(name: &str) -> PathBuf {
        let folder = std::env::temp_dir()
            .join(format!("unisson-settings-tests-{}", std::process::id()))
            .join(name);
        let _ = std::fs::remove_dir_all(&folder);
        folder.join("unisson").join("config.toml")
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn a_missing_file_gives_the_defaults() {
        let settings = Settings::load(&test_path("missing"));
        assert_eq!(settings, Settings::default());
        assert_eq!(settings.volume, 1.0);
        assert!(settings.resume_radio);
        assert_eq!(settings.priority, DEFAULT_PRIORITY);
        assert_eq!(settings.enabled_sources(), DEFAULT_PRIORITY);
    }

    #[test]
    fn settings_survive_a_save_and_a_load() {
        let path = test_path("round-trip");
        let mut settings = Settings::default();
        settings.set_volume(0.35);
        settings.resume_radio = false;
        settings.keep_in_tray = false;
        settings.notifications = false;
        settings.theme = ThemeChoice::Light;
        settings.language = LanguageChoice::Fixed(Language::English);
        settings.music_folder = Some(PathBuf::from("/data/Musique été"));
        settings.set_priority(&[SourceKind::YouTube, SourceKind::Local]);
        settings.set_enabled(SourceKind::Radio, false);

        settings.save(&path).unwrap();

        assert_eq!(Settings::load(&path), settings);
        // No temporary file is left behind.
        assert!(!path.with_extension("toml.tmp").exists());
        // The file is meant to be read by a person.
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("volume = 0.35"), "{text}");
        assert!(text.contains("\"youtube\""), "{text}");
        assert!(text.contains("[sources]"), "{text}");
    }

    #[test]
    fn a_partial_file_is_completed_with_the_defaults() {
        let path = test_path("partial");
        write(&path, "resume_radio = false\n");
        let settings = Settings::load(&path);
        assert!(!settings.resume_radio);
        assert_eq!(settings.volume, 1.0);
        assert_eq!(settings.priority, DEFAULT_PRIORITY);
        assert!(settings.disabled.is_empty());
    }

    #[test]
    fn odd_values_written_by_hand_are_made_safe() {
        let path = test_path("odd");
        write(
            &path,
            r#"
volume = 7.5
music_folder = "relative/folder"
some_future_setting = "kept out of the way"

[sources]
priority = ["youtube", "cassette", "youtube", " local "]
disabled = ["radio", "minidisc"]
"#,
        );

        let settings = Settings::load(&path);

        assert_eq!(settings.volume, 1.0);
        assert_eq!(settings.music_folder, None);
        assert_eq!(
            settings.priority,
            [SourceKind::YouTube, SourceKind::Local, SourceKind::Radio]
        );
        assert_eq!(settings.disabled, [SourceKind::Radio]);
        assert_eq!(
            settings.enabled_sources(),
            [SourceKind::YouTube, SourceKind::Local]
        );
    }

    #[test]
    fn a_broken_file_is_set_aside_and_replaced_by_the_defaults() {
        let path = test_path("broken");
        write(&path, "volume = = oops");

        assert_eq!(Settings::load(&path), Settings::default());

        assert!(!path.exists());
        let aside = path.with_extension("toml.broken");
        assert_eq!(std::fs::read_to_string(aside).unwrap(), "volume = = oops");
    }

    #[test]
    fn a_source_is_switched_off_once_and_on_again() {
        let mut settings = Settings::default();
        settings.set_enabled(SourceKind::Radio, false);
        settings.set_enabled(SourceKind::Radio, false);
        assert_eq!(settings.disabled, [SourceKind::Radio]);
        assert!(!settings.is_enabled(SourceKind::Radio));
        settings.set_enabled(SourceKind::Radio, true);
        assert!(settings.disabled.is_empty());
    }

    #[test]
    fn the_volume_stays_between_zero_and_one() {
        let mut settings = Settings::default();
        settings.set_volume(-3.0);
        assert_eq!(settings.volume, 0.0);
        settings.set_volume(f64::NAN);
        assert_eq!(settings.volume, 1.0);
    }

    #[test]
    fn saving_where_nothing_can_be_written_is_an_error() {
        let path = test_path("blocked");
        // A file stands where the folder should be.
        let folder = path.parent().unwrap();
        std::fs::create_dir_all(folder.parent().unwrap()).unwrap();
        std::fs::write(folder, "a file, not a folder").unwrap();

        let error = Settings::default().save(&path).unwrap_err();
        assert!(
            error.to_string().starts_with("cannot save the settings"),
            "{error}"
        );
    }

    #[test]
    fn the_file_is_under_the_xdg_config_folder() {
        let dir = |xdg: Option<&str>, home: Option<&str>| {
            config_dir(xdg.map(OsString::from), home.map(OsString::from))
        };
        assert_eq!(
            dir(Some("/cfg"), Some("/home/me")),
            Some(PathBuf::from("/cfg"))
        );
        assert_eq!(
            dir(None, Some("/home/me")),
            Some(PathBuf::from("/home/me/.config"))
        );
        assert_eq!(
            dir(Some("relative"), Some("/home/me")),
            Some(PathBuf::from("/home/me/.config"))
        );
        assert_eq!(dir(None, None), None);
    }
}
