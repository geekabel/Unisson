//! The YouTube component: yt-dlp, and Deno when needed, downloaded by
//! Unisson into a folder of its own, checked, and kept up to date.
//!
//! # Design
//!
//! Nothing is installed by hand. The first time YouTube is used,
//! [`Component::prepare`] downloads in the background:
//!
//! - the standalone yt-dlp of the latest release, which carries its own
//!   Python. Its file is checked against the `SHA2-256SUMS` list of the same
//!   release;
//! - Deno, the JavaScript runtime yt-dlp needs to read YouTube, unless a
//!   recent one is already in the `PATH`. Its archive is checked against
//!   the `.sha256sum` file published next to it.
//!
//! Each file is written next to its final place, checked, then renamed over
//! it: a cut download never leaves a broken program behind. Until the
//! download ends, the yt-dlp of the `PATH` keeps working, if there is one.
//!
//! Then, at most once a day while YouTube is used, the list of the latest
//! release is read again. If it names another file than the one in place,
//! the new one is downloaded the same way. Deno is looked at once a month.
//!
//! The release files are documented at
//! <https://github.com/yt-dlp/yt-dlp#release-files>, and the need for Deno
//! at <https://github.com/yt-dlp/yt-dlp/wiki/EJS>.

use std::fmt;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use sha2::{Digest, Sha256};

use crate::ytdlp::YtDlp;

/// Where the latest release of yt-dlp is.
const YT_DLP_RELEASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest/download/";

/// Where the latest release of Deno is.
const DENO_RELEASE: &str = "https://github.com/denoland/deno/releases/latest/download/";

/// The list of the checksums of a yt-dlp release.
const YT_DLP_SUMS: &str = "SHA2-256SUMS";

/// How often the latest yt-dlp is looked for.
const YT_DLP_CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

/// How often the latest Deno is looked for.
const DENO_CHECK_EVERY: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// How long after a failed download the next use of YouTube tries again on
/// its own. The user can try again sooner.
const RETRY_AFTER: Duration = Duration::from_secs(60);

/// The oldest Deno yt-dlp accepts.
const DENO_MINIMUM: (u32, u32) = (2, 3);

/// The largest file accepted, against a server that would never stop.
const SIZE_LIMIT: u64 = 512 * 1024 * 1024;

/// The files of the component for one system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Platform {
    /// The name of yt-dlp in its release.
    pub yt_dlp_asset: &'static str,
    /// The name of yt-dlp once in place.
    pub yt_dlp_file: &'static str,
    /// The name of the Deno archive in its release.
    pub deno_archive: &'static str,
    /// The name of Deno, in its archive and once in place.
    pub deno_file: &'static str,
}

impl Platform {
    /// Returns the files for the system Unisson runs on, or `None` when
    /// neither project publishes a program for it.
    pub fn current() -> Option<Self> {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            Some(Self {
                yt_dlp_asset: "yt-dlp_linux",
                yt_dlp_file: "yt-dlp",
                deno_archive: "deno-x86_64-unknown-linux-gnu.zip",
                deno_file: "deno",
            })
        } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
            Some(Self {
                yt_dlp_asset: "yt-dlp_linux_aarch64",
                yt_dlp_file: "yt-dlp",
                deno_archive: "deno-aarch64-unknown-linux-gnu.zip",
                deno_file: "deno",
            })
        } else if cfg!(all(windows, target_arch = "x86_64")) {
            Some(Self {
                yt_dlp_asset: "yt-dlp.exe",
                yt_dlp_file: "yt-dlp.exe",
                deno_archive: "deno-x86_64-pc-windows-msvc.zip",
                deno_file: "deno.exe",
            })
        } else {
            None
        }
    }
}

/// Where the component stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Not downloaded yet.
    Absent,
    /// Being downloaded, with the share received so far in percent, when
    /// the size is known.
    Installing {
        /// From 0 to 100.
        percent: Option<u8>,
    },
    /// In place.
    Ready,
    /// The last download failed, for the reason given, in English.
    Failed(String),
}

