//! Integration tests for the engine (the player on its own thread).

mod common;

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use common::{WAIT, make_wav, silent_player};
use unisson_audio::{AudioError, Engine, EngineEvent, FailureKind, PlaybackState, file_uri};
use unisson_core::StreamUri;

fn silent_engine(real_time: bool) -> (Engine, Receiver<EngineEvent>) {
    Engine::spawn_with(move || Ok(silent_player(real_time))).unwrap()
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

fn state(state: PlaybackState) -> impl Fn(&EngineEvent) -> bool {
    move |event| *event == EngineEvent::StateChanged(state)
}

fn position_of(event: &EngineEvent) -> Option<Duration> {
    match event {
        EngineEvent::Position { position, .. } => Some(*position),
        _ => None,
    }
}

#[test]
fn plays_to_the_end_and_reports_each_step() {
    let (engine, events) = silent_engine(false);
    engine
        .play_uri(file_uri(make_wav("engine-end.wav", 2)).unwrap())
        .unwrap();

    let seen = wait_for(&events, state(PlaybackState::Stopped));

    let playing = seen.iter().position(state(PlaybackState::Playing));
    let ended = seen
        .iter()
        .position(|event| *event == EngineEvent::EndOfStream);
    assert!(playing.is_some(), "{seen:?}");
    assert!(
        playing < ended,
        "the end must come after the start: {seen:?}"
    );
    assert_eq!(
        seen.last(),
        Some(&EngineEvent::StateChanged(PlaybackState::Stopped))
    );
}

#[test]
fn reports_positions_while_playing_and_none_while_paused() {
    let (engine, events) = silent_engine(true);
    engine
        .play_uri(file_uri(make_wav("engine-pause.wav", 5)).unwrap())
        .unwrap();

    // Positions arrive while playing, and they carry the duration.
    let seen = wait_for(&events, |event| {
        position_of(event).is_some_and(|at| at >= Duration::from_millis(300))
    });
    assert!(
        seen.contains(&EngineEvent::StateChanged(PlaybackState::Playing)),
        "{seen:?}"
    );
    assert!(matches!(
        seen.last(),
        Some(EngineEvent::Position { duration: Some(total), .. }) if *total == Duration::from_secs(5)
    ));

    // Pausing reports the state, then the position where playback stopped.
    engine.pause().unwrap();
    wait_for(&events, state(PlaybackState::Paused));
    let paused_at = position_of(&wait_for(&events, |event| position_of(event).is_some())[0]);
    assert!(paused_at.is_some());
    assert!(
        events.recv_timeout(Duration::from_millis(600)).is_err(),
        "no event is expected while paused"
    );

    // Resuming carries on from there.
    engine.play().unwrap();
    wait_for(&events, state(PlaybackState::Playing));
    wait_for(&events, |event| position_of(event) > paused_at);

    engine.stop().unwrap();
    wait_for(&events, state(PlaybackState::Stopped));
}

#[test]
fn seeking_jumps_to_the_requested_position() {
    let (engine, events) = silent_engine(true);
    engine
        .play_uri(file_uri(make_wav("engine-seek.wav", 10)).unwrap())
        .unwrap();
    wait_for(&events, |event| {
        position_of(event).is_some_and(|at| at >= Duration::from_millis(300))
    });

    // The engine answers at once with the requested position.
    let target = Duration::from_secs(8);
    engine.seek(target).unwrap();
    let seen = wait_for(&events, |event| position_of(event) >= Some(target));
    assert_eq!(position_of(seen.last().unwrap()), Some(target));

    // Playback then goes on from there: the end comes after 2 seconds, not 10.
    let start = Instant::now();
    let seen = wait_for(&events, |event| *event == EngineEvent::EndOfStream);
    assert!(start.elapsed() < Duration::from_secs(5), "{seen:?}");
    let before_target = seen
        .iter()
        .filter_map(position_of)
        .filter(|position| *position + Duration::from_millis(500) < target)
        .count();
    assert_eq!(before_target, 0, "{seen:?}");
}

#[test]
fn seeking_while_stopped_is_an_error_event() {
    let (engine, events) = silent_engine(false);
    engine
        .load(file_uri(make_wav("engine-seek-stopped.wav", 1)).unwrap())
        .unwrap();
    engine.seek(Duration::from_secs(1)).unwrap();
    let seen = wait_for(&events, |event| matches!(event, EngineEvent::Error(_)));
    assert!(
        matches!(seen.last(), Some(EngineEvent::Error(failure)) if failure.message.contains("position")),
        "{seen:?}"
    );
}

#[test]
fn reports_the_volume_actually_applied() {
    let (engine, events) = silent_engine(false);
    engine.set_volume(0.4).unwrap();
    engine.set_volume(3.0).unwrap();
    assert_eq!(
        events.recv_timeout(WAIT),
        Ok(EngineEvent::VolumeChanged(0.4))
    );
    assert_eq!(
        events.recv_timeout(WAIT),
        Ok(EngineEvent::VolumeChanged(1.0))
    );
}

#[test]
fn a_failed_command_becomes_an_error_event() {
    let (engine, events) = silent_engine(false);

    engine.play().unwrap();
    let nothing = AudioError::NothingLoaded.failure();
    assert_eq!(events.recv_timeout(WAIT), Ok(EngineEvent::Error(nothing)));

    engine
        .play_uri(StreamUri::new("file:///nonexistent/unisson/nothing.wav").unwrap())
        .unwrap();
    wait_for(&events, |event| matches!(event, EngineEvent::Error(_)));

    // The engine survives errors and still obeys.
    engine
        .play_uri(file_uri(make_wav("engine-after-error.wav", 1)).unwrap())
        .unwrap();
    wait_for(&events, |event| *event == EngineEvent::EndOfStream);
}

#[test]
fn a_new_load_replaces_the_current_media() {
    let (engine, events) = silent_engine(true);
    engine
        .play_uri(file_uri(make_wav("engine-first.wav", 5)).unwrap())
        .unwrap();
    wait_for(&events, |event| position_of(event).is_some());

    engine
        .play_uri(file_uri(make_wav("engine-second.wav", 1)).unwrap())
        .unwrap();

    // The second file is one second long: reaching its end proves the switch.
    let seen = wait_for(&events, |event| *event == EngineEvent::EndOfStream);
    assert!(seen.iter().any(|event| matches!(
        event,
        EngineEvent::Position { duration: Some(total), .. } if *total == Duration::from_secs(1)
    )));
}

#[test]
fn dropping_the_engine_stops_the_thread_and_closes_the_events() {
    let (engine, events) = silent_engine(true);
    engine
        .play_uri(file_uri(make_wav("engine-drop.wav", 5)).unwrap())
        .unwrap();
    wait_for(&events, state(PlaybackState::Playing));

    drop(engine);

    // Once the thread is gone, the channel is closed and iteration ends.
    assert!(events.iter().count() < 50);
}

#[test]
fn a_player_that_cannot_be_built_is_reported_at_startup() {
    let result = Engine::spawn_with(|| Err(AudioError::MissingElement("playbin")));
    assert_eq!(result.err(), Some(AudioError::MissingElement("playbin")));
}

/// Plays an address that fails, and returns the kind of the failure.
fn failure_kind_of(uri: StreamUri) -> FailureKind {
    let (engine, events) = silent_engine(false);
    engine.play_uri(uri).unwrap();
    let seen = wait_for(&events, |event| matches!(event, EngineEvent::Error(_)));
    match seen.last() {
        Some(EngineEvent::Error(failure)) => failure.kind,
        _ => unreachable!("wait_for stops on an error"),
    }
}

#[test]
fn a_missing_file_fails_as_not_found() {
    let uri = StreamUri::new("file:///nonexistent/unisson/nothing.wav").unwrap();
    assert_eq!(failure_kind_of(uri), FailureKind::NotFound);
}

#[test]
fn a_file_that_is_not_audio_fails_as_unsupported() {
    let path = std::env::temp_dir().join(format!("unisson-not-audio-{}.mp3", std::process::id()));
    std::fs::write(&path, "These are words, not sound. ".repeat(200)).unwrap();
    let kind = failure_kind_of(file_uri(&path).unwrap());
    let _ = std::fs::remove_file(&path);
    assert_eq!(kind, FailureKind::Unsupported);
}

#[test]
fn a_server_that_refuses_the_connection_fails_as_network() {
    // Nothing listens on this port: find a free one, then let go of it.
    let port = {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap().port()
    };
    let uri = StreamUri::new(format!("http://127.0.0.1:{port}/stream")).unwrap();
    assert_eq!(failure_kind_of(uri), FailureKind::Network);
}
