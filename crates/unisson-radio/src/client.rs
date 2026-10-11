//! The client of the Radio Browser directory.
//!
//! # Design
//!
//! Radio Browser runs on several mirror servers, and asks clients never to
//! depend on a single one. The client therefore fetches the list of mirrors
//! once, starts from a random one, and moves on to the next mirror when a
//! request fails.
//!
//! The directory documentation recommends finding the mirrors with a DNS
//! lookup followed by reverse lookups. The Rust standard library cannot do
//! reverse lookups, so the client uses the `/json/servers` endpoint, which
//! the documentation provides for that case.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde::de::DeserializeOwned;
use unisson_core::SourceError;
use ureq::Agent;

use crate::station::{RawStation, Station};

/// Where the list of mirror servers is fetched from.
const DISCOVERY_URL: &str = "https://all.api.radio-browser.info";

/// Sent with every request, as the directory asks, so its maintainers can
/// tell which application is calling.
const USER_AGENT: &str = concat!("Unisson/", env!("CARGO_PKG_VERSION"));

/// Longest time allowed for one request, from connection to last byte.
const TIMEOUT: Duration = Duration::from_secs(10);

/// How many times the whole list of mirrors is tried before giving up.
const ROUNDS: usize = 2;

/// Pause before the mirrors are tried again.
const PAUSE_BETWEEN_ROUNDS: Duration = Duration::from_millis(250);

/// What to search for in the directory.
///
/// Results are the most listened stations first. Stations whose stream was
/// found broken by the directory are left out.
///
/// # Examples
///
/// ```
/// use unisson_radio::StationQuery;
///
/// let by_name = StationQuery::by_name("fip");
/// let french_jazz = StationQuery::new().tag("jazz").country_code("FR").limit(10);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StationQuery {
    name: Option<String>,
    tag: Option<String>,
    country_code: Option<String>,
    limit: usize,
}

impl StationQuery {
    /// Number of stations returned by default.
    pub const DEFAULT_LIMIT: usize = 20;

    /// Creates a query with no criteria: the most listened stations overall.
    pub fn new() -> Self {
        Self {
            name: None,
            tag: None,
            country_code: None,
            limit: Self::DEFAULT_LIMIT,
        }
    }

    /// Creates a query for stations whose name contains `name`.
    pub fn by_name(name: impl AsRef<str>) -> Self {
        Self::new().name(name)
    }

    /// Keeps the stations whose name contains `name`.
    pub fn name(mut self, name: impl AsRef<str>) -> Self {
        self.name = criterion(name.as_ref());
        self
    }

    /// Keeps the stations with a tag containing `tag`, such as `jazz`.
    pub fn tag(mut self, tag: impl AsRef<str>) -> Self {
        self.tag = criterion(tag.as_ref());
        self
    }

    /// Keeps the stations of one country, given by its two-letter code.
    pub fn country_code(mut self, country_code: impl AsRef<str>) -> Self {
        self.country_code = criterion(country_code.as_ref()).map(|code| code.to_uppercase());
        self
    }

    /// Sets the maximum number of stations returned (at least 1).
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    /// Builds the parameters of the `/json/stations/search` endpoint.
    fn params(&self) -> Vec<(&'static str, String)> {
        let mut params = vec![
            ("hidebroken", "true".to_owned()),
            ("order", "clickcount".to_owned()),
            ("reverse", "true".to_owned()),
            ("limit", self.limit.to_string()),
        ];
        let criteria = [
            ("name", &self.name),
            ("tag", &self.tag),
            ("countrycode", &self.country_code),
        ];
        for (key, value) in criteria {
            if let Some(value) = value {
                params.push((key, value.clone()));
            }
        }
        params
    }
}

impl Default for StationQuery {
    fn default() -> Self {
        Self::new()
    }
}

/// Trims a criterion, and ignores it when it is empty.
fn criterion(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_owned())
    }
}

/// One entry of the `/json/servers` answer.
#[derive(Debug, Deserialize)]
struct ServerEntry {
    #[serde(default)]
    name: Option<String>,
}

/// Client of the Radio Browser directory.
///
/// Every method blocks until the directory answers, or until a timeout of
/// ten seconds per request.
#[derive(Debug)]
pub struct RadioBrowser {
    agent: Agent,
    /// Where to fetch the list of mirrors, or `None` if the list is fixed.
    discovery_url: Option<String>,
    /// Scheme used to reach the discovered mirrors.
    scheme: &'static str,
    /// Base addresses of the mirrors. The preferred one comes first.
    servers: Mutex<Vec<String>>,
}

