//! Running the yt-dlp program and reading its answers.
//!
//! # Design
//!
//! yt-dlp is a command-line program. Each question is one run of it:
//!
//! - a search prints one line of JSON per video found;
//! - asking for the audio address of a video prints that address.
//!
//! A run can be slow, since it talks to YouTube, and it can hang if the
//! network does. Every run therefore has a time limit, after which the
//! program is stopped. A run can also be cancelled from another thread:
//! when the user types one more letter, the search for the previous text is
//! of no use any more.
//!
//! The options of yt-dlp used here are documented in its `--help`:
//! `--flat-playlist`, `--no-playlist`, `-j/--dump-json`, `-f/--format`,
//! `-O/--print`, `--no-warnings`, `--socket-timeout`, and the `ytsearchN:`
//! prefix of the YouTube search.

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use serde::Deserialize;
use unisson_core::{Media, MediaId, SourceError, SourceKind, StreamUri};

use crate::title::split_title;
use crate::url::watch_url;

/// How often a run is checked for its end.
const POLL_INTERVAL: Duration = Duration::from_millis(15);

/// The yt-dlp program, ready to be asked.
///
/// The settings yt-dlp reads from its own configuration files still apply:
/// that is where a user sets cookies or a proxy, when YouTube asks for them.
#[derive(Debug, Clone)]
pub struct YtDlp {
    program: PathBuf,
    timeout: Duration,
}

impl Default for YtDlp {
    fn default() -> Self {
        Self::new()
    }
}

impl YtDlp {
    /// The name of the program, looked up in the `PATH`.
    pub const PROGRAM: &'static str = "yt-dlp";

    /// The longest a run may take by default.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

    /// Uses the `yt-dlp` program found in the `PATH`.
    pub fn new() -> Self {
        Self::with_program(Self::PROGRAM)
    }

    /// Uses the program at the given place. This is mostly useful in tests,
    /// to stand a script in for the real program.
    pub fn with_program(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            timeout: Self::DEFAULT_TIMEOUT,
        }
    }

    /// Sets the longest a run may take.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Returns the version of the program, such as `2026.08.19`.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the program is not installed
    /// or does not answer.
    pub fn version(&self) -> Result<String, SourceError> {
        let output = self.start(&["--version"])?.finish(self.timeout)?;
        let version = output.trim();
        if version.is_empty() {
            return Err(SourceError::Unavailable(
                "the YouTube helper gave no version".into(),
            ));
        }
        Ok(version.to_owned())
    }

    /// Starts a search of YouTube for `text`, asking for at most `limit`
    /// videos. The answer is read with [`read_search`](Self::read_search).
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::Unavailable`] if the program cannot be started.
    pub(crate) fn start_search(&self, text: &str, limit: usize) -> Result<Arc<Run>, SourceError> {
        // "ytsearch5:words" asks the YouTube search for 5 results.
        let target = format!("ytsearch{}:{}", limit.max(1), text);
        self.start(&[
            // One entry per result, without opening each video: much faster.
            "--flat-playlist",
            "--dump-json",
            "--no-warnings",
            "--socket-timeout",
            "10",
            // What follows is never an option, even if it starts with a dash.
            "--",
            &target,
        ])
    }

    /// Waits for a search started with [`start_search`](Self::start_search),
    /// and returns the videos found.
    ///
    /// # Errors
    ///
    /// Returns a [`SourceError`] if the search failed, took too long, or was
    /// cancelled.
    pub(crate) fn read_search(&self, run: &Run) -> Result<Vec<Media>, SourceError> {
        Ok(parse_entries(&run.finish(self.timeout)?))
    }

    /// Returns what a YouTube link points to: one video, or the videos of a
    /// playlist. A link to a video inside a playlist gives that video only.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::NotFound`] if the link points to nothing that
    /// can be played, or another [`SourceError`] if yt-dlp fails.
    pub fn lookup(&self, url: &str) -> Result<Vec<Media>, SourceError> {
        let run = self.start(&[
            "--flat-playlist",
            "--no-playlist",
            "--dump-json",
            "--no-warnings",
            "--socket-timeout",
            "10",
            "--",
            url,
        ])?;
        let media = parse_entries(&run.finish(self.timeout)?);
        if media.is_empty() {
            return Err(SourceError::NotFound);
        }
        Ok(media)
    }

    /// Returns the address of the audio stream of a video.
    ///
    /// The address only works for a few hours: ask for it right before
    /// playing.
    ///
    /// # Errors
    ///
    /// Returns [`SourceError::NotFound`] if the video does not exist any
    /// more, or another [`SourceError`] if yt-dlp fails.
    pub fn stream_uri(&self, video_id: &str) -> Result<StreamUri, SourceError> {
        let page = watch_url(video_id);
        let run = self.start(&[
            // The best audio-only stream, or the best stream of any kind
            // when the video has none (live streams, for instance).
            "--format",
            "bestaudio/best",
            "--no-playlist",
            "--print",
            "urls",
            "--no-warnings",
            "--socket-timeout",
            "10",
            "--",
            &page,
        ])?;
        let output = run.finish(self.timeout)?;
        let address = output
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .ok_or(SourceError::NotFound)?;
        StreamUri::new(address)
    }

    /// Starts the program with some arguments.
    fn start(&self, arguments: &[&str]) -> Result<Arc<Run>, SourceError> {
        let mut child = Command::new(&self.program)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => {
                    SourceError::Unavailable("the YouTube helper is not installed".into())
                }
                _ => SourceError::Unavailable(format!("cannot start the YouTube helper: {error}")),
            })?;
        // Read both outputs while the program runs: a program that fills
        // one of them and is not read would wait for ever.
        let stdout = read_in_background(child.stdout.take());
        let stderr = read_in_background(child.stderr.take());
        Ok(Arc::new(Run {
            child: Mutex::new(child),
            cancelled: AtomicBool::new(false),
            outputs: Mutex::new(Some((stdout, stderr))),
        }))
    }
}

