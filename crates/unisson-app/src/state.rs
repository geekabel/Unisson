//! What the application keeps alive while it runs.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use unisson_audio::Engine;
use unisson_core::{Media, MediaId, SourceKind, SourceRegistry};
use unisson_playback::{Session, SessionEvent, Snapshot};
use unisson_radio::{Favorites, RadioBrowser, RadioSource, Station};
use unisson_store::{LocalSource, Store, StoreError};
use unisson_youtube::YouTubeSource;

use crate::dto::SnapshotDto;
use crate::settings::{Settings, SettingsError};

/// Which way "previous" and "next" go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Towards what was before.
    Previous,
    /// Towards what comes after.
    Next,
}

/// The long-lived parts of the application, shared by every command.
pub struct AppState {
    /// Drives playback.
    pub session: Session,
    /// The database.
    pub store: Arc<Store>,
    /// The radio source, kept apart for what only it can do: exploring the
    /// directory, and learning stations added by address.
    pub radio: Arc<RadioSource>,
    /// The YouTube source, kept apart to play pasted links and to tell
    /// whether yt-dlp is installed.
    pub youtube: Arc<YouTubeSource>,
    /// Every source, for the unified search.
    pub sources: Arc<SourceRegistry>,
    /// The favorite stations, kept in memory. The database holds a copy.
    stations: Mutex<Favorites>,
    /// The radio station listened to most recently.
    last_radio: Mutex<Option<Media>>,
    /// The settings of the user. Their file holds a copy.
    settings: Mutex<Settings>,
    /// Where the settings are saved, when a home directory is known.
    settings_path: Option<PathBuf>,
    /// Whether the settings changed since they were last written.
    settings_unsaved: AtomicBool,
    /// The version of yt-dlp, `None` if it is not installed, with the time
    /// it was asked.
    yt_dlp: Mutex<Option<(Instant, Option<String>)>>,
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
        let settings_path = Settings::default_path();
        let settings = settings_path
            .as_deref()
            .map(Settings::load)
            .unwrap_or_default();

        let radio = Arc::new(RadioSource::new(RadioBrowser::new()));
        let stations = Favorites::from_stations(store.favorite_stations()?);
        // Favorite stations play without asking the directory first.
        for station in stations.stations() {
            radio.remember(station);
        }

        let mut sources = SourceRegistry::new();
        sources.register(Box::new(LocalSource::new(Arc::clone(&store))));
        sources.register(Box::new(Arc::clone(&radio)));
        let youtube = Arc::new(YouTubeSource::new());
        sources.register(Box::new(Arc::clone(&youtube)));
        let sources = Arc::new(sources);

        let (engine, engine_events) = Engine::spawn()?;
        let resolver = {
            let sources = Arc::clone(&sources);
            Arc::new(move |id: &MediaId| sources.resolve(id))
        };
        let (session, events) = Session::start(engine, engine_events, resolver);
        session.set_volume(settings.volume);

        // The station of the last session is the one to come back to.
        let last_radio = store
            .recently_played(HISTORY_DEPTH)?
            .into_iter()
            .find(|media| is_station(media) && crate::home::is_playable(media, &stations));

        let state = Self {
            session,
            store,
            radio,
            youtube,
            sources,
            stations: Mutex::new(stations),
            last_radio: Mutex::new(last_radio),
            settings: Mutex::new(settings),
            settings_path,
            settings_unsaved: AtomicBool::new(false),
            yt_dlp: Mutex::new(None),
        };
        state.apply_fallback();
        Ok((state, events))
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

    /// Changes the favorite stations, and saves them.
    ///
    /// The change is all or nothing: if the database refuses it, the list
    /// in memory stays as it was.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Database`] if the stations cannot be saved.
    pub fn update_stations<T>(
        &self,
        change: impl FnOnce(&mut Favorites) -> T,
    ) -> Result<T, StoreError> {
        let mut stations = self.stations();
        let mut changed = stations.clone();
        let result = change(&mut changed);
        self.store.save_favorite_stations(changed.stations())?;
        *stations = changed;
        Ok(result)
    }

    /// Returns the radio station listened to most recently, if any.
    pub fn last_radio(&self) -> Option<Media> {
        self.lock_last_radio().clone()
    }