impl RadioBrowser {
    /// Creates a client that discovers the mirror servers by itself.
    ///
    /// Nothing is fetched yet: the mirrors are looked up on the first request.
    pub fn new() -> Self {
        Self::build(Some(DISCOVERY_URL.to_owned()), "https", Vec::new(), true)
    }

    /// Creates a client that only uses the given servers, in that order.
    ///
    /// Each server is a base address such as
    /// `https://de1.api.radio-browser.info`.
    pub fn with_servers<I, S>(servers: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let servers = servers
            .into_iter()
            .map(|server| server.into().trim_end_matches('/').to_owned())
            .collect();
        Self::build(None, "https", servers, true)
    }

    fn build(
        discovery_url: Option<String>,
        scheme: &'static str,
        servers: Vec<String>,
        proxy_from_env: bool,
    ) -> Self {
        let mut config = Agent::config_builder()
            .user_agent(USER_AGENT)
            // The real directory is only reached over HTTPS, redirects
            // included. Tests talk to a local server in plain HTTP.
            .https_only(scheme == "https")
            .timeout_global(Some(TIMEOUT));
        if !proxy_from_env {
            config = config.proxy(None);
        }
        Self {
            agent: config.build().into(),
            discovery_url,
            scheme,
            servers: Mutex::new(servers),
        }
    }

    /// Searches the directory.
    ///
    /// Stations that cannot be played (no name or no stream address) are
    /// left out of the result.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Network`] if no mirror could be reached, or
    /// [`SourceError::Invalid`] if the answer cannot be understood.
    pub fn search(&self, query: &StationQuery) -> Result<Vec<Station>, SourceError> {
        let stations: Vec<RawStation> = self.get_json("/json/stations/search", &query.params())?;
        Ok(stations
            .into_iter()
            .filter_map(RawStation::into_station)
            .collect())
    }

    /// Fetches one station by its identifier.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::NotFound`] if the directory has no usable
    /// station with this identifier, or the same errors as
    /// [`search`](Self::search).
    pub fn station(&self, uuid: &str) -> Result<Station, SourceError> {
        let params = [("uuids", uuid.trim().to_owned())];
        let stations: Vec<RawStation> = self.get_json("/json/stations/byuuid", &params)?;
        stations
            .into_iter()
            .find_map(RawStation::into_station)
            .ok_or(SourceError::NotFound)
    }

    /// Tells the directory that the user started playing a station.
    ///
    /// The directory asks for this call, which it uses to rank stations by
    /// popularity. It counts at most one click per station and per day for a
    /// given network address.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Invalid`] if `uuid` is not an identifier, or
    /// the same errors as [`search`](Self::search).
    pub fn count_click(&self, uuid: &str) -> Result<(), SourceError> {
        let uuid = uuid.trim();
        // The identifier becomes part of the address: accept UUID characters only.
        let is_uuid = !uuid.is_empty() && uuid.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
        if !is_uuid {
            return Err(SourceError::Invalid(format!(
                "not a station identifier: {uuid}"
            )));
        }
        self.get_json::<serde::de::IgnoredAny>(&format!("/json/url/{uuid}"), &[])
            .map(|_| ())
    }

