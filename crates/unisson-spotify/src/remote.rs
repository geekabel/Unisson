//! The remote player: the official Spotify application, driven through the
//! Web API.
//!
//! # Design
//!
//! The Web API only sends orders. It never says when something happens, so
//! the remote player asks every second what the account plays, while one
//! of its tracks plays. A [`Tracker`] reads those answers and turns them
//! into the events the playback session understands: the state, the
//! position, and above all the end of the track, which lets the queue move
//! on.
//!
//! Three cases end a track:
//!
//! - it played to its end, and Spotify stopped;
//! - it played to its end, and Spotify went on by itself (its "autoplay"):
//!   Spotify is paused, and the queue of Unisson takes over;
//! - the user chose something else in the Spotify application before the
//!   end: Unisson steps back and stops, rather than fighting the user.
//!
//! The orders and the questions go through one thread, so they never cross.

use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use unisson_audio::{EngineEvent, Failure, FailureKind, PlaybackState as AudioState};
use unisson_core::{SourceError, StreamUri};
use unisson_playback::{EventSink, RemotePlayer};

use crate::SCHEME;
use crate::api::{NO_DEVICE, PlaybackState, SpotifyApi};
use crate::connection::Spotify;
use crate::source::choose_device;

/// How often the playback is followed.
const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// How long the Spotify application has to start a track.
const START_TIMEOUT: Duration = Duration::from_secs(15);
/// A track counts as played to its end within this much of its duration.
const END_MARGIN: Duration = Duration::from_secs(4);
/// How many questions in a row may fail before playback is said lost.
const MAX_POLL_FAILURES: u32 = 5;

/// An order for the thread of the remote player.
#[derive(Debug)]
enum Order {
    Play(StreamUri),
    Resume,
    Pause,
    Stop,
    Seek(Duration),
    Volume(f64),
}

/// Drives the official Spotify application. See the notes at the top of
/// this file.
#[derive(Debug)]
pub struct SpotifyRemote {
    orders: Sender<Order>,
}

impl SpotifyRemote {
    /// Starts the remote player. It reports to `sink`.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot start a thread.
    pub fn start(spotify: Arc<Spotify>, sink: EventSink) -> Self {
        Self::start_with(spotify, sink, POLL_INTERVAL, START_TIMEOUT)
    }

    /// Same as [`start`](Self::start), with other delays: for tests.
    fn start_with(
        spotify: Arc<Spotify>,
        sink: EventSink,
        poll: Duration,
        start_timeout: Duration,
    ) -> Self {
        let (orders, inbox) = mpsc::channel();
        let worker = Worker {
            spotify,
            sink,
            poll,
            start_timeout,
            device: None,
            supports_volume: false,
            tracker: None,
            poll_failures: 0,
        };
        thread::Builder::new()
            .name("unisson-spotify".into())
            .spawn(move || worker.run(&inbox))
            .expect("cannot start the Spotify thread");
        Self { orders }
    }

    fn send(&self, order: Order) {
        // The thread only ends with the remote player.
        let _ = self.orders.send(order);
    }
}

impl RemotePlayer for SpotifyRemote {
    fn scheme(&self) -> &'static str {
        SCHEME
    }

    fn play_uri(&self, uri: StreamUri) {
        self.send(Order::Play(uri));
    }

    fn play(&self) {
        self.send(Order::Resume);
    }

    fn pause(&self) {
        self.send(Order::Pause);
    }

    fn stop(&self) {
        self.send(Order::Stop);
    }

    fn seek(&self, position: Duration) {
        self.send(Order::Seek(position));
    }

    fn set_volume(&self, volume: f64) {
        self.send(Order::Volume(volume));
    }
}

/// Turns an error of the Web API into a failure of playback.
fn failure(error: &SourceError) -> Failure {
    let kind = match error {
        SourceError::Network(_) => FailureKind::Network,
        SourceError::NotFound => FailureKind::NotFound,
        _ => FailureKind::Other,
    };
    Failure {
        kind,
        message: error.to_string(),
    }
}

/// The state of the thread of the remote player.
struct Worker {
    spotify: Arc<Spotify>,
    sink: EventSink,
    poll: Duration,
    start_timeout: Duration,
    /// The Spotify application that plays.
    device: Option<String>,
    supports_volume: bool,
    /// Follows the track that plays, if one does.
    tracker: Option<Tracker>,
    poll_failures: u32,
}

