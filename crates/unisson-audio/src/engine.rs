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
    /// A command or the playback itself failed.
    Error(String),
}

/// A request sent to the audio thread.
enum Command {
    Load(StreamUri),
    Play,
    Pause,
    Stop,
    SetVolume(f64),
    Shutdown,
}

/// Handle to the audio thread.
///
/// Dropping the engine stops playback and waits for the thread to finish.
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
        let (commands, inbox) = mpsc::channel();
        let (events, outbox) = mpsc::channel();
        // Lets the audio thread report whether the player could be created.
        let (ready, started) = mpsc::channel();

        let thread = thread::Builder::new()
            .name("unisson-audio".into())
            .spawn(move || match make_player() {
                Ok(player) => {
                    let _ = ready.send(Ok(()));
                    Worker::new(player, events).run(&inbox);
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
        self.send(Command::Load(uri))
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

/// The state kept by the audio thread.
struct Worker {
    player: Player,
    events: Sender<EngineEvent>,
    /// Last state reported, to report changes only.
    last_state: PlaybackState,
    /// When the last position was reported.
    last_position_at: Option<Instant>,
}

impl Worker {
    fn new(player: Player, events: Sender<EngineEvent>) -> Self {
        Self {
            player,
            events,
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
            Command::Load(uri) => self.player.load(&uri),
            Command::Play => self.player.play(),
            Command::Pause => self.player.pause(),
            Command::Stop => self.player.stop(),
            Command::SetVolume(volume) => {
                self.player.set_volume(volume);
                self.emit(EngineEvent::VolumeChanged(self.player.volume()));
                Ok(())
            }
            // Handled by `run`, which stops the loop.
            Command::Shutdown => Ok(()),
        };
        if let Err(error) = result {
            self.emit(EngineEvent::Error(error.to_string()));
        }
    }

    /// Reports what happened since the last call: end, error, state, position.
    fn check_playback(&mut self) {
        match self.player.poll_event(Duration::ZERO) {
            Some(PlayerEvent::EndOfStream) => self.emit(EngineEvent::EndOfStream),
            Some(PlayerEvent::Error(message)) => self.emit(EngineEvent::Error(message)),
            None => {}
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
