//! A small client of the Spotify Web API: tokens, search, and the player.
//!
//! Only what Unisson needs is here. Every call blocks, like the other
//! sources of Unisson: the caller runs it on a thread of its own.
//!
//! Reference: <https://developer.spotify.com/documentation/web-api>.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};
use unisson_core::SourceError;
use ureq::Agent;

use crate::is_spotify_id;

/// Where tokens are delivered.
const ACCOUNTS: &str = "https://accounts.spotify.com";
/// The base of the Web API.
const API: &str = "https://api.spotify.com/v1";
/// Longest wait for one answer.
const TIMEOUT: Duration = Duration::from_secs(10);
/// An access token is renewed this long before it expires.
const EXPIRY_MARGIN: Duration = Duration::from_secs(60);
/// The most results the search gives for one request, since February 2026.
pub const SEARCH_LIMIT: usize = 10;
/// Sent with every request.
const USER_AGENT: &str = concat!("Unisson/", env!("CARGO_PKG_VERSION"));

/// What the player endpoints answer when no Spotify application is open.
pub const NO_DEVICE: &str = "no Spotify device: open the Spotify application";

/// A track found in the catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// Its identifier, 22 letters and digits.
    pub id: String,
    /// Its title.
    pub name: String,
    /// Its artists, in order.
    pub artists: Vec<String>,
    /// The title of its album. It can be empty.
    pub album: String,
    /// Its duration.
    pub duration: Duration,
    /// The address of the cover of its album.
    pub artwork: Option<String>,
}

/// A Spotify application that can play: the desktop application, a phone,
/// a speaker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    /// Its identifier. It can change: ask again rather than keeping it.
    pub id: Option<String>,
    /// Its name, such as the name of the computer.
    pub name: String,
    /// Its type, such as `Computer` or `Smartphone`.
    pub kind: String,
    /// Whether it is the one that plays.
    pub is_active: bool,
    /// Whether it refuses to be driven through the Web API.
    pub is_restricted: bool,
    /// Whether its volume can be set.
    pub supports_volume: bool,
}

/// What the Spotify account plays right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackState {
    /// The device that plays.
    pub device_id: Option<String>,
    /// Whether sound is coming out.
    pub is_playing: bool,
    /// Where the current item is.
    pub progress: Option<Duration>,
    /// The address of the current item, such as `spotify:track:...`.
    pub item_uri: Option<String>,
    /// The duration of the current item.
    pub duration: Option<Duration>,
}

/// The tokens of a connection.
#[derive(Debug)]
struct Tokens {
    refresh: String,
    /// The access token, and when it must be renewed.
    access: Option<(String, Instant)>,
}

/// What is told when Spotify gives a new refresh token.
type OnRefresh = Box<dyn Fn(&str) + Send + Sync>;

/// A connection to the Web API, on behalf of one account.
pub struct SpotifyApi {
    agent: Agent,
    accounts: String,
    api: String,
    client_id: String,
    tokens: Mutex<Tokens>,
    on_refresh: OnRefresh,
}

impl std::fmt::Debug for SpotifyApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // The tokens are secrets: they are never printed.
        f.debug_struct("SpotifyApi")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

/// The answer of the token endpoint.
#[derive(Deserialize)]
struct TokenAnswer {
    access_token: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

impl SpotifyApi {
    /// Opens a connection with a refresh token kept from a previous run.
    ///
    /// Nothing is sent yet: the first call gets an access token.
    /// `on_refresh` is told each new refresh token Spotify gives, to keep
    /// it.
    pub fn new(
        client_id: impl Into<String>,
        refresh_token: impl Into<String>,
        on_refresh: impl Fn(&str) + Send + Sync + 'static,
    ) -> Self {
        Self::with_bases(ACCOUNTS, API, client_id, refresh_token, on_refresh)
    }