/// Opens downloads. The real one is [`Web`]; tests use another.
pub trait Fetch: Send + Sync {
    /// Opens the file at `url`, and returns its content with its size,
    /// when the server announces it.
    ///
    /// # Errors
    ///
    /// Returns why the file cannot be read, in English.
    fn open(&self, url: &str) -> Result<(Box<dyn Read + Send>, Option<u64>), String>;
}

/// Downloads over HTTPS.
pub struct Web {
    agent: ureq::Agent,
}

impl fmt::Debug for Web {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Web")
    }
}

impl Default for Web {
    fn default() -> Self {
        Self::new()
    }
}

impl Web {
    /// Creates the downloader.
    pub fn new() -> Self {
        let agent = ureq::Agent::config_builder()
            .user_agent(concat!("Unisson/", env!("CARGO_PKG_VERSION")))
            .https_only(true)
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            // A large file on a slow line: long, but not for ever.
            .timeout_recv_body(Some(Duration::from_secs(15 * 60)))
            .build()
            .into();
        Self { agent }
    }
}

impl Fetch for Web {
    fn open(&self, url: &str) -> Result<(Box<dyn Read + Send>, Option<u64>), String> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|error| format!("cannot download {url}: {error}"))?;
        let body = response.into_body();
        let size = body.content_length();
        let reader = body.into_with_config().limit(SIZE_LIMIT).reader();
        Ok((Box::new(reader), size))
    }
}

/// The YouTube component of one user.
pub struct Component {
    dir: PathBuf,
    platform: Platform,
    tool: YtDlp,
    fetch: Box<dyn Fetch>,
    state: Mutex<State>,
    watchers: Mutex<Vec<Watcher>>,
}

/// Called at each change of the status.
type Watcher = Arc<dyn Fn(&Status) + Send + Sync>;

impl fmt::Debug for Component {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Component")
            .field("dir", &self.dir)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct State {
    status: Status,
    /// Whether a download or a check runs now.
    busy: bool,
    /// When the latest yt-dlp was last looked for.
    checked_at: Option<Instant>,
    /// When the last download failed.
    failed_at: Option<Instant>,
}

impl Component {
    /// Creates the component in `dir`, for `platform`. `tool` is the yt-dlp
    /// the YouTube source runs: it switches to the downloaded program as
    /// soon as there is one.
    ///
    /// A program already downloaded is used at once.
    pub fn new(dir: PathBuf, platform: Platform, tool: YtDlp, fetch: Box<dyn Fetch>) -> Arc<Self> {
        let component = Arc::new(Self {
            dir,
            platform,
            tool,
            fetch,
            state: Mutex::new(State {
                status: Status::Absent,
                busy: false,
                checked_at: None,
                failed_at: None,
            }),
            watchers: Mutex::new(Vec::new()),
        });
        if component.yt_dlp_path().is_file() {
            component.use_downloaded();
            component.lock().status = Status::Ready;
        }
        component
    }

    /// Returns the folder of the component.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Returns where the component stands.
    pub fn status(&self) -> Status {
        self.lock().status.clone()
    }

    /// Calls `watcher` at each change of the status, from the thread that
    /// downloads.
    pub fn watch(&self, watcher: impl Fn(&Status) + Send + Sync + 'static) {
        lock(&self.watchers).push(Arc::new(watcher));
    }

    /// Makes the component ready, in the background, and returns at once.
    ///
    /// Call it each time YouTube is used. The first time, it downloads the
    /// component. Afterwards, it looks for a newer yt-dlp once a day. After
    /// a failed download, it tries again only after a minute: a search
    /// starts at each letter typed.
    pub fn prepare(self: &Arc<Self>) {
        self.start(false);
    }

    /// Downloads the component now, even right after a failure. Does
    /// nothing if it is in place or on its way.
    pub fn install_now(self: &Arc<Self>) {
        self.start(true);
    }

