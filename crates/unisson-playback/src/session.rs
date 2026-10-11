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
//!
//! # A media that cannot start
//!
//! A deleted file or a closed stream must not stop a whole queue. When a
//! media cannot start and others are waiting, the session says so with
//! [`SessionEvent::Skipped`] and moves on. Three cases still stop playback,
//! with the error in the state:
//!
//! - nothing is waiting after the media;
//! - the network is the cause: every other media would fail the same way;
//! - several media in a row were skipped: something larger is wrong, and
//!   walking a long queue to its end would help nobody.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use unisson_audio::{Engine, EngineEvent, PlaybackState};
use unisson_core::{Media, MediaId, PlayQueue, SourceError, StreamUri};

use crate::snapshot::{ErrorKind, PlaybackError, QueueView, Snapshot, Status};

/// Turns a media into the address to play.
///
/// It is usually [`SourceRegistry::resolve`](unisson_core::SourceRegistry::resolve).
/// It may block, since it can hit the network.
pub type Resolver = Arc<dyn Fn(&MediaId) -> Result<StreamUri, SourceError> + Send + Sync>;

/// How many media in a row may be skipped before playback stops.
const MAX_SKIPS_IN_A_ROW: usize = 3;

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
    /// The queue changed: its content, its order, or what plays when it
    /// runs out. Read it with [`Session::queue`].
    QueueChanged,
    /// The sound of a media started. Sent once per media played: this is
    /// the moment to add it to the listening history.
    Started(Media),
    /// A media could not start, and the session moved on to the next one.
    Skipped {
        /// The media that was left out.
        media: Media,
        /// Why it could not start.
        error: PlaybackError,
    },
}

/// A message read by the session thread.
enum Message {
    PlayList {
        items: Vec<Media>,
        start: usize,
    },
    PlayNow(Media),
    PlayNext(Media),
    Enqueue(Media),
    JumpTo(usize),
    Remove(usize),
    Move {
        from: usize,
        to: usize,
    },
    ClearQueue,
    SetFallback(Option<Media>),
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
    shared_queue: Arc<Mutex<QueueView>>,
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
        let shared_queue = Arc::new(Mutex::new(QueueView::default()));

