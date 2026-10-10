//! Where the application keeps its data on disk.

use std::ffi::OsString;
use std::path::PathBuf;

use crate::error::StoreError;

/// Returns the default location of the database file.
///
/// This is `unisson/unisson.db` under the user data directory of the XDG
/// Base Directory specification: `$XDG_DATA_HOME`, or `~/.local/share` when
/// that variable is unset, empty or not an absolute path.
///
/// # Errors
///
/// Returns [`StoreError::Location`] if neither `$XDG_DATA_HOME` nor `$HOME`
/// gives a usable directory.
pub fn default_database_path() -> Result<PathBuf, StoreError> {
    data_dir(std::env::var_os("XDG_DATA_HOME"), std::env::var_os("HOME"))
        .map(|dir| dir.join("unisson").join("unisson.db"))
        .ok_or_else(|| StoreError::Location("no home directory is known".into()))
}

/// Applies the XDG rules to the values of `$XDG_DATA_HOME` and `$HOME`.
fn data_dir(xdg_data_home: Option<OsString>, home: Option<OsString>) -> Option<PathBuf> {
    // The specification says a relative path is invalid and must be ignored.
    let absolute =
        |value: Option<OsString>| value.map(PathBuf::from).filter(|path| path.is_absolute());
    absolute(xdg_data_home).or_else(|| absolute(home).map(|home| home.join(".local").join("share")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(xdg: Option<&str>, home: Option<&str>) -> Option<PathBuf> {
        data_dir(xdg.map(OsString::from), home.map(OsString::from))
    }

    #[test]
    fn xdg_data_home_is_used_when_it_is_absolute() {
        assert_eq!(
            dir(Some("/data"), Some("/home/me")),
            Some(PathBuf::from("/data"))
        );
    }

    #[test]
    fn the_default_is_under_the_home_directory() {
        let expected = Some(PathBuf::from("/home/me/.local/share"));
        assert_eq!(dir(None, Some("/home/me")), expected);
        assert_eq!(dir(Some(""), Some("/home/me")), expected);
        assert_eq!(dir(Some("relative/path"), Some("/home/me")), expected);
    }

    #[test]
    fn without_any_usable_directory_there_is_no_location() {
        assert_eq!(dir(None, None), None);
        assert_eq!(dir(Some("relative"), Some("also/relative")), None);
    }
}
