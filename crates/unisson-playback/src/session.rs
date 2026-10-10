//! The [`Session`]: the queue, the sources and the audio engine, together.
//!
//! # Design
//!
//! One thread owns everything: the queue, the engine and the state. It reads
//! a single stream of messages, which carries the orders of the interface,
//! the events of the engine and the answers of the sources. Nothing is
//! shared but a copy of the state, so there is no lock to get wrong.
//!
//! Two things take time, and neither may freeze the session:
//!
//! - asking a source for an address can hit the network, so it runs on a
//!   short-lived thread, and its answer comes back as a message;
//! - the engine is asynchronous by nature.
//!
//! The user can choose another media while one of those is pending. Late
//! answers must then be ignored. Each choice therefore gets a ticket number,
//! and an answer with an old ticket is dropped. For the engine, the session
//! waits for [`EngineEvent::Loaded`] with the expected address before it
//! believes any event.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use unisson_audio::{Engine, EngineEvent, PlaybackState};
use unisson_core::{Media, MediaId, PlayQueue, SourceError, StreamUri};

use crate::snapshot::{ErrorKind, PlaybackError, Snapshot, Status};

/// Turns a media into the address to play.
///
/// It is usually [`SourceRegistry::resolve`](unisson_core::SourceRegistry::resolve).
/// It may block, since it can hit the network.
pub type Resolver = Arc<dyn Fn(&MediaId) -> Result<StreamUri, SourceError> + Send + Sync>;

/// "Previous" restarts the current track when it played longer than this.
const RESTART_THRESHOLD: Duration = Duration::from_secs(3);

/// Something the session reports.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEvent {
    /// The state changed. The payload is the whole new state.
    Changed(Snapshot),
    /// Playback moved on. Sent a few times per second while playing, apart
    /// from [`Changed`](Self::Changed) so that frequent updates stay small.
    Position {
        /// Time elapsed since the beginning of the media.
        position: Duration,
        /// Total duration, when it is known.
        duration: Option<Duration>,
    },
    /// The sound of a media started. Sent once per media played: this is
    /// the moment to add it to the listening history.
    Started(Media),
}

/// A message read by the session thread.
enum Message {
    PlayList {
        items: Vec<Media>,
        start: usize,
    },
    Pause,
    Resume,
    Toggle,
    Next,
    Previous,
    Seek(Duration),
    SetVolume(f64),
    /// The answer of a source, for the choice numbered `ticket`.
    Resolved {
        ticket: u64,
        result: Result<StreamUri, SourceError>,
    },
    Engine(EngineEvent),
    Shutdown,
}

/// Handle to the playback session.
///
/// Orders return at once and never fail: what happens next is told by the
/// [`SessionEvent`]s. Dropping the session stops playback.
///
/// # Examples
///
/// ```no_run
/// use std::sync::Arc;
///
/// use unisson_audio::Engine;
/// use unisson_core::{Media, MediaId, SourceKind, StreamUri};
/// use unisson_playback::{Session, SessionEvent};
///
/// let (engine, engine_events) = Engine::spawn()?;
/// // A real application asks its sources instead.
/// let resolver = Arc::new(|id: &MediaId| StreamUri::from_file_path(&id.key));
/// let (session, events) = Session::start(engine, engine_events, resolver);
///
/// let song = Media::track(MediaId::new(SourceKind::Local, "/music/song.flac"), "Song");
/// session.play_list(vec![song], 0);
/// for event in events {
///     if let SessionEvent::Changed(snapshot) = event {
///         println!("{:?}", snapshot.status);
///     }
/// }
/// # Ok::<(), unisson_audio::AudioError>(())
/// ```
#[derive(Debug)]
pub struct Session {
    messages: Sender<Message>,
    shared: Arc<Mutex<Snapshot>>,
    threads: Vec<JoinHandle<()>>,
}

