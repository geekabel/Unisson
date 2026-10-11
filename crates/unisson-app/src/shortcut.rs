//! The keyboard shortcut that opens the quick launcher from anywhere.
//!
//! # Design
//!
//! An application cannot grab a key by itself under Wayland, the display
//! system of Ubuntu: only the desktop can. GNOME lets the user define
//! "custom shortcuts", each one a key combination and a command to run.
//! Unisson registers one that runs `unisson --lanceur`. The copy of Unisson
//! that is already open receives that request and shows the launcher.
//!
//! The same shortcuts are listed in the Settings of GNOME, under Keyboard,
//! where the user can change or remove them.
//!
//! GNOME keeps them in its settings database, read and written here with
//! the `gsettings` program:
//!
//! - one key holds the list of the custom shortcuts, as paths;
//! - each path holds a name, a command and a key combination.

use std::path::Path;
use std::process::Command;

/// The settings schema that lists the custom shortcuts.
const LIST_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const LIST_KEY: &str = "custom-keybindings";
/// The settings schema of one custom shortcut. It is used with a path.
const ENTRY_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
/// The path of the shortcut of Unisson.
const ENTRY_PATH: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/unisson/";

/// The argument that asks for the launcher: `unisson --lanceur`.
pub const LAUNCHER_ARGUMENT: &str = "--lanceur";

/// The key combinations offered, as GNOME writes them, with their name for
/// the user. Super+M, drawn in the first sketch, is already taken by GNOME
/// for its notification list.
pub const BINDINGS: [(&str, &str); 4] = [
    ("<Control><Alt>m", "Ctrl + Alt + M"),
    ("<Control><Alt>u", "Ctrl + Alt + U"),
    ("<Super>u", "Super + U"),
    ("<Control><Alt>space", "Ctrl + Alt + Espace"),
];

/// What is known about the shortcut on this desktop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutStatus {
    /// Whether this desktop has GNOME custom shortcuts at all.
    pub supported: bool,
    /// The key combination in place, as GNOME writes it, if there is one.
    pub binding: Option<String>,
}

/// Why the shortcut could not be changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutError(pub String);

impl std::fmt::Display for ShortcutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "cannot change the keyboard shortcut: {}", self.0)
    }
}

impl std::error::Error for ShortcutError {}

/// Tells whether a text is one of the offered key combinations.
pub fn is_offered(binding: &str) -> bool {
    BINDINGS.iter().any(|(offered, _)| *offered == binding)
}

/// Returns the state of the shortcut.
pub fn status() -> ShortcutStatus {
    let Ok(list) = gsettings(&["get", LIST_SCHEMA, LIST_KEY]) else {
        return ShortcutStatus {
            supported: false,
            binding: None,
        };
    };
    let registered = parse_list(&list).iter().any(|path| path == ENTRY_PATH);
    let binding = registered
        .then(|| gsettings(&["get", &entry(), "binding"]).ok())
        .flatten()
        .map(|value| unquoted(&value))
        .filter(|binding| !binding.is_empty());
    ShortcutStatus {
        supported: true,
        binding,
    }
}

/// Registers the shortcut, or changes its key combination.
///
/// `program` is the full path of Unisson.
///
/// # Errors
///
/// Returns a [`ShortcutError`] if GNOME refuses, or if this desktop has no
/// custom shortcuts.
pub fn install(program: &Path, binding: &str) -> Result<(), ShortcutError> {
    let list = gsettings(&["get", LIST_SCHEMA, LIST_KEY])?;
    let mut paths = parse_list(&list);
    if !paths.iter().any(|path| path == ENTRY_PATH) {
        paths.push(ENTRY_PATH.to_owned());
    }
    gsettings(&["set", &entry(), "name", "Unisson, lanceur rapide"])?;
    gsettings(&["set", &entry(), "command", &launcher_command(program)])?;
    gsettings(&["set", &entry(), "binding", binding])?;
    // The list is written last: GNOME reads the entry when it sees its path.
    gsettings(&["set", LIST_SCHEMA, LIST_KEY, &format_list(&paths)])?;
    Ok(())
}

