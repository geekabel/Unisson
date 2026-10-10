//! What the application keeps alive while it runs.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use unisson_audio::Engine;
use unisson_core::{MediaId, SourceRegistry};
use unisson_playback::{Session, SessionEvent};
use unisson_radio::{RadioBrowser, RadioSource};
use unisson_store::{LocalSource, Store};

/// The long-lived parts of the application, shared by every command.
pub struct AppState {
    /// Drives playback.
    pub session: Session,
    /// The database.
    pub store: Arc<Store>,
    /// The radio source, kept apart to teach it stations added by address.
    pub radio: Arc<RadioSource>,
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
        // Favorite stations play without asking the directory first.
        for station in store.favorite_stations()? {
            radio.remember(&station);
        }

        let mut sources = SourceRegistry::new();
        sources.register(Box::new(LocalSource::new(Arc::clone(&store))));
        sources.register(Box::new(Arc::clone(&radio)));

        let (engine, engine_events) = Engine::spawn()?;
        let resolver = Arc::new(move |id: &MediaId| sources.resolve(id));
        let (session, events) = Session::start(engine, engine_events, resolver);

        Ok((
            Self {
                session,
                store,
                radio,
            },
            events,
        ))
    }
}