    /// Same as [`new`](Self::new), with other addresses for the two
    /// services: for tests.
    pub fn with_bases(
        accounts: &str,
        api: &str,
        client_id: impl Into<String>,
        refresh_token: impl Into<String>,
        on_refresh: impl Fn(&str) + Send + Sync + 'static,
    ) -> Self {
        let secure = accounts.starts_with("https://") && api.starts_with("https://");
        let agent = Agent::config_builder()
            .user_agent(USER_AGENT)
            .timeout_global(Some(TIMEOUT))
            // The answers of errors are read, to tell the user why.
            .http_status_as_error(false)
            // Spotify is only reached over HTTPS. Tests use a local server.
            .https_only(secure)
            .build()
            .into();
        Self {
            agent,
            accounts: accounts.trim_end_matches('/').to_owned(),
            api: api.trim_end_matches('/').to_owned(),
            client_id: client_id.into(),
            tokens: Mutex::new(Tokens {
                refresh: refresh_token.into(),
                access: None,
            }),
            on_refresh: Box::new(on_refresh),
        }
    }

    /// Trades the code of an authorization for tokens, and returns the
    /// refresh token. See [`auth`](crate::auth).
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] if Spotify refuses the code, or cannot be
    /// reached.
    pub fn exchange_code(
        accounts: &str,
        client_id: &str,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<String, SourceError> {
        let probe = Self::with_bases(accounts, API, client_id, "", |_| {});
        let answer = probe.token_request(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("code_verifier", verifier),
        ])?;
        answer
            .refresh_token
            .ok_or_else(|| SourceError::Invalid("Spotify gave no refresh token".into()))
    }

    /// Returns the client ID of the connection.
    pub fn client_id(&self) -> &str {
        &self.client_id
    }

    /// Searches the catalog for tracks. At most [`SEARCH_LIMIT`] come back.
    ///
    /// Tracks that cannot be played in the country of the account are left
    /// out.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] if Spotify cannot be reached, or refuses.
    pub fn search_tracks(&self, text: &str, limit: usize) -> Result<Vec<Track>, SourceError> {
        let limit = limit.clamp(1, SEARCH_LIMIT).to_string();
        let answer = self.get(
            "/search",
            &[("q", text), ("type", "track"), ("limit", &limit)],
        )?;
        let found: SearchAnswer = parse(&answer)?;
        Ok(found
            .tracks
            .map(|tracks| tracks.items)
            .unwrap_or_default()
            .into_iter()
            .flatten()
            .filter_map(RawTrack::into_track)
            .collect())
    }

    /// Reads one track of the catalog, from its ID.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::NotFound`] if the ID is not a track, or
    /// another [`SourceError`] if Spotify cannot be reached, or refuses.
    pub fn track(&self, id: &str) -> Result<Track, SourceError> {
        if !is_spotify_id(id) {
            return Err(SourceError::NotFound);
        }
        let answer = self.get(&format!("/tracks/{id}"), &[])?;
        let track: RawTrack = parse(&answer)?;
        track.into_track().ok_or(SourceError::NotFound)
    }

    /// Lists the Spotify applications of the account that can play.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] if Spotify cannot be reached, or refuses.
    pub fn devices(&self) -> Result<Vec<Device>, SourceError> {
        let answer = self.get("/me/player/devices", &[])?;
        let found: DevicesAnswer = parse(&answer)?;
        Ok(found
            .devices
            .into_iter()
            .map(|device| Device {
                id: device.id,
                name: device.name.unwrap_or_default(),
                kind: device.kind.unwrap_or_default(),
                is_active: device.is_active,
                is_restricted: device.is_restricted,
                supports_volume: device.supports_volume,
            })
            .collect())
    }

    /// Returns what the account plays, or `None` when nothing does.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] if Spotify cannot be reached, or refuses.
    pub fn playback(&self) -> Result<Option<PlaybackState>, SourceError> {
        let answer = self.get("/me/player", &[])?;
        if answer.trim().is_empty() {
            return Ok(None);
        }
        let state: RawPlayback = parse(&answer)?;
        let item = state.item;
        Ok(Some(PlaybackState {
            device_id: state.device.and_then(|device| device.id),
            is_playing: state.is_playing,
            progress: state.progress_ms.map(Duration::from_millis),
            item_uri: item.as_ref().and_then(|item| item.uri.clone()),
            duration: item
                .and_then(|item| item.duration_ms)
                .map(Duration::from_millis),
        }))
    }