impl Worker {
    fn run(mut self, inbox: &Receiver<Order>) {
        loop {
            let order = if self.tracker.is_some() {
                match inbox.recv_timeout(self.poll) {
                    Ok(order) => Some(order),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            } else {
                match inbox.recv() {
                    Ok(order) => Some(order),
                    Err(_) => return,
                }
            };
            match order {
                Some(Order::Volume(volume)) => {
                    // The slider sends many values: only the last counts.
                    let mut last = volume;
                    let mut next = None;
                    while let Ok(order) = inbox.try_recv() {
                        match order {
                            Order::Volume(volume) => last = volume,
                            other => {
                                next = Some(other);
                                break;
                            }
                        }
                    }
                    self.set_volume(last);
                    if let Some(order) = next {
                        self.handle(order);
                    }
                }
                Some(order) => self.handle(order),
                None => self.follow(),
            }
        }
    }

    fn handle(&mut self, order: Order) {
        match order {
            Order::Play(uri) => self.play(uri),
            Order::Resume => self.resume(),
            Order::Pause => self.pause(),
            Order::Stop => self.stop(),
            Order::Seek(position) => self.seek(position),
            Order::Volume(volume) => self.set_volume(volume),
        }
    }

    fn api(&self) -> Result<Arc<SpotifyApi>, SourceError> {
        self.spotify
            .api()
            .ok_or_else(|| SourceError::Unavailable("not connected to Spotify".into()))
    }

    fn fail(&mut self, error: &SourceError) {
        self.tracker = None;
        self.sink.send(EngineEvent::Error(failure(error)));
    }

    fn play(&mut self, uri: StreamUri) {
        // First of all: the session only listens to what follows this.
        self.sink.send(EngineEvent::Loaded(uri.clone()));
        self.tracker = None;
        self.poll_failures = 0;
        let started = self.api().and_then(|api| {
            let devices = api.devices()?;
            let device = choose_device(&devices)
                .ok_or_else(|| SourceError::Unavailable(NO_DEVICE.into()))?;
            let id = device.id.clone().unwrap_or_default();
            self.supports_volume = device.supports_volume;
            api.play_track(&id, uri.as_str())?;
            Ok(id)
        });
        match started {
            Ok(device) => {
                self.device = Some(device);
                self.tracker = Some(Tracker::new(uri.as_str(), self.start_timeout));
            }
            Err(error) => self.fail(&error),
        }
    }

    /// Sends an order to the device, if there is one. A failure is left to
    /// the next question: it tells what really happened.
    fn order(&self, send: impl FnOnce(&SpotifyApi, &str) -> Result<(), SourceError>) -> bool {
        let (Ok(api), Some(device)) = (self.api(), self.device.as_deref()) else {
            return false;
        };
        match send(&api, device) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("unisson: Spotify did not obey: {error}");
                false
            }
        }
    }

    fn resume(&mut self) {
        if self.tracker.is_some() && self.order(SpotifyApi::resume) {
            self.sink
                .send(EngineEvent::StateChanged(AudioState::Playing));
        }
    }

    fn pause(&mut self) {
        if self.tracker.is_some() && self.order(SpotifyApi::pause) {
            self.sink
                .send(EngineEvent::StateChanged(AudioState::Paused));
        }
    }

    fn stop(&mut self) {
        if let Some(tracker) = self.tracker.take()
            && tracker.playing
        {
            self.order(SpotifyApi::pause);
        }
    }

    fn seek(&mut self, position: Duration) {
        let duration = self.tracker.as_ref().and_then(|tracker| tracker.duration);
        if self.tracker.is_some() && self.order(|api, device| api.seek(device, position)) {
            self.sink.send(EngineEvent::Position { position, duration });
        }
    }

    fn set_volume(&mut self, volume: f64) {
        if !self.supports_volume || self.tracker.is_none() {
            return;
        }
        // The volume is between 0 and 1: its percentage fits in a byte.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let percent = (volume.clamp(0.0, 1.0) * 100.0).round() as u8;
        self.order(|api, device| api.set_volume(device, percent));
    }

    /// Asks what plays, and reports it.
    fn follow(&mut self) {
        let Some(tracker) = self.tracker.as_mut() else {
            return;
        };
        let state = match self.spotify.api().map(|api| api.playback()) {
            Some(Ok(state)) => state,
            Some(Err(error)) => {
                self.poll_failures += 1;
                if self.poll_failures >= MAX_POLL_FAILURES {
                    self.fail(&error);
                }
                return;
            }
            None => {
                let error = SourceError::Unavailable("not connected to Spotify".into());
                self.fail(&error);
                return;
            }
        };
        self.poll_failures = 0;
        let seen = tracker.observe(state.as_ref(), Instant::now());
        for event in seen.events {
            self.sink.send(event);
        }
        match seen.outcome {
            Outcome::Continues => {}
            Outcome::Ended { moved_on } => {
                self.tracker = None;
                // Spotify went on by itself: the queue of Unisson decides.
                if moved_on {
                    self.order(SpotifyApi::pause);
                }
            }
            Outcome::Left => self.tracker = None,
        }
    }
}

