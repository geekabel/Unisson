//! The contract of a source, and the registry that queries all of them.
//!
//! # Design
//!
//! The methods of [`Source`] are blocking rather than `async`. According to
//! the Rust Reference, an `async fn` method cannot be called through a trait
//! object (`dyn Source`), and the registry stores its sources in a
//! `Vec<Box<dyn Source>>`. Each source is therefore called on its own thread,
//! so a slow source never blocks the others.

use std::sync::{Arc, mpsc};
use std::thread;

use crate::error::SourceError;
use crate::model::{Media, MediaId, SourceKind, StreamUri};

/// A search typed by the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    text: String,
    limit: usize,
}

impl SearchQuery {
    /// Number of results requested from each source by default.
    pub const DEFAULT_LIMIT: usize = 20;

    /// Creates a query. Leading and trailing whitespace is removed.
    pub fn new(text: impl AsRef<str>) -> Self {
        Self {
            text: text.as_ref().trim().to_owned(),
            limit: Self::DEFAULT_LIMIT,
        }
    }

    /// Sets the number of results requested from each source (at least 1).
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    /// Returns the searched text.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the maximum number of results per source.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Returns `true` if the text is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// What every source (radio, local files, YouTube) can do.
///
/// The `Send + Sync` bounds let the registry call a source from several
/// threads.
pub trait Source: Send + Sync {
    /// Returns which source this is.
    fn kind(&self) -> SourceKind;

    /// Searches for media.
    ///
    /// This call blocks and may be slow, since it can hit the network.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] if the source cannot be reached or sends an
    /// answer that cannot be understood.
    fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError>;

    /// Returns the address to play for a media.
    ///
    /// Call it right before playback, because some addresses expire.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::NotFound`] if the media is unknown, or another
    /// [`SourceError`] if the source cannot answer.
    fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError>;
}

/// A shared source is a source.
///
/// It lets the application keep a handle to a source it registered, for
/// what only that source can do.
impl<S: Source + ?Sized> Source for Arc<S> {
    fn kind(&self) -> SourceKind {
        (**self).kind()
    }

    fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError> {
        (**self).search(query)
    }

    fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
        (**self).resolve(id)
    }
}

/// The answer of one source to a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResults {
    /// The source that answered.
    pub source: SourceKind,
    /// Its media, or its error.
    pub result: Result<Vec<Media>, SourceError>,
}

/// The set of active sources, with at most one source per [`SourceKind`].
#[derive(Default)]
pub struct SourceRegistry {
    sources: Vec<Box<dyn Source>>,
}