    /// Plays a track from its beginning on a device.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`]: [`NO_DEVICE`] in an
    /// [`Unavailable`](SourceError::Unavailable) when the device is gone,
    /// or what Spotify answers, such as a Premium account being required.
    pub fn play_track(&self, device_id: &str, uri: &str) -> Result<(), SourceError> {
        self.player(
            "PUT",
            "/me/player/play",
            device_id,
            &[],
            Some(json!({ "uris": [uri] })),
        )
    }

    /// Resumes playback on a device.
    ///
    /// # Errors
    ///
    /// Same as [`play_track`](Self::play_track).
    pub fn resume(&self, device_id: &str) -> Result<(), SourceError> {
        self.player("PUT", "/me/player/play", device_id, &[], None)
    }

    /// Pauses playback on a device.
    ///
    /// # Errors
    ///
    /// Same as [`play_track`](Self::play_track).
    pub fn pause(&self, device_id: &str) -> Result<(), SourceError> {
        self.player("PUT", "/me/player/pause", device_id, &[], None)
    }

    /// Moves playback to a position on a device.
    ///
    /// # Errors
    ///
    /// Same as [`play_track`](Self::play_track).
    pub fn seek(&self, device_id: &str, position: Duration) -> Result<(), SourceError> {
        let position = position.as_millis().to_string();
        self.player(
            "PUT",
            "/me/player/seek",
            device_id,
            &[("position_ms", &position)],
            None,
        )
    }

    /// Sets the volume of a device, from 0 to 100.
    ///
    /// # Errors
    ///
    /// Same as [`play_track`](Self::play_track).
    pub fn set_volume(&self, device_id: &str, percent: u8) -> Result<(), SourceError> {
        let percent = percent.min(100).to_string();
        self.player(
            "PUT",
            "/me/player/volume",
            device_id,
            &[("volume_percent", &percent)],
            None,
        )
    }

    /// Sends a player order. A missing device becomes [`NO_DEVICE`].
    fn player(
        &self,
        method: &str,
        path: &str,
        device_id: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<(), SourceError> {
        let mut query = query.to_vec();
        query.push(("device_id", device_id));
        self.call(method, path, &query, body)
            .map(|_| ())
            .map_err(|error| match error {
                SourceError::NotFound => SourceError::Unavailable(NO_DEVICE.into()),
                other => other,
            })
    }

    fn get(&self, path: &str, query: &[(&str, &str)]) -> Result<String, SourceError> {
        self.call("GET", path, query, None)
    }

    /// Sends a request with the access token. When Spotify refuses the
    /// token, it is renewed and the request is sent once more.
    fn call(
        &self,
        method: &str,
        path: &str,
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<String, SourceError> {
        let mut renewed = false;
        loop {
            let token = self.access_token(renewed)?;
            let (status, text, retry_after) =
                self.send(method, path, query, body.as_ref(), &token)?;
            match status {
                200..=299 => return Ok(text),
                401 if !renewed => renewed = true,
                _ => return Err(status_error(status, &text, retry_after)),
            }
        }
    }

    fn send(
        &self,
        method: &str,
        path: &str,
        query: &[(&str, &str)],
        body: Option<&Value>,
        token: &str,
    ) -> Result<(u16, String, Option<String>), SourceError> {
        let address = format!("{}{path}", self.api);
        let bearer = format!("Bearer {token}");
        let result = match method {
            "GET" => self
                .agent
                .get(&address)
                .header("Authorization", &bearer)
                .query_pairs(query.iter().copied())
                .call(),
            _ => {
                let request = self
                    .agent
                    .put(&address)
                    .header("Authorization", &bearer)
                    .query_pairs(query.iter().copied());
                match body {
                    Some(body) => request.send_json(body),
                    None => request.send_empty(),
                }
            }
        };
        let mut response = result.map_err(|error| SourceError::Network(error.to_string()))?;
        let status = response.status().as_u16();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| SourceError::Network(error.to_string()))?;
        Ok((status, text, retry_after))
    }

    /// Returns a valid access token, renewed if it expired or if `renew`.
    fn access_token(&self, renew: bool) -> Result<String, SourceError> {
        let mut tokens = self
            .tokens
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !renew
            && let Some((access, until)) = &tokens.access
            && Instant::now() < *until
        {
            return Ok(access.clone());
        }
        let refresh = tokens.refresh.clone();
        let answer = self.token_request(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh),
            ("client_id", &self.client_id),
        ])?;
        let lifetime = Duration::from_secs(answer.expires_in).saturating_sub(EXPIRY_MARGIN);
        tokens.access = Some((answer.access_token.clone(), Instant::now() + lifetime));
        // Spotify may give a new refresh token: the old one may stop
        // working, so the new one is kept at once.
        if let Some(new_refresh) = answer.refresh_token
            && new_refresh != tokens.refresh
        {
            tokens.refresh = new_refresh;
            (self.on_refresh)(&tokens.refresh);
        }
        Ok(answer.access_token)
    }

    fn token_request(&self, form: &[(&str, &str)]) -> Result<TokenAnswer, SourceError> {
        let mut response = self
            .agent
            .post(format!("{}/api/token", self.accounts))
            .send_form(form.iter().copied())
            .map_err(|error| SourceError::Network(error.to_string()))?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|error| SourceError::Network(error.to_string()))?;
        if (200..300).contains(&status) {
            return parse(&text);
        }
        // RFC 6749: `invalid_grant` means the token or the code is no
        // longer valid. The user must connect again.
        let reason: TokenError = serde_json::from_str(&text).unwrap_or_default();
        Err(match (status, reason.error.as_deref()) {
            (400, Some("invalid_grant")) => {
                SourceError::Unavailable("the Spotify connection expired: connect again".into())
            }
            (400..=499, _) => SourceError::Unavailable(format!(
                "Spotify refused the connection: {}",
                reason.error_description.or(reason.error).unwrap_or(text)
            )),
            _ => SourceError::Network(format!("Spotify answered {status}")),
        })
    }
}