        let worker = Worker {
            engine,
            resolver,
            messages: messages.clone(),
            events,
            shared: Arc::clone(&shared),
            shared_queue: Arc::clone(&shared_queue),
            fallback: None,
            queue: PlayQueue::new(),
            snapshot: Snapshot::default(),
            ticket: 0,
            expected: None,
            engine_is_current: false,
            at_end: false,
            heard: false,
            skipped_in_a_row: 0,
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
                shared_queue,
                threads: vec![session_thread, engine_thread],
            },
            outbox,
        )
    }

    /// Returns the current state.
    pub fn snapshot(&self) -> Snapshot {
        lock(&self.shared).clone()
    }

    /// Returns the queue: the selected media and what plays after it.
    pub fn queue(&self) -> QueueView {
        lock(&self.shared_queue).clone()
    }

    /// Replaces the queue with `items`, and plays `items[start]`.
    ///
    /// The items after it play next, and "previous" walks up the items
    /// before it. Nothing happens if `start` is out of range.
    pub fn play_list(&self, items: Vec<Media>, start: usize) {
        self.send(Message::PlayList { items, start });
    }

    /// Plays a media right away, and keeps the rest of the queue.
    ///
    /// The media that was playing goes to the history, and the upcoming
    /// media play after the new one.
    pub fn play_now(&self, media: Media) {
        self.send(Message::PlayNow(media));
    }

    /// Adds a media right after the current one.
    ///
    /// If nothing is selected, or if playback reached the end of the queue,
    /// the media starts right away.
    pub fn play_next(&self, media: Media) {
        self.send(Message::PlayNext(media));
    }

    /// Adds a media at the end of the queue.
    ///
    /// If nothing is selected, or if playback reached the end of the queue,
    /// the media starts right away.
    pub fn enqueue(&self, media: Media) {
        self.send(Message::Enqueue(media));
    }

    /// Jumps to an upcoming media, and plays it.
    ///
    /// `index` counts from the next media, starting at 0. The media before
    /// it are removed from the queue. Nothing happens if `index` is out of
    /// range.
    pub fn jump_to(&self, index: usize) {
        self.send(Message::JumpTo(index));
    }

    /// Removes an upcoming media. `index` counts from the next media.
    pub fn remove(&self, index: usize) {
        self.send(Message::Remove(index));
    }

    /// Moves an upcoming media to another position. Both indexes count from
    /// the next media.
    pub fn move_item(&self, from: usize, to: usize) {
        self.send(Message::Move { from, to });
    }

    /// Removes every upcoming media. The current one keeps playing.
    pub fn clear_queue(&self) {
        self.send(Message::ClearQueue);
    }

    /// Chooses what plays when the last media of the queue ends.
    ///
    /// It is meant for a radio station: with one, playback never stops by
    /// itself. With `None`, playback stops at the end of the queue.
    pub fn set_fallback(&self, fallback: Option<Media>) {
        self.send(Message::SetFallback(fallback));
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

fn lock<T>(shared: &Mutex<T>) -> MutexGuard<'_, T> {
    // A copy of the state is always valid, even if a thread panicked while
    // holding it.
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
    /// The copy of the queue read by [`Session::queue`].
    shared_queue: Arc<Mutex<QueueView>>,
    /// What plays when the queue runs out.
    fallback: Option<Media>,
    queue: PlayQueue,
    snapshot: Snapshot,
    /// Number of the last media chosen.
    ticket: u64,
    /// The address handed to the engine, until the engine confirms it.
    expected: Option<StreamUri>,
    /// Whether the events of the engine are about the current media.
    engine_is_current: bool,
    /// Whether the last media of the queue played to its end.
    at_end: bool,
    /// Whether sound came out of the current media, and
    /// [`SessionEvent::Started`] was sent for it. The engine says "playing"
    /// as soon as it tries, so only a position that moved proves it.
    heard: bool,
    /// How many media were skipped since a media was last heard.
    skipped_in_a_row: usize,
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
                Message::PlayNow(media) => {
                    let media = self.queue.play_now(media).clone();
                    self.start(media);
                }
                Message::PlayNext(media) => {
                    self.queue.push_next(media);
                    self.queue_grew();
                }
                Message::Enqueue(media) => {
                    self.queue.push_back(media);
                    self.queue_grew();
                }
                Message::JumpTo(index) => {
                    if let Some(media) = self.queue.jump_to(index).cloned() {
                        self.start(media);
                    }
                }
                Message::Remove(index) => {
                    self.queue.remove(index);
                    self.publish();
                }
                Message::Move { from, to } => {
                    self.queue.move_item(from, to);
                    self.publish();
                }
                Message::ClearQueue => {
                    self.queue.clear();
                    self.publish();
                }
                Message::SetFallback(fallback) => {
                    self.fallback = fallback;
                    self.publish();
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
        self.heard = false;
        self.at_end = false;
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

    /// Reacts to a media added to the queue.
    ///
    /// When nothing was left to play, the new media starts by itself:
    /// adding to an idle player means "play this".
    fn queue_grew(&mut self) {
        let idle = self.queue.current().is_none() || self.at_end;
        if idle {
            self.next();
        } else {
            self.publish();
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
            Err(error) => self.media_failed(error.into()),
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
            }
            EngineEvent::Position { position, duration } => {
                if position > Duration::ZERO {
                    self.note_heard();
                }
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
            EngineEvent::EndOfStream => {
                // A media that reached its end did play, however short.
                self.note_heard();
                self.ended();
            }
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
            EngineEvent::Error(failure) => self.media_failed(failure.into()),
            EngineEvent::VolumeChanged(_) | EngineEvent::Loaded(_) => {}
        }
    }

    /// The current media played to its end.
    fn ended(&mut self) {
        if self.queue.upcoming().len() > 0 {
            self.next();
            return;
        }
        // The queue ran out: carry on with the fallback, if there is one.
        // Never restart what just ended: that would loop for ever.
        let ended = self.queue.current().map(|media| &media.id);
        if let Some(fallback) = self.fallback.clone()
            && ended != Some(&fallback.id)
        {
            let media = self.queue.play_now(fallback).clone();
            self.start(media);
            return;
        }
        // Keep the last media selected, so "play" starts it again.
        self.at_end = true;
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

    /// Notes that sound came out of the current media, and reports it once.
    fn note_heard(&mut self) {
        if self.heard {
            return;
        }
        self.heard = true;
        self.skipped_in_a_row = 0;
        if let Some(media) = self.snapshot.current.clone() {
            self.emit(SessionEvent::Started(media));
        }
    }

    /// Handles a failure of the current media: skips a media that cannot
    /// start when others are waiting, and stops otherwise. See the notes at
    /// the top of this file.
    fn media_failed(&mut self, error: PlaybackError) {
        let can_skip = !self.heard
            && error.kind != ErrorKind::Network
            && self.skipped_in_a_row < MAX_SKIPS_IN_A_ROW
            && self.queue.upcoming().len() > 0;
        let Some(media) = self.snapshot.current.clone().filter(|_| can_skip) else {
            self.fail(error);
            return;
        };
        self.skipped_in_a_row += 1;
        self.emit(SessionEvent::Skipped { media, error });
        self.next();
    }

    /// Stops on a failure, and reports it.
    fn fail(&mut self, error: PlaybackError) {
        self.skipped_in_a_row = 0;
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
        self.publish_queue();
    }

    /// Makes the queue visible and reports it, if it changed.
    ///
    /// Comparing is cheaper than tracking every place that touches the
    /// queue, and it cannot miss one.
    fn publish_queue(&mut self) {
        let mut shared = lock(&self.shared_queue);
        let unchanged = shared.current.as_ref() == self.queue.current()
            && shared.fallback == self.fallback
            && shared.upcoming.iter().eq(self.queue.upcoming());
        if unchanged {
            return;
        }
        *shared = QueueView {
            current: self.queue.current().cloned(),
            upcoming: self.queue.upcoming().cloned().collect(),
            fallback: self.fallback.clone(),
        };
        drop(shared);
        self.emit(SessionEvent::QueueChanged);
    }

    fn emit(&self, event: SessionEvent) {
        // Nobody listening is not an error: playback simply goes on.
        let _ = self.events.send(event);
    }
}