    /// Sends a GET request to the first mirror that answers.
    ///
    /// The mirrors are volunteer machines, and each of them hiccups now and
    /// then: a connection that was kept open is dropped, a request is
    /// refused while the machine is busy. If every mirror failed, they are
    /// all asked once more after a short pause, which is usually enough.
    fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, String)],
    ) -> Result<T, SourceError> {
        let servers = self.known_servers()?;
        let mut last_error = SourceError::Unavailable("no radio directory server is known".into());
        for round in 0..ROUNDS {
            if round > 0 {
                std::thread::sleep(PAUSE_BETWEEN_ROUNDS);
            }
            for server in &servers {
                match self.fetch(&format!("{server}{path}"), params) {
                    Ok(value) => {
                        self.prefer(server);
                        return Ok(value);
                    }
                    // This mirror is down or unwell: try the next one.
                    Err(error @ SourceError::Network(_)) => last_error = error,
                    // The mirror answered: another one would say the same.
                    Err(error) => return Err(error),
                }
            }
        }
        Err(last_error)
    }

    /// Sends one GET request, and decodes its JSON answer.
    fn fetch<T: DeserializeOwned>(
        &self,
        url: &str,
        params: &[(&str, String)],
    ) -> Result<T, SourceError> {
        let mut response = self
            .agent
            .get(url)
            .query_pairs(params.iter().map(|(key, value)| (*key, value.as_str())))
            .call()
            .map_err(|error| match error {
                ureq::Error::StatusCode(404) => SourceError::NotFound,
                ureq::Error::StatusCode(code) => {
                    SourceError::Network(format!("the server answered with status {code}"))
                }
                other => SourceError::Network(other.to_string()),
            })?;
        response
            .body_mut()
            .read_json::<T>()
            .map_err(|error| SourceError::Invalid(format!("unreadable answer: {error}")))
    }

    /// Returns the mirrors to try, discovering them first if needed.
    fn known_servers(&self) -> Result<Vec<String>, SourceError> {
        let known = self.servers().clone();
        if !known.is_empty() {
            return Ok(known);
        }
        let Some(discovery_url) = &self.discovery_url else {
            return Err(SourceError::Unavailable(
                "no radio directory server is configured".into(),
            ));
        };
        let entries: Vec<ServerEntry> =
            self.fetch(&format!("{discovery_url}/json/servers"), &[])?;
        let mut servers = server_addresses(entries, self.scheme, self.scheme == "https");
        if servers.is_empty() {
            return Err(SourceError::Unavailable(
                "the radio directory lists no server".into(),
            ));
        }
        // Start from a random mirror, to spread the load as the directory asks.
        let offset = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos() as usize);
        let start = offset % servers.len();
        servers.rotate_left(start);
        *self.servers() = servers.clone();
        Ok(servers)
    }

    /// Moves a mirror that just answered to the front of the list.
    fn prefer(&self, server: &str) {
        let mut servers = self.servers();
        if let Some(index) = servers.iter().position(|known| known == server) {
            servers[..=index].rotate_right(1);
        }
    }

    fn servers(&self) -> MutexGuard<'_, Vec<String>> {
        // The list is always valid, even if a thread panicked while holding it.
        self.servers
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Default for RadioBrowser {
    fn default() -> Self {
        Self::new()
    }
}

/// The domain every mirror of the directory belongs to.
const MIRROR_DOMAIN: &str = ".api.radio-browser.info";

/// Tells whether a name given by the directory is one of its mirrors.
///
/// The list of mirrors comes from the network. A name outside the domain
/// of the directory, or one that carries a path or a port, is not followed.
fn is_mirror_name(name: &str) -> bool {
    name.ends_with(MIRROR_DOMAIN)
        && name.len() > MIRROR_DOMAIN.len()
        && name
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || letter == '.' || letter == '-')
}

/// Turns the `/json/servers` answer into base addresses, without duplicates.
///
/// With `only_mirrors`, names that are not mirrors of the directory are
/// left out. Tests turn it off, since their servers are local addresses.
fn server_addresses(entries: Vec<ServerEntry>, scheme: &str, only_mirrors: bool) -> Vec<String> {
    let mut servers = Vec::new();
    for name in entries.into_iter().filter_map(|entry| entry.name) {
        let name = name.trim();
        if only_mirrors && !is_mirror_name(name) {
            continue;
        }
        let address = format!("{scheme}://{name}");
        if !name.is_empty() && !servers.contains(&address) {
            servers.push(address);
        }
    }
    servers
}

#[cfg(test)]
impl RadioBrowser {
    /// Client for tests: plain HTTP, fixed servers, no proxy.
    pub(crate) fn for_tests(servers: &[&str]) -> Self {
        let servers = servers.iter().map(|server| (*server).to_owned()).collect();
        Self::build(None, "http", servers, false)
    }