/// Turns an answer that is not a success into an error the user can act
/// on.
fn status_error(status: u16, text: &str, retry_after: Option<String>) -> SourceError {
    let message = serde_json::from_str::<ErrorAnswer>(text)
        .ok()
        .and_then(|answer| answer.error)
        .and_then(|error| error.message)
        .unwrap_or_default();
    match status {
        401 => SourceError::Unavailable(format!(
            "Spotify refused the connection: connect again ({message})"
        )),
        403 => SourceError::Unavailable(format!("Spotify refused: {message}")),
        404 => SourceError::NotFound,
        429 => SourceError::Network(format!(
            "Spotify asks to wait {} seconds",
            retry_after.as_deref().unwrap_or("a few")
        )),
        500..=599 => SourceError::Network(format!("Spotify answered {status}")),
        _ => SourceError::Invalid(format!("Spotify answered {status}: {message}")),
    }
}

fn parse<T: for<'a> Deserialize<'a>>(text: &str) -> Result<T, SourceError> {
    serde_json::from_str(text).map_err(|error| SourceError::Invalid(error.to_string()))
}

#[derive(Deserialize, Default)]
struct TokenError {
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct ErrorAnswer {
    error: Option<ErrorBody>,
}

#[derive(Deserialize)]
struct ErrorBody {
    message: Option<String>,
}

#[derive(Deserialize)]
struct SearchAnswer {
    tracks: Option<TrackPage>,
}

#[derive(Deserialize)]
struct TrackPage {
    // Spotify leaves holes (`null`) in some lists.
    #[serde(default)]
    items: Vec<Option<RawTrack>>,
}

#[derive(Deserialize)]
struct RawTrack {
    id: Option<String>,
    name: Option<String>,
    #[serde(default)]
    artists: Vec<RawName>,
    album: Option<RawAlbum>,
    duration_ms: Option<u64>,
    is_playable: Option<bool>,
}

#[derive(Deserialize)]
struct RawName {
    name: Option<String>,
}

#[derive(Deserialize)]
struct RawAlbum {
    name: Option<String>,
    #[serde(default)]
    images: Vec<RawImage>,
}

#[derive(Deserialize)]
struct RawImage {
    url: Option<String>,
    width: Option<u32>,
}

impl RawTrack {
    fn into_track(self) -> Option<Track> {
        if self.is_playable == Some(false) {
            return None;
        }
        let id = self.id.filter(|id| is_spotify_id(id))?;
        let name = self.name.filter(|name| !name.trim().is_empty())?;
        let (album, artwork) = match self.album {
            Some(album) => (album.name.unwrap_or_default(), cover(album.images)),
            None => (String::new(), None),
        };
        Some(Track {
            id,
            name,
            artists: self
                .artists
                .into_iter()
                .filter_map(|artist| artist.name)
                .collect(),
            album,
            duration: Duration::from_millis(self.duration_ms.unwrap_or(0)),
            artwork,
        })
    }
}

/// Picks the cover to show: the smallest one at least 200 pixels wide, or
/// else the largest. Only `https` addresses are kept.
fn cover(images: Vec<RawImage>) -> Option<String> {
    let mut images: Vec<(u32, String)> = images
        .into_iter()
        .filter_map(|image| Some((image.width.unwrap_or(0), image.url?)))
        .filter(|(_, url)| url.starts_with("https://"))
        .collect();
    images.sort_by_key(|(width, _)| *width);
    let chosen = images
        .iter()
        .find(|(width, _)| *width >= 200)
        .or_else(|| images.last());
    chosen.map(|(_, url)| url.clone())
}

#[derive(Deserialize)]
struct DevicesAnswer {
    #[serde(default)]
    devices: Vec<RawDevice>,
}

#[derive(Deserialize)]
struct RawDevice {
    id: Option<String>,
    name: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
    #[serde(default)]
    is_active: bool,
    #[serde(default)]
    is_restricted: bool,
    #[serde(default)]
    supports_volume: bool,
}

#[derive(Deserialize)]
struct RawPlayback {
    device: Option<RawDevice>,
    #[serde(default)]
    is_playing: bool,
    progress_ms: Option<u64>,
    item: Option<RawItem>,
}

#[derive(Deserialize)]
struct RawItem {
    uri: Option<String>,
    duration_ms: Option<u64>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::test_server::{Request, TestServer};

