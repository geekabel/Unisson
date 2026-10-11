//! Integration tests of the session, with a real engine whose sound is
//! discarded, and sources replaced by a table of test files.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use unisson_audio::gst;
use unisson_audio::gst::prelude::*;
use unisson_audio::{Engine, Player};
use unisson_core::{Media, MediaId, SourceError, SourceKind, StreamUri};
use unisson_playback::{ErrorKind, QueueView, Session, SessionEvent, Snapshot, Status};

/// Longest time a test waits for something to happen.
const WAIT: Duration = Duration::from_secs(10);

fn test_folder() -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("unisson-playback-tests-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// Writes a WAV file holding `seconds` of a test tone, and returns its path.
fn make_wav(name: &str, seconds: u32) -> PathBuf {
    gst::init().unwrap();
    let path = test_folder().join(name);
    // 10 buffers of 4410 samples make one second at 44100 Hz.
    let description = format!(
        "audiotestsrc num-buffers={} samplesperbuffer=4410 \
         ! audio/x-raw,rate=44100,channels=1 ! wavenc ! filesink location=\"{}\"",
        seconds * 10,
        path.display()
    );
    let pipeline = gst::parse::launch(&description).unwrap();
    pipeline.set_state(gst::State::Playing).unwrap();
    let message = pipeline
        .bus()
        .unwrap()
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(10),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("the test file was not written in time");
    assert_eq!(message.type_(), gst::MessageType::Eos);
    pipeline.set_state(gst::State::Null).unwrap();
    path
}

fn track(key: &str) -> Media {
    Media::track(MediaId::new(SourceKind::Local, key), key)
}

/// A session over test files.
struct Fixture {
    session: Session,
    events: Receiver<SessionEvent>,
}

impl Fixture {
    /// Builds a session whose sources know `files`: a key and a path each.
    ///
    /// With `real_time`, audio is consumed at normal speed, like with a
    /// sound card. Without it, files play as fast as possible. The key
    /// `slow` takes a while to resolve, like a slow web service.
    fn new(real_time: bool, files: &[(&str, PathBuf)]) -> Self {
        gst::init().unwrap();
        let (engine, engine_events) = Engine::spawn_with(move || {
            let sink = gst::ElementFactory::make("fakesink")
                .property("sync", real_time)
                .build()
                .unwrap();
            Player::with_audio_sink(sink)
        })
        .unwrap();

        let files: HashMap<String, PathBuf> = files
            .iter()
            .map(|(key, path)| ((*key).to_owned(), path.clone()))
            .collect();
        let resolver = Arc::new(move |id: &MediaId| {
            if id.key == "slow" {
                std::thread::sleep(Duration::from_millis(500));
            }
            if id.key == "offline" {
                return Err(SourceError::Network("no route".into()));
            }
            let path = files.get(&id.key).ok_or(SourceError::NotFound)?;
            StreamUri::from_file_path(path)
        });
        let (session, events) = Session::start(engine, engine_events, resolver);
        Self { session, events }
    }

    /// Waits for the first event accepted by `wanted`, and returns every
    /// event received up to and including that one.
    fn wait_for(&self, wanted: impl Fn(&SessionEvent) -> bool) -> Vec<SessionEvent> {
        let deadline = Instant::now() + WAIT;
        let mut seen = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = self
                .events
                .recv_timeout(remaining)
                .unwrap_or_else(|_| panic!("expected event never came, got: {seen:#?}"));
            let found = wanted(&event);
            seen.push(event);
            if found {
                return seen;
            }
        }
    }

    /// Waits for a state accepted by `wanted`, and returns it.
    fn wait_for_state(&self, wanted: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let seen = self.wait_for(|event| matches!(event, SessionEvent::Changed(s) if wanted(s)));
        match seen.into_iter().last() {
            Some(SessionEvent::Changed(snapshot)) => snapshot,
            _ => unreachable!(),
        }
    }

    fn wait_for_position(&self, wanted: impl Fn(Duration) -> bool) -> Duration {
        let seen = self.wait_for(
            |event| matches!(event, SessionEvent::Position { position, .. } if wanted(*position)),
        );
        match seen.last() {
            Some(SessionEvent::Position { position, .. }) => *position,
            _ => unreachable!(),
        }
    }
}