impl Session {
    /// Starts a session on top of an engine.
    ///
    /// `engine_events` is the receiver that came with the engine. Returns
    /// the session and the receiver of its events.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot start a thread.
    pub fn start(
        engine: Engine,
        engine_events: Receiver<EngineEvent>,
        resolver: Resolver,
    ) -> (Self, Receiver<SessionEvent>) {
        let (messages, inbox) = mpsc::channel();
        let (events, outbox) = mpsc::channel();
        let shared = Arc::new(Mutex::new(Snapshot::default()));

        let worker = Worker {
            engine,
            resolver,
            messages: messages.clone(),
            events,
            shared: Arc::clone(&shared),
            queue: PlayQueue::new(),
            snapshot: Snapshot::default(),
            ticket: 0,
            expected: None,
            engine_is_current: false,
            started_reported: false,
        };
        let session_thread = thread::Builder::new()
            .name("unisson-session".into())
            .spawn(move || worker.run(&inbox))
            .expect("cannot start the session thread");

        // Brings the events of the engine into the single message stream.
        // It ends when the engine is dropped, with the session thread.
        let forward = messages.clone();
        let engine_thread = thread::Builder::new()
            .name("unisson-session-engine".into())
            .spawn(move || {
                for event in engine_events {
                    if forward.send(Message::Engine(event)).is_err() {
                        break;
                    }
                }
            })
            .expect("cannot start the engine listener thread");

        (
            Self {
                messages,
                shared,
                threads: vec![session_thread, engine_thread],
            },
            outbox,
        )
    }

    /// Returns the current state.
    pub fn snapshot(&self) -> Snapshot {
        lock(&self.shared).clone()
    }

    /// Replaces the queue with `items`, and plays `items[start]`.
    ///
    /// The items after it play next, and "previous" walks up the items
    /// before it. Nothing happens if `start` is out of range.
    pub fn play_list(&self, items: Vec<Media>, start: usize) {
        self.send(Message::PlayList { items, start });
    }

    /// Suspends playback.
    pub fn pause(&self) {
        self.send(Message::Pause);
    }

    /// Resumes playback after a pause, or starts the selected media again
    /// if it was stopped.
    pub fn resume(&self) {
        self.send(Message::Resume);
    }

    /// Pauses if something plays, resumes otherwise.
    pub fn toggle(&self) {
        self.send(Message::Toggle);
    }

    /// Plays the next media of the queue, if there is one.
    pub fn next(&self) {
        self.send(Message::Next);
    }

    /// Goes back to the beginning of the current track, or to the previous
    /// media when the track has just started.
    pub fn previous(&self) {
        self.send(Message::Previous);
    }

    /// Moves playback to `position`. Ignored for live streams.
    pub fn seek(&self, position: Duration) {
        self.send(Message::Seek(position));
    }

    /// Sets the volume, from `0.0` (silent) to `1.0` (full).
    pub fn set_volume(&self, volume: f64) {
        self.send(Message::SetVolume(volume));
    }

