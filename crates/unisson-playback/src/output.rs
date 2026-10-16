//! Where the session sends the sound: the audio engine, or a remote player.
//!
//! # Design
//!
//! Most media are played by the audio [`Engine`] of Unisson. Some are not:
//! a Spotify track can only be played by an official Spotify application,
//! which Unisson drives from the outside. The session should not care. It
//! talks to an [`Output`], and the [`Router`] picks, for each address, the
//! engine or the [`RemotePlayer`] that handles its scheme.
//!
//! Both report the same [`EngineEvent`]s, merged into one stream. Only the
//! one that plays the current address is heard: the events of the other
//! are dropped. The engine and the remote player run on threads of their
//! own, so without this filter a late "stopped" from the engine could
//! arrive after the remote player started, and stop the session.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use unisson_audio::{AudioError, Engine, EngineEvent};
use unisson_core::StreamUri;

/// What the session needs from whatever plays its media.
///
/// Every method returns at once. What happens next is reported as
/// [`EngineEvent`]s, as the [`Engine`] does.
pub trait Output: Send {
    /// Starts playing an address from its beginning.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] if the order cannot be sent.
    fn play_uri(&self, uri: StreamUri) -> Result<(), AudioError>;

    /// Starts playing a live stream, which reconnects when it drops.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] if the order cannot be sent.
    fn play_live(&self, uri: StreamUri) -> Result<(), AudioError>;

    /// Resumes playback.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] if the order cannot be sent.
    fn play(&self) -> Result<(), AudioError>;

    /// Suspends playback.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] if the order cannot be sent.
    fn pause(&self) -> Result<(), AudioError>;

    /// Stops playback.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] if the order cannot be sent.
    fn stop(&self) -> Result<(), AudioError>;

    /// Moves playback to a position.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] if the order cannot be sent.
    fn seek(&self, position: Duration) -> Result<(), AudioError>;

    /// Sets the volume, from `0.0` to `1.0`.
    ///
    /// # Errors
    ///
    /// Returns an [`AudioError`] if the order cannot be sent.
    fn set_volume(&self, volume: f64) -> Result<(), AudioError>;
}

impl Output for Engine {
    fn play_uri(&self, uri: StreamUri) -> Result<(), AudioError> {
        Engine::play_uri(self, uri)
    }

    fn play_live(&self, uri: StreamUri) -> Result<(), AudioError> {
        Engine::play_live(self, uri)
    }

    fn play(&self) -> Result<(), AudioError> {
        Engine::play(self)
    }

    fn pause(&self) -> Result<(), AudioError> {
        Engine::pause(self)
    }

    fn stop(&self) -> Result<(), AudioError> {
        Engine::stop(self)
    }

    fn seek(&self, position: Duration) -> Result<(), AudioError> {
        Engine::seek(self, position)
    }

    fn set_volume(&self, volume: f64) -> Result<(), AudioError> {
        Engine::set_volume(self, volume)
    }
}

/// A player outside Unisson, driven from the outside: the Spotify
/// application, for instance.
///
/// It reports through the [`EventSink`] it was built with: first
/// [`EngineEvent::Loaded`] with the address it was asked to play, then the
/// state, the position, the end or a failure, like the [`Engine`].
pub trait RemotePlayer: Send {
    /// Returns the start of the addresses it plays, such as `spotify:`.
    fn scheme(&self) -> &'static str;

    /// Starts playing an address.
    fn play_uri(&self, uri: StreamUri);

    /// Resumes playback.
    fn play(&self);

    /// Suspends playback.
    fn pause(&self);

    /// Stops playback. Another player is about to take over.
    fn stop(&self);

    /// Moves playback to a position.
    fn seek(&self, position: Duration);

    /// Sets the volume, from `0.0` to `1.0`.
    fn set_volume(&self, volume: f64);
}

/// Where a [`RemotePlayer`] sends its events.
///
/// The events only reach the session while the remote player is the one
/// that plays.
#[derive(Debug, Clone)]
pub struct EventSink {
    events: Sender<EngineEvent>,
    active: Arc<AtomicBool>,
}

impl EventSink {
    /// Reports an event, if the remote player is the one that plays.
    pub fn send(&self, event: EngineEvent) {
        if self.active.load(Ordering::SeqCst) {
            // Nobody listens any more when the session is gone.
            let _ = self.events.send(event);
        }
    }

