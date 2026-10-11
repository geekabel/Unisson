//! Integration tests against a stand-in for yt-dlp.
//!
//! The real program needs YouTube. These tests replace it with a small
//! shell script that answers like it does, so they check everything around
//! the program: how it is called, how its answers are read, and what
//! happens when it is slow, fails, or is cancelled.

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use unisson_core::{MediaId, SearchQuery, Source, SourceError, SourceKind};
use unisson_youtube::{YouTubeSource, YtDlp};

/// The script. It notes each call in a file next to itself.
const SCRIPT: &str = r#"#!/bin/sh
echo "$*" >> "$(dirname "$0")/calls.log"
for argument in "$@"; do last="$argument"; done

case "$*" in
  *--version*) echo "2026.08.19"; exit 0 ;;
esac

case "$last" in
  ytsearch*:slow*)
    sleep 5
    echo '{"id": "late", "title": "Too late"}' ;;
  ytsearch*:offline*)
    echo "WARNING: retrying" >&2
    echo "ERROR: Unable to download API page: timed out" >&2
    exit 1 ;;
  ytsearch*:nothing*) ;;
  ytsearch*)
    echo '{"id": "abc123", "title": "Miles Davis - So What (Official Audio)", "duration": 562, "channel": "MilesDavisVEVO", "ie_key": "Youtube"}'
    echo '{"id": "def456", "title": "Take Five", "duration": 324, "channel": "Dave Brubeck - Topic", "ie_key": "Youtube"}' ;;
  *watch?v=gone)
    echo "ERROR: [youtube] gone: Video unavailable" >&2
    exit 1 ;;
  *playlist?list=*)
    echo '{"id": "one", "title": "First", "duration": 60, "channel": "Someone", "ie_key": "Youtube"}'
    echo '{"id": "x", "title": "[Deleted video]", "ie_key": "Youtube"}'
    echo '{"id": "two", "title": "Second", "duration": 61, "channel": "Someone", "ie_key": "Youtube"}' ;;
  *empty-channel*) ;;
  *)
    case "$*" in
      *"--print urls"*) echo "https://audio.example.org/stream?video=${last##*=}" ;;
      *) echo '{"id": "abc123", "title": "So What", "duration": 562.4, "channel": "Miles Davis - Topic"}' ;;
    esac ;;
esac
"#;

/// Writes the script once, and returns its path.
///
/// Linux refuses to run a file that some process still has open for
/// writing. Tests run on several threads, and starting a program briefly
/// copies the open files of the whole process: a script written by one
/// test while another starts a program could not be run. Writing the
/// script once, before any test starts a program, avoids that.
fn shared_script() -> &'static PathBuf {
    static SCRIPT_PATH: OnceLock<PathBuf> = OnceLock::new();
    SCRIPT_PATH.get_or_init(|| {
        let folder =
            std::env::temp_dir().join(format!("unisson-youtube-tests-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join("yt-dlp-script");
        std::fs::write(&path, SCRIPT).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    })
}

/// A stand-in program, with the list of the calls it received.
struct Fake {
    program: PathBuf,
    log: PathBuf,
}

impl Fake {
    fn new(name: &str) -> Self {
        let folder = shared_script().parent().unwrap().join(name);
        std::fs::create_dir_all(&folder).unwrap();
        let (program, log) = (folder.join("yt-dlp"), folder.join("calls.log"));
        // Each test gets its own name for the one script, so each has its
        // own list of calls.
        std::fs::hard_link(shared_script(), &program).unwrap();
        Self { program, log }
    }

    fn tool(&self) -> YtDlp {
        YtDlp::with_program(&self.program)
    }

    fn source(&self) -> YouTubeSource {
        YouTubeSource::with_tool(self.tool())
    }

    /// Returns the arguments of each call, one call per line.
    fn calls(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }
}

fn video(key: &str) -> MediaId {
    MediaId::new(SourceKind::YouTube, key)
}

#[test]
fn the_version_is_read() {
    let fake = Fake::new("version");
    assert_eq!(fake.tool().version().as_deref(), Ok("2026.08.19"));
}

#[test]
fn a_search_asks_for_the_right_number_of_videos_and_reads_them() {
    let fake = Fake::new("search");
    let source = fake.source();

    let found = source
        .search(&SearchQuery::new("miles davis").with_limit(7))
        .unwrap();

    let titles: Vec<(&str, Option<&str>)> = found
        .iter()
        .map(|media| (media.title.as_str(), media.artist.as_deref()))
        .collect();
    assert_eq!(
        titles,
        [
            ("So What", Some("Miles Davis")),
            ("Take Five", Some("Dave Brubeck"))
        ]
    );
    assert!(
        found
            .iter()
            .all(|media| media.id.source == SourceKind::YouTube)
    );
    assert_eq!(found[0].duration, Some(Duration::from_secs(562)));

    let calls = fake.calls();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].ends_with("-- ytsearch7:miles davis"), "{calls:?}");
    assert!(calls[0].contains("--flat-playlist"), "{calls:?}");
}

