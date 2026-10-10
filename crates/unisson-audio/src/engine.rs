//! The [`Engine`]: a [`Player`] running on its own thread.
//!
//! # Design
//!
//! The user interface must never wait for audio work. The engine therefore
//! owns the [`Player`] on a dedicated thread and talks to the rest of the
//! application through two channels:
//!
//! - commands go in ([`Engine::play`], [`Engine::pause`], and so on) and
//!   return immediately;
//! - [`EngineEvent`]s come out through the receiver returned by
//!   [`Engine::spawn`].
//!
//! Commands never fail directly. If one cannot be carried out, the reason
//! arrives later as an [`EngineEvent::Error`].
//!
//! # Live streams
//!
//! A radio stream has no end: when it stops, the connection was lost. For an
//! address started with [`Engine::play_live`], the engine therefore connects
//! again by itself, a few times and with growing delays. It reports
//! [`EngineEvent::Reconnecting`] then [`EngineEvent::Reconnected`], and only
//! gives up with an [`EngineEvent::Error`] after the last attempt.
//!
//! A stream that never played is not retried: a wrong address fails at once.

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use unisson_core::StreamUri;

use crate::error::AudioError;
use crate::player::{PlaybackState, Player, PlayerEvent};

/// How long the audio thread waits for a command before checking playback.
const TICK: Duration = Duration::from_millis(50);

/// Minimum delay between two [`EngineEvent::Position`] events.
const POSITION_INTERVAL: Duration = Duration::from_millis(250);

/// How much of a live stream must play before it counts as connected.
const CONNECTED_AFTER: Duration = Duration::from_millis(300);

/// Settings of the [`Engine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineConfig {
    /// How long to wait before each attempt to reconnect a live stream.
    ///
    /// The number of delays is the number of attempts. An empty list
    /// disables reconnection.
    pub reconnect_delays: Vec<Duration>,
}

impl Default for EngineConfig {
    /// Five attempts spread over thirty seconds.
    fn default() -> Self {
        let seconds = [1, 2, 4, 8, 15];
        Self {
            reconnect_delays: seconds.map(Duration::from_secs).to_vec(),
        }
    }
}

/// Something the engine reports to the application.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineEvent {
    /// The playback state changed.
    StateChanged(PlaybackState),
    /// Where playback is. Sent a few times per second while playing.
    Position {
        /// Time elapsed since the beginning of the media.
        position: Duration,
        /// Total duration, when it is known. Live streams have none.
        duration: Option<Duration>,
    },
    /// The volume changed. The value is between `0.0` and `1.0`.
    VolumeChanged(f64),
    /// The current media reached its end.
    EndOfStream,
    /// A live stream was lost, and the engine is about to connect again.
    Reconnecting {
        /// Number of this attempt, starting at 1.
        attempt: usize,
        /// Number of attempts made before giving up.
        max_attempts: usize,
    },
    /// A lost live stream is playing again.
    Reconnected,
    /// A command or the playback itself failed.
    Error(String),
}

/// A request sent to the audio thread.
enum Command {
    Load { uri: StreamUri, live: bool },
    Play,
    Pause,
    Stop,
    SetVolume(f64),
    Shutdown,
}

/// Handle to the audio thread.
///
/// Commands return at once and never fail directly: if one cannot be carried
/// out, the reason arrives later as an [`EngineEvent::Error`]. Dropping the
/// engine stops playback and waits for the thread to finish.
///
/// # Live streams
///
/// A radio stream has no end: when it stops, the connection was lost. For an
/// address started with [`play_live`](Self::play_live), the engine connects
/// again by itself, a few times and with growing delays (see
/// [`EngineConfig`]). It reports [`EngineEvent::Reconnecting`] then
/// [`EngineEvent::Reconnected`], and only gives up with an
/// [`EngineEvent::Error`] after the last attempt.
///
/// A stream that never played is not retried: a wrong address fails at once.
///
/// # Examples
///
/// ```no_run
/// use unisson_audio::{Engine, EngineEvent, file_uri};
///
/// let (engine, events) = Engine::spawn()?;
/// engine.play_uri(file_uri("/home/me/Music/song.flac")?)?;
/// for event in events {
///     match event {
///         EngineEvent::EndOfStream => break,
///         EngineEvent::Error(message) => return Err(message.into()),
///         other => println!("{other:?}"),
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Engine {
    commands: Sender<Command>,
    thread: Option<JoinHandle<()>>,
}