    /// Tells whether the remote player is the one that plays.
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    /// Returns a sink whose events are collected in a channel, for tests of
    /// a remote player. It is always active.
    pub fn for_tests() -> (Self, Receiver<EngineEvent>) {
        let (events, received) = mpsc::channel();
        let sink = Self {
            events,
            active: Arc::new(AtomicBool::new(true)),
        };
        (sink, received)
    }
}

/// An [`Output`] that sends each address to the engine, or to the remote
/// player that handles its scheme. See the notes at the top of this file.
pub struct Router {
    engine: Engine,
    remote: Box<dyn RemotePlayer>,
    /// Whether the remote player plays, as opposed to the engine.
    remote_active: Arc<AtomicBool>,
}

impl Router {
    /// Puts an engine and a remote player behind one output.
    ///
    /// `engine_events` is the receiver that came with the engine.
    /// `make_remote` builds the remote player around the sink it reports
    /// to. Returns the router and the receiver of the merged events, to
    /// give to [`Session::start`](crate::Session::start).
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot start a thread.
    pub fn new(
        engine: Engine,
        engine_events: Receiver<EngineEvent>,
        make_remote: impl FnOnce(EventSink) -> Box<dyn RemotePlayer>,
    ) -> (Self, Receiver<EngineEvent>) {
        let (events, merged) = mpsc::channel();
        let remote_active = Arc::new(AtomicBool::new(false));
        let remote = make_remote(EventSink {
            events: events.clone(),
            active: Arc::clone(&remote_active),
        });

        let engine_is_heard = Arc::clone(&remote_active);
        thread::Builder::new()
            .name("unisson-router".into())
            .spawn(move || {
                for event in engine_events {
                    // The volume belongs to Unisson, whoever plays.
                    let always = matches!(event, EngineEvent::VolumeChanged(_));
                    if (always || !engine_is_heard.load(Ordering::SeqCst))
                        && events.send(event).is_err()
                    {
                        break;
                    }
                }
            })
            .expect("cannot start the router thread");

        let router = Self {
            engine,
            remote,
            remote_active,
        };
        (router, merged)
    }

    fn remote_plays(&self) -> bool {
        self.remote_active.load(Ordering::SeqCst)
    }

    /// Hands playback to the engine or to the remote player.
    fn switch_to_remote(&self, remote: bool) {
        let was_remote = self.remote_active.swap(remote, Ordering::SeqCst);
        match (was_remote, remote) {
            // Its events are already muted: what it says while it stops is
            // not heard.
            (true, false) => self.remote.stop(),
            (false, true) => {
                let _ = self.engine.stop();
            }
            _ => {}
        }
    }

    fn play_address(&self, uri: StreamUri, live: bool) -> Result<(), AudioError> {
        if uri.as_str().starts_with(self.remote.scheme()) {
            self.switch_to_remote(true);
            self.remote.play_uri(uri);
            return Ok(());
        }
        self.switch_to_remote(false);
        if live {
            self.engine.play_live(uri)
        } else {
            self.engine.play_uri(uri)
        }
    }
}

impl Output for Router {
    fn play_uri(&self, uri: StreamUri) -> Result<(), AudioError> {
        self.play_address(uri, false)
    }

    fn play_live(&self, uri: StreamUri) -> Result<(), AudioError> {
        self.play_address(uri, true)
    }

    fn play(&self) -> Result<(), AudioError> {
        if self.remote_plays() {
            self.remote.play();
            return Ok(());
        }
        self.engine.play()
    }

    fn pause(&self) -> Result<(), AudioError> {
        if self.remote_plays() {
            self.remote.pause();
            return Ok(());
        }
        self.engine.pause()
    }

    fn stop(&self) -> Result<(), AudioError> {
        // Stopping hands the sound back to the engine, which plays nothing
        // until it is given an address. The remote player is stopped once.
        if self.remote_active.swap(false, Ordering::SeqCst) {
            self.remote.stop();
            return Ok(());
        }
        self.engine.stop()
    }

    fn seek(&self, position: Duration) -> Result<(), AudioError> {
        if self.remote_plays() {
            self.remote.seek(position);
            return Ok(());
        }
        self.engine.seek(position)
    }

    fn set_volume(&self, volume: f64) -> Result<(), AudioError> {
        // The engine keeps the volume and reports it, even while the remote
        // player plays: the slider must not jump back.
        if self.remote_plays() {
            self.remote.set_volume(volume);
        }
        self.engine.set_volume(volume)
    }
}