#[test]
fn a_text_that_looks_like_an_option_is_still_searched_as_text() {
    let fake = Fake::new("dash");
    fake.source()
        .search(&SearchQuery::new("--version"))
        .unwrap();
    // The text comes after "--", so yt-dlp does not read it as an option.
    assert!(
        fake.calls()[0].contains("-- ytsearch"),
        "{:?}",
        fake.calls()
    );
}

#[test]
fn an_empty_search_starts_nothing_and_no_result_is_not_an_error() {
    let fake = Fake::new("empty");
    let source = fake.source();
    assert_eq!(source.search(&SearchQuery::new("   ")), Ok(Vec::new()));
    assert!(fake.calls().is_empty());
    assert_eq!(
        source.search(&SearchQuery::new("nothing here")),
        Ok(Vec::new())
    );
}

#[test]
fn a_network_failure_is_told_apart() {
    let fake = Fake::new("offline");
    let result = fake.source().search(&SearchQuery::new("offline"));
    assert!(
        matches!(&result, Err(SourceError::Network(message)) if message.contains("Unable to download")),
        "{result:?}"
    );
}

#[test]
fn a_run_that_takes_too_long_is_stopped() {
    let fake = Fake::new("timeout");
    let source = YouTubeSource::with_tool(fake.tool().with_timeout(Duration::from_millis(300)));

    let start = Instant::now();
    let result = source.search(&SearchQuery::new("slow"));

    assert!(matches!(result, Err(SourceError::Network(_))), "{result:?}");
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "{:?}",
        start.elapsed()
    );
}

#[test]
fn a_newer_search_cancels_the_one_in_progress() {
    let fake = Fake::new("cancel");
    let source = Arc::new(fake.source());

    let start = Instant::now();
    let slow = {
        let source = Arc::clone(&source);
        std::thread::spawn(move || source.search(&SearchQuery::new("slow")))
    };
    // Let the slow search start, then ask for something else.
    std::thread::sleep(Duration::from_millis(300));
    let quick = source.search(&SearchQuery::new("miles")).unwrap();

    assert_eq!(quick.len(), 2);
    let cancelled = slow.join().unwrap();
    assert!(
        matches!(cancelled, Err(SourceError::Unavailable(_))),
        "{cancelled:?}"
    );
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "the slow search was not stopped"
    );
}

#[test]
fn the_audio_address_of_a_video_is_asked_once_then_reused() {
    let fake = Fake::new("resolve");
    let source = fake.source();

    let address = source.resolve(&video("abc123")).unwrap();
    assert_eq!(
        address.as_str(),
        "https://audio.example.org/stream?video=abc123"
    );
    assert_eq!(source.resolve(&video("abc123")).unwrap(), address);
    source.resolve(&video("def456")).unwrap();

    let calls = fake.calls();
    assert_eq!(calls.len(), 2, "{calls:?}");
    assert!(calls[0].contains("--format bestaudio/best"), "{calls:?}");
    assert!(calls[0].contains("--print urls"), "{calls:?}");
    assert!(
        calls[0].ends_with("-- https://www.youtube.com/watch?v=abc123"),
        "{calls:?}"
    );
}

#[test]
fn a_removed_video_and_a_media_of_another_source_are_refused() {
    let fake = Fake::new("refused");
    let source = fake.source();
    assert_eq!(source.resolve(&video("gone")), Err(SourceError::NotFound));
    let local = MediaId::new(SourceKind::Local, "/m/a.mp3");
    assert!(matches!(
        source.resolve(&local),
        Err(SourceError::Invalid(_))
    ));
    // A failure is not remembered: the next try asks again.
    assert_eq!(source.resolve(&video("gone")), Err(SourceError::NotFound));
    assert_eq!(fake.calls().len(), 2);
}

#[test]
fn a_link_gives_one_video_or_the_videos_of_a_playlist() {
    let fake = Fake::new("lookup");
    let tool = fake.tool();

    let single = tool.lookup("https://youtu.be/abc123").unwrap();
    assert_eq!(single.len(), 1);
    assert_eq!(single[0].title, "So What");
    assert_eq!(single[0].artist.as_deref(), Some("Miles Davis"));
    assert_eq!(single[0].duration, Some(Duration::from_millis(562_400)));

    let playlist = tool
        .lookup("https://www.youtube.com/playlist?list=PL123")
        .unwrap();
    let titles: Vec<&str> = playlist.iter().map(|media| media.title.as_str()).collect();
    assert_eq!(titles, ["First", "Second"], "the deleted video is skipped");

    assert_eq!(
        tool.lookup("https://www.youtube.com/@empty-channel"),
        Err(SourceError::NotFound)
    );
    assert_eq!(
        tool.lookup("https://www.youtube.com/watch?v=gone"),
        Err(SourceError::NotFound)
    );
    assert!(
        fake.calls()[0].contains("--no-playlist"),
        "{:?}",
        fake.calls()
    );
}