    fn send(&self, message: Message) {
        // The session thread only stops when the session is dropped.
        let _ = self.messages.send(message);
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.send(Message::Shutdown);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn lock(shared: &Mutex<Snapshot>) -> MutexGuard<'_, Snapshot> {
    // A snapshot is always valid, even if a thread panicked while holding it.
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The state kept by the session thread.
struct Worker {
    engine: Engine,
    resolver: Resolver,
    /// To hand to the threads that ask the sources.
    messages: Sender<Message>,
    events: Sender<SessionEvent>,
    /// The copy of the state read by [`Session::snapshot`].
    shared: Arc<Mutex<Snapshot>>,
    queue: PlayQueue,
    snapshot: Snapshot,
    /// Number of the last media chosen.
    ticket: u64,
    /// The address handed to the engine, until the engine confirms it.
    expected: Option<StreamUri>,
    /// Whether the events of the engine are about the current media.
    engine_is_current: bool,
    /// Whether [`SessionEvent::Started`] was sent for the current media.
    started_reported: bool,
}

impl Worker {
    /// Runs until a shutdown is requested.
    fn run(mut self, inbox: &Receiver<Message>) {
        for message in inbox {
            match message {
                Message::Shutdown => break,
                Message::PlayList { items, start } => {
                    if let Some(media) = self.queue.play_list(items, start).cloned() {
                        self.start(media);
                    }
                }
                Message::Pause => self.pause(),
                Message::Resume => self.resume(),
                Message::Toggle => match self.snapshot.status {
                    Status::Playing | Status::Reconnecting { .. } => self.pause(),
                    Status::Paused | Status::Stopped => self.resume(),
                    // The sound has not started yet: there is nothing to pause.
                    Status::Loading => {}
                },
                Message::Next => self.next(),
                Message::Previous => self.previous(),
                Message::Seek(position) => self.seek(position),
                Message::SetVolume(volume) => {
                    let _ = self.engine.set_volume(volume);
                }
                Message::Resolved { ticket, result } => self.resolved(ticket, result),
                Message::Engine(event) => self.engine_event(event),
            }
        }
    }

    /// Starts playing a media: asks its source for the address first.
    fn start(&mut self, media: Media) {
        self.ticket += 1;
        self.expected = None;
        self.engine_is_current = false;
        self.started_reported = false;
        // Silence the previous media now, not when the address arrives.
        let _ = self.engine.stop();

        self.snapshot.status = Status::Loading;
        self.snapshot.stream_title = None;
        self.snapshot.position = Duration::ZERO;
        self.snapshot.duration = media.duration;
        self.snapshot.error = None;
        let id = media.id.clone();
        self.snapshot.current = Some(media);
        self.publish();

        let ticket = self.ticket;
        let resolver = Arc::clone(&self.resolver);
        let messages = self.messages.clone();
        let spawned = thread::Builder::new()
            .name("unisson-resolve".into())
            .spawn(move || {
                let result = resolver(&id);
                let _ = messages.send(Message::Resolved { ticket, result });
            });
        if let Err(error) = spawned {
            self.fail(PlaybackError {
                kind: ErrorKind::Unavailable,
                detail: format!("cannot start a thread: {error}"),
            });
        }
    }

    /// Receives the address of a media from its source.
    fn resolved(&mut self, ticket: u64, result: Result<StreamUri, SourceError>) {
        // The user chose something else in the meantime.
        if ticket != self.ticket {
            return;
        }
        let live = self.current_is_live();
        match result {
            Ok(uri) => {
                self.expected = Some(uri.clone());
                let _ = if live {
                    self.engine.play_live(uri)
                } else {
                    self.engine.play_uri(uri)
                };
            }
            Err(error) => self.fail(error.into()),
        }
    }

    fn engine_event(&mut self, event: EngineEvent) {
        match event {
            // The volume does not depend on the media.
            EngineEvent::VolumeChanged(volume) => {
                self.snapshot.volume = volume;
                self.publish();
                return;
            }
            EngineEvent::Loaded(uri) => {
                self.engine_is_current = self.expected.as_ref() == Some(&uri);
                if self.engine_is_current {
                    self.expected = None;
                }
                return;
            }
            _ => {}
        }
        // Anything else is about a media the user already left.
        if !self.engine_is_current {
            return;
        }
        match event {
            EngineEvent::StateChanged(state) => {
                self.snapshot.status = match state {
                    PlaybackState::Playing => Status::Playing,
                    PlaybackState::Paused => Status::Paused,
                    PlaybackState::Stopped => Status::Stopped,
                };
                self.publish();
                if state == PlaybackState::Playing && !self.started_reported {
                    self.started_reported = true;
                    if let Some(media) = self.snapshot.current.clone() {
                        self.emit(SessionEvent::Started(media));
                    }
                }
            }
            EngineEvent::Position { position, duration } => {
                self.snapshot.position = position;
                // Keep the duration announced by the source until the
                // engine knows better.
                if duration.is_some() {
                    self.snapshot.duration = duration;
                }
                let duration = self.snapshot.duration;
                lock(&self.shared).clone_from(&self.snapshot);
                self.emit(SessionEvent::Position { position, duration });
            }
            EngineEvent::TitleChanged(title) => {
                // A file repeats tags the library already knows. Only a
                // live stream announces something new.
                if self.current_is_live() {
                    self.snapshot.stream_title = Some(title);
                    self.publish();
                }
            }
            EngineEvent::EndOfStream => self.ended(),
            EngineEvent::Reconnecting {
                attempt,
                max_attempts,
            } => {
                self.snapshot.status = Status::Reconnecting {
                    attempt,
                    max_attempts,
                };
                self.publish();
            }
            EngineEvent::Reconnected => {
                self.snapshot.status = Status::Playing;
                self.publish();
            }
            EngineEvent::Error(detail) => self.fail(PlaybackError {
                kind: ErrorKind::Playback,
                detail,
            }),
            EngineEvent::VolumeChanged(_) | EngineEvent::Loaded(_) => {}
        }
    }

    /// The current media played to its end.
    fn ended(&mut self) {
        if self.queue.upcoming().len() > 0 {
            self.next();
            return;
        }
        // Keep the last media selected, so "play" starts it again.
        self.engine_is_current = false;
        let _ = self.engine.stop();
        self.snapshot.status = Status::Stopped;
        self.snapshot.position = Duration::ZERO;
        self.publish();
    }

    fn pause(&mut self) {
        match self.snapshot.status {
            Status::Playing => {
                let _ = self.engine.pause();
            }
            Status::Reconnecting { .. } => {
                // The engine gives up the reconnection without a state
                // change to report: show the pause right away.
                let _ = self.engine.pause();
                self.snapshot.status = Status::Paused;
                self.publish();
            }
            _ => {}
        }
    }

    fn resume(&mut self) {
        match self.snapshot.status {
            Status::Paused => {
                let _ = self.engine.play();
            }
            // Ask the source again: some addresses expire.
            Status::Stopped => {
                if let Some(media) = self.snapshot.current.clone() {
                    self.start(media);
                }
            }
            _ => {}
        }
    }

    fn next(&mut self) {
        // Stay on the last media instead of emptying the player.
        if self.queue.upcoming().len() == 0 {
            return;
        }
        if let Some(media) = self.queue.advance().cloned() {
            self.start(media);
        }
    }

    fn previous(&mut self) {
        let can_restart = !self.current_is_live() && self.can_seek();
        if can_restart && self.snapshot.position > RESTART_THRESHOLD {
            self.seek(Duration::ZERO);
        } else if let Some(media) = self.queue.go_back().cloned() {
            self.start(media);
        } else if can_restart {
            self.seek(Duration::ZERO);
        }
    }

    fn seek(&mut self, position: Duration) {
        if self.can_seek() && !self.current_is_live() {
            let _ = self.engine.seek(position);
        }
    }

    /// Returns `true` if the engine holds the current media, started.
    fn can_seek(&self) -> bool {
        self.engine_is_current && matches!(self.snapshot.status, Status::Playing | Status::Paused)
    }

    fn current_is_live(&self) -> bool {
        self.snapshot.current.as_ref().is_some_and(Media::is_live)
    }

    /// Stops on a failure, and reports it.
    fn fail(&mut self, error: PlaybackError) {
        self.engine_is_current = false;
        let _ = self.engine.stop();
        self.snapshot.status = Status::Stopped;
        self.snapshot.error = Some(error);
        self.publish();
    }

    /// Makes the new state visible, and reports it.
    fn publish(&mut self) {
        self.snapshot.has_previous = !self.queue.history().is_empty();
        self.snapshot.has_next = self.queue.upcoming().len() > 0;
        lock(&self.shared).clone_from(&self.snapshot);
        self.emit(SessionEvent::Changed(self.snapshot.clone()));
    }

    fn emit(&self, event: SessionEvent) {
        // Nobody listening is not an error: playback simply goes on.
        let _ = self.events.send(event);
    }
}
