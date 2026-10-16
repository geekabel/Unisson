//! The connection to a Spotify account, shared by the source and the
//! players.
//!
//! # Design
//!
//! Two sign-ins may exist side by side:
//!
//! - the **account**: the built-in playback, signed in with the identity of
//!   the Spotify desktop application (see [`auth::BUILTIN`]). It plays
//!   inside Unisson. It is what "Sign in with Spotify" makes;
//! - the **backup application**: an application the user registered on the
//!   Spotify developer site. It drives the official Spotify application,
//!   when the built-in playback fails.
//!
//! Search goes through the backup application when there is one: its quota
//! is the user's own, while the built-in identity is shared by every
//! librespot player in the world, and Spotify often answers it "too many
//! requests".
//!
//! Everything is kept in one file: see [`Credentials`]. The connection also
//! holds the [`PlaybackStatus`], which the player updates and the
//! interface watches.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use unisson_core::SourceError;

use crate::api::SpotifyApi;
use crate::auth::{self, Callback, CallbackPage, Flow, Pkce};
use crate::credentials::{AccountToken, AppToken, Credentials};
use crate::status::{PlaybackStatus, Route};

/// Where tokens are delivered.
const ACCOUNTS: &str = "https://accounts.spotify.com";

/// Something told each time the playback status changes.
type Watcher = Box<dyn Fn(&PlaybackStatus) + Send + Sync>;

/// What a sign-in kept, and where.
struct Kept {
    path: Option<PathBuf>,
    credentials: Mutex<Credentials>,
}

impl Kept {
    fn lock(&self) -> MutexGuard<'_, Credentials> {
        self.credentials
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Changes what is kept, and writes it at once.
    fn change(&self, change: impl FnOnce(&mut Credentials)) -> Result<(), SourceError> {
        let mut credentials = self.lock();
        change(&mut credentials);
        match &self.path {
            Some(path) => credentials.save(path),
            None => Ok(()),
        }
    }

    /// Same as [`change`](Self::change), for a change that cannot wait for
    /// an answer: a failure is only reported.
    fn change_quietly(&self, change: impl FnOnce(&mut Credentials)) {
        if let Err(error) = self.change(change) {
            eprintln!("unisson: cannot keep the Spotify connection: {error}");
        }
    }
}

/// The connection to a Spotify account. See the notes at the top of this
/// file.
pub struct Spotify {
    kept: Arc<Kept>,
    accounts: String,
    account: RwLock<Option<Arc<SpotifyApi>>>,
    app: RwLock<Option<Arc<SpotifyApi>>>,
    status: RwLock<PlaybackStatus>,
    watchers: Mutex<Vec<Watcher>>,
    /// Set to stop a sign-in that waits for the browser.
    cancel: AtomicBool,
}

impl std::fmt::Debug for Spotify {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Spotify")
            .field("signed_in", &self.has_account())
            .field("backup", &self.has_app())
            .finish_non_exhaustive()
    }
}