impl Engine {
    /// Starts the audio thread with a player on the default output device.
    ///
    /// Returns the engine and the receiver of its events.
    ///
    /// # Errors
    ///
    /// Returns the error of [`Player::new`] if the player cannot be created,
    /// or [`AudioError::Init`] if the thread cannot be started.
    pub fn spawn() -> Result<(Self, Receiver<EngineEvent>), AudioError> {
        Self::spawn_with(Player::new)
    }

    /// Starts the audio thread with custom settings.
    ///
    /// See [`spawn_with`](Self::spawn_with) for `make_player`.
    ///
    /// # Errors
    ///
    /// Same as [`spawn_with`](Self::spawn_with).
    pub fn spawn_configured<F>(
        config: EngineConfig,
        make_player: F,
    ) -> Result<(Self, Receiver<EngineEvent>), AudioError>
    where
        F: FnOnce() -> Result<Player, AudioError> + Send + 'static,
    {
        Self::start(config, make_player)
    }

    /// Starts the audio thread with a player built by `make_player`.
    ///
    /// The closure runs on the audio thread. This is mostly useful in tests,
    /// to build a player with [`Player::with_audio_sink`].
    ///
    /// # Errors
    ///
    /// Returns the error of `make_player`, or [`AudioError::Init`] if the
    /// thread cannot be started.
    pub fn spawn_with<F>(make_player: F) -> Result<(Self, Receiver<EngineEvent>), AudioError>
    where
        F: FnOnce() -> Result<Player, AudioError> + Send + 'static,
    {
        Self::start(EngineConfig::default(), make_player)
    }

