//! The built-in player: a librespot session and player, on a thread of its
//! own.
//!
//! # Design
//!
//! librespot runs on tokio. The player owns a small runtime, and one task
//! that handles in turn the orders of Unisson and the events of librespot,
//! so they never cross.
//!
//! The session opens with the first track, with an access token of the
//! built-in sign-in, and stays open for the next ones. It opens again after
//! Spotify closes it.
//!
//! When a track cannot play, librespot only says "unavailable": the reason
//! is in its logs. The player then asks for the key of the track itself, to
//! tell the cases apart:
//!
//! - the key comes: the audio servers failed, or the protocol changed. It
//!   may pass: [`Issue::Limited`];
//! - Spotify refuses the key, three times in a row: [`Issue::Refused`]. On
//!   many accounts this does not pass;
//! - the key does not come in time: the track is tried once more.
//!
//! The player does not decide what happens next. It reports a [`Failed`],
//! and the [`SpotifyPlayer`](crate::SpotifyPlayer) chooses.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use librespot_core::audio_key::AudioKeyError;
use librespot_core::authentication::Credentials;
use librespot_core::error::ErrorKind;
use librespot_core::{Error as LibrespotError, Session, SessionConfig, SpotifyUri};
use librespot_metadata::{Metadata, Track};
use librespot_playback::config::PlayerConfig;
use librespot_playback::mixer::NoOpVolume;
use librespot_playback::player::{Player, PlayerEvent, PlayerEventChannel};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use unisson_audio::{EngineEvent, PlaybackState};
use unisson_core::SourceError;
use unisson_playback::EventSink;

use super::sink::{GstSink, Volume};
use crate::connection::Spotify;

/// How often the position is reported while playing.
const POSITION_INTERVAL: Duration = Duration::from_secs(1);
/// How many times the key of a failed track is asked for.
const KEY_ATTEMPTS: u32 = 3;
/// The wait between two of those.
const KEY_RETRY_DELAY: Duration = Duration::from_secs(1);

/// Why a track did not play inside Unisson.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Issue {
    /// Spotify refused the key of the track.
    Refused,
    /// Spotify limits the playback for now: its audio servers fail, or it
    /// asks to wait.
    Limited(String),
    /// The account is not Premium.
    Premium,
    /// Spotify cannot be reached.
    Offline(String),
    /// Anything else: the track itself cannot play, here or elsewhere.
    Other(String),
}

/// A track that did not play inside Unisson.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failed {
    /// The `spotify:track:` address of the track.
    pub uri: String,
    /// Why.
    pub issue: Issue,
}

#[derive(Debug)]
enum Order {
    Play(String),
    Resume,
    Pause,
    Stop,
    Seek(Duration),
}

/// Plays Spotify inside Unisson. See the notes at the top of this file.
#[derive(Debug)]
pub struct BuiltinPlayer {
    orders: UnboundedSender<Order>,
    volume: Volume,
}

impl BuiltinPlayer {
    /// Starts the player. Its events go to `sink`, and the tracks that do
    /// not play are reported to `on_failure`.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot start a thread.
    pub fn start(
        spotify: Arc<Spotify>,
        sink: EventSink,
        on_failure: impl Fn(Failed) + Send + 'static,
    ) -> Self {
        let (orders, inbox) = unbounded_channel();
        let volume = Volume::default();
        let worker = Worker {
            spotify,
            sink,
            on_failure: Box::new(on_failure),
            volume: volume.clone(),
            session: None,
            player: None,
            events: None,
            current: None,
        };
        thread::Builder::new()
            .name("unisson-librespot".into())
            .spawn(move || {
                let runtime = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_name("unisson-librespot-io")
                    .enable_all()
                    .build()
                {
                    Ok(runtime) => runtime,
                    Err(error) => {
                        eprintln!("unisson: cannot start the Spotify player: {error}");
                        return;
                    }
                };
                runtime.block_on(worker.run(inbox));
            })
            .expect("cannot start the Spotify thread");
        Self { orders, volume }
    }

    fn send(&self, order: Order) {
        // The thread only ends with the player.
        let _ = self.orders.send(order);
    }

