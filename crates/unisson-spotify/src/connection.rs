//! The connection to a Spotify account, shared by the source and the
//! remote player.

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use unisson_core::SourceError;

use crate::api::SpotifyApi;
use crate::auth::{self, Callback, CallbackPage, Pkce};
use crate::credentials::Credentials;

/// Where tokens are delivered.
const ACCOUNTS: &str = "https://accounts.spotify.com";

/// The connection to a Spotify account, if there is one.
///
/// It is kept in a file between two runs: see [`Credentials`].
#[derive(Debug)]
pub struct Spotify {
    credentials_path: Option<PathBuf>,
    api: RwLock<Option<Arc<SpotifyApi>>>,
    /// The client ID of the last connection, kept to fill the form again
    /// after a disconnection.
    last_client_id: RwLock<Option<String>>,
}

impl Spotify {
    /// Opens the connection kept in `credentials_path`, if there is one.
    ///
    /// Without a path, nothing is kept: the connection lasts until the
    /// program ends.
    pub fn open(credentials_path: Option<PathBuf>) -> Self {
        let credentials = credentials_path.as_deref().and_then(Credentials::load);
        let spotify = Self {
            credentials_path,
            api: RwLock::new(None),
            last_client_id: RwLock::new(None),
        };
        if let Some(credentials) = credentials {
            spotify.set(credentials);
        }
        spotify
    }

    /// Uses a connection that is already open, and keeps nothing on disk:
    /// for tests, which point it at a local server.
    pub fn with_api(api: SpotifyApi) -> Self {
        let client_id = api.client_id().to_owned();
        Self {
            credentials_path: None,
            api: RwLock::new(Some(Arc::new(api))),
            last_client_id: RwLock::new(Some(client_id)),
        }
    }