    /// Returns the version of yt-dlp, or `None` if it cannot be used.
    ///
    /// Asking means starting the program, which takes a moment: the answer
    /// is kept for a minute. Pass `refresh` to ask again now.
    pub fn yt_dlp_version(&self, refresh: bool) -> Option<String> {
        let known = self
            .yt_dlp
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some((asked_at, version)) = known
            && !refresh
            && asked_at.elapsed() < YT_DLP_CHECK_LIFETIME
        {
            return version;
        }
        // The lock is not held while the program runs.
        let version = self.youtube.tool().version().ok();
        *self
            .yt_dlp
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) =
            Some((Instant::now(), version.clone()));
        version
    }

    /// Returns a copy of the settings.
    pub fn settings(&self) -> Settings {
        self.lock_settings().clone()
    }

    /// Returns where the settings are saved, when a home directory is known.
    pub fn settings_path(&self) -> Option<&PathBuf> {
        self.settings_path.as_ref()
    }

    /// Changes the settings, and writes them to their file at once.
    ///
    /// The change is all or nothing: if the file cannot be written, the
    /// settings in memory stay as they were.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] if the file cannot be written.
    pub fn update_settings(&self, change: impl FnOnce(&mut Settings)) -> Result<(), SettingsError> {
        let mut settings = self.lock_settings();
        let mut changed = settings.clone();
        change(&mut changed);
        if let Some(path) = &self.settings_path {
            changed.save(path)?;
        }
        *settings = changed;
        drop(settings);
        self.settings_unsaved.store(false, Ordering::Relaxed);
        self.apply_fallback();
        Ok(())
    }

    /// Records the volume the user set. It is written to the settings file
    /// a moment later, by [`save_settings_if_changed`](Self::save_settings_if_changed).
    pub fn note_volume(&self, volume: f64) {
        let mut settings = self.lock_settings();
        if settings.volume != volume {
            settings.set_volume(volume);
            self.settings_unsaved.store(true, Ordering::Relaxed);
        }
    }

    /// Writes the settings to their file, if they changed since last time.
    pub fn save_settings_if_changed(&self) {
        if !self.settings_unsaved.swap(false, Ordering::Relaxed) {
            return;
        }
        let settings = self.settings();
        if let Some(path) = &self.settings_path
            && let Err(error) = settings.save(path)
        {
            eprintln!("unisson: {error}");
        }
    }

    /// Returns the folder of the local library: the one the user chose, or
    /// the music folder of the desktop.
    pub fn music_folder(&self) -> Option<PathBuf> {
        self.settings()
            .music_folder
            .or_else(unisson_local::music_directory)
    }

    fn lock_settings(&self) -> MutexGuard<'_, Settings> {
        // The settings are always valid, even if a thread panicked while
        // holding them.
        self.settings
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Records that a media started playing.
    ///
    /// A radio station becomes the one to come back to.
    pub fn note_started(&self, media: &Media) {
        if is_station(media) {
            *self.lock_last_radio() = Some(media.clone());
            self.apply_fallback();
        }
    }

    /// Tells the session what plays when the queue runs out.
    fn apply_fallback(&self) {
        let fallback = self
            .last_radio()
            .filter(|_| self.lock_settings().resume_radio);
        self.session.set_fallback(fallback);
    }

    fn lock_last_radio(&self) -> MutexGuard<'_, Option<Media>> {
        self.last_radio
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Tells whether a media is a favorite: a favorite station for a radio
    /// station, a favorite media for anything else.
    pub fn is_favorite(&self, media: &Media) -> bool {
        if is_station(media) {
            self.stations().contains(&media.id.key)
        } else {
            // A database that cannot answer shows an empty heart: harmless.
            self.store.is_favorite(&media.id).unwrap_or(false)
        }
    }

    /// Describes a playback state for the interface.
    ///
    /// While a radio station plays, "previous" and "next" also walk the
    /// favorite stations, so they are reported as available.
    pub fn snapshot_dto(&self, snapshot: &Snapshot) -> SnapshotDto {
        let mut dto = SnapshotDto::from(snapshot);
        if let Some(current) = &snapshot.current {
            dto.current_is_favorite = self.is_favorite(current);
        }
        let stations = self.stations();
        if let Some(current) = live_station(snapshot)
            && neighbor(&stations, &current.id.key, Direction::Next).is_some()
        {
            dto.has_previous = true;
            dto.has_next = true;
        }
        dto
    }

    /// Plays the next media of the queue.
    ///
    /// On a radio station with nothing after it, this moves to the next
    /// favorite station instead.
    pub fn next(&self) {
        let snapshot = self.session.snapshot();
        if snapshot.has_next || !self.switch_station(&snapshot, Direction::Next) {
            self.session.next();
        }
    }

    /// Restarts the current track, or goes back to the previous media.
    ///
    /// On a radio station with nothing before it, this moves to the
    /// previous favorite station instead.
    pub fn previous(&self) {
        let snapshot = self.session.snapshot();
        if snapshot.has_previous || !self.switch_station(&snapshot, Direction::Previous) {
            self.session.previous();
        }
    }

    /// Plays a favorite station, by its place in the list. The other
    /// favorites surround it in the queue.
    pub fn play_favorite_station(&self, index: usize) {
        let media: Vec<Media> = self
            .stations()
            .stations()
            .iter()
            .map(Station::to_media)
            .collect();
        if index < media.len() {
            self.session.play_list(media, index);
        }
    }

    /// Moves from the station that plays to its neighbor in the favorites.
    ///
    /// Returns `false`, and does nothing, if no station plays or if there
    /// is no other favorite to go to.
    pub fn switch_station(&self, snapshot: &Snapshot, direction: Direction) -> bool {
        let Some(current) = live_station(snapshot) else {
            return false;
        };
        let (media, start) = {
            let stations = self.stations();
            let Some(start) = neighbor(&stations, &current.id.key, direction) else {
                return false;
            };
            let media = stations.stations().iter().map(Station::to_media).collect();
            (media, start)
        };
        self.session.play_list(media, start);
        true
    }
}