/// One run of the program, in progress or over.
#[derive(Debug)]
pub(crate) struct Run {
    child: Mutex<Child>,
    cancelled: AtomicBool,
    /// The threads that read the standard output and the error output.
    outputs: Mutex<Option<(JoinHandle<String>, JoinHandle<String>)>>,
}

impl Run {
    /// Stops the program. The thread that waits for its answer gets an
    /// error at once.
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        // An error means the program already ended: nothing to stop.
        let _ = lock(&self.child).kill();
    }

    /// Waits for the end of the program, and returns what it printed.
    fn finish(&self, timeout: Duration) -> Result<String, SourceError> {
        let deadline = Instant::now() + timeout;
        let status = loop {
            let ended = lock(&self.child).try_wait().map_err(|error| {
                SourceError::Unavailable(format!("the YouTube helper was lost: {error}"))
            })?;
            if let Some(status) = ended {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = lock(&self.child).kill();
                // Reap the stopped program, so it does not linger.
                let _ = lock(&self.child).wait();
                return Err(SourceError::Network(
                    "the YouTube helper did not answer in time".into(),
                ));
            }
            thread::sleep(POLL_INTERVAL);
        };

        // A stopped program may leave helpers behind that keep its outputs
        // open for a while: do not wait for them.
        if self.cancelled.load(Ordering::SeqCst) {
            return Err(SourceError::Unavailable("the request was cancelled".into()));
        }
        let (stdout, stderr) = match lock(&self.outputs).take() {
            Some((stdout, stderr)) => (
                stdout.join().unwrap_or_default(),
                stderr.join().unwrap_or_default(),
            ),
            None => Default::default(),
        };
        if status.success() {
            Ok(stdout)
        } else {
            Err(classify_failure(&stderr))
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The value is still usable if a thread panicked while holding it.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Reads a whole output of the program on another thread.
fn read_in_background(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut bytes);
        }
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// Turns the error text of yt-dlp into an error of the right kind.
fn classify_failure(stderr: &str) -> SourceError {
    // yt-dlp starts its error lines with "ERROR:". Keep the first one.
    let message = stderr
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("ERROR:"))
        .or_else(|| stderr.lines().map(str::trim).find(|line| !line.is_empty()))
        .unwrap_or("the YouTube helper failed without a message");
    let message: String = message.chars().take(300).collect();
    let lower = message.to_lowercase();

    let gone = [
        "video unavailable",
        "private video",
        "has been removed",
        "does not exist",
        "is not available",
        "this video is unavailable",
    ];
    let network = [
        "unable to download",
        "unable to connect",
        "timed out",
        "connection",
        "name resolution",
        "network is unreachable",
        "proxy",
    ];
    if gone.iter().any(|sign| lower.contains(sign)) {
        SourceError::NotFound
    } else if network.iter().any(|sign| lower.contains(sign)) {
        SourceError::Network(message)
    } else {
        SourceError::Unavailable(message)
    }
}

/// One video, as yt-dlp describes it in JSON. Only what is used is read.
#[derive(Debug, Deserialize)]
struct Entry {
    id: Option<String>,
    title: Option<String>,
    /// In seconds. Missing for live streams.
    duration: Option<f64>,
    channel: Option<String>,
    uploader: Option<String>,
    live_status: Option<String>,
    is_live: Option<bool>,
    /// The part of yt-dlp that handles the entry: `Youtube` for a video.
    ie_key: Option<String>,
}

/// Reads the lines of JSON printed by yt-dlp, one video per line.
///
/// Lines that are not a playable video are skipped: a channel or a nested
/// playlist in the results, a deleted video in a playlist, a line that is
/// not JSON.
fn parse_entries(output: &str) -> Vec<Media> {
    output
        .lines()
        .filter_map(|line| serde_json::from_str::<Entry>(line).ok())
        .filter_map(media_of)
        .collect()
}