    fn start(self: &Arc<Self>, forced: bool) {
        let job = {
            let mut state = self.lock();
            if state.busy {
                return;
            }
            let job = match &state.status {
                Status::Ready => {
                    let due = state
                        .checked_at
                        .is_none_or(|at| at.elapsed() >= YT_DLP_CHECK_EVERY);
                    if !due {
                        return;
                    }
                    Job::Update
                }
                Status::Failed(_) if !forced => {
                    let waited = state.failed_at.is_none_or(|at| at.elapsed() >= RETRY_AFTER);
                    if !waited {
                        return;
                    }
                    Job::Install
                }
                Status::Installing { .. } => return,
                Status::Absent | Status::Failed(_) => Job::Install,
            };
            state.busy = true;
            job
        };
        if job == Job::Install {
            self.set_status(Status::Installing { percent: Some(0) });
        }
        let component = Arc::clone(self);
        let spawned = thread::Builder::new()
            .name("unisson-youtube-component".into())
            .spawn(move || component.run(job));
        if let Err(error) = spawned {
            self.lock().busy = false;
            self.fail(format!("cannot start the download: {error}"));
        }
    }

    fn run(&self, job: Job) {
        let result = match job {
            Job::Install => self.install(),
            Job::Update => self.update(),
        };
        let mut state = self.lock();
        state.busy = false;
        match result {
            Ok(()) => {
                state.checked_at = Some(Instant::now());
                state.failed_at = None;
                drop(state);
                self.set_status(Status::Ready);
            }
            // A failed update keeps the program in place: it still works.
            // The next check comes with the next day.
            Err(error) if job == Job::Update => {
                state.checked_at = Some(Instant::now());
                drop(state);
                eprintln!("unisson: the YouTube component was not updated: {error}");
            }
            Err(error) => {
                drop(state);
                eprintln!("unisson: the YouTube component was not downloaded: {error}");
                self.fail(error);
            }
        }
    }

    fn fail(&self, error: String) {
        self.lock().failed_at = Some(Instant::now());
        self.set_status(Status::Failed(error));
    }

    /// Downloads what is missing.
    fn install(&self) -> Result<(), String> {
        fs::create_dir_all(&self.dir)
            .map_err(|error| format!("cannot create {}: {error}", self.dir.display()))?;
        // yt-dlp may be in place already, when Deno failed the last time.
        let yt_dlp = !self.yt_dlp_path().is_file();
        let deno = !self.deno_path().is_file() && !system_deno_is_recent();
        // Both files weigh about the same: each one is half of the way.
        let deno_start = if yt_dlp { 50 } else { 0 };
        if yt_dlp {
            let share = if deno { 50 } else { 100 };
            self.download_yt_dlp(&mut |received, size| self.progress(0, share, received, size))?;
        }
        if deno {
            self.download_deno(&mut |received, size| {
                self.progress(deno_start, 100 - deno_start, received, size);
            })?;
        }
        self.use_downloaded();
        Ok(())
    }

    /// Replaces yt-dlp if the latest release has another one, and Deno if it
    /// was not looked at for a month.
    fn update(&self) -> Result<(), String> {
        let sums = self.read_text(&format!("{YT_DLP_RELEASE}{YT_DLP_SUMS}"))?;
        let latest = listed_hash(&sums, self.platform.yt_dlp_asset)
            .ok_or_else(|| format!("{YT_DLP_SUMS} does not list {}", self.platform.yt_dlp_asset))?;
        let current = file_hash(&self.yt_dlp_path()).ok();
        if current.as_deref() != Some(latest.as_str()) {
            self.download_yt_dlp(&mut |_, _| {})?;
        }
        if !self.deno_path().is_file() {
            // Missing since a failed download: yt-dlp cannot read YouTube
            // without it.
            if !system_deno_is_recent() {
                self.download_deno(&mut |_, _| {})?;
            }
        } else if is_older_than(&self.deno_stamp_path(), DENO_CHECK_EVERY) {
            let latest = self.deno_hash()?;
            let current = fs::read_to_string(self.deno_stamp_path()).unwrap_or_default();
            if current.trim() == latest {
                // Up to date: only the time of the check changes.
                fs::write(self.deno_stamp_path(), &latest).map_err(|error| error.to_string())?;
            } else {
                self.download_deno(&mut |_, _| {})?;
            }
        }
        self.use_downloaded();
        Ok(())
    }