impl SourceRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a source.
    ///
    /// If a source of the same kind is already registered, it is replaced and
    /// keeps its position in the order.
    pub fn register(&mut self, source: Box<dyn Source>) {
        let kind = source.kind();
        match self
            .sources
            .iter_mut()
            .find(|existing| existing.kind() == kind)
        {
            Some(slot) => *slot = source,
            None => self.sources.push(source),
        }
    }

    /// Removes a source. Returns `true` if it was registered.
    pub fn unregister(&mut self, kind: SourceKind) -> bool {
        let before = self.sources.len();
        self.sources.retain(|source| source.kind() != kind);
        self.sources.len() != before
    }

    /// Returns the active sources, in registration order.
    pub fn kinds(&self) -> Vec<SourceKind> {
        self.sources.iter().map(|source| source.kind()).collect()
    }

    /// Queries every source at the same time.
    ///
    /// There is one answer per source, in registration order. A source that
    /// fails, or even panics, does not prevent the others from answering: its
    /// own entry carries the error. An empty query contacts no source.
    pub fn search_all(&self, query: &SearchQuery) -> Vec<SourceResults> {
        if query.is_empty() {
            return Vec::new();
        }
        thread::scope(|scope| {
            let handles: Vec<_> = self
                .sources
                .iter()
                .map(|source| (source.kind(), scope.spawn(|| source.search(query))))
                .collect();
            handles
                .into_iter()
                .map(|(source, handle)| {
                    // `join` returns `Err` when the thread panicked. Turn that
                    // into a regular error for this source only.
                    let result = handle.join().unwrap_or_else(|_| {
                        Err(SourceError::Unavailable("the source panicked".into()))
                    });
                    SourceResults { source, result }
                })
                .collect()
        })
    }

    /// Searches every source at the same time, and hands over each answer
    /// as soon as it arrives.
    ///
    /// A fast source, such as the local library, is not held back by a slow
    /// one that waits for the network. `on_result` is called once per
    /// source, on the calling thread, in the order the sources answer. The
    /// call returns when every source has answered.
    ///
    /// An empty query contacts no source.
    pub fn search_each(&self, query: &SearchQuery, on_result: impl FnMut(SourceResults)) {
        self.search_each_in(query, &self.kinds(), on_result);
    }

    /// Same as [`search_each`](Self::search_each), asking only the sources
    /// listed in `kinds`.
    ///
    /// It serves a user who switched some sources off.
    pub fn search_each_in(
        &self,
        query: &SearchQuery,
        kinds: &[SourceKind],
        mut on_result: impl FnMut(SourceResults),
    ) {
        if query.is_empty() {
            return;
        }
        thread::scope(|scope| {
            let (answers, inbox) = mpsc::channel();
            let handles: Vec<_> = self
                .sources
                .iter()
                .filter(|source| kinds.contains(&source.kind()))
                .map(|source| {
                    let answers = answers.clone();
                    let handle = scope.spawn(move || {
                        let result = source.search(query);
                        let _ = answers.send(SourceResults {
                            source: source.kind(),
                            result,
                        });
                    });
                    (source.kind(), handle)
                })
                .collect();
            // Only the threads hold a sender now: the loop below ends when
            // the last of them is done.
            drop(answers);

            let mut answered = Vec::new();
            for results in inbox {
                answered.push(results.source);
                on_result(results);
            }
            // A source that panicked sent nothing: report it as unavailable.
            for (source, handle) in handles {
                if handle.join().is_err() && !answered.contains(&source) {
                    on_result(SourceResults {
                        source,
                        result: Err(SourceError::Unavailable("the source panicked".into())),
                    });
                }
            }
        });
    }

    /// Asks the source of a media for its playback address.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the source of the media is not
    /// registered. Otherwise, forwards the error of [`Source::resolve`].
    pub fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
        self.sources
            .iter()
            .find(|source| source.kind() == id.source)
            .ok_or_else(|| SourceError::Unavailable(format!("{} is not enabled", id.source)))?
            .resolve(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test double: a source that answers with what it was given.
    struct FakeSource {
        kind: SourceKind,
        behaviour: Behaviour,
    }

    enum Behaviour {
        Titles(Vec<&'static str>),
        Fails(SourceError),
        Panics,
    }

    impl FakeSource {
        fn boxed(kind: SourceKind, behaviour: Behaviour) -> Box<dyn Source> {
            Box::new(Self { kind, behaviour })
        }
    }

    impl Source for FakeSource {
        fn kind(&self) -> SourceKind {
            self.kind
        }

        fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError> {
            match &self.behaviour {
                Behaviour::Titles(titles) => Ok(titles
                    .iter()
                    .filter(|title| title.to_lowercase().contains(&query.text().to_lowercase()))
                    .take(query.limit())
                    .map(|title| Media::track(MediaId::new(self.kind, *title), *title))
                    .collect()),
                Behaviour::Fails(error) => Err(error.clone()),
                Behaviour::Panics => panic!("deliberate test failure"),
            }
        }

        fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
            match &self.behaviour {
                Behaviour::Titles(titles) if titles.contains(&id.key.as_str()) => {
                    StreamUri::new(format!("https://example.org/{}", id.key.replace(' ', "-")))
                }
                Behaviour::Titles(_) => Err(SourceError::NotFound),
                Behaviour::Fails(error) => Err(error.clone()),
                Behaviour::Panics => panic!("deliberate test failure"),
            }
        }
    }

    fn titles(source: &SourceResults) -> Vec<&str> {
        source
            .result
            .as_ref()
            .unwrap()
            .iter()
            .map(|media| media.title.as_str())
            .collect()
    }

    #[test]
    fn query_is_trimmed_and_limited() {
        let query = SearchQuery::new("  jazz ");
        assert_eq!(query.text(), "jazz");
        assert_eq!(query.limit(), SearchQuery::DEFAULT_LIMIT);
        assert_eq!(query.with_limit(0).limit(), 1);
        assert!(SearchQuery::new("   ").is_empty());
    }

    #[test]
    fn every_source_answers_in_registration_order() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Radio,
            Behaviour::Titles(vec!["TSF Jazz", "FIP"]),
        ));
        registry.register(FakeSource::boxed(
            SourceKind::YouTube,
            Behaviour::Titles(vec!["Jazz for work", "Lofi", "Smooth jazz"]),
        ));

        let results = registry.search_all(&SearchQuery::new("jazz"));

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, SourceKind::Radio);
        assert_eq!(titles(&results[0]), ["TSF Jazz"]);
        assert_eq!(results[1].source, SourceKind::YouTube);
        assert_eq!(titles(&results[1]), ["Jazz for work", "Smooth jazz"]);
    }

    #[test]
    fn the_limit_applies_to_each_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["a1", "a2", "a3"]),
        ));
        let results = registry.search_all(&SearchQuery::new("a").with_limit(2));
        assert_eq!(titles(&results[0]), ["a1", "a2"]);
    }

    #[test]
    fn a_failing_or_crashing_source_does_not_break_the_others() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Radio,
            Behaviour::Fails(SourceError::Network("offline".into())),
        ));
        registry.register(FakeSource::boxed(SourceKind::YouTube, Behaviour::Panics));
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["So What"]),
        ));

        let results = registry.search_all(&SearchQuery::new("so"));

        assert_eq!(
            results[0].result,
            Err(SourceError::Network("offline".into()))
        );
        assert!(matches!(
            results[1].result,
            Err(SourceError::Unavailable(_))
        ));
        assert_eq!(titles(&results[2]), ["So What"]);
    }

    /// A source that answers after a delay.
    struct SlowSource {
        kind: SourceKind,
        delay: std::time::Duration,
    }

    impl Source for SlowSource {
        fn kind(&self) -> SourceKind {
            self.kind
        }

        fn search(&self, _: &SearchQuery) -> Result<Vec<Media>, SourceError> {
            thread::sleep(self.delay);
            Ok(Vec::new())
        }

        fn resolve(&self, _: &MediaId) -> Result<StreamUri, SourceError> {
            Err(SourceError::NotFound)
        }
    }

    #[test]
    fn search_each_hands_over_answers_as_they_arrive() {
        let mut registry = SourceRegistry::new();
        registry.register(Box::new(SlowSource {
            kind: SourceKind::Radio,
            delay: std::time::Duration::from_millis(300),
        }));
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["So What"]),
        ));

        let start = std::time::Instant::now();
        let mut seen = Vec::new();
        registry.search_each(&SearchQuery::new("so"), |results| {
            seen.push((results.source, start.elapsed()));
        });

        // The fast source answers first, long before the slow one.
        assert_eq!(seen[0].0, SourceKind::Local);
        assert_eq!(seen[1].0, SourceKind::Radio);
        assert!(
            seen[0].1 < std::time::Duration::from_millis(200),
            "{seen:?}"
        );
        assert!(
            seen[1].1 >= std::time::Duration::from_millis(300),
            "{seen:?}"
        );
    }

    #[test]
    fn search_each_reports_failures_and_crashes_per_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(SourceKind::Radio, Behaviour::Panics));
        registry.register(FakeSource::boxed(
            SourceKind::YouTube,
            Behaviour::Fails(SourceError::Network("down".into())),
        ));
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["So What"]),
        ));

        let mut seen = Vec::new();
        registry.search_each(&SearchQuery::new("so"), |results| seen.push(results));

        assert_eq!(seen.len(), 3);
        let of = |kind| {
            &seen
                .iter()
                .find(|results| results.source == kind)
                .unwrap()
                .result
        };
        assert_eq!(of(SourceKind::Local).as_ref().unwrap().len(), 1);
        assert_eq!(
            of(SourceKind::YouTube),
            &Err(SourceError::Network("down".into()))
        );
        assert!(matches!(
            of(SourceKind::Radio),
            Err(SourceError::Unavailable(_))
        ));

        let mut called = false;
        registry.search_each(&SearchQuery::new("  "), |_| called = true);
        assert!(!called, "an empty query contacts no source");
    }

    #[test]
    fn search_each_in_asks_only_the_listed_sources() {
        let mut registry = SourceRegistry::new();
        // A source that is not asked cannot crash the search.
        registry.register(FakeSource::boxed(SourceKind::Radio, Behaviour::Panics));
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["So What"]),
        ));

        let mut seen = Vec::new();
        registry.search_each_in(&SearchQuery::new("so"), &[SourceKind::Local], |results| {
            seen.push(results.source);
        });
        assert_eq!(seen, [SourceKind::Local]);

        let mut called = false;
        registry.search_each_in(&SearchQuery::new("so"), &[], |_| called = true);
        assert!(!called);
    }

    #[test]
    fn an_empty_query_contacts_no_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(SourceKind::YouTube, Behaviour::Panics));
        assert!(registry.search_all(&SearchQuery::new("  ")).is_empty());
    }

    #[test]
    fn registering_the_same_kind_replaces_the_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Radio,
            Behaviour::Titles(vec!["old"]),
        ));
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec![]),
        ));
        registry.register(FakeSource::boxed(
            SourceKind::Radio,
            Behaviour::Titles(vec!["new"]),
        ));

        assert_eq!(registry.kinds(), [SourceKind::Radio, SourceKind::Local]);
        let results = registry.search_all(&SearchQuery::new("e"));
        assert_eq!(titles(&results[0]), ["new"]);

        assert!(registry.unregister(SourceKind::Radio));
        assert!(!registry.unregister(SourceKind::Radio));
        assert_eq!(registry.kinds(), [SourceKind::Local]);
    }

    #[test]
    fn resolve_goes_to_the_right_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["So What"]),
        ));

        let found = registry.resolve(&MediaId::new(SourceKind::Local, "So What"));
        assert_eq!(found.unwrap().as_str(), "https://example.org/So-What");

        let missing = registry.resolve(&MediaId::new(SourceKind::Local, "unknown"));
        assert_eq!(missing, Err(SourceError::NotFound));

        let inactive = registry.resolve(&MediaId::new(SourceKind::YouTube, "abc"));
        assert_eq!(
            inactive,
            Err(SourceError::Unavailable("YouTube is not enabled".into()))
        );
    }
}