fn media_of(entry: Entry) -> Option<Media> {
    if entry.ie_key.as_deref().is_some_and(|key| key != "Youtube") {
        return None;
    }
    let id = entry.id.filter(|id| !id.trim().is_empty())?;
    let raw_title = entry.title.filter(|title| !title.trim().is_empty())?;
    if raw_title == "[Deleted video]" || raw_title == "[Private video]" {
        return None;
    }
    let channel = entry.channel.or(entry.uploader);
    let (title, artist) = split_title(&raw_title, channel.as_deref());
    let live = entry.is_live == Some(true) || entry.live_status.as_deref() == Some("is_live");

    let media_id = MediaId::new(SourceKind::YouTube, &id);
    let mut media = if live {
        Media::station(media_id, title)
    } else {
        Media::track(media_id, title)
    };
    media.artist = artist;
    if let Some(seconds) = entry
        .duration
        .filter(|seconds| seconds.is_finite() && *seconds > 0.0)
    {
        media = media.with_duration(Duration::from_secs_f64(seconds));
    }
    // YouTube serves a small picture of each video at a fixed address.
    Some(media.with_artwork(format!("https://i.ytimg.com/vi/{id}/mqdefault.jpg")))
}

#[cfg(test)]
mod tests {
    use unisson_core::MediaKind;

    use super::*;

    #[test]
    fn videos_are_read_one_per_line() {
        let output = concat!(
            r#"{"id": "abc123", "title": "Miles Davis - So What (Official Audio)", "duration": 562.0, "channel": "MilesDavisVEVO", "ie_key": "Youtube", "url": "https://www.youtube.com/watch?v=abc123"}"#,
            "\n",
            r#"{"id": "def456", "title": "Take Five", "duration": 324.5, "uploader": "Dave Brubeck - Topic"}"#,
            "\n",
        );

        let media = parse_entries(output);

        assert_eq!(media.len(), 2);
        assert_eq!(media[0].id, MediaId::new(SourceKind::YouTube, "abc123"));
        assert_eq!(media[0].kind, MediaKind::Track);
        assert_eq!(media[0].title, "So What");
        assert_eq!(media[0].artist.as_deref(), Some("Miles Davis"));
        assert_eq!(media[0].duration, Some(Duration::from_secs(562)));
        assert_eq!(
            media[0].artwork.as_deref(),
            Some("https://i.ytimg.com/vi/abc123/mqdefault.jpg")
        );
        // Without a channel, the uploader is used. Half seconds are kept.
        assert_eq!(media[1].artist.as_deref(), Some("Dave Brubeck"));
        assert_eq!(media[1].duration, Some(Duration::from_millis(324_500)));
    }

    #[test]
    fn a_live_stream_is_a_station_without_duration() {
        let output = r#"{"id": "live1", "title": "lofi hip hop radio", "channel": "Lofi Girl", "live_status": "is_live", "duration": null}"#;
        let media = parse_entries(output);
        assert_eq!(media[0].kind, MediaKind::Station);
        assert!(media[0].is_live());
        assert_eq!(media[0].duration, None);
        assert_eq!(media[0].artist.as_deref(), Some("Lofi Girl"));
    }

    #[test]
    fn what_cannot_be_played_is_skipped() {
        let output = [
            r#"{"id": "chan", "title": "A channel", "ie_key": "YoutubeTab"}"#,
            r#"{"id": "gone", "title": "[Deleted video]"}"#,
            r#"{"id": "priv", "title": "[Private video]"}"#,
            r#"{"id": "", "title": "No identifier"}"#,
            r#"{"id": "notitle"}"#,
            r#"{"title": "No id"}"#,
            "WARNING: this is not JSON",
            "",
            r#"{"id": "ok", "title": "Kept", "duration": 0}"#,
        ]
        .join("\n");

        let media = parse_entries(&output);

        assert_eq!(media.len(), 1);
        assert_eq!(media[0].title, "Kept");
        assert_eq!(media[0].duration, None, "a zero duration means unknown");
    }

    #[test]
    fn failures_are_sorted_by_kind() {
        let gone =
            classify_failure("ERROR: [youtube] abc: Video unavailable. This video is private");
        assert_eq!(gone, SourceError::NotFound);

        let offline = classify_failure(
            "WARNING: something\nERROR: [youtube] abc: Unable to download API page: <urlopen error>",
        );
        assert!(matches!(&offline, SourceError::Network(message) if message.starts_with("ERROR:")));

        let other = classify_failure("ERROR: Sign in to confirm you're not a bot");
        assert!(matches!(other, SourceError::Unavailable(_)));

        let silent = classify_failure("");
        assert!(
            matches!(silent, SourceError::Unavailable(message) if message.contains("without a message"))
        );
    }

    #[test]
    fn a_missing_program_is_reported_as_not_installed() {
        let tool = YtDlp::with_program("/nonexistent/unisson-test/yt-dlp");
        assert_eq!(
            tool.version(),
            Err(SourceError::Unavailable(
                "the YouTube helper is not installed".into()
            ))
        );
    }
}