    fn download_yt_dlp(&self, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<(), String> {
        let asset = self.platform.yt_dlp_asset;
        // A release may come out between the list and the file: then the
        // file does not match, and one more try reads the new list.
        let mut last_error = String::new();
        for _ in 0..2 {
            let sums = self.read_text(&format!("{YT_DLP_RELEASE}{YT_DLP_SUMS}"))?;
            let expected = listed_hash(&sums, asset)
                .ok_or_else(|| format!("{YT_DLP_SUMS} does not list {asset}"))?;
            let part = self
                .dir
                .join(format!(".{}.part", self.platform.yt_dlp_file));
            match self.download(
                &format!("{YT_DLP_RELEASE}{asset}"),
                &part,
                &expected,
                progress,
            ) {
                Ok(()) => {
                    make_executable(&part)?;
                    return replace(&part, &self.yt_dlp_path());
                }
                Err(error) => last_error = error,
            }
        }
        Err(last_error)
    }

    fn download_deno(&self, progress: &mut dyn FnMut(u64, Option<u64>)) -> Result<(), String> {
        let archive = self.platform.deno_archive;
        let expected = self.deno_hash()?;
        let zip = self.dir.join(format!(".{archive}.part"));
        let result = self
            .download(
                &format!("{DENO_RELEASE}{archive}"),
                &zip,
                &expected,
                progress,
            )
            .and_then(|()| {
                let part = self.dir.join(format!(".{}.part", self.platform.deno_file));
                extract(&zip, self.platform.deno_file, &part)?;
                make_executable(&part)?;
                replace(&part, &self.deno_path())
            });
        let _ = fs::remove_file(&zip);
        result?;
        fs::write(self.deno_stamp_path(), &expected).map_err(|error| error.to_string())
    }

    /// Returns the checksum published for the latest Deno archive.
    fn deno_hash(&self) -> Result<String, String> {
        let archive = self.platform.deno_archive;
        let text = self.read_text(&format!("{DENO_RELEASE}{archive}.sha256sum"))?;
        first_hash(&text).ok_or_else(|| format!("no checksum in {archive}.sha256sum"))
    }

    /// Downloads `url` into `to`, and checks its SHA-256 against
    /// `expected`. A file that does not match is deleted.
    fn download(
        &self,
        url: &str,
        to: &Path,
        expected: &str,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<(), String> {
        let (mut body, size) = self.fetch.open(url)?;
        let result = (|| {
            let mut file = File::create(to)
                .map_err(|error| format!("cannot write {}: {error}", to.display()))?;
            let mut hasher = Sha256::new();
            let mut buffer = vec![0; 64 * 1024];
            let mut received = 0;
            loop {
                let count = match body.read(&mut buffer) {
                    Ok(0) => break,
                    Ok(count) => count,
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => return Err(format!("the download of {url} stopped: {error}")),
                };
                hasher.update(&buffer[..count]);
                file.write_all(&buffer[..count])
                    .map_err(|error| format!("cannot write {}: {error}", to.display()))?;
                received += count as u64;
                progress(received, size);
            }
            file.sync_all().map_err(|error| error.to_string())?;
            let found = hex(&hasher.finalize());
            if found == expected {
                Ok(())
            } else {
                Err(format!("{url} does not match its checksum"))
            }
        })();
        if result.is_err() {
            let _ = fs::remove_file(to);
        }
        result
    }

    /// Reads a small text file.
    fn read_text(&self, url: &str) -> Result<String, String> {
        let (body, _) = self.fetch.open(url)?;
        let mut text = String::new();
        body.take(1024 * 1024)
            .read_to_string(&mut text)
            .map_err(|error| format!("cannot read {url}: {error}"))?;
        Ok(text)
    }

    /// Tells the progress of one part of the download, which covers `share`
    /// percent of the whole from `start`.
    fn progress(&self, start: u8, share: u8, received: u64, size: Option<u64>) {
        let percent = size.filter(|size| *size > 0).map(|size| {
            let done = received.min(size) * u64::from(share) / size;
            start + u8::try_from(done).unwrap_or(share)
        });
        if self.status() != (Status::Installing { percent }) {
            self.set_status(Status::Installing { percent });
        }
    }

    /// Points the YouTube source at the downloaded programs.
    fn use_downloaded(&self) {
        let deno = self.deno_path();
        let deno = deno.is_file().then_some(deno.as_path());
        self.tool.use_program(self.yt_dlp_path(), deno);
    }

    fn set_status(&self, status: Status) {
        self.lock().status = status.clone();
        let watchers = lock(&self.watchers).clone();
        for watcher in watchers {
            watcher(&status);
        }
    }

    fn yt_dlp_path(&self) -> PathBuf {
        self.dir.join(self.platform.yt_dlp_file)
    }

    fn deno_path(&self) -> PathBuf {
        self.dir.join(self.platform.deno_file)
    }

    /// The file that keeps the checksum of the Deno archive in place. Its
    /// date is the date of the last check.
    fn deno_stamp_path(&self) -> PathBuf {
        self.dir.join("deno.sha256")
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        lock(&self.state)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Job {
    Install,
    Update,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The value is still usable if a thread panicked while holding it.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Finds the checksum of `name` in a list of lines `HASH  NAME`.
fn listed_hash(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        // `*` marks a file read in binary mode.
        let file = parts.next()?.trim_start_matches('*');
        (file == name && is_hash(hash)).then(|| hash.to_ascii_lowercase())
    })
}

/// Finds the first SHA-256 in a text. Deno publishes it as `HASH  NAME` for
/// Linux, and as a PowerShell table for Windows.
fn first_hash(text: &str) -> Option<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric())
        .find(|word| is_hash(word))
        .map(str::to_ascii_lowercase)
}

fn is_hash(word: &str) -> bool {
    word.len() == 64 && word.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Returns the SHA-256 of a file.
fn file_hash(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    Ok(hex(&hasher.finalize()))
}

/// Whether the file is missing, or was last written more than `age` ago.
fn is_older_than(path: &Path, age: Duration) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        .is_none_or(|elapsed| elapsed >= age)
}

/// Writes the entry `name` of the archive `zip` into `to`.
fn extract(zip: &Path, name: &str, to: &Path) -> Result<(), String> {
    let file = File::open(zip).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|error| format!("the Deno archive is unreadable: {error}"))?;
    let mut entry = archive
        .by_name(name)
        .map_err(|error| format!("the Deno archive has no {name}: {error}"))?;
    if !entry.is_file() || entry.size() > SIZE_LIMIT {
        return Err(format!("the {name} of the Deno archive is not a program"));
    }
    let mut out =
        File::create(to).map_err(|error| format!("cannot write {}: {error}", to.display()))?;
    io::copy(&mut (&mut entry).take(SIZE_LIMIT), &mut out)
        .map_err(|error| format!("cannot unpack {name}: {error}"))?;
    out.sync_all().map_err(|error| error.to_string())
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// Puts `part` in the place of `target`, in one step.
fn replace(part: &Path, target: &Path) -> Result<(), String> {
    fs::rename(part, target).map_err(|error| {
        let _ = fs::remove_file(part);
        format!("cannot put {} in place: {error}", target.display())
    })
}

/// Whether the `deno` of the `PATH` is recent enough for yt-dlp.
fn system_deno_is_recent() -> bool {
    let mut command = Command::new("deno");
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        /// `CREATE_NO_WINDOW`, from the Windows API.
        const NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(NO_WINDOW);
    }
    command
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| deno_version(&String::from_utf8_lossy(&output.stdout)))
        .is_some_and(|version| version >= DENO_MINIMUM)
}

