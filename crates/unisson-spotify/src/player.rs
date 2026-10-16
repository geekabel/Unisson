//! The player of Spotify tracks: inside Unisson first, the official Spotify
//! application as a backup.
//!
//! # Design
//!
//! [`SpotifyPlayer`] is the [`RemotePlayer`] the router hands Spotify
//! tracks to. It holds two players:
//!
//! - the [`BuiltinPlayer`], which plays inside Unisson through librespot;
//! - the [`SpotifyRemote`], which drives the official application, through
//!   the backup application the user registered.
//!
//! Each track goes to the built-in player, unless it is known not to work.
//! When it fails, [`after_failure`] decides, from the reason:
//!
//! | Reason | Backup set up, switch on | Otherwise |
//! | --- | --- | --- |
//! | Spotify limits it, or refuses a key once | the backup plays the track; built-in tried again in 10 min | the track fails; built-in tried again in 2 min |
//! | Spotify refuses keys on two tracks in a row | same, and the built-in is no longer tried | the track fails, and the account is marked as refused |
//! | not Premium | no switch: the backup needs Premium too | |
//! | offline, or the track cannot play anywhere | no switch | |
//!
//! Each player speaks through its own gate of the [`EventSink`]: only the
//! one that plays is heard. The [`PlaybackStatus`] of the [`Spotify`]
//! connection tells the interface where the sound comes from, and why.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, SystemTime};

use unisson_audio::{EngineEvent, Failure, FailureKind};
use unisson_core::StreamUri;
use unisson_playback::{EventSink, RemotePlayer};

use crate::SCHEME;
use crate::builtin::{BuiltinPlayer, Failed, Issue};
use crate::connection::Spotify;
use crate::remote::SpotifyRemote;
use crate::status::{PlaybackStatus, Problem, Route};

/// How long the backup plays before the built-in playback is tried again.
pub const RETRY_WITH_BACKUP: Duration = Duration::from_secs(10 * 60);
/// How long Spotify waits, without a backup, before the built-in playback
/// is tried again.
pub const RETRY_WITHOUT_BACKUP: Duration = Duration::from_secs(2 * 60);
/// How many tracks in a row must have their key refused for the account to
/// count as refused.
const REFUSALS_FOR_ACCOUNT: usize = 2;

#[derive(Debug)]
enum Order {
    Play(StreamUri),
    Resume,
    Pause,
    Stop,
    Seek(Duration),
    Volume(f64),
    Failed(Failed),
}

/// Plays Spotify tracks. See the notes at the top of this file.
#[derive(Debug)]
pub struct SpotifyPlayer {
    orders: Sender<Order>,
}

impl SpotifyPlayer {
    /// Starts the player. It reports to `sink`.
    ///
    /// # Panics
    ///
    /// Panics if the operating system cannot start a thread.
    pub fn start(spotify: Arc<Spotify>, sink: EventSink) -> Self {
        let (orders, inbox) = mpsc::channel();
        let builtin_gate = Arc::new(AtomicBool::new(false));
        let app_gate = Arc::new(AtomicBool::new(false));
        let failures = orders.clone();
        let builtin = BuiltinPlayer::start(
            Arc::clone(&spotify),
            sink.gated(Arc::clone(&builtin_gate)),
            move |failed| {
                let _ = failures.send(Order::Failed(failed));
            },
        );
        let app = SpotifyRemote::start(Arc::clone(&spotify), sink.gated(Arc::clone(&app_gate)));
        let worker = Worker {
            spotify,
            sink,
            builtin,
            app,
            builtin_gate,
            app_gate,
            current: None,
            route: None,
            refused: Vec::new(),
        };
        thread::Builder::new()
            .name("unisson-spotify-player".into())
            .spawn(move || worker.run(&inbox))
            .expect("cannot start the Spotify thread");
        Self { orders }
    }

    fn send(&self, order: Order) {
        let _ = self.orders.send(order);
    }
}

impl RemotePlayer for SpotifyPlayer {
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

/// What is known when choosing a route.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Situation {
    /// The built-in playback is signed in.
    pub has_account: bool,
    /// Spotify refuses the built-in playback for this account.
    pub refused: bool,
    /// The backup application is set up.
    pub has_app: bool,
    /// Playback moves to the backup on its own.
    pub auto_switch: bool,
}

impl Situation {
    /// Reads the situation of a connection.
    pub fn of(spotify: &Spotify) -> Self {
        Self {
            has_account: spotify.has_account(),
            refused: spotify.builtin_refused(),
            has_app: spotify.has_app(),
            auto_switch: spotify.auto_switch(),
        }
    }

