//! What is kept on disk to stay connected: the client ID and the refresh
//! token.
//!
//! The refresh token gives access to the Spotify account, so the file is
//! readable by the user only, and written whole or not at all.

use std::path::Path;

use serde::{Deserialize, Serialize};
use unisson_core::SourceError;

/// The connection to keep between two runs of Unisson.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    /// The client ID of the application the user registered.
    pub client_id: String,
    /// The token that gives new access tokens.
    pub refresh_token: String,
}

impl Credentials {
    /// Reads the credentials from a file.
    ///
    /// Returns `None` if the file is missing or cannot be understood: the
    /// user is then simply not connected.
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    /// Writes the credentials to a file, readable by the user only.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), SourceError> {
        let failed = |error: std::io::Error| {
            SourceError::Unavailable(format!("{}: {error}", path.display()))
        };
        if let Some(folder) = path.parent() {
            std::fs::create_dir_all(folder).map_err(failed)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|error| SourceError::Invalid(error.to_string()))?;
        // Written next to the file, then put in its place: a crash never
        // leaves half a file.
        let temporary = path.with_extension("json.tmp");
        write_private(&temporary, text.as_bytes()).map_err(failed)?;
        std::fs::rename(&temporary, path).map_err(failed)
    }

    /// Removes the file. A missing file is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the file cannot be removed.
    pub fn forget(path: &Path) -> Result<(), SourceError> {
        match std::fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(
                SourceError::Unavailable(format!("{}: {error}", path.display())),
            ),
            _ => Ok(()),
        }
    }
}

/// Creates a file only its owner can read and write.
#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Creates the file. On Windows, the profile folder of the user is already
/// private.
#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    std::fs::write(path, bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(name: &str) -> std::path::PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "unisson-spotify-tests-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&folder);
        folder.join("spotify.json")
    }

    #[test]
    fn credentials_survive_a_save_and_a_load_and_can_be_forgotten() {
        let path = path("round-trip");
        let credentials = Credentials {
            client_id: "0123456789abcdef0123456789abcdef".into(),
            refresh_token: "AQD-secret".into(),
        };
        assert_eq!(Credentials::load(&path), None);

        credentials.save(&path).unwrap();
        assert_eq!(Credentials::load(&path), Some(credentials));
        assert!(!path.with_extension("json.tmp").exists());

        Credentials::forget(&path).unwrap();
        assert_eq!(Credentials::load(&path), None);
        Credentials::forget(&path).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn only_the_user_can_read_the_file() {
        use std::os::unix::fs::PermissionsExt;

        let path = path("private");
        Credentials {
            client_id: "a".into(),
            refresh_token: "b".into(),
        }
        .save(&path)
        .unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_damaged_file_means_not_connected() {
        let path = path("damaged");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(Credentials::load(&path), None);
    }
}
