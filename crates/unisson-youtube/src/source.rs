//! The [`YouTubeSource`]: YouTube behind the common source contract.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use unisson_core::{Media, MediaId, SearchQuery, Source, SourceError, SourceKind, StreamUri};

use crate::component::Component;
use crate::ytdlp::{Run, YtDlp};

/// How long the address of an audio stream is reused. YouTube keeps an
/// address alive for a few hours: this stays well below.
const ADDRESS_LIFETIME: Duration = Duration::from_secs(20 * 60);

/// Most addresses remembered at a time.
const MAX_ADDRESSES: usize = 100;

/// The YouTube source.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
///
/// use unisson_core::{SearchQuery, SourceRegistry};
/// use unisson_youtube::YouTubeSource;
///
/// let mut registry = SourceRegistry::new();
/// registry.register(Box::new(YouTubeSource::new()));
/// let results = registry.search_all(&SearchQuery::new("take five"));
/// ```
#[derive(Debug)]
pub struct YouTubeSource {
    tool: YtDlp,
    /// The downloaded yt-dlp, when Unisson manages it.
    component: Option<Arc<Component>>,
    /// The search in progress, to cancel it when a newer one starts.
    search: Mutex<Option<Arc<Run>>>,
    /// The audio addresses found lately, by video identifier.
    addresses: Mutex<HashMap<String, (StreamUri, Instant)>>,
}

impl Default for YouTubeSource {
    fn default() -> Self {
        Self::new()
    }
}

impl YouTubeSource {
    /// Creates the source on top of the `yt-dlp` program of the `PATH`.
    pub fn new() -> Self {
        Self::with_tool(YtDlp::new())
    }

    /// Creates the source on top of a given yt-dlp.
    pub fn with_tool(tool: YtDlp) -> Self {
        Self {
            tool,
            component: None,
            search: Mutex::new(None),
            addresses: Mutex::new(HashMap::new()),
        }
    }

    /// Creates the source on top of a downloaded yt-dlp. Each use of the
    /// source gets the component ready: see [`Component::prepare`]. Until
    /// it is, the yt-dlp of the `PATH` answers, if there is one.
    pub fn with_component(tool: YtDlp, component: Arc<Component>) -> Self {
        Self {
            component: Some(component),
            ..Self::with_tool(tool)
        }
    }

    /// Returns the yt-dlp the source uses, to ask its version or to look up
    /// a link.
    pub fn tool(&self) -> &YtDlp {
        &self.tool
    }

    /// Returns the downloaded yt-dlp, when Unisson manages it.
    pub fn component(&self) -> Option<&Arc<Component>> {
        self.component.as_ref()
    }

    /// Gets the component ready, in the background. Does nothing without
    /// one.
    pub fn prepare(&self) {
        if let Some(component) = &self.component {
            component.prepare();
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The value is still usable if a thread panicked while holding it.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl Source for YouTubeSource {
    fn kind(&self) -> SourceKind {
        SourceKind::YouTube
    }

    /// Searches YouTube.
    ///
    /// Only one search runs at a time: starting one cancels the previous
    /// one, whose caller gets a [`SourceError::Unavailable`]. Each search
    /// starts a program, and the answer to an older text is of no use once
    /// the user has typed more.
    fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        self.prepare();
        let run = self.tool.start_search(query.text(), query.limit())?;
        if let Some(previous) = lock(&self.search).replace(Arc::clone(&run)) {
            previous.cancel();
        }
        let result = self.tool.read_search(&run);
        // Forget the search once it is over, unless a newer one took its place.
        let mut current = lock(&self.search);
        if current
            .as_ref()
            .is_some_and(|other| Arc::ptr_eq(other, &run))
        {
            *current = None;
        }
        result
    }

    /// Returns the address of the audio stream of a video.
    ///
    /// An address found less than twenty minutes ago is reused, so that
    /// going back to a track does not wait for yt-dlp again.
    fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
        if id.source != SourceKind::YouTube {
            return Err(SourceError::Invalid(format!(
                "not a YouTube video: {}",
                id.source
            )));
        }
        self.prepare();
        if let Some((address, found_at)) = lock(&self.addresses).get(&id.key)
            && found_at.elapsed() < ADDRESS_LIFETIME
        {
            return Ok(address.clone());
        }
        let address = self.tool.stream_uri(&id.key)?;

        let mut addresses = lock(&self.addresses);
        addresses.retain(|_, (_, found_at)| found_at.elapsed() < ADDRESS_LIFETIME);
        if addresses.len() >= MAX_ADDRESSES {
            addresses.clear();
        }
        addresses.insert(id.key.clone(), (address.clone(), Instant::now()));
        Ok(address)
    }
}