    fn backup_ready(self) -> bool {
        self.has_app && self.auto_switch
    }
}

/// Chooses where a new track plays.
pub fn choose_route(
    situation: Situation,
    status: &PlaybackStatus,
    now: SystemTime,
) -> Option<Route> {
    let builtin_usable = situation.has_account && !situation.refused;
    let waiting = status.retry_at.is_some_and(|retry_at| now < retry_at);
    if builtin_usable && !(waiting && status.route == Some(Route::App)) {
        Some(Route::Builtin)
    } else if situation.has_app {
        Some(Route::App)
    } else if builtin_usable {
        Some(Route::Builtin)
    } else {
        None
    }
}

/// What to do after the built-in player failed.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    /// Whether the backup plays the track.
    pub switch: bool,
    /// The new status.
    pub status: PlaybackStatus,
    /// Whether the account is marked as refused.
    pub refuse_account: bool,
    /// The failure to report when the track does not play at all.
    pub failure: Option<Failure>,
}

/// Decides what happens after the built-in player failed. See the table at
/// the top of this file.
///
/// `refusals` counts the tracks in a row whose key was refused, this one
/// included.
pub fn after_failure(
    issue: &Issue,
    refusals: usize,
    situation: Situation,
    current: PlaybackStatus,
    now: SystemTime,
) -> Decision {
    let failure = |kind, message: &str| Failure {
        kind,
        message: message.to_owned(),
    };
    let problem = match issue {
        Issue::Refused if refusals >= REFUSALS_FOR_ACCOUNT => Problem::AccountRefused,
        Issue::Refused | Issue::Limited(_) => Problem::Limited,
        Issue::Premium => {
            return Decision {
                switch: false,
                status: PlaybackStatus {
                    route: None,
                    problem: Some(Problem::PremiumRequired),
                    retry_at: None,
                },
                refuse_account: false,
                failure: Some(failure(FailureKind::Other, "Spotify Premium is required")),
            };
        }
        Issue::Offline(message) => {
            return Decision {
                switch: false,
                status: current,
                refuse_account: false,
                failure: Some(failure(FailureKind::Network, message)),
            };
        }
        Issue::Other(message) => {
            return Decision {
                switch: false,
                status: current,
                refuse_account: false,
                failure: Some(failure(FailureKind::Other, message)),
            };
        }
    };
    let refuse_account = problem == Problem::AccountRefused;
    let backup = situation.backup_ready();
    let retry_at = (!refuse_account).then(|| {
        now + if backup {
            RETRY_WITH_BACKUP
        } else {
            RETRY_WITHOUT_BACKUP
        }
    });
    Decision {
        switch: backup,
        status: PlaybackStatus {
            route: backup.then_some(Route::App),
            problem: Some(problem),
            retry_at,
        },
        refuse_account,
        failure: (!backup).then(|| {
            failure(
                FailureKind::Other,
                match problem {
                    Problem::AccountRefused => "Spotify refuses to play this account in Unisson",
                    _ => "Spotify limits playback in Unisson for now",
                },
            )
        }),
    }
}

struct Worker {
    spotify: Arc<Spotify>,
    sink: EventSink,
    builtin: BuiltinPlayer,
    app: SpotifyRemote,
    builtin_gate: Arc<AtomicBool>,
    app_gate: Arc<AtomicBool>,
    /// The track that plays.
    current: Option<StreamUri>,
    /// Who plays it.
    route: Option<Route>,
    /// The tracks in a row whose key was refused.
    refused: Vec<String>,
}

impl Worker {
    fn run(mut self, inbox: &Receiver<Order>) {
        while let Ok(order) = inbox.recv() {
            match order {
                Order::Play(uri) => self.play(uri),
                Order::Resume => self.on_route(BuiltinPlayer::resume, |app| app.play()),
                Order::Pause => self.on_route(BuiltinPlayer::pause, |app| app.pause()),
                Order::Stop => {
                    self.on_route(BuiltinPlayer::stop, |app| app.stop());
                    self.current = None;
                }
                Order::Seek(position) => {
                    self.on_route(|builtin| builtin.seek(position), |app| app.seek(position))
                }
                Order::Volume(volume) => {
                    self.builtin.set_volume(volume);
                    self.app.set_volume(volume);
                }
                Order::Failed(failed) => self.failed(failed),
            }
        }
    }

    fn on_route(&self, builtin: impl FnOnce(&BuiltinPlayer), app: impl FnOnce(&SpotifyRemote)) {
        match self.route {
            Some(Route::Builtin) => builtin(&self.builtin),
            Some(Route::App) => app(&self.app),
            None => {}
        }
    }