    /// Plays a `spotify:track:` address from its start.
    pub fn play(&self, uri: &str) {
        self.send(Order::Play(uri.to_owned()));
    }

    /// Resumes after a pause.
    pub fn resume(&self) {
        self.send(Order::Resume);
    }

    /// Pauses.
    pub fn pause(&self) {
        self.send(Order::Pause);
    }

    /// Stops, and forgets the track.
    pub fn stop(&self) {
        self.send(Order::Stop);
    }

    /// Moves to a position.
    pub fn seek(&self, position: Duration) {
        self.send(Order::Seek(position));
    }

    /// Sets the volume, from `0.0` to `1.0`. It applies at once.
    pub fn set_volume(&self, volume: f64) {
        self.volume.set(volume);
    }
}

/// The track that plays.
struct Current {
    uri: String,
    parsed: SpotifyUri,
    /// The number librespot gives this play: events of others are old.
    request: Option<u64>,
    duration: Option<Duration>,
    /// Whether the track was already tried twice.
    retried: bool,
}

struct Worker {
    spotify: Arc<Spotify>,
    sink: EventSink,
    on_failure: Box<dyn Fn(Failed) + Send>,
    volume: Volume,
    session: Option<Session>,
    player: Option<Arc<Player>>,
    events: Option<PlayerEventChannel>,
    current: Option<Current>,
}

/// Waits for the next event of the player, or forever without a player.
async fn next_event(events: &mut Option<PlayerEventChannel>) -> Option<PlayerEvent> {
    match events {
        Some(events) => events.recv().await,
        None => std::future::pending().await,
    }
}

impl Worker {
    async fn run(mut self, mut inbox: UnboundedReceiver<Order>) {
        loop {
            tokio::select! {
                order = inbox.recv() => match order {
                    Some(order) => self.handle(order).await,
                    None => break,
                },
                event = next_event(&mut self.events) => match event {
                    Some(event) => self.follow(event).await,
                    // The player ended: a new one comes with the next track.
                    None => self.close(),
                },
            }
        }
        self.close();
    }

    async fn handle(&mut self, order: Order) {
        match order {
            Order::Play(uri) => self.play(uri).await,
            Order::Resume => {
                if let (Some(player), Some(_)) = (&self.player, &self.current) {
                    player.play();
                }
            }
            Order::Pause => {
                if let (Some(player), Some(_)) = (&self.player, &self.current) {
                    player.pause();
                }
            }
            Order::Stop => {
                if let Some(player) = &self.player {
                    player.stop();
                }
                self.current = None;
            }
            Order::Seek(position) => {
                if let (Some(player), Some(_)) = (&self.player, &self.current) {
                    player.seek(u32::try_from(position.as_millis()).unwrap_or(u32::MAX));
                }
            }
        }
    }

    fn fail(&mut self, uri: String, issue: Issue) {
        self.current = None;
        (self.on_failure)(Failed { uri, issue });
    }

    async fn play(&mut self, uri: String) {
        if let Some(player) = &self.player {
            player.stop();
        }
        self.current = None;
        let parsed = match SpotifyUri::from_uri(&uri) {
            Ok(parsed @ SpotifyUri::Track { .. }) => parsed,
            _ => return self.fail(uri, Issue::Other("not a Spotify track".into())),
        };
        if let Err(issue) = self.open_session().await {
            return self.fail(uri, issue);
        }
        let Some(player) = &self.player else {
            return self.fail(uri, Issue::Other("no player".into()));
        };
        player.load(parsed.clone(), true, 0);
        self.current = Some(Current {
            uri,
            parsed,
            request: None,
            duration: None,
            retried: false,
        });
    }

