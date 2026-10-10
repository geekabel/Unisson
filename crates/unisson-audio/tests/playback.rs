//! Integration tests for the player.
//!
//! They need no sound card: audio goes to a GStreamer `fakesink`, and the
//! test files are generated on the fly with `audiotestsrc`.

mod common;

use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

use common::{WAIT, make_wav, ogg_file, silent_player};
use unisson_audio::{AudioError, PlaybackState, Player, PlayerEvent, file_uri};
use unisson_core::StreamUri;

fn load(player: &Player, path: &Path) {
    player.load(&file_uri(path).unwrap()).unwrap();
}

/// Waits until the position is known and at least `minimum`.
fn wait_for_position(player: &Player, minimum: Duration) -> Duration {
    let start = Instant::now();
    loop {
        if let Some(position) = player.position().filter(|position| *position >= minimum) {
            return position;
        }
        assert!(start.elapsed() < WAIT, "position never reached {minimum:?}");
        sleep(Duration::from_millis(20));
    }
}

#[test]
fn plays_a_file_to_its_end() {
    let player = silent_player(false);
    load(&player, &make_wav("to-the-end.wav", 2));
    assert_eq!(player.state(), PlaybackState::Stopped);

    player.play().unwrap();
    assert_eq!(player.state(), PlaybackState::Playing);

    assert_eq!(player.poll_event(WAIT), Some(PlayerEvent::EndOfStream));
    assert_eq!(player.state(), PlaybackState::Stopped);
}

#[test]
fn pauses_resumes_and_stops() {
    let player = silent_player(true);
    load(&player, &make_wav("pause-resume.wav", 5));

    player.play().unwrap();
    wait_for_position(&player, Duration::from_millis(200));
    assert_eq!(player.duration(), Some(Duration::from_secs(5)));

    player.pause().unwrap();
    assert_eq!(player.state(), PlaybackState::Paused);
    let paused_at = wait_for_position(&player, Duration::from_millis(200));
    sleep(Duration::from_millis(300));
    let still_at = player.position().unwrap();
    assert!(
        still_at.abs_diff(paused_at) < Duration::from_millis(50),
        "the position moved while paused: {paused_at:?} then {still_at:?}"
    );

    player.play().unwrap();
    assert_eq!(player.state(), PlaybackState::Playing);
    wait_for_position(&player, paused_at + Duration::from_millis(200));

    player.stop().unwrap();
    assert_eq!(player.state(), PlaybackState::Stopped);
    assert_eq!(player.poll_event(Duration::from_millis(100)), None);
}

#[test]
fn reports_the_title_stored_in_a_file() {
    let player = silent_player(false);
    load(&player, &ogg_file("titled.ogg", 1, Some("Blue in Green")));
    player.play().unwrap();

    assert_eq!(
        player.poll_event(WAIT),
        Some(PlayerEvent::Title("Blue in Green".into()))
    );
    // The title does not stop playback: the end still comes afterwards.
    let mut event = player.poll_event(WAIT);
    while matches!(event, Some(PlayerEvent::Title(_))) {
        event = player.poll_event(WAIT);
    }
    assert_eq!(event, Some(PlayerEvent::EndOfStream));
}

#[test]
fn a_zero_timeout_checks_without_waiting() {
    let player = silent_player(false);
    load(&player, &make_wav("zero-timeout.wav", 1));
    assert_eq!(player.poll_event(Duration::ZERO), None);

    player.play().unwrap();
    let start = Instant::now();
    while player.poll_event(Duration::ZERO).is_none() {
        assert!(
            start.elapsed() < WAIT,
            "the end of the file was never reported"
        );
        sleep(Duration::from_millis(20));
    }
    assert_eq!(player.state(), PlaybackState::Stopped);
}

#[test]
fn restarts_from_the_beginning_after_a_stop() {
    let player = silent_player(false);
    load(&player, &make_wav("restart.wav", 1));

    player.play().unwrap();
    assert_eq!(player.poll_event(WAIT), Some(PlayerEvent::EndOfStream));
    player.play().unwrap();
    assert_eq!(player.poll_event(WAIT), Some(PlayerEvent::EndOfStream));
}

#[test]
fn volume_is_clamped_between_zero_and_one() {
    let player = silent_player(false);
    assert_eq!(player.volume(), 1.0);
    player.set_volume(0.25);
    assert_eq!(player.volume(), 0.25);
    player.set_volume(7.0);
    assert_eq!(player.volume(), 1.0);
    player.set_volume(-3.0);
    assert_eq!(player.volume(), 0.0);
    player.set_volume(f64::NAN);
    assert_eq!(player.volume(), 0.0);
}

#[test]
fn commands_need_a_loaded_address() {
    let player = silent_player(false);
    assert_eq!(player.play(), Err(AudioError::NothingLoaded));
    assert_eq!(player.pause(), Err(AudioError::NothingLoaded));
    assert_eq!(player.stop(), Ok(()));
}

#[test]
fn a_missing_file_is_reported_as_an_error_event() {
    let player = silent_player(false);
    player
        .load(&StreamUri::new("file:///nonexistent/unisson/nothing.wav").unwrap())
        .unwrap();
    // Starting may fail right away or later on the bus, depending on GStreamer.
    match player.play() {
        Ok(()) => assert!(matches!(
            player.poll_event(WAIT),
            Some(PlayerEvent::Error(_))
        )),
        Err(error) => assert!(matches!(error, AudioError::StateChange(_)), "{error}"),
    }
    assert_eq!(player.state(), PlaybackState::Stopped);
}

#[test]
fn file_uri_escapes_and_checks_the_path() {
    let path = make_wav("with space.wav", 1);
    let uri = file_uri(&path).unwrap();
    assert!(uri.as_str().starts_with("file:///"), "{uri}");
    assert!(uri.as_str().ends_with("/with%20space.wav"), "{uri}");

    let missing = file_uri("/nonexistent/unisson/nothing.wav");
    assert!(matches!(missing, Err(AudioError::InvalidPath(_))));
}