    /// Lets only `route` be heard.
    fn open_gate(&self, route: Option<Route>) {
        self.builtin_gate
            .store(route == Some(Route::Builtin), Ordering::SeqCst);
        self.app_gate
            .store(route == Some(Route::App), Ordering::SeqCst);
    }

    fn play(&mut self, uri: StreamUri) {
        // The session believes nothing before this, about this track.
        self.sink.send(EngineEvent::Loaded(uri.clone()));
        let situation = Situation::of(&self.spotify);
        let status = self.spotify.status();
        let route = choose_route(situation, &status, SystemTime::now());
        // Leave the other player before the new one speaks.
        match (self.route, route) {
            (Some(Route::Builtin), Some(Route::App)) => self.builtin.stop(),
            (Some(Route::App), Some(Route::Builtin)) => self.app.stop(),
            _ => {}
        }
        self.open_gate(route);
        self.route = route;
        self.current = Some(uri.clone());
        match route {
            Some(Route::Builtin) => {
                // A retry that is due: back to the normal status, for now.
                if status.route != Some(Route::Builtin)
                    && status.problem != Some(Problem::PremiumRequired)
                {
                    self.spotify
                        .set_status(PlaybackStatus::normal(Some(Route::Builtin)));
                }
                self.builtin.play(uri.as_str());
            }
            Some(Route::App) => self.app.play_uri(uri),
            None => {
                self.current = None;
                self.sink.send(EngineEvent::Error(Failure {
                    kind: FailureKind::Other,
                    message: "not connected to Spotify".into(),
                }));
            }
        }
    }