fn playing(key: &'static str) -> impl Fn(&Snapshot) -> bool {
    move |snapshot| snapshot.status == Status::Playing && title(snapshot) == Some(key)
}

fn title(snapshot: &Snapshot) -> Option<&str> {
    snapshot.current.as_ref().map(|media| media.title.as_str())
}

fn started_titles(events: &[SessionEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Started(media) => Some(media.title.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_list_plays_from_the_chosen_media_to_the_end() {
    let fixture = Fixture::new(
        false,
        &[
            ("a", make_wav("list-a.wav", 1)),
            ("b", make_wav("list-b.wav", 1)),
            ("c", make_wav("list-c.wav", 1)),
        ],
    );
    assert_eq!(fixture.session.snapshot(), Snapshot::default());

    fixture
        .session
        .play_list(vec![track("a"), track("b"), track("c")], 1);

    // The choice shows at once, before any sound.
    let loading = fixture.wait_for_state(|_| true);
    assert_eq!(loading.status, Status::Loading);
    assert_eq!(title(&loading), Some("b"));
    assert!(loading.has_previous);
    assert!(loading.has_next);

    // "b" plays, then "c" follows by itself, then playback stops on "c".
    let seen = fixture.wait_for(|event| {
        matches!(event, SessionEvent::Changed(s)
            if s.status == Status::Stopped && title(s) == Some("c"))
    });
    assert_eq!(started_titles(&seen), ["b", "c"]);

    let end = fixture.session.snapshot();
    assert_eq!(end.status, Status::Stopped);
    assert_eq!(title(&end), Some("c"), "the last media stays selected");
    assert_eq!(end.position, Duration::ZERO);
    assert!(end.has_previous);
    assert!(!end.has_next);
    assert_eq!(end.error, None);

    // "Play" starts the last media again.
    fixture.session.toggle();
    let seen = fixture.wait_for(|event| matches!(event, SessionEvent::Started(_)));
    assert_eq!(started_titles(&seen), ["c"]);
}

#[test]
fn toggle_pauses_and_resumes() {
    let fixture = Fixture::new(true, &[("long", make_wav("toggle.wav", 10))]);
    fixture.session.play_list(vec![track("long")], 0);
    fixture.wait_for_state(playing("long"));
    let duration = fixture.wait_for(|event| matches!(event, SessionEvent::Position { .. }));
    assert!(matches!(
        duration.last(),
        Some(SessionEvent::Position { duration: Some(total), .. })
            if *total == Duration::from_secs(10)
    ));

    fixture.session.toggle();
    fixture.wait_for_state(|snapshot| snapshot.status == Status::Paused);
    let paused_at = fixture.wait_for_position(|_| true);
    assert_eq!(fixture.session.snapshot().position, paused_at);

    fixture.session.toggle();
    fixture.wait_for_state(playing("long"));
    fixture.wait_for_position(|position| position > paused_at);
}

#[test]
fn next_and_previous_walk_the_queue() {
    let fixture = Fixture::new(
        true,
        &[
            ("a", make_wav("walk-a.wav", 10)),
            ("b", make_wav("walk-b.wav", 10)),
        ],
    );
    fixture.session.play_list(vec![track("a"), track("b")], 0);
    fixture.wait_for_state(playing("a"));

    fixture.session.next();
    let on_b = fixture.wait_for_state(playing("b"));
    assert!(on_b.has_previous);
    assert!(!on_b.has_next);

    // There is nothing after "b": "next" changes nothing.
    fixture.session.next();
    // "b" has just started: "previous" goes back to "a".
    fixture.session.previous();
    let on_a = fixture.wait_for_state(playing("a"));
    assert!(!on_a.has_previous);
    assert!(on_a.has_next);
}

#[test]
fn previous_restarts_a_track_that_played_for_a_while() {
    let fixture = Fixture::new(
        true,
        &[
            ("a", make_wav("restart-a.wav", 10)),
            ("b", make_wav("restart-b.wav", 10)),
        ],
    );
    fixture.session.play_list(vec![track("a"), track("b")], 1);
    fixture.wait_for_state(playing("b"));
    fixture.wait_for_position(|position| position > Duration::from_millis(200));

    fixture.session.seek(Duration::from_secs(6));
    fixture.wait_for_position(|position| position >= Duration::from_secs(6));

    fixture.session.previous();
    fixture.wait_for_position(|position| position < Duration::from_secs(1));
    assert_eq!(title(&fixture.session.snapshot()), Some("b"));

    // Back at the start: "previous" now leaves the track.
    fixture.session.previous();
    fixture.wait_for_state(playing("a"));
}

#[test]
fn a_media_its_source_cannot_find_is_reported() {
    let fixture = Fixture::new(false, &[("good", make_wav("errors.wav", 1))]);

    fixture.session.play_list(vec![track("gone")], 0);
    let failed = fixture.wait_for_state(|snapshot| snapshot.error.is_some());
    assert_eq!(failed.status, Status::Stopped);
    assert_eq!(title(&failed), Some("gone"));
    assert_eq!(failed.error.unwrap().kind, ErrorKind::NotFound);

    fixture.session.play_list(vec![track("offline")], 0);
    let failed = fixture.wait_for_state(|snapshot| snapshot.error.is_some());
    assert_eq!(failed.error.unwrap().kind, ErrorKind::Network);

    // The next media that starts clears the error.
    fixture.session.play_list(vec![track("good")], 0);
    let playing = fixture.wait_for_state(playing("good"));
    assert_eq!(playing.error, None);
}

#[test]
fn a_file_that_is_not_audio_is_reported() {
    let text = test_folder().join("notes.wav");
    std::fs::write(&text, "this is not music").unwrap();
    let fixture = Fixture::new(false, &[("text", text)]);

    fixture.session.play_list(vec![track("text")], 0);

    let failed = fixture.wait_for_state(|snapshot| snapshot.error.is_some());
    assert_eq!(failed.status, Status::Stopped);
    assert_eq!(failed.error.unwrap().kind, ErrorKind::Unsupported);
    assert_eq!(fixture.session.snapshot().status, Status::Stopped);
}

/// Returns the titles of the media that were skipped.
fn skipped_titles(events: &[SessionEvent]) -> Vec<&str> {
    events
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Skipped { media, .. } => Some(media.title.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_media_that_cannot_start_is_skipped_when_others_wait() {
    let text = test_folder().join("skip-notes.wav");
    std::fs::write(&text, "this is not music").unwrap();
    let fixture = Fixture::new(
        false,
        &[("text", text), ("good", make_wav("skip-good.wav", 1))],
    );

    // One the source cannot find, one that is not audio, then a good one.
    fixture
        .session
        .play_list(vec![track("gone"), track("text"), track("good")], 0);

    let seen = fixture.wait_for(|event| matches!(event, SessionEvent::Started(_)));
    assert_eq!(skipped_titles(&seen), ["gone", "text"]);
    assert_eq!(started_titles(&seen), ["good"]);
    let kinds: Vec<ErrorKind> = seen
        .iter()
        .filter_map(|event| match event {
            SessionEvent::Skipped { error, .. } => Some(error.kind),
            _ => None,
        })
        .collect();
    assert_eq!(kinds, [ErrorKind::NotFound, ErrorKind::Unsupported]);
    assert_eq!(fixture.session.snapshot().error, None);
}

#[test]
fn a_network_failure_stops_instead_of_skipping() {
    let fixture = Fixture::new(false, &[("good", make_wav("skip-network.wav", 1))]);

    fixture
        .session
        .play_list(vec![track("offline"), track("good")], 0);

    let seen = fixture.wait_for(
        |event| matches!(event, SessionEvent::Changed(snapshot) if snapshot.error.is_some()),
    );
    assert!(skipped_titles(&seen).is_empty(), "{seen:#?}");
    let state = fixture.session.snapshot();
    assert_eq!(title(&state), Some("offline"));
    assert_eq!(state.status, Status::Stopped);
    // The good one still waits: the user decides.
    assert!(state.has_next);
}

#[test]
fn too_many_failures_in_a_row_stop_playback() {
    let fixture = Fixture::new(false, &[("good", make_wav("skip-many.wav", 1))]);
    let list = ["gone 1", "gone 2", "gone 3", "gone 4", "good"];

    fixture
        .session
        .play_list(list.iter().map(|key| track(key)).collect(), 0);

    let seen = fixture.wait_for(
        |event| matches!(event, SessionEvent::Changed(snapshot) if snapshot.error.is_some()),
    );
    assert_eq!(skipped_titles(&seen), ["gone 1", "gone 2", "gone 3"]);
    let state = fixture.session.snapshot();
    assert_eq!(title(&state), Some("gone 4"));
    assert_eq!(state.status, Status::Stopped);

    // The user insists: the count starts again.
    fixture.session.next();
    fixture.wait_for_state(playing("good"));
}

#[test]
fn a_late_answer_for_a_media_the_user_left_is_ignored() {
    let fixture = Fixture::new(
        true,
        &[
            ("slow", make_wav("late-slow.wav", 10)),
            ("quick", make_wav("late-quick.wav", 10)),
        ],
    );

    // "slow" is still being looked up when "quick" is chosen.
    fixture.session.play_list(vec![track("slow")], 0);
    fixture.session.play_list(vec![track("quick")], 0);
    fixture.wait_for_state(playing("quick"));

    // The answer for "slow" arrives now, and must not take over.
    let seen = fixture.wait_for(
        |event| matches!(event, SessionEvent::Position { position, .. } if *position >= Duration::from_secs(1)),
    );
    assert!(
        !started_titles(&seen).contains(&"slow"),
        "the abandoned media started: {seen:#?}"
    );
    assert_eq!(title(&fixture.session.snapshot()), Some("quick"));
    assert_eq!(fixture.session.snapshot().status, Status::Playing);
}

#[test]
fn the_end_of_a_media_the_user_left_does_not_skip_the_new_one() {
    let fixture = Fixture::new(
        false,
        &[
            ("a", make_wav("stale-a.wav", 1)),
            ("b", make_wav("stale-b.wav", 1)),
            ("c", make_wav("stale-c.wav", 1)),
        ],
    );
    // Files play instantly here, so ends and new choices cross each other.
    for _ in 0..20 {
        fixture
            .session
            .play_list(vec![track("a"), track("b"), track("c")], 0);
    }

    let seen = fixture.wait_for(|event| {
        matches!(event, SessionEvent::Changed(s)
            if s.status == Status::Stopped && title(s) == Some("c"))
    });
    // Whatever happened before, the last choice played in order to its end.
    let started = started_titles(&seen);
    assert!(started.ends_with(&["a", "b", "c"]), "{started:?}");
}

#[test]
fn play_now_keeps_the_rest_of_the_queue() {
    let fixture = Fixture::new(
        true,
        &[
            ("a", make_wav("now-a.wav", 10)),
            ("b", make_wav("now-b.wav", 10)),
            ("x", make_wav("now-x.wav", 10)),
        ],
    );
    fixture.session.play_list(vec![track("a"), track("b")], 0);
    fixture.wait_for_state(playing("a"));

    fixture.session.play_now(track("x"));
    let on_x = fixture.wait_for_state(playing("x"));
    assert!(
        on_x.has_previous,
        "the media that was playing is kept behind"
    );
    assert!(on_x.has_next, "the upcoming media still follow");

    fixture.session.next();
    fixture.wait_for_state(playing("b"));
}

#[test]
fn added_media_wait_for_their_turn() {
    let fixture = Fixture::new(
        true,
        &[
            ("a", make_wav("add-a.wav", 10)),
            ("last", make_wav("add-last.wav", 10)),
            ("soon", make_wav("add-soon.wav", 10)),
        ],
    );
    fixture.session.play_list(vec![track("a")], 0);
    fixture.wait_for_state(playing("a"));

    fixture.session.enqueue(track("last"));
    let grown = fixture.wait_for_state(|snapshot| snapshot.has_next);
    assert_eq!(title(&grown), Some("a"), "adding does not interrupt");
    assert_eq!(grown.status, Status::Playing);
    fixture.session.play_next(track("soon"));

    fixture.session.next();
    fixture.wait_for_state(playing("soon"));
    fixture.session.next();
    fixture.wait_for_state(playing("last"));
}

#[test]
fn adding_to_an_idle_player_starts_playback() {
    let fixture = Fixture::new(
        false,
        &[
            ("a", make_wav("idle-a.wav", 1)),
            ("b", make_wav("idle-b.wav", 1)),
        ],
    );
    let ended = |key: &'static str| {
        move |snapshot: &Snapshot| {
            snapshot.status == Status::Stopped && title(snapshot) == Some(key)
        }
    };

    // Nothing is selected: the first media added starts.
    fixture.session.enqueue(track("a"));
    let seen = fixture.wait_for(|event| matches!(event, SessionEvent::Started(_)));
    assert_eq!(started_titles(&seen), ["a"]);
    fixture.wait_for_state(ended("a"));

    // The queue played to its end: a media added now starts as well.
    fixture.session.play_next(track("b"));
    let seen = fixture.wait_for(|event| matches!(event, SessionEvent::Started(_)));
    assert_eq!(started_titles(&seen), ["b"]);
    fixture.wait_for_state(ended("b"));
}

fn upcoming_titles(queue: &QueueView) -> Vec<&str> {
    queue
        .upcoming
        .iter()
        .map(|media| media.title.as_str())
        .collect()
}

impl Fixture {
    /// Waits for the queue to look as `wanted` says, and returns it.
    fn wait_for_queue(&self, wanted: impl Fn(&QueueView) -> bool) -> QueueView {
        let deadline = Instant::now() + WAIT;
        loop {
            let queue = self.session.queue();
            if wanted(&queue) {
                return queue;
            }
            assert!(
                Instant::now() < deadline,
                "the queue never matched: {queue:#?}"
            );
            // Each change of the queue is announced.
            let remaining = deadline.saturating_duration_since(Instant::now());
            let _ = self.events.recv_timeout(remaining);
        }
    }
}

#[test]
fn the_queue_can_be_read_and_edited() {
    let files: Vec<(&str, PathBuf)> = ["a", "b", "c", "d", "e"]
        .into_iter()
        .map(|key| (key, make_wav(&format!("edit-{key}.wav"), 10)))
        .collect();
    let fixture = Fixture::new(true, &files);
    assert_eq!(fixture.session.queue(), QueueView::default());

    let all = ["a", "b", "c", "d", "e"].map(track).to_vec();
    fixture.session.play_list(all, 0);
    let queue = fixture.wait_for_queue(|queue| queue.upcoming.len() == 4);
    assert_eq!(
        queue.current.as_ref().map(|media| media.title.as_str()),
        Some("a")
    );
    assert_eq!(upcoming_titles(&queue), ["b", "c", "d", "e"]);

    // Move "e" to the front, then remove "c".
    fixture.session.move_item(3, 0);
    fixture.session.remove(2);
    let queue = fixture.wait_for_queue(|queue| queue.upcoming.len() == 3);
    assert_eq!(upcoming_titles(&queue), ["e", "b", "d"]);

    // Jump to "d": what was before it is skipped.
    fixture.session.jump_to(2);
    fixture.wait_for_state(playing("d"));
    let queue = fixture.wait_for_queue(|queue| queue.upcoming.is_empty());
    assert_eq!(
        queue.current.as_ref().map(|media| media.title.as_str()),
        Some("d")
    );

    // Out of range: nothing changes.
    fixture.session.jump_to(5);
    fixture.session.remove(5);
    fixture.session.move_item(0, 5);
    fixture.session.enqueue(track("a"));
    fixture.session.enqueue(track("b"));
    fixture.wait_for_queue(|queue| queue.upcoming.len() == 2);

    // Clearing keeps what plays.
    fixture.session.clear_queue();
    let queue = fixture.wait_for_queue(|queue| queue.upcoming.is_empty());
    assert_eq!(
        queue.current.as_ref().map(|media| media.title.as_str()),
        Some("d")
    );
    assert_eq!(fixture.session.snapshot().status, Status::Playing);
    assert!(!fixture.session.snapshot().has_next);
}

#[test]
fn queue_changes_are_announced_once_each() {
    let fixture = Fixture::new(true, &[("a", make_wav("announce.wav", 10))]);
    fixture.session.play_list(vec![track("a")], 0);
    fixture.wait_for_state(playing("a"));
    // Let the first position events pass.
    fixture.wait_for(|event| matches!(event, SessionEvent::Position { .. }));

    fixture.session.enqueue(track("a"));
    let seen = fixture.wait_for(|event| *event == SessionEvent::QueueChanged);
    assert_eq!(
        seen.iter()
            .filter(|event| **event == SessionEvent::QueueChanged)
            .count(),
        1
    );

    // Pausing changes the state, not the queue.
    fixture.session.pause();
    let seen = fixture.wait_for(|event| matches!(event, SessionEvent::Position { .. }));
    assert!(!seen.contains(&SessionEvent::QueueChanged), "{seen:#?}");
}

#[test]
fn the_fallback_plays_when_the_queue_runs_out() {
    let fixture = Fixture::new(
        false,
        &[
            ("a", make_wav("fallback-a.wav", 1)),
            ("b", make_wav("fallback-b.wav", 1)),
            ("radio", make_wav("fallback-radio.wav", 1)),
        ],
    );
    fixture.session.set_fallback(Some(track("radio")));
    let queue = fixture.wait_for_queue(|queue| queue.fallback.is_some());
    assert_eq!(queue.fallback.unwrap().title, "radio");

    fixture.session.play_list(vec![track("a"), track("b")], 0);

    // "a", then "b", then the fallback, with no order from outside.
    let seen = fixture
        .wait_for(|event| matches!(event, SessionEvent::Started(media) if media.title == "radio"));
    assert_eq!(started_titles(&seen), ["a", "b", "radio"]);
    let snapshot = fixture.session.snapshot();
    assert_eq!(title(&snapshot), Some("radio"));
    assert!(snapshot.has_previous, "what played before is kept behind");

    // Without a fallback, playback stops at the end of the queue.
    fixture.session.set_fallback(None);
    fixture.wait_for_queue(|queue| queue.fallback.is_none());
    fixture.session.play_list(vec![track("a")], 0);
    let end = fixture.wait_for_state(|snapshot| {
        snapshot.status == Status::Stopped && title(snapshot) == Some("a")
    });
    assert_eq!(end.error, None);
}

#[test]
fn the_volume_is_reported() {
    let fixture = Fixture::new(false, &[]);
    fixture.session.set_volume(0.25);
    let changed = fixture.wait_for_state(|_| true);
    assert_eq!(changed.volume, 0.25);
    fixture.session.set_volume(7.0);
    assert_eq!(fixture.wait_for_state(|_| true).volume, 1.0);
}

#[test]
fn orders_without_a_media_do_nothing() {
    let fixture = Fixture::new(false, &[]);
    fixture.session.toggle();
    fixture.session.next();
    fixture.session.previous();
    fixture.session.seek(Duration::from_secs(3));
    fixture.session.play_list(Vec::new(), 0);
    assert!(
        fixture
            .events
            .recv_timeout(Duration::from_millis(300))
            .is_err()
    );
    assert_eq!(fixture.session.snapshot(), Snapshot::default());
}
