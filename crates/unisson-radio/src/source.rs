//! The radio directory as a [`Source`] of the application.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use unisson_core::{Media, MediaId, SearchQuery, Source, SourceError, SourceKind, StreamUri};

use crate::client::{RadioBrowser, StationQuery};
use crate::station::Station;

/// Most stream addresses remembered from past searches.
const MAX_KNOWN_STREAMS: usize = 2000;

/// Internet radio stations, searched in the Radio Browser directory.
///
/// # Examples
///
/// ```no_run
/// use unisson_core::{SearchQuery, SourceRegistry};
/// use unisson_radio::{RadioBrowser, RadioSource};
///
/// let mut registry = SourceRegistry::new();
/// registry.register(Box::new(RadioSource::new(RadioBrowser::new())));
/// let results = registry.search_all(&SearchQuery::new("jazz"));
/// ```
#[derive(Debug)]
pub struct RadioSource {
    client: Arc<RadioBrowser>,
    /// Stream address of the stations seen in past searches, by identifier.
    /// It saves a request to the directory when one of them is played.
    known_streams: Mutex<HashMap<String, String>>,
}

impl RadioSource {
    /// Creates the source on top of a directory client.
    pub fn new(client: RadioBrowser) -> Self {
        Self {
            client: Arc::new(client),
            known_streams: Mutex::new(HashMap::new()),
        }
    }

    /// Returns the directory client, for searches by tag or by country.
    pub fn client(&self) -> &RadioBrowser {
        &self.client
    }

    /// Teaches the source the stream address of a station.
    ///
    /// Call it for stations that do not come from a search, such as the
    /// favorites loaded at startup. They can then be played without asking
    /// the directory, which is the only way for stations added by address.
    pub fn remember(&self, station: &Station) {
        self.known_streams()
            .insert(station.uuid.clone(), station.stream_url.clone());
    }

    /// Returns the stream address of a station the source has met, in a
    /// search or through [`remember`](Self::remember).
    pub fn stream_url(&self, uuid: &str) -> Option<String> {
        self.known_streams().get(uuid).cloned()
    }

