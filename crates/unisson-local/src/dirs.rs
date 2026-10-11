//! Finding the user's music folder.

use std::path::{Path, PathBuf};

/// Returns the user's music folder.
///
/// On Linux desktops, the folder is declared in `user-dirs.dirs`, under
/// `$XDG_CONFIG_HOME` (by default `~/.config`). Its name depends on the
/// language of the system: `Music`, `Musique`, `Musik`... Without that file,
/// `~/Music` is assumed.
///
/// On Windows it is the `Music` folder of the user profile.
///
/// Returns `None` if the home folder is unknown, or if the user disabled the
/// music folder by pointing it at the home folder itself.
#[cfg(not(windows))]
pub fn music_directory() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())?;
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(".config"));
    let user_dirs = std::fs::read_to_string(config_home.join("user-dirs.dirs")).ok();
    music_directory_from(user_dirs.as_deref(), &home)
}

/// Returns the user's music folder: `Music` in the user profile.
#[cfg(windows)]
pub fn music_directory() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .map(|home| home.join("Music"))
}

/// Finds the music folder in the content of `user-dirs.dirs`.
#[cfg_attr(windows, allow(dead_code))]
fn music_directory_from(user_dirs: Option<&str>, home: &Path) -> Option<PathBuf> {
    // The file is made of shell assignments such as:
    //     XDG_MUSIC_DIR="$HOME/Musique"
    let declared = user_dirs.and_then(|content| {
        content
            .lines()
            .filter_map(|line| line.trim().strip_prefix("XDG_MUSIC_DIR="))
            .filter_map(|value| value.trim().strip_prefix('"')?.strip_suffix('"'))
            .next_back()
    });
    let Some(value) = declared else {
        return Some(home.join("Music"));
    };
    let folder = match value.strip_prefix("$HOME") {
        Some(rest) => home.join(rest.trim_start_matches('/')),
        None => PathBuf::from(value),
    };
    // Pointing a folder at the home folder is the documented way to disable it.
    (folder.is_absolute() && folder != home).then_some(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(user_dirs: Option<&str>) -> Option<PathBuf> {
        music_directory_from(user_dirs, Path::new("/home/me"))
    }

    #[test]
    fn the_declared_folder_is_used() {
        let content =
            "# comment\nXDG_DESKTOP_DIR=\"$HOME/Bureau\"\nXDG_MUSIC_DIR=\"$HOME/Musique\"\n";
        assert_eq!(find(Some(content)), Some(PathBuf::from("/home/me/Musique")));
    }

    #[test]
    fn an_absolute_folder_is_accepted() {
        let content = "XDG_MUSIC_DIR=\"/data/music\"";
        assert_eq!(find(Some(content)), Some(PathBuf::from("/data/music")));
    }

    #[test]
    fn without_a_declaration_music_under_home_is_assumed() {
        let expected = Some(PathBuf::from("/home/me/Music"));
        assert_eq!(find(None), expected);
        assert_eq!(find(Some("XDG_DESKTOP_DIR=\"$HOME/Desktop\"")), expected);
    }

    #[test]
    fn a_folder_pointing_at_home_means_disabled() {
        assert_eq!(find(Some("XDG_MUSIC_DIR=\"$HOME/\"")), None);
        assert_eq!(find(Some("XDG_MUSIC_DIR=\"$HOME\"")), None);
    }

    #[test]
    fn a_relative_folder_is_refused() {
        assert_eq!(find(Some("XDG_MUSIC_DIR=\"music\"")), None);
    }
}