/// Reads `deno 2.5.1 (stable, release, x86_64-unknown-linux-gnu)`.
fn deno_version(text: &str) -> Option<(u32, u32)> {
    let version = text.lines().next()?.strip_prefix("deno ")?;
    let mut numbers = version.split(|c: char| !c.is_ascii_digit());
    Some((numbers.next()?.parse().ok()?, numbers.next()?.parse().ok()?))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::sync::mpsc;

    use super::*;

    const PLATFORM: Platform = Platform {
        yt_dlp_asset: "yt-dlp_linux",
        yt_dlp_file: "yt-dlp",
        deno_archive: "deno-x86_64-unknown-linux-gnu.zip",
        deno_file: "deno",
    };

    /// Serves files from memory, and counts what is asked.
    #[derive(Default, Clone)]
    struct Files {
        files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
        asked: Arc<Mutex<Vec<String>>>,
    }

    impl Files {
        fn put(&self, name: &str, content: &[u8]) {
            lock(&self.files).insert(name.to_owned(), content.to_vec());
        }

        fn release(&self, yt_dlp: &[u8]) {
            self.put(&format!("{YT_DLP_RELEASE}yt-dlp_linux"), yt_dlp);
            let sums = format!(
                "{}  yt-dlp\n{}  yt-dlp_linux\n",
                hex(&Sha256::digest(b"other")),
                hex(&Sha256::digest(yt_dlp))
            );
            self.put(&format!("{YT_DLP_RELEASE}{YT_DLP_SUMS}"), sums.as_bytes());
        }

        fn deno(&self, program: &[u8]) {
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            zip.start_file("deno", zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(program).unwrap();
            let archive = zip.finish().unwrap().into_inner();
            let name = format!("{DENO_RELEASE}{}", PLATFORM.deno_archive);
            self.put(
                &format!("{name}.sha256sum"),
                format!(
                    "{}  {}\n",
                    hex(&Sha256::digest(&archive)),
                    PLATFORM.deno_archive
                )
                .as_bytes(),
            );
            self.put(&name, &archive);
        }

        fn count(&self, ending: &str) -> usize {
            lock(&self.asked)
                .iter()
                .filter(|url| url.ends_with(ending))
                .count()
        }
    }

    impl Fetch for Files {
        fn open(&self, url: &str) -> Result<(Box<dyn Read + Send>, Option<u64>), String> {
            lock(&self.asked).push(url.to_owned());
            let content = lock(&self.files)
                .get(url)
                .cloned()
                .ok_or_else(|| format!("404 for {url}"))?;
            let size = content.len() as u64;
            Ok((Box::new(Cursor::new(content)), Some(size)))
        }
    }

    fn folder(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("unisson-component-tests-{}", std::process::id()))
            .join(name);
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    /// Starts `job` and waits until the component settles on a status
    /// other than "installing".
    fn settle(component: &Arc<Component>, job: impl FnOnce(&Arc<Component>)) -> Status {
        let (sender, receiver) = mpsc::channel();
        let sender = Mutex::new(sender);
        component.watch(move |status| {
            if !matches!(status, Status::Installing { .. }) {
                let _ = lock(&sender).send(status.clone());
            }
        });
        job(component);
        receiver
            .recv_timeout(Duration::from_secs(10))
            .expect("the component settles")
    }

    /// Waits for a background job that changes no status.
    fn wait_idle(component: &Component) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while component.lock().busy {
            assert!(Instant::now() < deadline, "the job ends");
            thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn the_first_use_downloads_checks_and_switches_the_program() {
        let files = Files::default();
        files.release(b"yt-dlp v1");
        files.deno(b"deno v1");
        let dir = folder("first");
        let tool = YtDlp::with_program("yt-dlp");
        let component =
            Component::new(dir.clone(), PLATFORM, tool.clone(), Box::new(files.clone()));
        assert_eq!(component.status(), Status::Absent);

        let progress = Arc::new(Mutex::new(Vec::new()));
        let seen = Arc::clone(&progress);
        component.watch(move |status| {
            if let Status::Installing { percent } = status {
                lock(&seen).push(*percent);
            }
        });
        assert_eq!(settle(&component, |c| c.prepare()), Status::Ready);

        assert_eq!(fs::read(dir.join("yt-dlp")).unwrap(), b"yt-dlp v1");
        assert_eq!(tool.program(), dir.join("yt-dlp"));
        let progress = lock(&progress).clone();
        assert_eq!(progress.first(), Some(&Some(0)));
        assert_eq!(progress.last(), Some(&Some(100)));
        assert!(
            progress.windows(2).all(|pair| pair[0] <= pair[1]),
            "{progress:?}"
        );
        if dir.join("deno").is_file() {
            assert_eq!(fs::read(dir.join("deno")).unwrap(), b"deno v1");
            assert!(dir.join("deno.sha256").is_file());
            assert!(progress.contains(&Some(50)));
        }
        // Nothing is left half-written.
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".part"))
            .collect();
        assert!(leftovers.is_empty());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dir.join("yt-dlp"))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }

        // A component found on disk is used at once, with nothing downloaded.
        let again = YtDlp::with_program("yt-dlp");
        let found = Component::new(
            dir.clone(),
            PLATFORM,
            again.clone(),
            Box::new(Files::default()),
        );
        assert_eq!(found.status(), Status::Ready);
        assert_eq!(again.program(), dir.join("yt-dlp"));
    }

    #[test]
    fn a_file_that_does_not_match_is_refused_and_nothing_is_kept() {
        let files = Files::default();
        files.release(b"yt-dlp v1");
        // The server sends another file than the list announces.
        files.put(&format!("{YT_DLP_RELEASE}yt-dlp_linux"), b"tampered");
        let dir = folder("tampered");
        let tool = YtDlp::with_program("yt-dlp");
        let component =
            Component::new(dir.clone(), PLATFORM, tool.clone(), Box::new(files.clone()));

        let status = settle(&component, |c| c.prepare());
        assert!(
            matches!(&status, Status::Failed(reason) if reason.contains("checksum")),
            "{status:?}"
        );
        assert!(!dir.join("yt-dlp").exists());
        assert!(!dir.join(".yt-dlp.part").exists());
        assert_eq!(
            tool.program(),
            PathBuf::from("yt-dlp"),
            "the program of the PATH stays"
        );
        // The list was read twice: a new release may have come out between.
        assert_eq!(files.count(YT_DLP_SUMS), 2);

        // A failure is not tried again at each letter typed.
        component.prepare();
        assert_eq!(component.status(), status);
        assert_eq!(files.count(YT_DLP_SUMS), 2);

        // The user can try again at once.
        files.release(b"yt-dlp v1");
        files.deno(b"deno v1");
        assert_eq!(settle(&component, |c| c.install_now()), Status::Ready);
        assert_eq!(tool.program(), dir.join("yt-dlp"));
    }

    #[test]
    fn an_offline_computer_gets_a_failure_and_keeps_its_program() {
        let dir = folder("offline");
        let tool = YtDlp::with_program("yt-dlp");
        let component = Component::new(dir, PLATFORM, tool.clone(), Box::new(Files::default()));
        let status = settle(&component, |c| c.prepare());
        assert!(
            matches!(&status, Status::Failed(reason) if reason.contains("404")),
            "{status:?}"
        );
        assert_eq!(tool.program(), PathBuf::from("yt-dlp"));
    }

    #[test]
    fn a_new_release_replaces_the_program_once_a_day() {
        let files = Files::default();
        files.release(b"yt-dlp v1");
        files.deno(b"deno v1");
        let dir = folder("update");
        let component = Component::new(
            dir.clone(),
            PLATFORM,
            YtDlp::with_program("yt-dlp"),
            Box::new(files.clone()),
        );
        assert_eq!(settle(&component, |c| c.prepare()), Status::Ready);
        let downloads = files.count("/yt-dlp_linux");

        // Just installed: no check before tomorrow.
        files.release(b"yt-dlp v2");
        component.prepare();
        wait_idle(&component);
        assert_eq!(files.count("/yt-dlp_linux"), downloads);

        // A day later: the new release is taken, in silence.
        component.lock().checked_at = Instant::now().checked_sub(YT_DLP_CHECK_EVERY);
        component.prepare();
        assert_eq!(component.status(), Status::Ready);
        wait_idle(&component);
        assert_eq!(fs::read(dir.join("yt-dlp")).unwrap(), b"yt-dlp v2");

        // The same release the next day: only the list is read.
        component.lock().checked_at = Instant::now().checked_sub(YT_DLP_CHECK_EVERY);
        let downloads = files.count("/yt-dlp_linux");
        component.prepare();
        wait_idle(&component);
        assert_eq!(files.count("/yt-dlp_linux"), downloads);
    }

    #[test]
    fn a_failed_update_keeps_the_program_in_place() {
        let files = Files::default();
        files.release(b"yt-dlp v1");
        files.deno(b"deno v1");
        let dir = folder("failed-update");
        let component = Component::new(
            dir.clone(),
            PLATFORM,
            YtDlp::with_program("yt-dlp"),
            Box::new(files.clone()),
        );
        assert_eq!(settle(&component, |c| c.prepare()), Status::Ready);
        lock(&files.files).clear();
        component.lock().checked_at = Instant::now().checked_sub(YT_DLP_CHECK_EVERY);
        component.prepare();
        wait_idle(&component);
        assert_eq!(component.status(), Status::Ready);
        assert_eq!(fs::read(dir.join("yt-dlp")).unwrap(), b"yt-dlp v1");
    }

    #[test]
    fn deno_comes_after_yt_dlp_when_it_failed_first() {
        if system_deno_is_recent() {
            // Deno is never downloaded on this computer.
            return;
        }
        let files = Files::default();
        files.release(b"yt-dlp v1");
        let dir = folder("deno-later");
        let tool = YtDlp::with_program("yt-dlp");
        let component =
            Component::new(dir.clone(), PLATFORM, tool.clone(), Box::new(files.clone()));
        let status = settle(&component, |c| c.prepare());
        assert!(
            matches!(&status, Status::Failed(reason) if reason.contains("sha256sum")),
            "{status:?}"
        );
        assert!(dir.join("yt-dlp").is_file() && !dir.join("deno").exists());

        // The retry only fetches Deno.
        files.deno(b"deno v1");
        let downloads = files.count("/yt-dlp_linux");
        assert_eq!(settle(&component, |c| c.install_now()), Status::Ready);
        assert_eq!(files.count("/yt-dlp_linux"), downloads);
        assert!(dir.join("deno").is_file());

        // At the next start with Deno missing, the first check fetches it.
        fs::remove_file(dir.join("deno")).unwrap();
        let again = Component::new(dir.clone(), PLATFORM, tool, Box::new(files.clone()));
        assert_eq!(again.status(), Status::Ready);
        again.prepare();
        wait_idle(&again);
        assert!(dir.join("deno").is_file());
    }

    #[test]
    fn checksums_are_read_from_both_formats() {
        let hash = "a".repeat(64);
        let upper = "A".repeat(64);
        let sums = format!("{}  yt-dlp\n{hash} *yt-dlp_linux\n", "b".repeat(64));
        assert_eq!(listed_hash(&sums, "yt-dlp_linux"), Some(hash.clone()));
        assert_eq!(listed_hash(&sums, "yt-dlp_linux.zip"), None);
        let windows =
            format!("\nAlgorithm : SHA256\nHash      : {upper}\nPath      : C:\\a\\deno.zip\n");
        assert_eq!(first_hash(&windows), Some(hash.clone()));
        assert_eq!(first_hash(&format!("{upper}  deno.zip")), Some(hash));
        assert_eq!(first_hash("no hash here"), None);
    }

    #[test]
    fn the_deno_version_is_read() {
        assert_eq!(
            deno_version("deno 2.5.1 (stable, release, x86_64-unknown-linux-gnu)\nv8 14.0\n"),
            Some((2, 5))
        );
        assert_eq!(deno_version("deno 1.46.3 (release)"), Some((1, 46)));
        assert_eq!(deno_version("node v22"), None);
        assert!((2, 3) >= DENO_MINIMUM && (1, 46) < DENO_MINIMUM);
    }

    #[test]
    fn this_system_has_its_files() {
        if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
            assert_eq!(Platform::current(), Some(PLATFORM));
        }
    }
}