/// What a [`Tracker`] concludes from one answer.
#[derive(Debug, PartialEq)]
struct Seen {
    events: Vec<EngineEvent>,
    outcome: Outcome,
}

/// Whether the track still needs following.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// It plays, or is paused, or is about to start.
    Continues,
    /// It played to its end. `moved_on` tells whether Spotify went on to
    /// something else by itself.
    Ended { moved_on: bool },
    /// It is over without reaching its end: the user chose something else,
    /// or it never started.
    Left,
}

/// Follows one track through the answers of the Web API. See the notes at
/// the top of this file.
#[derive(Debug)]
struct Tracker {
    uri: String,
    /// When the order to play was sent.
    asked_at: Instant,
    start_timeout: Duration,
    /// Whether the track was seen playing.
    started: bool,
    playing: bool,
    progress: Duration,
    duration: Option<Duration>,
}

impl Tracker {
    fn new(uri: &str, start_timeout: Duration) -> Self {
        Self {
            uri: uri.to_owned(),
            asked_at: Instant::now(),
            start_timeout,
            started: false,
            playing: false,
            progress: Duration::ZERO,
            duration: None,
        }
    }

    /// Whether the last position seen was at the end of the track.
    fn near_end(&self) -> bool {
        self.duration
            .is_some_and(|duration| self.progress + END_MARGIN >= duration)
    }

    fn position(&self) -> EngineEvent {
        EngineEvent::Position {
            position: self.progress,
            duration: self.duration,
        }
    }

    fn observe(&mut self, state: Option<&PlaybackState>, now: Instant) -> Seen {
        let ours = state.filter(|state| state.item_uri.as_deref() == Some(self.uri.as_str()));
        if !self.started {
            return match ours {
                Some(state) if state.is_playing => {
                    self.started = true;
                    self.playing = true;
                    self.take(state);
                    Seen {
                        events: vec![
                            EngineEvent::StateChanged(AudioState::Playing),
                            self.position(),
                        ],
                        outcome: Outcome::Continues,
                    }
                }
                // The order may still be on its way.
                _ if now.duration_since(self.asked_at) < self.start_timeout => Seen {
                    events: Vec::new(),
                    outcome: Outcome::Continues,
                },
                _ => Seen {
                    events: vec![EngineEvent::Error(Failure {
                        kind: FailureKind::Other,
                        message: "the Spotify application did not start the track".into(),
                    })],
                    outcome: Outcome::Left,
                },
            };
        }
        let Some(state) = ours else {
            // Something else plays, or nothing does.
            let moved_on = state.is_some_and(|state| state.item_uri.is_some());
            return if self.near_end() {
                self.ended(moved_on)
            } else {
                self.playing = false;
                Seen {
                    events: vec![EngineEvent::StateChanged(AudioState::Stopped)],
                    outcome: Outcome::Left,
                }
            };
        };
        let back_to_start = state.progress.is_none_or(|progress| progress.is_zero());
        if !state.is_playing && (back_to_start || self.near_end()) {
            return self.ended(false);
        }
        let mut events = Vec::new();
        if state.is_playing != self.playing {
            self.playing = state.is_playing;
            events.push(EngineEvent::StateChanged(if state.is_playing {
                AudioState::Playing
            } else {
                AudioState::Paused
            }));
        }
        self.take(state);
        events.push(self.position());
        Seen {
            events,
            outcome: Outcome::Continues,
        }
    }

    fn take(&mut self, state: &PlaybackState) {
        if let Some(progress) = state.progress {
            self.progress = progress;
        }
        if state.duration.is_some() {
            self.duration = state.duration;
        }
    }