    fn start<F>(
        config: EngineConfig,
        make_player: F,
    ) -> Result<(Self, Receiver<EngineEvent>), AudioError>
    where
        F: FnOnce() -> Result<Player, AudioError> + Send + 'static,
    {
        let (commands, inbox) = mpsc::channel();
        let (events, outbox) = mpsc::channel();
        // Lets the audio thread report whether the player could be created.
        let (ready, started) = mpsc::channel();

        let thread = thread::Builder::new()
            .name("unisson-audio".into())
            .spawn(move || match make_player() {
                Ok(player) => {
                    let _ = ready.send(Ok(()));
                    Worker::new(player, events, config).run(&inbox);
                }
                Err(error) => {
                    let _ = ready.send(Err(error));
                }
            })
            .map_err(|error| AudioError::Init(error.to_string()))?;

        match started.recv() {
            Ok(Ok(())) => Ok((
                Self {
                    commands,
                    thread: Some(thread),
                },
                outbox,
            )),
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(error)
            }
            // The thread ended without reporting: `make_player` panicked.
            Err(_) => Err(AudioError::Init(
                "the audio thread stopped while starting".into(),
            )),
        }
    }

    /// Stops any current playback and selects the address to play next.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::EngineStopped`] if the audio thread is gone.
    pub fn load(&self, uri: StreamUri) -> Result<(), AudioError> {
        self.send(Command::Load { uri, live: false })
    }

    /// Starts playback, or resumes it after a pause.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::EngineStopped`] if the audio thread is gone.
    pub fn play(&self) -> Result<(), AudioError> {
        self.send(Command::Play)
    }

    /// Loads an address and plays it right away.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::EngineStopped`] if the audio thread is gone.
    pub fn play_uri(&self, uri: StreamUri) -> Result<(), AudioError> {
        self.load(uri)?;
        self.play()
    }

    /// Loads a live stream, such as a radio station, and plays it right away.
    ///
    /// Unlike [`play_uri`](Self::play_uri), the engine connects again by
    /// itself when the stream is lost. See [`Engine`] for the details.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::EngineStopped`] if the audio thread is gone.
    pub fn play_live(&self, uri: StreamUri) -> Result<(), AudioError> {
        self.send(Command::Load { uri, live: true })?;
        self.play()
    }

    /// Suspends playback, keeping the current position.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::EngineStopped`] if the audio thread is gone.
    pub fn pause(&self) -> Result<(), AudioError> {
        self.send(Command::Pause)
    }

    /// Stops playback and releases the audio device.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::EngineStopped`] if the audio thread is gone.
    pub fn stop(&self) -> Result<(), AudioError> {
        self.send(Command::Stop)
    }

    /// Sets the volume, from `0.0` (silent) to `1.0` (full).
    ///
    /// The engine answers with an [`EngineEvent::VolumeChanged`] carrying the
    /// value actually applied.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::EngineStopped`] if the audio thread is gone.
    pub fn set_volume(&self, volume: f64) -> Result<(), AudioError> {
        self.send(Command::SetVolume(volume))
    }

    fn send(&self, command: Command) -> Result<(), AudioError> {
        self.commands
            .send(command)
            .map_err(|_| AudioError::EngineStopped)
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// A live stream the engine keeps alive.
struct LiveStream {
    uri: StreamUri,
    /// Whether the stream played at least once since it was loaded.
    connected: bool,
    /// Reconnection attempts made since the stream last played.
    attempts: usize,
    /// When the next attempt is due, if one is waiting.
    retry_at: Option<Instant>,
}

/// The state kept by the audio thread.
struct Worker {
    player: Player,
    events: Sender<EngineEvent>,
    config: EngineConfig,
    /// The loaded live stream, or `None` for an ordinary media.
    live: Option<LiveStream>,
    /// Whether the user asked for playback (as opposed to pause or stop).
    wants_playing: bool,
    /// Last state reported, to report changes only.
    last_state: PlaybackState,
    /// When the last position was reported.
    last_position_at: Option<Instant>,
}

impl Worker {
    fn new(player: Player, events: Sender<EngineEvent>, config: EngineConfig) -> Self {
        Self {
            player,
            events,
            config,
            live: None,
            wants_playing: false,
            last_state: PlaybackState::Stopped,
            last_position_at: None,
        }
    }

    /// Runs until a shutdown is requested or every [`Engine`] handle is gone.
    fn run(mut self, inbox: &Receiver<Command>) {
        loop {
            match inbox.recv_timeout(TICK) {
                Ok(Command::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(command) => self.handle(command),
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.check_playback();
        }
    }

    fn handle(&mut self, command: Command) {
        let result = match command {
            Command::Load { uri, live } => {
                self.wants_playing = false;
                let result = self.player.load(&uri);
                self.live = live.then_some(LiveStream {
                    uri,
                    connected: false,
                    attempts: 0,
                    retry_at: None,
                });
                result
            }
            Command::Play => {
                self.wants_playing = true;
                match &mut self.live {
                    // A reconnection is waiting: do not wait any longer.
                    Some(LiveStream {
                        retry_at: retry_at @ Some(_),
                        ..
                    }) => {
                        *retry_at = Some(Instant::now());
                        Ok(())
                    }
                    _ => self.player.play(),
                }
            }
            Command::Pause => {
                self.cancel_reconnection();
                self.player.pause()
            }
            Command::Stop => {
                self.cancel_reconnection();
                self.player.stop()
            }
            Command::SetVolume(volume) => {
                self.player.set_volume(volume);
                self.emit(EngineEvent::VolumeChanged(self.player.volume()));
                Ok(())
            }
            // Handled by `run`, which stops the loop.
            Command::Shutdown => Ok(()),
        };
        if let Err(error) = result {
            self.playback_failed(error.to_string());
        }
    }

    /// Reports what happened since the last call: end, error, state, position.
    fn check_playback(&mut self) {
        match self.player.poll_event(Duration::ZERO) {
            // A live stream has no end: reaching it means the connection dropped.
            Some(PlayerEvent::EndOfStream) if self.live.is_some() => {
                self.playback_failed("the live stream ended".into());
            }
            Some(PlayerEvent::EndOfStream) => self.emit(EngineEvent::EndOfStream),
            Some(PlayerEvent::Error(message)) => self.playback_failed(message),
            None => {}
        }

        self.reconnect_if_due();

        // While reconnecting, the player goes through stopped and playing
        // states that would only confuse the listener: keep them private.
        if self.is_reconnecting() {
            self.confirm_reconnection();
            return;
        }

        let state = self.player.state();
        if state != self.last_state {
            self.last_state = state;
            self.emit(EngineEvent::StateChanged(state));
            // Report the exact position where playback was paused.
            if state == PlaybackState::Paused {
                self.emit_position();
            }
        }

        let due = self
            .last_position_at
            .is_none_or(|at| at.elapsed() >= POSITION_INTERVAL);
        if state == PlaybackState::Playing && due {
            self.emit_position();
        }

        // Remember that the live stream played, so a later loss is retried.
        if let Some(live) = &mut self.live
            && !live.connected
            && self
                .player
                .position()
                .is_some_and(|position| position >= CONNECTED_AFTER)
        {
            live.connected = true;
        }
    }

    /// Handles a failure: retries a live stream that was playing, or reports.
    fn playback_failed(&mut self, message: String) {
        let max_attempts = self.config.reconnect_delays.len();
        let retry = match &mut self.live {
            Some(live) if self.wants_playing && (live.connected || live.attempts > 0) => self
                .config
                .reconnect_delays
                .get(live.attempts)
                .map(|delay| {
                    live.attempts += 1;
                    live.retry_at = Some(Instant::now() + *delay);
                    live.attempts
                }),
            _ => None,
        };
        match retry {
            Some(attempt) => self.emit(EngineEvent::Reconnecting {
                attempt,
                max_attempts,
            }),
            None => {
                self.cancel_reconnection();
                self.emit(EngineEvent::Error(message));
            }
        }
    }

    /// Starts the waiting reconnection attempt once its delay has passed.
    fn reconnect_if_due(&mut self) {
        let Some(live) = &mut self.live else { return };
        if live.retry_at.is_none_or(|at| Instant::now() < at) {
            return;
        }
        live.retry_at = None;
        let uri = live.uri.clone();
        if let Err(error) = self.player.load(&uri).and_then(|()| self.player.play()) {
            self.playback_failed(error.to_string());
        }
    }

    /// Reports the end of a reconnection once the stream plays again.
    fn confirm_reconnection(&mut self) {
        let playing = self
            .player
            .position()
            .is_some_and(|position| position >= CONNECTED_AFTER);
        if let Some(live) = &mut self.live
            && live.retry_at.is_none()
            && playing
        {
            live.attempts = 0;
            live.connected = true;
            self.emit(EngineEvent::Reconnected);
        }
    }

    fn is_reconnecting(&self) -> bool {
        self.live.as_ref().is_some_and(|live| live.attempts > 0)
    }

    /// Forgets any reconnection in progress: the user took over.
    fn cancel_reconnection(&mut self) {
        self.wants_playing = false;
        if let Some(live) = &mut self.live {
            live.attempts = 0;
            live.retry_at = None;
        }
    }

    fn emit_position(&mut self) {
        if let Some(position) = self.player.position() {
            self.last_position_at = Some(Instant::now());
            self.emit(EngineEvent::Position {
                position,
                duration: self.player.duration(),
            });
        }
    }

    fn emit(&self, event: EngineEvent) {
        // Nobody listening is not an error: playback simply goes on.
        let _ = self.events.send(event);
    }
}