    const TOKEN: &str = r#"{"access_token":"A1","token_type":"Bearer","expires_in":3600}"#;

    fn ok(body: &str) -> (u16, String, Vec<(String, String)>) {
        (200, body.to_owned(), Vec::new())
    }

    fn api_on(server: &TestServer) -> SpotifyApi {
        SpotifyApi::with_bases(&server.url, &server.url, "cid", "R1", |_| {})
    }

    #[test]
    fn a_search_gets_a_token_first_then_reads_the_tracks() {
        let server = TestServer::start(|request: &Request| {
            if request.target == "/api/token" {
                return ok(TOKEN);
            }
            ok(r#"{"tracks":{"items":[
                {"id":"6rqhFgbbKwnb9MLmUQDhG6","name":"So What","duration_ms":562000,
                 "artists":[{"name":"Miles Davis"},{"name":"John Coltrane"}],
                 "album":{"name":"Kind of Blue","images":[
                    {"url":"https://i.scdn.co/640","width":640},
                    {"url":"https://i.scdn.co/300","width":300},
                    {"url":"https://i.scdn.co/64","width":64}]}},
                null,
                {"id":"7aaaaaaaaaaaaaaaaaaaaa","name":"Gone","is_playable":false},
                {"id":"bad/id","name":"Trap"}
            ]}}"#)
        });
        let api = api_on(&server);

        let tracks = api.search_tracks("so what", 25).unwrap();

        assert_eq!(
            tracks,
            [Track {
                id: "6rqhFgbbKwnb9MLmUQDhG6".into(),
                name: "So What".into(),
                artists: vec!["Miles Davis".into(), "John Coltrane".into()],
                album: "Kind of Blue".into(),
                duration: Duration::from_secs(562),
                artwork: Some("https://i.scdn.co/300".into()),
            }]
        );
        let requests = server.requests();
        assert_eq!(requests[0].method, "POST");
        assert_eq!(
            requests[0].body,
            "grant_type=refresh_token&refresh_token=R1&client_id=cid"
        );
        assert_eq!(
            requests[1].target,
            "/search?q=so%20what&type=track&limit=10"
        );
        assert_eq!(requests[1].authorization.as_deref(), Some("Bearer A1"));

