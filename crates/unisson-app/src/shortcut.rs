//! The keyboard shortcut that opens the quick launcher from anywhere.
//!
//! # Design
//!
//! An application cannot grab a key by itself under Wayland, the display
//! system of Ubuntu: only the desktop can. GNOME lets the user define
//! "custom shortcuts", each one a key combination and a command to run.
//! Unisson registers one that runs `unisson --launcher`. The copy of Unisson
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
//!
//! # Changing the key combination
//!
//! GNOME grabs the keys of a shortcut when its path enters the list, and
//! lets go of them when the path leaves it. Changing the combination of a
//! path that is already listed proved unreliable: the old keys could stay
//! active. So a change is done as GNOME itself would do a removal followed
//! by a creation: the old path leaves the list and is emptied, then a new
//! path, never seen before, is filled and listed.

use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// The settings schema that lists the custom shortcuts.
const LIST_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys";
const LIST_KEY: &str = "custom-keybindings";
/// The settings schema of one custom shortcut. It is used with a path.
const ENTRY_SCHEMA: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
/// Where the paths of the shortcuts of Unisson start. A number follows,
/// which grows each time the shortcut is installed again.
const ENTRY_PREFIX: &str =
    "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/unisson";
/// How long GNOME is given to let go of the old keys before the new
/// shortcut is listed. It groups the changes it sees for a few tens of
/// milliseconds before acting.
const RELEASE_TIME: Duration = Duration::from_millis(400);

/// The argument that asks for the launcher: `unisson --launcher`.
pub const LAUNCHER_ARGUMENT: &str = "--launcher";
/// The first name of that argument. Shortcuts installed by the first
/// versions still use it.
const OLD_LAUNCHER_ARGUMENT: &str = "--lanceur";

/// Tells whether an argument of the command line asks for the launcher.
pub fn asks_for_launcher(argument: &str) -> bool {
    argument == LAUNCHER_ARGUMENT || argument == OLD_LAUNCHER_ARGUMENT
}

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
    let binding = parse_list(&list)
        .iter()
        .filter(|path| is_ours(path))
        .filter_map(|path| gsettings(&["get", &entry(path), "binding"]).ok())
        .map(|value| unquoted(&value))
        .find(|binding| !binding.is_empty());
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
pub fn install(program: &Path, binding: &str, name: &str) -> Result<(), ShortcutError> {
    let list = gsettings(&["get", LIST_SCHEMA, LIST_KEY])?;
    let paths = parse_list(&list);
    let path = next_path(&paths);
    let mut others = unlist_ours(&paths)?;

    let command = launcher_command(program);
    for (key, value) in [("name", name), ("command", &command), ("binding", binding)] {
        gsettings(&["set", &entry(&path), key, &variant_string(value)])?;
    }
    // The list is written last: GNOME reads the entry when it sees its path.
    others.push(path);
    gsettings(&["set", LIST_SCHEMA, LIST_KEY, &format_list(&others)])?;
    Ok(())
}

/// Removes the shortcut. Other custom shortcuts are left alone.
///
/// # Errors
///
/// Returns a [`ShortcutError`] if GNOME refuses.
pub fn remove() -> Result<(), ShortcutError> {
    let list = gsettings(&["get", LIST_SCHEMA, LIST_KEY])?;
    unlist_ours(&parse_list(&list)).map(|_| ())
}

/// Takes the shortcuts of Unisson out of the list and empties them. When
/// there was one, waits until GNOME let go of its keys. Returns the paths
/// that are left: the shortcuts of the user.
fn unlist_ours(paths: &[String]) -> Result<Vec<String>, ShortcutError> {
    let (ours, others): (Vec<String>, Vec<String>) =
        paths.iter().cloned().partition(|path| is_ours(path));
    if ours.is_empty() {
        return Ok(others);
    }
    gsettings(&["set", LIST_SCHEMA, LIST_KEY, &format_list(&others)])?;
    for path in &ours {
        for key in ["name", "command", "binding"] {
            gsettings(&["reset", &entry(path), key])?;
        }
    }
    std::thread::sleep(RELEASE_TIME);
    Ok(others)
}

/// Tells whether a path of the list is a shortcut of Unisson.
fn is_ours(path: &str) -> bool {
    number_of(path).is_some()
}