fn read<T: Clone>(lock: &RwLock<T>) -> T {
    lock.read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn write<T>(lock: &RwLock<T>, value: T) {
    *lock
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = value;
}

impl Spotify {
    /// Opens the connection kept in `credentials_path`, if there is one.
    ///
    /// Without a path, nothing is kept: the connection lasts until the
    /// program ends.
    pub fn open(credentials_path: Option<PathBuf>) -> Self {
        Self::open_with(ACCOUNTS, credentials_path)
    }

    /// Same as [`open`](Self::open), with another address for the token
    /// service: for tests.
    fn open_with(accounts: &str, credentials_path: Option<PathBuf>) -> Self {
        let credentials = credentials_path
            .as_deref()
            .and_then(Credentials::load)
            .unwrap_or_default();
        let spotify = Self {
            kept: Arc::new(Kept {
                path: credentials_path,
                credentials: Mutex::new(credentials.clone()),
            }),
            accounts: accounts.to_owned(),
            account: RwLock::new(None),
            app: RwLock::new(None),
            status: RwLock::new(PlaybackStatus::default()),
            watchers: Mutex::new(Vec::new()),
            cancel: AtomicBool::new(false),
        };
        if let Some(account) = credentials.account {
            spotify.use_account(account.refresh_token);
        }
        if let Some(app) = credentials.app {
            spotify.use_app(app);
        }
        spotify.reset_status();
        spotify
    }

    /// Uses connections that are already open, and keeps nothing on disk:
    /// for tests, which point them at a local server.
    pub fn with_apis(account: Option<SpotifyApi>, app: Option<SpotifyApi>) -> Self {
        let spotify = Self::open(None);
        write(&spotify.account, account.map(Arc::new));
        if let Some(app) = &app {
            let client_id = app.client_id().to_owned();
            spotify
                .kept
                .change_quietly(|kept| kept.app_client_id = Some(client_id));
        }
        write(&spotify.app, app.map(Arc::new));
        spotify.reset_status();
        spotify
    }

    /// Returns the connection of the built-in playback, if signed in.
    pub fn account_api(&self) -> Option<Arc<SpotifyApi>> {
        read(&self.account)
    }

    /// Returns the connection of the backup application, if set up.
    pub fn app_api(&self) -> Option<Arc<SpotifyApi>> {
        read(&self.app)
    }

    /// Returns the connection to search with: the backup application, whose
    /// quota is the user's own, else the built-in one.
    pub fn search_api(&self) -> Option<Arc<SpotifyApi>> {
        self.app_api().or_else(|| self.account_api())
    }

    /// Tells whether the built-in playback is signed in.
    pub fn has_account(&self) -> bool {
        self.account_api().is_some()
    }

    /// Tells whether the backup application is set up.
    pub fn has_app(&self) -> bool {
        self.app_api().is_some()
    }

    /// Tells whether Spotify can be used at all.
    pub fn is_connected(&self) -> bool {
        self.has_account() || self.has_app()
    }

    /// Returns the client ID of the backup application, current or last.
    pub fn app_client_id(&self) -> Option<String> {
        self.kept.lock().app_client_id.clone()
    }

    /// Tells whether playback moves to the backup application on its own.
    pub fn auto_switch(&self) -> bool {
        self.kept.lock().auto_switch
    }

    /// Chooses whether playback moves to the backup application on its own.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the choice cannot be kept.
    pub fn set_auto_switch(&self, enabled: bool) -> Result<(), SourceError> {
        self.kept.change(|kept| kept.auto_switch = enabled)
    }

    /// Tells whether Spotify refused the built-in playback for this account.
    pub fn builtin_refused(&self) -> bool {
        self.kept.lock().builtin_refused
    }

    /// Remembers whether Spotify refuses the built-in playback for this
    /// account.
    pub fn set_builtin_refused(&self, refused: bool) {
        self.kept
            .change_quietly(|kept| kept.builtin_refused = refused);
    }

    /// Returns where Spotify plays now.
    pub fn status(&self) -> PlaybackStatus {
        read(&self.status)
    }

    /// Changes the playback status, and tells the watchers.
    pub fn set_status(&self, status: PlaybackStatus) {
        if self.status() == status {
            return;
        }
        write(&self.status, status);
        let watchers = self
            .watchers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        for watcher in watchers.iter() {
            watcher(&status);
        }
    }

    /// Calls `watcher` each time the playback status changes.
    pub fn watch(&self, watcher: impl Fn(&PlaybackStatus) + Send + Sync + 'static) {
        self.watchers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(Box::new(watcher));
    }

    /// The route to use when nothing went wrong: the built-in playback when
    /// signed in, else the backup application.
    pub fn natural_route(&self) -> Option<Route> {
        if self.has_account() {
            Some(Route::Builtin)
        } else if self.has_app() {
            Some(Route::App)
        } else {
            None
        }
    }

    /// Goes back to the natural route, and forgets past problems.
    pub fn reset_status(&self) {
        self.set_status(PlaybackStatus::normal(self.natural_route()));
    }

    /// Signs in the built-in playback: opens the Spotify consent page with
    /// `open_browser`, waits for the user to accept, and keeps the sign-in.
    /// Blocks until then, at most [`CONSENT_TIMEOUT`](auth::CONSENT_TIMEOUT),
    /// or until [`cancel_sign_in`](Self::cancel_sign_in).
    ///
    /// `page` holds the words the browser shows at the end.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] if the browser cannot be opened, the user
    /// refuses, or Spotify cannot be reached.
    pub fn sign_in(
        &self,
        open_browser: impl FnOnce(&str) -> Result<(), String>,
        page: &CallbackPage,
    ) -> Result<(), SourceError> {
        let refresh_token = self.authorize(&auth::BUILTIN, open_browser, page)?;
        self.kept.change(|kept| {
            kept.account = Some(AccountToken {
                refresh_token: refresh_token.clone(),
            });
            kept.builtin_refused = false;
        })?;
        self.use_account(refresh_token);
        self.reset_status();
        Ok(())
    }

    /// Sets up the backup application: same as [`sign_in`](Self::sign_in),
    /// with the application the user registered.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Invalid`] if the client ID is not valid, and
    /// the errors of [`sign_in`](Self::sign_in).
    pub fn set_up_app(
        &self,
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
        let refresh_token = self.authorize(&auth::app_flow(&client_id), open_browser, page)?;
        let app = AppToken {
            client_id: client_id.clone(),
            refresh_token,
        };
        self.kept.change(|kept| {
            kept.app = Some(app.clone());
            kept.app_client_id = Some(client_id);
        })?;
        self.use_app(app);
        if self.status().route.is_none() {
            self.reset_status();
        }
        Ok(())
    }

    /// Stops a sign-in that waits for the browser.
    pub fn cancel_sign_in(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    /// Removes the backup application. Its client ID is kept to fill the
    /// form again.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the change cannot be kept.
    pub fn remove_app(&self) -> Result<(), SourceError> {
        write(&self.app, None);
        self.kept.change(|kept| kept.app = None)?;
        self.reset_status();
        Ok(())
    }

    /// Signs out: forgets the account and the sign-in of the backup
    /// application. The client ID of the backup application is kept.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the change cannot be kept.
    pub fn sign_out(&self) -> Result<(), SourceError> {
        write(&self.account, None);
        write(&self.app, None);
        self.kept.change(|kept| {
            kept.account = None;
            kept.app = None;
            kept.builtin_refused = false;
        })?;
        self.reset_status();
        Ok(())
    }

    /// Runs the consent in the browser, and returns the refresh token.
    fn authorize(
        &self,
        flow: &Flow<'_>,
        open_browser: impl FnOnce(&str) -> Result<(), String>,
        page: &CallbackPage,
    ) -> Result<String, SourceError> {
        self.cancel.store(false, Ordering::SeqCst);
        let pkce = Pkce::new()?;
        let state = auth::new_state()?;
        let callback = Callback::open(flow)?;
        let redirect_uri = callback.redirect_uri()?;
        let address = auth::authorize_url(flow, &redirect_uri, &pkce, &state)?;
        open_browser(&address).map_err(SourceError::Unavailable)?;
        let code = callback.wait_for_code(&state, auth::CONSENT_TIMEOUT, page, &self.cancel)?;
        SpotifyApi::exchange_code(
            &self.accounts,
            flow.client_id,
            &code,
            &redirect_uri,
            &pkce.verifier,
        )
    }

    fn use_account(&self, refresh_token: String) {
        let kept = Arc::clone(&self.kept);
        let api = SpotifyApi::new(auth::BUILTIN_CLIENT_ID, refresh_token, move |refresh| {
            let refresh_token = refresh.to_owned();
            kept.change_quietly(|kept| kept.account = Some(AccountToken { refresh_token }));
        });
        write(&self.account, Some(Arc::new(api)));
    }

    fn use_app(&self, app: AppToken) {
        let kept = Arc::clone(&self.kept);
        let client_id = app.client_id.clone();
        let api = SpotifyApi::new(app.client_id, app.refresh_token, move |refresh| {
            let app = AppToken {
                client_id: client_id.clone(),
                refresh_token: refresh.to_owned(),
            };
            kept.change_quietly(|kept| kept.app = Some(app));
        });
        write(&self.app, Some(Arc::new(api)));
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    use super::*;
    use crate::status::Problem;
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
        let path = redirect.path().to_owned();
        std::thread::spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
            write!(
                stream,
                "GET {path}?code=C0DE&state={state} HTTP/1.1\r\n\r\n"
            )
            .unwrap();
            let mut answer = String::new();
            let _ = stream.read_to_string(&mut answer);
        });
        Ok(())
    }

    fn token_server() -> TestServer {
        TestServer::start(|_: &Request| {
            (
                200,
                r#"{"access_token":"A","expires_in":3600,"refresh_token":"R"}"#.into(),
                vec![],
            )
        })
    }

    #[test]
    fn setting_up_the_backup_keeps_it_and_signing_out_forgets_it() {
        let server = token_server();
        let path = path("app");
        let spotify = Spotify::open_with(&server.url, Some(path.clone()));
        assert!(!spotify.is_connected());
        assert_eq!(spotify.status().route, None);

        spotify
            .set_up_app(" 0123456789ABCDEF0123456789abcdef ", accept, &PAGE)
            .unwrap();

        assert!(spotify.has_app() && !spotify.has_account());
        assert_eq!(spotify.status().route, Some(Route::App));
        assert_eq!(
            spotify.app_client_id().as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        let body = &server.requests()[0].body;
        assert!(body.contains("code=C0DE"), "{body}");
        assert!(body.contains("code_verifier="), "{body}");
        // The next run finds the connection again.
        let again = Spotify::open_with(&server.url, Some(path.clone()));
        assert!(again.has_app());

        again.sign_out().unwrap();
        assert!(!again.is_connected());
        assert_eq!(
            again.app_client_id().as_deref(),
            Some("0123456789abcdef0123456789abcdef")
        );
        assert!(!Spotify::open(Some(path)).is_connected());
    }

    #[test]
    fn signing_in_uses_the_built_in_identity_and_becomes_the_natural_route() {
        let server = token_server();
        let spotify = Spotify::open_with(&server.url, Some(path("account")));
        spotify.set_builtin_refused(true);

        spotify.sign_in(accept, &PAGE).unwrap();

        assert!(spotify.has_account());
        assert!(!spotify.builtin_refused());
        assert_eq!(
            spotify.status(),
            PlaybackStatus::normal(Some(Route::Builtin))
        );
        let body = &server.requests()[0].body;
        assert!(
            body.contains("client_id=65b708073fc0480ea92a077233ca87bd"),
            "{body}"
        );
        assert!(
            body.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A"),
            "{body}"
        );
        assert!(body.contains("%2Flogin"), "{body}");
    }

    #[test]
    fn a_wrong_client_id_is_refused_before_opening_anything() {
        let spotify = Spotify::open(None);
        let result = spotify.set_up_app("not-a-client-id", |_| panic!("no browser"), &PAGE);
        assert!(matches!(result, Err(SourceError::Invalid(_))));
    }

    #[test]
    fn a_browser_that_cannot_open_is_reported() {
        let spotify = Spotify::open(None);
        let result = spotify.set_up_app(
            "0123456789abcdef0123456789abcdef",
            |_| Err("no browser".into()),
            &PAGE,
        );
        assert_eq!(result, Err(SourceError::Unavailable("no browser".into())));
    }

    #[test]
    fn watchers_hear_each_change_of_status_once() {
        let spotify = Spotify::open(None);
        let heard = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&heard);
        spotify.watch(move |status| log.lock().unwrap().push(*status));
        let fallback = PlaybackStatus {
            route: Some(Route::App),
            problem: Some(Problem::Limited),
            retry_at: None,
        };

        spotify.set_status(fallback);
        spotify.set_status(fallback);
        spotify.reset_status();

        assert_eq!(
            *heard.lock().unwrap(),
            [fallback, PlaybackStatus::default()]
        );
    }

    #[test]
    fn the_choices_are_kept_between_two_runs() {
        let path = path("choices");
        let spotify = Spotify::open(Some(path.clone()));
        assert!(spotify.auto_switch());
        spotify.set_auto_switch(false).unwrap();
        spotify.set_builtin_refused(true);

        let again = Spotify::open(Some(path));
        assert!(!again.auto_switch());
        assert!(again.builtin_refused());
    }
}