    fn failed(&mut self, failed: Failed) {
        // A failure of a track that is no longer played changes nothing.
        let is_current = self.route == Some(Route::Builtin)
            && self
                .current
                .as_ref()
                .is_some_and(|uri| uri.as_str() == failed.uri);
        if !is_current {
            return;
        }
        if failed.issue == Issue::Refused {
            if !self.refused.contains(&failed.uri) {
                self.refused.push(failed.uri.clone());
            }
        } else {
            self.refused.clear();
        }
        let decision = after_failure(
            &failed.issue,
            self.refused.len(),
            Situation::of(&self.spotify),
            self.spotify.status(),
            SystemTime::now(),
        );
        if decision.refuse_account {
            self.spotify.set_builtin_refused(true);
            self.refused.clear();
        }
        self.spotify.set_status(decision.status);
        if decision.switch {
            self.open_gate(Some(Route::App));
            self.route = Some(Route::App);
            if let Some(uri) = self.current.clone() {
                self.app.play_uri(uri);
            }
        } else if let Some(failure) = decision.failure {
            self.current = None;
            self.sink.send(EngineEvent::Error(failure));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::SpotifyApi;
    use crate::test_server::{Request, TestServer};

    const URI: &str = "spotify:track:6rqhFgbbKwnb9MLmUQDhG6";

    #[test]
    fn without_any_connection_the_track_is_announced_then_fails() {
        let (sink, events) = EventSink::for_tests();
        let player = SpotifyPlayer::start(Arc::new(Spotify::open(None)), sink);

        player.play_uri(StreamUri::new(URI).unwrap());

        let timeout = Duration::from_secs(5);
        assert_eq!(
            events.recv_timeout(timeout).unwrap(),
            EngineEvent::Loaded(StreamUri::new(URI).unwrap())
        );
        assert!(matches!(
            events.recv_timeout(timeout),
            Ok(EngineEvent::Error(_))
        ));
    }

    #[test]
    fn with_the_backup_only_the_official_application_plays_and_is_heard() {
        let server = TestServer::start(|request: &Request| {
            let ok = |body: &str| (200, body.to_owned(), Vec::new());
            match request.target.as_str() {
                "/api/token" => ok(r#"{"access_token":"A","expires_in":3600}"#),
                "/me/player/devices" => ok(
                    r#"{"devices":[{"id":"d1","name":"PC","type":"Computer","is_active":true,"is_restricted":false,"supports_volume":true}]}"#,
                ),
                "/me/player" => ok(&format!(
                    r#"{{"is_playing":true,"progress_ms":1000,"item":{{"uri":"{URI}","duration_ms":200000}}}}"#
                )),
                _ => (204, String::new(), Vec::new()),
            }
        });
        let app = SpotifyApi::with_bases(&server.url, &server.url, "cid", "R", |_| {});
        let spotify = Arc::new(Spotify::with_apis(None, Some(app)));
        assert_eq!(spotify.status().route, Some(Route::App));
        let (sink, events) = EventSink::for_tests();
        let player = SpotifyPlayer::start(spotify, sink);

        player.play_uri(StreamUri::new(URI).unwrap());

        let timeout = Duration::from_secs(5);
        assert_eq!(
            events.recv_timeout(timeout).unwrap(),
            EngineEvent::Loaded(StreamUri::new(URI).unwrap())
        );
        // The official application is heard: no second `Loaded`.
        assert_eq!(
            events.recv_timeout(timeout).unwrap(),
            EngineEvent::StateChanged(unisson_audio::PlaybackState::Playing)
        );
        assert!(
            server
                .lines()
                .iter()
                .any(|line| line.starts_with("PUT /me/player/play?device_id=d1"))
        );
    }

    const NOW: SystemTime = SystemTime::UNIX_EPOCH;

    fn situation(has_app: bool, auto_switch: bool) -> Situation {
        Situation {
            has_account: true,
            refused: false,
            has_app,
            auto_switch,
        }
    }

    fn normal() -> PlaybackStatus {
        PlaybackStatus::normal(Some(Route::Builtin))
    }

    #[test]
    fn a_new_track_goes_inside_unisson_unless_it_is_known_not_to_work() {
        let fallback = PlaybackStatus {
            route: Some(Route::App),
            problem: Some(Problem::Limited),
            retry_at: Some(NOW + Duration::from_secs(60)),
        };
        assert_eq!(
            choose_route(situation(true, true), &normal(), NOW),
            Some(Route::Builtin)
        );
        // While the backup waits, it keeps playing...
        assert_eq!(
            choose_route(situation(true, true), &fallback, NOW),
            Some(Route::App)
        );
        // ...until the built-in playback is due again.
        let later = NOW + Duration::from_secs(61);
        assert_eq!(
            choose_route(situation(true, true), &fallback, later),
            Some(Route::Builtin)
        );

        let refused = Situation {
            refused: true,
            ..situation(true, true)
        };
        assert_eq!(choose_route(refused, &normal(), NOW), Some(Route::App));
        let no_account = Situation {
            has_account: false,
            ..situation(true, true)
        };
        assert_eq!(choose_route(no_account, &normal(), NOW), Some(Route::App));
        let nothing = Situation {
            has_account: false,
            ..situation(false, true)
        };
        assert_eq!(choose_route(nothing, &normal(), NOW), None);
    }

    #[test]
    fn a_limit_switches_to_the_backup_and_tries_again_in_ten_minutes() {
        let decision = after_failure(
            &Issue::Limited("CDN".into()),
            0,
            situation(true, true),
            normal(),
            NOW,
        );
        assert!(decision.switch);
        assert_eq!(decision.failure, None);
        assert_eq!(
            decision.status,
            PlaybackStatus {
                route: Some(Route::App),
                problem: Some(Problem::Limited),
                retry_at: Some(NOW + RETRY_WITH_BACKUP),
            }
        );
    }

    #[test]
    fn without_a_backup_the_track_fails_and_waits_two_minutes() {
        for situation in [situation(false, true), situation(true, false)] {
            let decision = after_failure(&Issue::Refused, 1, situation, normal(), NOW);
            assert!(!decision.switch);
            assert!(!decision.refuse_account);
            assert_eq!(decision.status.route, None);
            assert_eq!(decision.status.retry_at, Some(NOW + RETRY_WITHOUT_BACKUP));
            assert_eq!(decision.failure.unwrap().kind, FailureKind::Other);
        }
    }

    #[test]
    fn two_refused_tracks_in_a_row_mark_the_account() {
        let decision = after_failure(&Issue::Refused, 2, situation(true, true), normal(), NOW);
        assert!(decision.switch);
        assert!(decision.refuse_account);
        assert_eq!(decision.status.problem, Some(Problem::AccountRefused));
        assert_eq!(decision.status.retry_at, None);
    }

    #[test]
    fn what_the_backup_cannot_fix_never_switches() {
        let premium = after_failure(&Issue::Premium, 0, situation(true, true), normal(), NOW);
        assert!(!premium.switch);
        assert_eq!(premium.status.problem, Some(Problem::PremiumRequired));

        let offline = after_failure(
            &Issue::Offline("no route".into()),
            0,
            situation(true, true),
            normal(),
            NOW,
        );
        assert!(!offline.switch);
        assert_eq!(offline.status, normal());
        assert_eq!(offline.failure.unwrap().kind, FailureKind::Network);

        let elsewhere = after_failure(
            &Issue::Other("not in this country".into()),
            0,
            situation(true, true),
            normal(),
            NOW,
        );
        assert!(!elsewhere.switch);
    }
}