/// Returns the number at the end of a path of Unisson. The very first
/// version used a path without a number: it counts as zero.
fn number_of(path: &str) -> Option<u32> {
    let end = path.strip_prefix(ENTRY_PREFIX)?.strip_suffix('/')?;
    match end.strip_prefix('-') {
        Some(number) => number.parse().ok(),
        None => end.is_empty().then_some(0),
    }
}

/// Returns a path for the shortcut that the list never held.
fn next_path(paths: &[String]) -> String {
    let last = paths.iter().filter_map(|path| number_of(path)).max();
    let next = last.map_or(1, |number| number.wrapping_add(1).max(1));
    format!("{ENTRY_PREFIX}-{next}/")
}

/// The schema and the path of the entry, as `gsettings` wants them.
fn entry(path: &str) -> String {
    format!("{ENTRY_SCHEMA}:{path}")
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

/// Writes a text as `gsettings set` reads it: a string between double
/// quotes, in the text format of GVariant.
///
/// Without this, `gsettings` guesses. A value that starts with a quote,
/// such as a quoted path, is then read as a string followed by rubbish,
/// and refused.
fn variant_string(text: &str) -> String {
    let mut written = String::with_capacity(text.len() + 2);
    written.push('"');
    for letter in text.chars() {
        if letter == '"' || letter == '\\' {
            written.push('\\');
        }
        written.push(letter);
    }
    written.push('"');
    written
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
        let paths = vec!["/a/".to_owned(), format!("{ENTRY_PREFIX}-1/")];
        assert_eq!(parse_list(&format_list(&paths)), paths);
        assert_eq!(format_list(&[]), "[]");
    }

    #[test]
    fn the_shortcuts_of_unisson_are_told_from_the_others() {
        let custom = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings";
        assert!(is_ours(&format!("{custom}/unisson/")), "the first version");
        assert!(is_ours(&format!("{custom}/unisson-12/")));
        assert!(!is_ours(&format!("{custom}/custom0/")));
        assert!(!is_ours(&format!("{custom}/unisson-notes/")));
        assert!(!is_ours(&format!("{custom}/unissonne/")));
    }

    #[test]
    fn each_installation_gets_a_path_never_used_before() {
        let custom = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings";
        let user = format!("{custom}/custom0/");
        assert_eq!(next_path(&[]), format!("{custom}/unisson-1/"));
        assert_eq!(
            next_path(std::slice::from_ref(&user)),
            format!("{custom}/unisson-1/")
        );
        assert_eq!(
            next_path(&[format!("{custom}/unisson/"), user.clone()]),
            format!("{custom}/unisson-1/")
        );
        assert_eq!(
            next_path(&[user, format!("{custom}/unisson-7/")]),
            format!("{custom}/unisson-8/")
        );
    }

    #[test]
    fn the_command_quotes_an_awkward_path() {
        assert_eq!(
            launcher_command(Path::new("/usr/bin/unisson")),
            "/usr/bin/unisson --launcher"
        );
        assert_eq!(
            launcher_command(Path::new("/home/me/Mes projets/unisson")),
            "'/home/me/Mes projets/unisson' --launcher"
        );
        assert_eq!(
            launcher_command(Path::new("/home/me/l'app/unisson")),
            r"'/home/me/l'\''app/unisson' --launcher"
        );
    }

    #[test]
    fn a_value_is_written_as_a_variant_string() {
        assert_eq!(variant_string("<Super>u"), "\"<Super>u\"");
        assert_eq!(
            variant_string("'/home/me/Mes projets/unisson' --launcher"),
            "\"'/home/me/Mes projets/unisson' --launcher\""
        );
        assert_eq!(variant_string(r#"a"b\c"#), r#""a\"b\\c""#);
    }

    #[test]
    fn quotes_around_a_value_are_removed() {
        assert_eq!(unquoted("'<Control><Alt>m'"), "<Control><Alt>m");
        assert_eq!(unquoted("  ''  "), "");
    }

    #[test]
    fn both_names_of_the_launcher_argument_are_understood() {
        assert!(asks_for_launcher("--launcher"));
        assert!(asks_for_launcher("--lanceur"));
        assert!(!asks_for_launcher("--help"));
    }

    #[test]
    fn only_the_offered_combinations_are_accepted() {
        assert!(is_offered("<Control><Alt>m"));
        assert!(!is_offered("<Super>m"));
        assert!(!is_offered("rm -rf"));
    }
}