    /// Client for tests that discovers its servers at `discovery_url`.
    pub(crate) fn discovering_at(discovery_url: &str) -> Self {
        Self::build(Some(discovery_url.to_owned()), "http", Vec::new(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_server::{TestServer, closed_port_url};

    const TWO_STATIONS: &str = r#"[
        {"stationuuid": "aaaa-1111", "name": " FIP ", "url": "http://example.org/fip.pls",
         "url_resolved": "http://stream.example.org/fip.mp3", "favicon": "",
         "tags": "eclectic,jazz", "countrycode": "FR", "codec": "MP3", "bitrate": 128,
         "hls": 0, "votes": 1234, "unknown_field": {"nested": true}},
        {"stationuuid": "bbbb-2222", "name": "No stream", "url": "", "url_resolved": null}
    ]"#;

    fn stations_server() -> TestServer {
        TestServer::start(|_| (200, TWO_STATIONS.to_owned()))
    }

    #[test]
    fn a_query_becomes_the_documented_parameters() {
        let query = StationQuery::by_name("  fip ")
            .tag("jazz")
            .country_code("fr")
            .limit(0);
        assert_eq!(
            query.params(),
            [
                ("hidebroken", "true".to_owned()),
                ("order", "clickcount".to_owned()),
                ("reverse", "true".to_owned()),
                ("limit", "1".to_owned()),
                ("name", "fip".to_owned()),
                ("tag", "jazz".to_owned()),
                ("countrycode", "FR".to_owned()),
            ]
        );
        // Empty criteria are not sent at all.
        assert_eq!(StationQuery::by_name("  ").params().len(), 4);
    }

    #[test]
    fn search_calls_the_search_endpoint_and_keeps_playable_stations() {
        let server = stations_server();
        let client = RadioBrowser::for_tests(&[&server.url]);

        let stations = client
            .search(&StationQuery::by_name("fip & co").limit(5))
            .unwrap();

        assert_eq!(stations.len(), 1, "the station without a stream is dropped");
        assert_eq!(stations[0].name, "FIP");
        assert_eq!(stations[0].stream_url, "http://stream.example.org/fip.mp3");
        assert_eq!(stations[0].tags, ["eclectic", "jazz"]);
        assert_eq!(stations[0].bitrate, Some(128));

        let request = server.requests().remove(0);
        assert!(request.starts_with("/json/stations/search?"), "{request}");
        assert!(request.contains("limit=5"), "{request}");
        assert!(request.contains("hidebroken=true"), "{request}");
        // The ampersand of the name is escaped, so it cannot add a parameter.
        assert!(request.contains("%26"), "{request}");
        assert_eq!(request.matches('&').count(), 4, "{request}");
    }

    #[test]
    fn requests_carry_the_application_name() {
        let server = stations_server();
        RadioBrowser::for_tests(&[&server.url])
            .search(&StationQuery::new())
            .unwrap();
        assert_eq!(server.user_agents(), [USER_AGENT]);
    }

    #[test]
    fn a_dead_server_is_skipped_and_the_working_one_becomes_preferred() {
        let server = stations_server();
        let dead = closed_port_url();
        let client = RadioBrowser::for_tests(&[&dead, &server.url]);

        assert_eq!(client.search(&StationQuery::new()).unwrap().len(), 1);
        assert_eq!(*client.servers(), [server.url.clone(), dead]);
    }

    #[test]
    fn a_server_error_moves_on_to_the_next_server() {
        let failing = TestServer::start(|_| (503, "busy".to_owned()));
        let server = stations_server();
        let client = RadioBrowser::for_tests(&[&failing.url, &server.url]);

        assert_eq!(client.search(&StationQuery::new()).unwrap().len(), 1);
        assert_eq!(failing.requests().len(), 1);
    }

    #[test]
    fn a_passing_failure_of_every_server_is_tried_again() {
        // The only server is busy for its first request, then answers.
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let server = TestServer::start(move |_| {
            if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                (503, "busy".to_owned())
            } else {
                (200, "[]".to_owned())
            }
        });
        let client = RadioBrowser::for_tests(&[&server.url]);

        assert_eq!(client.search(&StationQuery::new()), Ok(Vec::new()));
        assert_eq!(server.requests().len(), 2);
    }

    #[test]
    fn a_lasting_failure_is_given_up_after_two_rounds() {
        let failing = TestServer::start(|_| (503, "busy".to_owned()));
        let client = RadioBrowser::for_tests(&[&failing.url]);

        let error = client.search(&StationQuery::new()).unwrap_err();

        assert!(matches!(error, SourceError::Network(_)), "{error}");
        assert_eq!(failing.requests().len(), ROUNDS);
    }

    #[test]
    fn when_every_server_is_down_the_error_is_a_network_error() {
        let client = RadioBrowser::for_tests(&[&closed_port_url(), &closed_port_url()]);
        let error = client.search(&StationQuery::new()).unwrap_err();
        assert!(matches!(error, SourceError::Network(_)), "{error}");
    }

    #[test]
    fn an_unreadable_answer_is_reported_without_trying_other_servers() {
        let broken = TestServer::start(|_| (200, "<html>not json</html>".to_owned()));
        let other = stations_server();
        let client = RadioBrowser::for_tests(&[&broken.url, &other.url]);

        let error = client.search(&StationQuery::new()).unwrap_err();
        assert!(matches!(error, SourceError::Invalid(_)), "{error}");
        assert!(other.requests().is_empty());
    }

    #[test]
    fn a_client_without_servers_says_so() {
        let error = RadioBrowser::for_tests(&[])
            .search(&StationQuery::new())
            .unwrap_err();
        assert!(matches!(error, SourceError::Unavailable(_)), "{error}");
    }

    #[test]
    fn servers_are_discovered_once_then_reused() {
        let mirror = stations_server();
        let mirror_name = mirror.url.trim_start_matches("http://").to_owned();
        let discovery = TestServer::start(move |_| {
            let body = format!(
                r#"[{{"ip": "127.0.0.1", "name": "{mirror_name}"}},
                    {{"ip": "::1", "name": "{mirror_name}"}}, {{"ip": "10.0.0.1"}}]"#
            );
            (200, body)
        });
        let client = RadioBrowser::discovering_at(&discovery.url);

        client.search(&StationQuery::new()).unwrap();
        client.search(&StationQuery::new()).unwrap();

        assert_eq!(discovery.requests(), ["/json/servers"]);
        assert_eq!(mirror.requests().len(), 2);
        assert_eq!(
            *client.servers(),
            std::slice::from_ref(&mirror.url),
            "duplicates are merged"
        );
    }