        // The token is kept for the next call.
        api.search_tracks("blue", 3).unwrap();
        assert_eq!(
            server
                .lines()
                .iter()
                .filter(|line| line.contains("/api/token"))
                .count(),
            1
        );
    }

    #[test]
    fn a_refused_token_is_renewed_once_and_a_new_refresh_token_kept() {
        let calls = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&calls);
        let server = TestServer::start(move |request: &Request| {
            if request.target == "/api/token" {
                return ok(r#"{"access_token":"A2","expires_in":3600,"refresh_token":"R2"}"#);
            }
            let mut calls = counter.lock().unwrap();
            *calls += 1;
            if *calls == 1 {
                (
                    401,
                    r#"{"error":{"status":401,"message":"The access token expired"}}"#.into(),
                    vec![],
                )
            } else {
                ok(r#"{"devices":[]}"#)
            }
        });
        let kept = Arc::new(Mutex::new(Vec::new()));
        let keeper = Arc::clone(&kept);
        let api = SpotifyApi::with_bases(&server.url, &server.url, "cid", "R1", move |token| {
            keeper.lock().unwrap().push(token.to_owned());
        });

        assert_eq!(api.devices().unwrap(), []);

        assert_eq!(*kept.lock().unwrap(), ["R2"]);
        let tokens = server
            .requests()
            .into_iter()
            .filter(|request| request.target == "/api/token")
            .map(|request| request.body)
            .collect::<Vec<_>>();
        assert_eq!(
            tokens,
            [
                "grant_type=refresh_token&refresh_token=R1&client_id=cid",
                "grant_type=refresh_token&refresh_token=R2&client_id=cid"
            ]
        );
    }

    #[test]
    fn an_expired_connection_asks_to_connect_again() {
        let server = TestServer::start(|_: &Request| {
            (
                400,
                r#"{"error":"invalid_grant","error_description":"Refresh token revoked"}"#.into(),
                vec![],
            )
        });
        let error = api_on(&server).devices().unwrap_err();
        assert!(
            matches!(error, SourceError::Unavailable(message) if message.contains("connect again"))
        );
    }

    #[test]
    fn devices_and_playback_are_read() {
        let server = TestServer::start(|request: &Request| match request.target.as_str() {
            "/api/token" => ok(TOKEN),
            "/me/player/devices" => ok(r#"{"devices":[
                {"id":"d1","is_active":false,"is_restricted":false,"name":"Bureau","type":"Computer",
                 "volume_percent":50,"supports_volume":true}]}"#),
            _ => ok(
                r#"{"device":{"id":"d1","name":"Bureau"},"is_playing":true,"progress_ms":1500,
                 "item":{"uri":"spotify:track:6rqhFgbbKwnb9MLmUQDhG6","duration_ms":562000},
                 "currently_playing_type":"track"}"#,
            ),
        });
        let api = api_on(&server);
        assert_eq!(
            api.devices().unwrap(),
            [Device {
                id: Some("d1".into()),
                name: "Bureau".into(),
                kind: "Computer".into(),
                is_active: false,
                is_restricted: false,
                supports_volume: true,
            }]
        );
        assert_eq!(
            api.playback().unwrap(),
            Some(PlaybackState {
                device_id: Some("d1".into()),
                is_playing: true,
                progress: Some(Duration::from_millis(1500)),
                item_uri: Some("spotify:track:6rqhFgbbKwnb9MLmUQDhG6".into()),
                duration: Some(Duration::from_secs(562)),
            })
        );
    }

    #[test]
    fn nothing_playing_is_none() {
        let server = TestServer::start(|request: &Request| match request.target.as_str() {
            "/api/token" => ok(TOKEN),
            _ => (204, String::new(), vec![]),
        });
        assert_eq!(api_on(&server).playback().unwrap(), None);
    }

    #[test]
    fn a_track_is_read_from_its_id() {
        let server = TestServer::start(|request: &Request| match request.target.as_str() {
            "/api/token" => ok(TOKEN),
            "/tracks/6rqhFgbbKwnb9MLmUQDhG6" => ok(
                r#"{"id":"6rqhFgbbKwnb9MLmUQDhG6","name":"So What","duration_ms":562000,
                    "artists":[{"name":"Miles Davis"}],"album":{"name":"Kind of Blue","images":[]}}"#,
            ),
            _ => (404, String::new(), vec![]),
        });
        let api = api_on(&server);
        let track = api.track("6rqhFgbbKwnb9MLmUQDhG6").unwrap();
        assert_eq!(track.name, "So What");
        assert_eq!(track.artists, ["Miles Davis"]);
        assert_eq!(api.track("../me"), Err(SourceError::NotFound));
        assert_eq!(
            api.track("0000000000000000000000"),
            Err(SourceError::NotFound)
        );
    }

    #[test]
    fn player_orders_name_their_device_and_carry_their_values() {
        let server = TestServer::start(|request: &Request| match request.target.as_str() {
            "/api/token" => ok(TOKEN),
            _ => (204, String::new(), vec![]),
        });
        let api = api_on(&server);
        api.play_track("d1", "spotify:track:6rqhFgbbKwnb9MLmUQDhG6")
            .unwrap();
        api.pause("d1").unwrap();
        api.resume("d1").unwrap();
        api.seek("d1", Duration::from_millis(61_500)).unwrap();
        api.set_volume("d1", 140).unwrap();

        let requests: Vec<Request> = server
            .requests()
            .into_iter()
            .filter(|request| request.target != "/api/token")
            .collect();
        let lines: Vec<String> = requests
            .iter()
            .map(|request| {
                // The JSON is compared without its layout.
                let body = serde_json::from_str::<serde_json::Value>(&request.body)
                    .map_or_else(|_| request.body.clone(), |json| json.to_string());
                format!("{} {} {body}", request.method, request.target)
            })
            .collect();
        assert_eq!(
            lines,
            [
                r#"PUT /me/player/play?device_id=d1 {"uris":["spotify:track:6rqhFgbbKwnb9MLmUQDhG6"]}"#,
                "PUT /me/player/pause?device_id=d1 ",
                "PUT /me/player/play?device_id=d1 ",
                "PUT /me/player/seek?position_ms=61500&device_id=d1 ",
                "PUT /me/player/volume?volume_percent=100&device_id=d1 ",
            ]
        );
    }

    #[test]
    fn errors_say_what_to_do() {
        let server = TestServer::start(|request: &Request| {
            match request.target.as_str() {
            "/api/token" => ok(TOKEN),
            target if target.starts_with("/me/player/play") => (
                404,
                r#"{"error":{"status":404,"message":"Player command failed: No active device found","reason":"NO_ACTIVE_DEVICE"}}"#.into(),
                vec![],
            ),
            target if target.starts_with("/me/player/pause") => (
                403,
                r#"{"error":{"status":403,"message":"Player command failed: Premium required","reason":"PREMIUM_REQUIRED"}}"#.into(),
                vec![],
            ),
            _ => (429, String::new(), vec![("Retry-After".into(), "7".into())]),
        }
        });
        let api = api_on(&server);
        assert_eq!(
            api.play_track("d1", "spotify:track:6rqhFgbbKwnb9MLmUQDhG6"),
            Err(SourceError::Unavailable(NO_DEVICE.into()))
        );
        assert!(matches!(
            api.pause("d1"),
            Err(SourceError::Unavailable(message)) if message.contains("Premium required")
        ));
        assert!(matches!(
            api.search_tracks("x", 5),
            Err(SourceError::Network(message)) if message.contains("7 seconds")
        ));
    }

    #[test]
    fn a_code_is_traded_for_a_refresh_token() {
        let server = TestServer::start(|_: &Request| {
            ok(r#"{"access_token":"A","expires_in":3600,"refresh_token":"R","scope":"x"}"#)
        });
        let refresh = SpotifyApi::exchange_code(
            &server.url,
            "cid",
            "the code",
            "http://127.0.0.1:5/callback",
            "verifier",
        )
        .unwrap();
        assert_eq!(refresh, "R");
        assert_eq!(
            server.requests()[0].body,
            "grant_type=authorization_code&code=the+code&redirect_uri=http%3A%2F%2F127.0.0.1%3A5%2Fcallback&client_id=cid&code_verifier=verifier"
        );
    }
}