    /// Returns the connection to the Web API, if the user is connected.
    pub fn api(&self) -> Option<Arc<SpotifyApi>> {
        self.api
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Tells whether the user is connected.
    pub fn is_connected(&self) -> bool {
        self.api().is_some()
    }

    /// Returns the client ID of the current or of the last connection.
    pub fn client_id(&self) -> Option<String> {
        self.last_client_id
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Connects to an account: opens the authorization page in the browser
    /// with `open_browser`, waits for the user to accept, and keeps the
    /// connection. Blocks until then, at most
    /// [`CONSENT_TIMEOUT`](auth::CONSENT_TIMEOUT).
    ///
    /// `page` holds the words the browser shows at the end.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] if the client ID is not valid, the browser
    /// cannot be opened, the user refuses, or Spotify cannot be reached.
    pub fn connect(
        &self,
        client_id: &str,
        open_browser: impl FnOnce(&str) -> Result<(), String>,
        page: &CallbackPage,
    ) -> Result<(), SourceError> {
        self.connect_with(ACCOUNTS, client_id, open_browser, page)
    }

    /// Same as [`connect`](Self::connect), with another address for the
    /// token service: for tests.
    fn connect_with(
        &self,
        accounts: &str,
        client_id: &str,
        open_browser: impl FnOnce(&str) -> Result<(), String>,
        page: &CallbackPage,
    ) -> Result<(), SourceError> {
        let client_id = client_id.trim().to_ascii_lowercase();
        if !auth::is_client_id(&client_id) {
            return Err(SourceError::Invalid(
                "a client ID is 32 letters and digits".into(),
            ));
        }
        let pkce = Pkce::new()?;
        let state = auth::new_state()?;
        let callback = Callback::open()?;
        let redirect_uri = callback.redirect_uri()?;
        let address = auth::authorize_url(&client_id, &redirect_uri, &pkce, &state)?;
        open_browser(&address).map_err(SourceError::Unavailable)?;
        let code = callback.wait_for_code(&state, auth::CONSENT_TIMEOUT, page)?;
        let refresh_token =
            SpotifyApi::exchange_code(accounts, &client_id, &code, &redirect_uri, &pkce.verifier)?;
        let credentials = Credentials {
            client_id,
            refresh_token,
        };
        if let Some(path) = &self.credentials_path {
            credentials.save(path)?;
        }
        self.set(credentials);
        Ok(())
    }

    /// Forgets the connection, here and on disk.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the file cannot be removed.
    pub fn disconnect(&self) -> Result<(), SourceError> {
        *self
            .api
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
        match &self.credentials_path {
            Some(path) => Credentials::forget(path),
            None => Ok(()),
        }
    }

    /// Uses a connection. A new refresh token from Spotify is written to
    /// the file at once.
    fn set(&self, credentials: Credentials) {
        let path = self.credentials_path.clone();
        let client_id = credentials.client_id.clone();
        let api = SpotifyApi::new(
            credentials.client_id.clone(),
            credentials.refresh_token,
            move |refresh_token| {
                let Some(path) = &path else { return };
                let kept = Credentials {
                    client_id: client_id.clone(),
                    refresh_token: refresh_token.to_owned(),
                };
                if let Err(error) = kept.save(path) {
                    eprintln!("unisson: cannot keep the Spotify connection: {error}");
                }
            },
        );
        *self
            .last_client_id
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(credentials.client_id);
        *self
            .api
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(Arc::new(api));
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    use super::*;
    use crate::test_server::{Request, TestServer};

    const PAGE: CallbackPage = CallbackPage {
        title: "Unisson",
        success: "Connected.",
        failure: "Not connected.",
    };

    fn path(name: &str) -> PathBuf {
        let folder = std::env::temp_dir().join(format!(
            "unisson-spotify-connection-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&folder);
        folder.join("spotify.json")
    }

    /// Plays the part of the browser: follows the authorization address
    /// back to Unisson, as Spotify would after the user accepted.
    fn accept(address: &str) -> Result<(), String> {
        let parsed = url::Url::parse(address).map_err(|error| error.to_string())?;
        let query: std::collections::HashMap<String, String> =
            parsed.query_pairs().into_owned().collect();
        let redirect =
            url::Url::parse(&query["redirect_uri"]).map_err(|error| error.to_string())?;
        let state = query["state"].clone();
        let port = redirect.port().ok_or("no port")?;
        std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(
                stream,
                "GET /callback?code=C0DE&state={state} HTTP/1.1\r\n\r\n"
            )
            .unwrap();
            let mut answer = String::new();
            let _ = stream.read_to_string(&mut answer);
        });
        Ok(())
    }

    #[test]
    fn connecting_keeps_the_connection_and_disconnecting_forgets_it() {
        let server = TestServer::start(|_: &Request| {
            (
                200,
                r#"{"access_token":"A","expires_in":3600,"refresh_token":"R"}"#.into(),
                vec![],
            )
        });
        let path = path("connect");
        let spotify = Spotify::open(Some(path.clone()));
        assert!(!spotify.is_connected());

        spotify
            .connect_with(
                &server.url,
                " 0123456789ABCDEF0123456789abcdef ",
                accept,
                &PAGE,
            )
            .unwrap();

        assert!(spotify.is_connected());
        assert_eq!(
            spotify.client_id().as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        let body = &server.requests()[0].body;
        assert!(body.contains("code=C0DE"), "{body}");
        assert!(body.contains("code_verifier="), "{body}");
        // The next run finds the connection again.
        let again = Spotify::open(Some(path.clone()));
        assert!(again.is_connected());

        again.disconnect().unwrap();
        assert!(!again.is_connected());
        assert_eq!(
            again.client_id().as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert!(!Spotify::open(Some(path)).is_connected());
    }

    #[test]
    fn a_wrong_client_id_is_refused_before_opening_anything() {
        let spotify = Spotify::open(None);
        let result = spotify.connect("not-a-client-id", |_| panic!("no browser"), &PAGE);
        assert!(matches!(result, Err(SourceError::Invalid(_))));
    }

    #[test]
    fn a_browser_that_cannot_open_is_reported() {
        let spotify = Spotify::open(None);
        let result = spotify.connect(
            "0123456789abcdef0123456789abcdef",
            |_| Err("no browser".into()),
            &PAGE,
        );
        assert_eq!(result, Err(SourceError::Unavailable("no browser".into())));
    }
}