    fn ended(&mut self, moved_on: bool) -> Seen {
        self.playing = false;
        Seen {
            events: vec![EngineEvent::EndOfStream],
            outcome: Outcome::Ended { moved_on },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::test_server::{Request, TestServer};

    const URI: &str = "spotify:track:6rqhFgbbKwnb9MLmUQDhG6";
    const OTHER: &str = "spotify:track:7aaaaaaaaaaaaaaaaaaaaa";

    fn state(uri: Option<&str>, playing: bool, progress_ms: u64) -> PlaybackState {
        PlaybackState {
            device_id: Some("d1".into()),
            is_playing: playing,
            progress: Some(Duration::from_millis(progress_ms)),
            item_uri: uri.map(str::to_owned),
            duration: Some(Duration::from_secs(200)),
        }
    }

    fn position(ms: u64) -> EngineEvent {
        EngineEvent::Position {
            position: Duration::from_millis(ms),
            duration: Some(Duration::from_secs(200)),
        }
    }

    fn started() -> Tracker {
        let mut tracker = Tracker::new(URI, Duration::from_secs(15));
        tracker.observe(Some(&state(Some(URI), true, 500)), Instant::now());
        tracker
    }

    #[test]
    fn the_track_starts_once_spotify_plays_it() {
        let mut tracker = Tracker::new(URI, Duration::from_secs(15));
        let now = Instant::now();
        // The previous track is still reported for a moment.
        let before = tracker.observe(Some(&state(Some(OTHER), true, 90_000)), now);
        assert_eq!(before.events, []);
        assert_eq!(before.outcome, Outcome::Continues);

        let seen = tracker.observe(Some(&state(Some(URI), true, 300)), now);
        assert_eq!(
            seen.events,
            [
                EngineEvent::StateChanged(AudioState::Playing),
                position(300)
            ]
        );
    }

    #[test]
    fn a_track_that_never_starts_is_a_failure() {
        let mut tracker = Tracker::new(URI, Duration::from_secs(15));
        let late = Instant::now() + Duration::from_secs(16);
        let seen = tracker.observe(None, late);
        assert_eq!(seen.outcome, Outcome::Left);
        assert!(matches!(seen.events[..], [EngineEvent::Error(_)]));
    }

    #[test]
    fn the_position_moves_and_a_pause_in_spotify_is_reported() {
        let mut tracker = started();
        let now = Instant::now();
        assert_eq!(
            tracker
                .observe(Some(&state(Some(URI), true, 1500)), now)
                .events,
            [position(1500)]
        );
        assert_eq!(
            tracker
                .observe(Some(&state(Some(URI), false, 1600)), now)
                .events,
            [
                EngineEvent::StateChanged(AudioState::Paused),
                position(1600)
            ]
        );
        assert_eq!(
            tracker
                .observe(Some(&state(Some(URI), true, 1700)), now)
                .events,
            [
                EngineEvent::StateChanged(AudioState::Playing),
                position(1700)
            ]
        );
    }

    #[test]
    fn a_track_that_plays_to_its_end_ends() {
        let mut tracker = started();
        let now = Instant::now();
        tracker.observe(Some(&state(Some(URI), true, 198_000)), now);
        let seen = tracker.observe(Some(&state(Some(URI), false, 0)), now);
        assert_eq!(seen.events, [EngineEvent::EndOfStream]);
        assert_eq!(seen.outcome, Outcome::Ended { moved_on: false });
    }

    #[test]
    fn spotify_going_on_by_itself_after_the_end_is_an_end() {
        let mut tracker = started();
        let now = Instant::now();
        tracker.observe(Some(&state(Some(URI), true, 197_500)), now);
        let seen = tracker.observe(Some(&state(Some(OTHER), true, 1000)), now);
        assert_eq!(seen.events, [EngineEvent::EndOfStream]);
        assert_eq!(seen.outcome, Outcome::Ended { moved_on: true });
    }

    #[test]
    fn the_user_choosing_something_else_in_spotify_stops_unisson() {
        let mut tracker = started();
        let now = Instant::now();
        tracker.observe(Some(&state(Some(URI), true, 60_000)), now);
        let seen = tracker.observe(Some(&state(Some(OTHER), true, 1000)), now);
        assert_eq!(
            seen.events,
            [EngineEvent::StateChanged(AudioState::Stopped)]
        );
        assert_eq!(seen.outcome, Outcome::Left);
    }

    #[test]
    fn spotify_closed_in_the_middle_stops_unisson() {
        let mut tracker = started();
        let seen = tracker.observe(None, Instant::now());
        assert_eq!(seen.outcome, Outcome::Left);
    }

    /// The remote player against a fake Spotify, from the order to play to
    /// the end of the track.
    #[test]
    fn the_remote_player_plays_follows_and_ends_a_track() {
        // What the fake Spotify reports, in turn, when asked what plays.
        let answers = Arc::new(Mutex::new(vec![
            format!(
                r#"{{"is_playing":true,"progress_ms":1000,"item":{{"uri":"{URI}","duration_ms":10000}}}}"#
            ),
            format!(
                r#"{{"is_playing":true,"progress_ms":9000,"item":{{"uri":"{URI}","duration_ms":10000}}}}"#
            ),
            format!(
                r#"{{"is_playing":true,"progress_ms":500,"item":{{"uri":"{OTHER}","duration_ms":90000}}}}"#
            ),
        ]));
        let server = TestServer::start(move |request: &Request| {
            let ok = |body: &str| (200, body.to_owned(), Vec::new());
            match request.target.as_str() {
                "/api/token" => ok(r#"{"access_token":"A","expires_in":3600}"#),
                "/me/player/devices" => ok(
                    r#"{"devices":[{"id":"d1","name":"Bureau","type":"Computer","is_active":false,"is_restricted":false,"supports_volume":true}]}"#,
                ),
                "/me/player" => {
                    let mut answers = answers.lock().unwrap();
                    let answer = if answers.len() > 1 {
                        answers.remove(0)
                    } else {
                        answers[0].clone()
                    };
                    ok(&answer)
                }
                _ => (204, String::new(), Vec::new()),
            }
        });
        let api = SpotifyApi::with_bases(&server.url, &server.url, "cid", "R", |_| {});
        let spotify = Arc::new(Spotify::with_api(api));
        let (sink, events) = EventSink::for_tests();
        let remote = SpotifyRemote::start_with(
            spotify,
            sink,
            Duration::from_millis(30),
            Duration::from_secs(5),
        );

        remote.play_uri(StreamUri::new(URI).unwrap());
        remote.set_volume(0.3);
        remote.set_volume(0.42);

        let mut seen = Vec::new();
        while let Ok(event) = events.recv_timeout(Duration::from_secs(5)) {
            let end = event == EngineEvent::EndOfStream;
            seen.push(event);
            if end {
                break;
            }
        }
        assert_eq!(
            seen,
            [
                EngineEvent::Loaded(StreamUri::new(URI).unwrap()),
                EngineEvent::StateChanged(AudioState::Playing),
                EngineEvent::Position {
                    position: Duration::from_millis(1000),
                    duration: Some(Duration::from_secs(10)),
                },
                EngineEvent::Position {
                    position: Duration::from_millis(9000),
                    duration: Some(Duration::from_secs(10)),
                },
                EngineEvent::EndOfStream,
            ]
        );
        // Give the pause after the end a moment to be sent.
        thread::sleep(Duration::from_millis(200));
        let orders: Vec<String> = server
            .lines()
            .into_iter()
            .filter(|line| line.starts_with("PUT"))
            .collect();
        assert_eq!(
            orders,
            [
                "PUT /me/player/play?device_id=d1",
                // Two volumes in a row: only the last is sent.
                "PUT /me/player/volume?volume_percent=42&device_id=d1",
                // Spotify went on by itself after the end: it is paused.
                "PUT /me/player/pause?device_id=d1",
            ]
        );
    }

    #[test]
    fn without_a_spotify_application_the_track_fails_at_once() {
        let server = TestServer::start(|request: &Request| match request.target.as_str() {
            "/api/token" => (
                200,
                r#"{"access_token":"A","expires_in":3600}"#.into(),
                Vec::new(),
            ),
            _ => (200, r#"{"devices":[]}"#.into(), Vec::new()),
        });
        let api = SpotifyApi::with_bases(&server.url, &server.url, "cid", "R", |_| {});
        let (sink, events) = EventSink::for_tests();
        let remote = SpotifyRemote::start(Arc::new(Spotify::with_api(api)), sink);

        remote.play_uri(StreamUri::new(URI).unwrap());

        assert!(matches!(
            events.recv_timeout(Duration::from_secs(5)),
            Ok(EngineEvent::Loaded(_))
        ));
        assert!(matches!(
            events.recv_timeout(Duration::from_secs(5)),
            Ok(EngineEvent::Error(failure)) if failure.message.contains("open the Spotify application")
        ));
    }
}