    #[test]
    fn a_failed_discovery_is_a_network_error_and_is_retried_later() {
        let client = RadioBrowser::discovering_at(&closed_port_url());
        let error = client.search(&StationQuery::new()).unwrap_err();
        assert!(matches!(error, SourceError::Network(_)), "{error}");
        assert!(client.servers().is_empty());
    }

    #[test]
    fn station_fetches_one_station_by_identifier() {
        let server = stations_server();
        let client = RadioBrowser::for_tests(&[&server.url]);

        assert_eq!(client.station("aaaa-1111").unwrap().name, "FIP");
        assert_eq!(server.requests(), ["/json/stations/byuuid?uuids=aaaa-1111"]);

        let empty = TestServer::start(|_| (200, "[]".to_owned()));
        let client = RadioBrowser::for_tests(&[&empty.url]);
        assert_eq!(client.station("cccc-3333"), Err(SourceError::NotFound));
    }

    #[test]
    fn count_click_calls_the_click_endpoint() {
        let server = TestServer::start(|_| {
            (
                200,
                r#"{"ok": "true", "message": "retrieved station url", "url": "x"}"#.to_owned(),
            )
        });
        let client = RadioBrowser::for_tests(&[&server.url]);

        assert_eq!(client.count_click("aaaa-1111"), Ok(()));
        assert_eq!(server.requests(), ["/json/url/aaaa-1111"]);
    }

    #[test]
    fn count_click_rejects_anything_that_is_not_an_identifier() {
        let server = stations_server();
        let client = RadioBrowser::for_tests(&[&server.url]);
        for bad in ["", "../servers", "aaaa/1111", "aaaa?x=1"] {
            let error = client.count_click(bad).unwrap_err();
            assert!(matches!(error, SourceError::Invalid(_)), "{bad}: {error}");
        }
        assert!(server.requests().is_empty());
    }

    #[test]
    fn server_addresses_are_cleaned_up() {
        let entries = [
            "de1.example.org",
            " de1.example.org ",
            "",
            "fi1.example.org",
        ]
        .map(|name| ServerEntry {
            name: Some(name.to_owned()),
        });
        assert_eq!(
            server_addresses(entries.into(), "https", false),
            ["https://de1.example.org", "https://fi1.example.org"]
        );
    }

    #[test]
    fn only_mirrors_of_the_directory_are_followed() {
        let entries = [
            "de1.api.radio-browser.info",
            "evil.example.org",
            "api.radio-browser.info.evil.example.org",
            "de1.api.radio-browser.info/path",
            "user@fi1.api.radio-browser.info",
            "fi1.api.radio-browser.info:8443",
            ".api.radio-browser.info",
            "fi1.api.radio-browser.info",
        ]
        .map(|name| ServerEntry {
            name: Some(name.to_owned()),
        });
        assert_eq!(
            server_addresses(entries.into(), "https", true),
            [
                "https://de1.api.radio-browser.info",
                "https://fi1.api.radio-browser.info"
            ]
        );
    }
}
