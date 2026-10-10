//! What the application keeps alive while it runs.

use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard};

use unisson_audio::Engine;
use unisson_core::{MediaId, SourceRegistry};
use unisson_playback::{Session, SessionEvent};
use unisson_radio::{Favorites, RadioBrowser, RadioSource};
use unisson_store::{LocalSource, Store};

/// The long-lived parts of the application, shared by every command.
pub struct AppState {
    /// Drives playback.
    pub session: Session,
    /// The database.
    pub store: Arc<Store>,
    /// The radio source, kept apart for what only it can do: exploring the
    /// directory, and learning stations added by address.
    pub radio: Arc<RadioSource>,
    /// Every active source, for the unified search.
    pub sources: Arc<SourceRegistry>,
    /// The favorite stations, kept in memory. The database holds a copy.
    stations: Mutex<Favorites>,
}

impl AppState {
    /// Opens the database, starts the audio engine and the session.
    ///
    /// Returns the state and the receiver of the session events.
    ///
    /// # Errors
    ///
    /// Returns an error if the database cannot be opened or if the audio
    /// engine cannot start.
    pub fn start() -> Result<(Self, Receiver<SessionEvent>), Box<dyn std::error::Error>> {
        let store = Arc::new(Store::open_default()?);

        let radio = Arc::new(RadioSource::new(RadioBrowser::new()));
        let stations = Favorites::from_stations(store.favorite_stations()?);
        // Favorite stations play without asking the directory first.
        for station in stations.stations() {
            radio.remember(station);
        }

        let mut sources = SourceRegistry::new();
        sources.register(Box::new(LocalSource::new(Arc::clone(&store))));
        sources.register(Box::new(Arc::clone(&radio)));
        let sources = Arc::new(sources);

        let (engine, engine_events) = Engine::spawn()?;
        let resolver = {
            let sources = Arc::clone(&sources);
            Arc::new(move |id: &MediaId| sources.resolve(id))
        };
        let (session, events) = Session::start(engine, engine_events, resolver);

        Ok((
            Self {
                session,
                store,
                radio,
                sources,
                stations: Mutex::new(stations),
            },
            events,
        ))
    }

    /// Gives access to the favorite stations.
    ///
    /// Keep the guard for a short time only: it blocks the other commands
    /// that need the favorites.
    pub fn stations(&self) -> MutexGuard<'_, Favorites> {
        // The list is always valid, even if a thread panicked while holding it.
        self.stations
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