/// How long the answer to "is yt-dlp installed?" is trusted.
const YT_DLP_CHECK_LIFETIME: Duration = Duration::from_secs(60);

/// How far back in the history the last radio station is looked for.
const HISTORY_DEPTH: usize = 200;

/// Tells whether a media is a radio station.
pub fn is_station(media: &Media) -> bool {
    media.is_live() && media.id.source == SourceKind::Radio
}

/// Returns the radio station a snapshot is about, if any.
fn live_station(snapshot: &Snapshot) -> Option<&Media> {
    snapshot.current.as_ref().filter(|media| is_station(media))
}

/// Returns the index of the favorite to go to from the station `uuid`.
///
/// The favorites form a loop. From a station that is not a favorite, "next"
/// goes to the first favorite and "previous" to the last. Returns `None`
/// when there is no other favorite.
fn neighbor(stations: &Favorites, uuid: &str, direction: Direction) -> Option<usize> {
    let count = stations.len();
    let position = stations
        .stations()
        .iter()
        .position(|station| station.uuid == uuid);
    match (position, direction) {
        (_, _) if count == 0 => None,
        (Some(_), _) if count == 1 => None,
        (Some(index), Direction::Next) => Some((index + 1) % count),
        (Some(index), Direction::Previous) => Some((index + count - 1) % count),
        (None, Direction::Next) => Some(0),
        (None, Direction::Previous) => Some(count - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn favorites(names: &[&str]) -> Favorites {
        Favorites::from_stations(names.iter().map(|name| {
            let mut station =
                Station::custom(name, &format!("https://example.org/{name}")).unwrap();
            station.uuid = (*name).to_owned();
            station
        }))
    }

    #[test]
    fn the_favorites_form_a_loop() {
        let stations = favorites(&["a", "b", "c"]);
        assert_eq!(neighbor(&stations, "a", Direction::Next), Some(1));
        assert_eq!(neighbor(&stations, "c", Direction::Next), Some(0));
        assert_eq!(neighbor(&stations, "a", Direction::Previous), Some(2));
        assert_eq!(neighbor(&stations, "b", Direction::Previous), Some(0));
    }

    #[test]
    fn a_station_outside_the_favorites_enters_the_loop_by_its_ends() {
        let stations = favorites(&["a", "b", "c"]);
        assert_eq!(neighbor(&stations, "other", Direction::Next), Some(0));
        assert_eq!(neighbor(&stations, "other", Direction::Previous), Some(2));
        let single = favorites(&["a"]);
        assert_eq!(neighbor(&single, "other", Direction::Next), Some(0));
    }

    #[test]
    fn there_is_nowhere_to_go_without_another_favorite() {
        assert_eq!(neighbor(&favorites(&[]), "a", Direction::Next), None);
        assert_eq!(neighbor(&favorites(&["a"]), "a", Direction::Next), None);
        assert_eq!(neighbor(&favorites(&["a"]), "a", Direction::Previous), None);
    }
}