    fn known_streams(&self) -> MutexGuard<'_, HashMap<String, String>> {
        // The map is always valid, even if a thread panicked while holding it.
        self.known_streams
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Source for RadioSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Radio
    }

    /// Searches stations by name.
    fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError> {
        let stations = self
            .client
            .search(&StationQuery::by_name(query.text()).limit(query.limit()))?;

        let mut known = self.known_streams();
        if known.len() + stations.len() > MAX_KNOWN_STREAMS {
            known.clear();
        }
        for station in &stations {
            known.insert(station.uuid.clone(), station.stream_url.clone());
        }
        Ok(stations.iter().map(|station| station.to_media()).collect())
    }

    /// Returns the stream address of a station, and counts a click for it.
    ///
    /// The click is sent in the background, so playback never waits for it.
    fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
        if id.source != SourceKind::Radio {
            return Err(SourceError::Invalid(format!(
                "not a radio station: {}",
                id.source
            )));
        }
        let known = self.known_streams().get(&id.key).cloned();
        let stream_url = match known {
            Some(stream_url) => stream_url,
            // The directory does not know stations added by address.
            None if id.key.starts_with(Station::CUSTOM_PREFIX) => {
                return Err(SourceError::NotFound);
            }
            None => self.client.station(&id.key)?.stream_url,
        };
        let uri = StreamUri::new(stream_url)?;
        // The address comes from the directory, or from a saved favorite: a
        // station is on the network, and must never open a file of this
        // computer or anything else.
        if !uri.is_network() {
            return Err(SourceError::Invalid(
                "a station needs a network address".into(),
            ));
        }

        let client = Arc::clone(&self.client);
        let uuid = id.key.clone();
        thread::spawn(move || {
            // Best effort: a lost click must never disturb playback.
            let _ = client.count_click(&uuid);
        });
        Ok(uri)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use unisson_core::SourceRegistry;

    use super::*;
    use crate::test_server::TestServer;

    const FIP: &str = r#"[{"stationuuid": "aaaa-1111", "name": "FIP",
        "url_resolved": "http://stream.example.org/fip.mp3",
        "favicon": "http://example.org/fip.png"}]"#;

    fn source_on(server: &TestServer) -> RadioSource {
        RadioSource::new(RadioBrowser::for_tests(&[&server.url]))
    }

    /// Waits for the background click request to reach the server.
    fn wait_for_request(server: &TestServer, wanted: &str) {
        let start = Instant::now();
        while !server.requests().iter().any(|request| request == wanted) {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "{wanted} was never requested"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn search_returns_live_radio_media() {
        let server = TestServer::start(|_| (200, FIP.to_owned()));
        let media = source_on(&server)
            .search(&SearchQuery::new("fip").with_limit(3))
            .unwrap();

        assert_eq!(media.len(), 1);
        assert_eq!(media[0].id, MediaId::new(SourceKind::Radio, "aaaa-1111"));
        assert_eq!(media[0].title, "FIP");
        assert!(media[0].is_live());
        assert_eq!(
            media[0].artwork.as_deref(),
            Some("http://example.org/fip.png")
        );

        let request = server.requests().remove(0);
        assert!(
            request.contains("name=fip") && request.contains("limit=3"),
            "{request}"
        );
    }

    #[test]
    fn resolve_reuses_the_address_seen_in_a_search_and_counts_a_click() {
        let server = TestServer::start(|_| (200, FIP.to_owned()));
        let source = source_on(&server);
        let media = source.search(&SearchQuery::new("fip")).unwrap();

        let uri = source.resolve(&media[0].id).unwrap();

        assert_eq!(uri.as_str(), "http://stream.example.org/fip.mp3");
        wait_for_request(&server, "/json/url/aaaa-1111");
        let lookups = server
            .requests()
            .iter()
            .filter(|request| request.contains("byuuid"))
            .count();
        assert_eq!(lookups, 0, "the address was already known");
    }

    #[test]
    fn resolve_asks_the_directory_for_a_station_it_has_not_seen() {
        let server = TestServer::start(|_| (200, FIP.to_owned()));
        let source = source_on(&server);

        let uri = source
            .resolve(&MediaId::new(SourceKind::Radio, "aaaa-1111"))
            .unwrap();

        assert_eq!(uri.as_str(), "http://stream.example.org/fip.mp3");
        assert_eq!(
            server.requests()[0],
            "/json/stations/byuuid?uuids=aaaa-1111"
        );
        wait_for_request(&server, "/json/url/aaaa-1111");
    }

    #[test]
    fn a_remembered_station_is_resolved_without_the_directory() {
        let server = TestServer::start(|_| (200, "[]".to_owned()));
        let source = source_on(&server);
        let custom = Station::custom("My radio", "https://example.org/stream.mp3").unwrap();
        let id = custom.to_media().id;

        // Unknown at first, and not worth asking the directory about.
        assert_eq!(source.resolve(&id), Err(SourceError::NotFound));
        assert!(server.requests().is_empty());

        source.remember(&custom);
        assert_eq!(
            source.resolve(&id).unwrap().as_str(),
            "https://example.org/stream.mp3"
        );
        let lookups = server
            .requests()
            .iter()
            .filter(|request| request.contains("byuuid"))
            .count();
        assert_eq!(lookups, 0);
    }

    #[test]
    fn resolve_reports_an_unknown_station() {
        let server = TestServer::start(|_| (200, "[]".to_owned()));
        let source = source_on(&server);
        let missing = source.resolve(&MediaId::new(SourceKind::Radio, "cccc-3333"));
        assert_eq!(missing, Err(SourceError::NotFound));
    }

    #[test]
    fn resolve_refuses_media_from_another_source() {
        let server = TestServer::start(|_| (200, FIP.to_owned()));
        let error = source_on(&server).resolve(&MediaId::new(SourceKind::Local, "a.mp3"));
        assert!(matches!(error, Err(SourceError::Invalid(_))));
        assert!(server.requests().is_empty());
    }

    #[test]
    fn a_station_never_opens_a_local_file() {
        let server = TestServer::start(|_| (200, "[]".to_owned()));
        let source = source_on(&server);
        let mut station = Station::custom("Trap", "https://example.org/stream").unwrap();
        // As a tampered favorite, or a hostile directory entry, would be.
        station.stream_url = "file:///etc/hostname".to_owned();
        source.remember(&station);

        let error = source.resolve(&MediaId::new(SourceKind::Radio, &station.uuid));

        assert!(matches!(error, Err(SourceError::Invalid(_))), "{error:?}");
    }

    #[test]
    fn a_station_never_hands_over_another_kind_of_address() {
        let server = TestServer::start(|_| (200, "[]".to_owned()));
        let source = source_on(&server);
        let mut station = Station::custom("Trap", "https://example.org/stream").unwrap();
        station.stream_url = "spotify:track:6rqhFgbbKwnb9MLmUQDhG6".to_owned();
        source.remember(&station);

        let error = source.resolve(&MediaId::new(SourceKind::Radio, &station.uuid));

        assert!(matches!(error, Err(SourceError::Invalid(_))), "{error:?}");
    }

    #[test]
    fn the_source_works_through_the_registry() {
        let server = TestServer::start(|_| (200, FIP.to_owned()));
        let mut registry = SourceRegistry::new();
        registry.register(Box::new(source_on(&server)));

        let results = registry.search_all(&SearchQuery::new("fip"));

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SourceKind::Radio);
        let media = results[0].result.as_ref().unwrap();
        assert_eq!(
            registry.resolve(&media[0].id).unwrap().as_str(),
            "http://stream.example.org/fip.mp3"
        );
    }
}