    /// Opens the session and the player, unless they are open.
    async fn open_session(&mut self) -> Result<(), Issue> {
        if let (Some(session), Some(_)) = (&self.session, &self.player)
            && !session.is_invalid()
        {
            return Ok(());
        }
        self.close();
        let Some(api) = self.spotify.account_api() else {
            return Err(Issue::Other("not signed in to Spotify".into()));
        };
        // The token comes over blocking HTTP: off the runtime.
        let token = tokio::task::spawn_blocking(move || api.access_token_now())
            .await
            .map_err(|error| Issue::Other(error.to_string()))?
            .map_err(|error| match error {
                SourceError::Network(message) => Issue::Offline(message),
                other => Issue::Other(other.to_string()),
            })?;
        let session = Session::new(SessionConfig::default(), None);
        session
            .connect(Credentials::with_access_token(token), false)
            .await
            .map_err(|error| connect_issue(&error))?;
        let config = PlayerConfig {
            position_update_interval: Some(POSITION_INTERVAL),
            ..PlayerConfig::default()
        };
        let volume = self.volume.clone();
        let player = Player::new(config, session.clone(), Box::new(NoOpVolume), move || {
            Box::new(GstSink::open(&volume))
        });
        self.events = Some(player.get_player_event_channel());
        self.player = Some(player);
        self.session = Some(session);
        Ok(())
    }

    fn close(&mut self) {
        if let Some(player) = self.player.take() {
            player.stop();
        }
        if let Some(session) = self.session.take() {
            session.shutdown();
        }
        self.events = None;
    }

    /// Tells whether an event is about the track that plays.
    fn is_current(&mut self, request: u64) -> bool {
        match &mut self.current {
            Some(current) if current.request.is_none() => {
                current.request = Some(request);
                true
            }
            Some(current) => current.request == Some(request),
            None => false,
        }
    }

    fn position(&self, position_ms: u32) -> EngineEvent {
        EngineEvent::Position {
            position: Duration::from_millis(u64::from(position_ms)),
            duration: self.current.as_ref().and_then(|current| current.duration),
        }
    }

    async fn follow(&mut self, event: PlayerEvent) {
        match event {
            PlayerEvent::PlayRequestIdChanged { play_request_id } => {
                if let Some(current) = &mut self.current {
                    current.request = Some(play_request_id);
                }
            }
            PlayerEvent::TrackChanged { audio_item } => {
                if let Some(current) = &mut self.current
                    && current.parsed == audio_item.track_id
                {
                    current.duration =
                        Some(Duration::from_millis(u64::from(audio_item.duration_ms)));
                }
            }
            PlayerEvent::Playing {
                play_request_id,
                position_ms,
                ..
            } if self.is_current(play_request_id) => {
                self.sink
                    .send(EngineEvent::StateChanged(PlaybackState::Playing));
                self.sink.send(self.position(position_ms));
            }
            PlayerEvent::Paused {
                play_request_id,
                position_ms,
                ..
            } if self.is_current(play_request_id) => {
                self.sink
                    .send(EngineEvent::StateChanged(PlaybackState::Paused));
                self.sink.send(self.position(position_ms));
            }
            PlayerEvent::PositionChanged {
                play_request_id,
                position_ms,
                ..
            }
            | PlayerEvent::Seeked {
                play_request_id,
                position_ms,
                ..
            }
            | PlayerEvent::PositionCorrection {
                play_request_id,
                position_ms,
                ..
            } if self.is_current(play_request_id) => {
                self.sink.send(self.position(position_ms));
            }
            PlayerEvent::EndOfTrack {
                play_request_id, ..
            } if self.is_current(play_request_id) => {
                self.current = None;
                self.sink.send(EngineEvent::EndOfStream);
            }
            PlayerEvent::Unavailable {
                play_request_id, ..
            } if self.is_current(play_request_id) => self.unavailable().await,
            PlayerEvent::SessionDisconnected { .. } => {
                // The next track opens a new session. The one that played
                // is lost.
                if let Some(current) = self.current.take() {
                    self.close();
                    self.fail(
                        current.uri,
                        Issue::Offline("Spotify closed the session".into()),
                    );
                } else {
                    self.close();
                }
            }
            _ => {}
        }
    }

    /// The track that plays cannot: finds why, and tries once more when it
    /// may help.
    async fn unavailable(&mut self) {
        let Some(current) = &self.current else { return };
        let (uri, parsed, retried) = (current.uri.clone(), current.parsed.clone(), current.retried);
        let Some(session) = self.session.clone() else {
            return self.fail(uri, Issue::Offline("no Spotify session".into()));
        };
        match diagnose(&session, &parsed).await {
            Diagnosis::TryAgain if !retried => {
                if let (Some(player), Some(current)) = (&self.player, &mut self.current) {
                    current.retried = true;
                    current.request = None;
                    player.load(parsed, true, 0);
                }
            }
            Diagnosis::TryAgain => {
                self.fail(uri, Issue::Limited("the audio key does not come".into()))
            }
            Diagnosis::Issue(issue) => self.fail(uri, issue),
        }
    }
}

