//! Integration tests for live streams: the engine reconnects by itself.
//!
//! The "radio" is a local HTTP server that closes each connection after a
//! short clip, like a network that keeps dropping.

mod common;

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use common::{
    IcyServer, StreamServer, WAIT, closed_port_url, noisy_ogg_clip, ogg_clip, silent_player,
};
use unisson_audio::{Engine, EngineConfig, EngineEvent, PlaybackState};
use unisson_core::StreamUri;

/// Starts an engine that retries after each of `delays`.
fn engine_retrying(delays: &[Duration]) -> (Engine, Receiver<EngineEvent>) {
    let config = EngineConfig {
        reconnect_delays: delays.to_vec(),
    };
    Engine::spawn_configured(config, || Ok(silent_player(true))).unwrap()
}

fn uri(url: &str) -> StreamUri {
    StreamUri::new(url).unwrap()
}

/// Waits for the first event accepted by `wanted`, and returns every event
/// received up to and including that one.
fn wait_for(
    events: &Receiver<EngineEvent>,
    wanted: impl Fn(&EngineEvent) -> bool,
) -> Vec<EngineEvent> {
    let deadline = Instant::now() + WAIT;
    let mut seen = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        let event = events
            .recv_timeout(remaining)
            .unwrap_or_else(|_| panic!("expected event never came, got: {seen:?}"));
        let found = wanted(&event);
        seen.push(event);
        if found {
            return seen;
        }
    }
}

fn is_failure(event: &EngineEvent) -> bool {
    matches!(event, EngineEvent::Error(_) | EngineEvent::EndOfStream)
}

const SHORT: Duration = Duration::from_millis(100);

#[test]
fn a_lost_live_stream_is_reconnected() {
    let server = StreamServer::start(ogg_clip("live-reconnect.ogg", 2), None);
    let (engine, events) = engine_retrying(&[SHORT, SHORT, SHORT]);

    engine.play_live(uri(&server.url)).unwrap();

    // The clip ends after two seconds: the engine must notice and try again.
    let seen = wait_for(&events, |event| *event == EngineEvent::Reconnected);
    assert!(
        seen.contains(&EngineEvent::StateChanged(PlaybackState::Playing)),
        "{seen:?}"
    );
    assert!(
        seen.contains(&EngineEvent::Reconnecting {
            attempt: 1,
            max_attempts: 3
        }),
        "{seen:?}"
    );
    assert!(
        !seen.iter().any(is_failure),
        "a reconnection is not a failure: {seen:?}"
    );
    assert!(
        server.connections() >= 2,
        "{} connection(s)",
        server.connections()
    );

    // The count starts over after a success: the next loss is attempt 1 again.
    let seen = wait_for(&events, |event| {
        matches!(event, EngineEvent::Reconnecting { .. })
    });
    assert_eq!(
        seen.last(),
        Some(&EngineEvent::Reconnecting {
            attempt: 1,
            max_attempts: 3
        })
    );
}

#[test]
fn the_engine_gives_up_after_the_last_attempt() {
    // The server answers once, then disappears.
    let server = StreamServer::start(ogg_clip("live-give-up.ogg", 1), Some(1));
    let (engine, events) = engine_retrying(&[SHORT, SHORT]);

    engine.play_live(uri(&server.url)).unwrap();

    let seen = wait_for(&events, |event| matches!(event, EngineEvent::Error(_)));
    let attempts: Vec<_> = seen
        .iter()
        .filter_map(|event| match event {
            EngineEvent::Reconnecting { attempt, .. } => Some(*attempt),
            _ => None,
        })
        .collect();
    assert_eq!(attempts, [1, 2], "{seen:?}");

    // Once it gave up, the engine reports that playback is stopped.
    wait_for(&events, |event| {
        *event == EngineEvent::StateChanged(PlaybackState::Stopped)
    });
}

#[test]
fn a_live_stream_that_never_played_fails_at_once() {
    let (engine, events) = engine_retrying(&[SHORT, SHORT]);

    engine.play_live(uri(&closed_port_url())).unwrap();

    let seen = wait_for(&events, |event| matches!(event, EngineEvent::Error(_)));
    assert!(
        !seen
            .iter()
            .any(|event| matches!(event, EngineEvent::Reconnecting { .. })),
        "a wrong address must not be retried: {seen:?}"
    );
}

#[test]
fn stopping_cancels_a_waiting_reconnection() {
    let server = StreamServer::start(ogg_clip("live-stop.ogg", 1), Some(1));
    // A delay much longer than the test: the attempt must never start.
    let (engine, events) = engine_retrying(&[Duration::from_secs(3600)]);
    engine.play_live(uri(&server.url)).unwrap();
    wait_for(&events, |event| {
        matches!(event, EngineEvent::Reconnecting { .. })
    });

    engine.stop().unwrap();

    wait_for(&events, |event| {
        *event == EngineEvent::StateChanged(PlaybackState::Stopped)
    });
    assert!(
        events.recv_timeout(Duration::from_millis(400)).is_err(),
        "nothing more is expected"
    );
    assert_eq!(server.connections(), 1);
}

#[test]
fn an_ordinary_address_is_never_reconnected() {
    let server = StreamServer::start(ogg_clip("not-live.ogg", 1), None);
    let (engine, events) = engine_retrying(&[SHORT, SHORT]);

    engine.play_uri(uri(&server.url)).unwrap();

    let seen = wait_for(&events, |event| *event == EngineEvent::EndOfStream);
    assert!(
        !seen
            .iter()
            .any(|event| matches!(event, EngineEvent::Reconnecting { .. }))
    );
    wait_for(&events, |event| {
        *event == EngineEvent::StateChanged(PlaybackState::Stopped)
    });
}

/// Plays a radio announcing `titles`, and returns the titles the engine
/// reported, in order.
fn titles_reported(clip_name: &str, titles: &[&str], block_size: usize) -> Vec<String> {
    let server = IcyServer::start(noisy_ogg_clip(clip_name, 4), titles, block_size);
    // No reconnection, so the stream is heard exactly once.
    let (engine, events) = engine_retrying(&[]);

    engine.play_live(uri(&server.url)).unwrap();

    wait_for(&events, |event| matches!(event, EngineEvent::Error(_)))
        .into_iter()
        .filter_map(|event| match event {
            EngineEvent::TitleChanged(title) => Some(title),
            _ => None,
        })
        .collect()
}

const TITLES: [&str; 2] = ["Miles Davis - So What", "Dave Brubeck - Take Five"];

#[test]
fn a_radio_announces_each_new_title_once() {
    // 16000 bytes between two titles is a value real radios commonly use.
    assert_eq!(titles_reported("live-titles.ogg", &TITLES, 16000), TITLES);
}

#[test]
fn the_title_on_air_when_tuning_in_is_reported_too() {
    // The first title comes after only one kilobyte, before GStreamer knows
    // the audio format. It never becomes an event: the engine looks it up.
    assert_eq!(
        titles_reported("live-early-title.ogg", &TITLES, 1024),
        TITLES
    );
}

#[test]
fn the_default_settings_retry_five_times_over_thirty_seconds() {
    let delays = EngineConfig::default().reconnect_delays;
    assert_eq!(delays.len(), 5);
    assert_eq!(delays.iter().sum::<Duration>(), Duration::from_secs(30));
}