/// Removes the shortcut. Other custom shortcuts are left alone.
///
/// # Errors
///
/// Returns a [`ShortcutError`] if GNOME refuses.
pub fn remove() -> Result<(), ShortcutError> {
    let list = gsettings(&["get", LIST_SCHEMA, LIST_KEY])?;
    let paths: Vec<String> = parse_list(&list)
        .into_iter()
        .filter(|path| path != ENTRY_PATH)
        .collect();
    gsettings(&["set", LIST_SCHEMA, LIST_KEY, &format_list(&paths)])?;
    for key in ["name", "command", "binding"] {
        gsettings(&["reset", &entry(), key])?;
    }
    Ok(())
}

/// The schema and the path of the entry, as `gsettings` wants them.
fn entry() -> String {
    format!("{ENTRY_SCHEMA}:{ENTRY_PATH}")
}

/// Runs `gsettings`, and returns what it printed.
fn gsettings(arguments: &[&str]) -> Result<String, ShortcutError> {
    let output = Command::new("gsettings")
        .args(arguments)
        .output()
        .map_err(|error| ShortcutError(format!("gsettings cannot be run: {error}")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        Err(ShortcutError(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ))
    }
}

/// Builds the command of the shortcut. The path is quoted when it holds
/// characters a command line would split or interpret.
fn launcher_command(program: &Path) -> String {
    let path = program.to_string_lossy();
    let plain = path
        .chars()
        .all(|letter| letter.is_ascii_alphanumeric() || "/._-+".contains(letter));
    if plain {
        format!("{path} {LAUNCHER_ARGUMENT}")
    } else {
        // Inside single quotes, only the single quote itself needs care.
        format!("'{}' {LAUNCHER_ARGUMENT}", path.replace('\'', r"'\''"))
    }
}

/// Reads a list of paths as `gsettings get` prints it:
/// `['/a/', '/b/']`, or `@as []` when it is empty.
fn parse_list(printed: &str) -> Vec<String> {
    let inside = printed
        .trim()
        .trim_start_matches("@as")
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    inside
        .split(',')
        .map(unquoted)
        .filter(|path| !path.is_empty())
        .collect()
}

/// Writes a list of paths as `gsettings set` reads it.
fn format_list(paths: &[String]) -> String {
    let quoted: Vec<String> = paths.iter().map(|path| format!("'{path}'")).collect();
    format!("[{}]", quoted.join(", "))
}

/// Removes the quotes `gsettings` puts around a text.
fn unquoted(printed: &str) -> String {
    printed
        .trim()
        .trim_matches(|letter| letter == '\'' || letter == '"')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_of_gnome_is_read_in_its_two_shapes() {
        assert!(parse_list("@as []").is_empty());
        assert!(parse_list("[]").is_empty());
        assert_eq!(
            parse_list("['/org/gnome/custom0/', '/org/gnome/custom1/']"),
            ["/org/gnome/custom0/", "/org/gnome/custom1/"]
        );
        assert_eq!(parse_list("['/only/']"), ["/only/"]);
    }

    #[test]
    fn the_list_survives_a_round_trip() {
        let paths = vec!["/a/".to_owned(), ENTRY_PATH.to_owned()];
        assert_eq!(parse_list(&format_list(&paths)), paths);
        assert_eq!(format_list(&[]), "[]");
    }

    #[test]
    fn the_command_quotes_an_awkward_path() {
        assert_eq!(
            launcher_command(Path::new("/usr/bin/unisson")),
            "/usr/bin/unisson --lanceur"
        );
        assert_eq!(
            launcher_command(Path::new("/home/me/Mes projets/unisson")),
            "'/home/me/Mes projets/unisson' --lanceur"
        );
        assert_eq!(
            launcher_command(Path::new("/home/me/l'app/unisson")),
            r"'/home/me/l'\''app/unisson' --lanceur"
        );
    }

    #[test]
    fn quotes_around_a_value_are_removed() {
        assert_eq!(unquoted("'<Control><Alt>m'"), "<Control><Alt>m");
        assert_eq!(unquoted("  ''  "), "");
    }

    #[test]
    fn only_the_offered_combinations_are_accepted() {
        assert!(is_offered("<Control><Alt>m"));
        assert!(!is_offered("<Super>m"));
        assert!(!is_offered("rm -rf"));
    }
}