/// What the key of a failed track says.
#[derive(Debug, PartialEq, Eq)]
enum Diagnosis {
    /// Nothing certain: the track may play if tried again.
    TryAgain,
    /// The reason is known.
    Issue(Issue),
}

/// Asks for the key of a track that failed. See the notes at the top of
/// this file.
async fn diagnose(session: &Session, uri: &SpotifyUri) -> Diagnosis {
    let SpotifyUri::Track { id } = uri else {
        return Diagnosis::Issue(Issue::Other("not a Spotify track".into()));
    };
    let track = match Track::get(session, uri).await {
        Ok(track) => track,
        Err(error) => return Diagnosis::Issue(transport_issue(&error)),
    };
    let Some(file) = track.files.values().next().copied() else {
        // Not available in the country of the account: no route plays it.
        return Diagnosis::Issue(Issue::Other("this track cannot be played here".into()));
    };
    let mut refusals = 0;
    for attempt in 0..KEY_ATTEMPTS {
        if attempt > 0 {
            tokio::time::sleep(KEY_RETRY_DELAY).await;
        }
        match session.audio_key().request(*id, file).await {
            // The key comes: the audio itself failed.
            Ok(_) => {
                return Diagnosis::Issue(Issue::Limited(
                    "the audio servers of Spotify do not answer".into(),
                ));
            }
            Err(error) if is_key_refusal(&error) => refusals += 1,
            Err(_) => {}
        }
    }
    classify_keys(refusals)
}

/// Tells what a run of key requests means.
fn classify_keys(refusals: u32) -> Diagnosis {
    if refusals == KEY_ATTEMPTS {
        Diagnosis::Issue(Issue::Refused)
    } else {
        Diagnosis::TryAgain
    }
}

/// Tells whether Spotify answered a key request with a refusal.
fn is_key_refusal(error: &LibrespotError) -> bool {
    matches!(
        error.error.downcast_ref::<AudioKeyError>(),
        Some(AudioKeyError::AesKey)
    )
}

/// Tells why the session did not open.
fn connect_issue(error: &LibrespotError) -> Issue {
    let message = error.to_string();
    if message.contains("Premium account required") {
        Issue::Premium
    } else {
        transport_issue(error)
    }
}

/// Tells whether an error of librespot comes from the network.
fn transport_issue(error: &LibrespotError) -> Issue {
    let message = error.to_string();
    match error.kind {
        ErrorKind::Unavailable | ErrorKind::DeadlineExceeded | ErrorKind::Aborted => {
            Issue::Offline(message)
        }
        ErrorKind::ResourceExhausted => Issue::Limited(message),
        _ => Issue::Other(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_three_refusals_in_a_row_mean_the_account_is_refused() {
        assert_eq!(classify_keys(3), Diagnosis::Issue(Issue::Refused));
        assert_eq!(classify_keys(2), Diagnosis::TryAgain);
        assert_eq!(classify_keys(0), Diagnosis::TryAgain);
    }

    #[test]
    fn a_refused_key_is_told_from_a_late_one() {
        assert!(is_key_refusal(&AudioKeyError::AesKey.into()));
        assert!(!is_key_refusal(&AudioKeyError::Timeout.into()));
        assert!(!is_key_refusal(&LibrespotError::unavailable("down")));
    }

    #[test]
    fn errors_of_the_session_are_sorted() {
        assert_eq!(
            connect_issue(&LibrespotError::permission_denied(
                "Login failed with reason: Premium account required"
            )),
            Issue::Premium
        );
        assert!(matches!(
            connect_issue(&LibrespotError::unavailable("no route to host")),
            Issue::Offline(_)
        ));
        assert!(matches!(
            transport_issue(&LibrespotError::resource_exhausted("429")),
            Issue::Limited(_)
        ));
        assert!(matches!(
            transport_issue(&LibrespotError::not_found("gone")),
            Issue::Other(_)
        ));
    }
}
