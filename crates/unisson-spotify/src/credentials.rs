//! What is kept on disk to stay connected between two runs.
//!
//! The refresh tokens give access to the Spotify account, so the file is
//! readable by the user only, and written whole or not at all.
//!
//! The file of the first version of the Spotify source held only the backup
//! application: `{"client_id": …, "refresh_token": …}`. It is still read.

use std::path::Path;

use serde::{Deserialize, Serialize};
use unisson_core::SourceError;

/// The sign-in of the built-in playback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountToken {
    /// The token that gives new access tokens.
    pub refresh_token: String,
}

/// The backup application the user registered, and its sign-in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppToken {
    /// The client ID of the application.
    pub client_id: String,
    /// The token that gives new access tokens.
    pub refresh_token: String,
}

/// Everything kept about Spotify.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    /// The built-in playback, once signed in.
    #[serde(default)]
    pub account: Option<AccountToken>,
    /// The backup application, once set up.
    #[serde(default)]
    pub app: Option<AppToken>,
    /// The client ID of the backup application, kept after a sign-out to
    /// fill the form again.
    #[serde(default)]
    pub app_client_id: Option<String>,
    /// Whether playback moves to the backup application on its own.
    #[serde(default = "yes")]
    pub auto_switch: bool,
    /// Whether Spotify refused the built-in playback for this account. It
    /// stays refused until the user asks to try again.
    #[serde(default)]
    pub builtin_refused: bool,
}

fn yes() -> bool {
    true
}

impl Default for Credentials {
    fn default() -> Self {
        Self {
            account: None,
            app: None,
            app_client_id: None,
            auto_switch: true,
            builtin_refused: false,
        }
    }
}

/// The two shapes the file may have.
#[derive(Deserialize)]
#[serde(untagged)]
enum File {
    /// The first version: the backup application only.
    First {
        client_id: String,
        refresh_token: String,
    },
    Current(Credentials),
}

impl Credentials {
    /// Reads the credentials from a file.
    ///
    /// Returns `None` if the file is missing or cannot be understood: the
    /// user is then simply not connected.
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        match serde_json::from_str(&text).ok()? {
            File::First {
                client_id,
                refresh_token,
            } => Some(Self {
                app_client_id: Some(client_id.clone()),
                app: Some(AppToken {
                    client_id,
                    refresh_token,
                }),
                ..Self::default()
            }),
            File::Current(credentials) => Some(credentials),
        }
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
    fn credentials_survive_a_save_and_a_load() {
        let path = path("round-trip");
        let credentials = Credentials {
            account: Some(AccountToken {
                refresh_token: "AQD-account".into(),
            }),
            app: Some(AppToken {
                client_id: "0123456789abcdef0123456789abcdef".into(),
                refresh_token: "AQD-app".into(),
            }),
            app_client_id: Some("0123456789abcdef0123456789abcdef".into()),
            auto_switch: false,
            builtin_refused: true,
        };
        assert_eq!(Credentials::load(&path), None);

        credentials.save(&path).unwrap();
        assert_eq!(Credentials::load(&path), Some(credentials));
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn the_file_of_the_first_version_becomes_the_backup_application() {
        let path = path("first");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r#"{"client_id":"0123456789abcdef0123456789abcdef","refresh_token":"R"}"#,
        )
        .unwrap();

        let credentials = Credentials::load(&path).unwrap();

        assert_eq!(credentials.account, None);
        assert_eq!(credentials.app.unwrap().refresh_token, "R");
        assert!(credentials.auto_switch);
    }

    #[test]
    #[cfg(unix)]
    fn only_the_user_can_read_the_file() {
        use std::os::unix::fs::PermissionsExt;

        let path = path("private");
        Credentials::default().save(&path).unwrap();
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
