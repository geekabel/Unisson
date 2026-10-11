//! Desktop notifications: a short message when a new title starts.
//!
//! # Design
//!
//! Two parts, kept apart so the first one can be tested without a desktop:
//!
//! - an [`Announcer`] decides *whether* a playback state is news. It
//!   remembers the last title it saw, so a pause, a resume or a volume
//!   change never brings the same message twice;
//! - a [`Notifier`] *shows* the message, on a thread of its own because
//!   talking to the desktop can take a moment.
//!
//! The message is sent with the `notify-rust` crate, over the standard
//! [desktop notification interface][spec]. Each new message replaces the
//! previous one instead of piling up, and it is marked as transient: GNOME
//! does not keep it in its list afterwards.
//!
//! Nothing is shown while the main window or the mini-player has the
//! keyboard: the title is already in front of the user.
//!
//! [spec]: https://specifications.freedesktop.org/notification-spec/latest/

use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};

use notify_rust::{Hint, Notification, Timeout};
use tauri::{AppHandle, Manager};
use unisson_core::MediaId;
use unisson_playback::{Snapshot, Status};

use crate::now_playing::NowPlaying;
use crate::state::AppState;
use crate::windows;

/// The name the desktop shows above the message.
const APP_NAME: &str = "Unisson";
/// The name of the desktop file and of the icon.
const DESKTOP_ENTRY: &str = "unisson";
/// How long a message stays on screen, in milliseconds.
const TIMEOUT_MS: u32 = 5_000;

/// A message to show: what just started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Announcement {
    /// The first line: the title, or what a radio station has on air.
    pub summary: String,
    /// The second line: the artist, or the name of the station. It can be
    /// empty.
    pub body: String,
}

impl Announcement {
    fn of(now: &NowPlaying) -> Self {
        Self {
            summary: now.title.clone(),
            body: now.artist.clone().unwrap_or_default(),
        }
    }
}

/// Decides which playback states deserve a message.
#[derive(Debug, Default)]
pub struct Announcer {
    /// What the last state with sound was about.
    last: Option<(MediaId, Announcement)>,
}

impl Announcer {
    /// Looks at a new playback state, and returns a message when a new
    /// title just started.
    ///
    /// Only a state with sound counts. A media that is still loading may
    /// fail, and a message for something that never plays would be noise.
    /// A radio station brings a message each time it announces a new
    /// title on air.
    pub fn observe(&mut self, snapshot: &Snapshot) -> Option<Announcement> {
        let Some(media) = &snapshot.current else {
            // The queue ran out: the same track played again later is news.
            self.last = None;
            return None;
        };
        if snapshot.status != Status::Playing {
            return None;
        }
        let now = NowPlaying::of(snapshot)?;
        let seen = (media.id.clone(), Announcement::of(&now));
        if self.last.as_ref() == Some(&seen) {
            return None;
        }
        let announcement = seen.1.clone();
        self.last = Some(seen);
        Some(announcement)
    }
}

/// Handle to the notification thread.
#[derive(Debug)]
pub struct Notifier {
    announcer: Mutex<Announcer>,
    messages: Sender<Announcement>,
}

impl Notifier {
    /// Starts the notification thread.
    ///
    /// A desktop without notifications is not an error: the first message
    /// that cannot be shown leaves a line in the log, and that is all.
    pub fn start() -> Self {
        let (messages, inbox) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("unisson-notify".into())
            .spawn(move || show_all(&inbox));
        if let Err(error) = spawned {
            eprintln!("unisson: cannot start the notification thread: {error}");
        }
        Self {
            announcer: Mutex::new(Announcer::default()),
            messages,
        }
    }

    /// Looks at a new playback state, and shows a message if a new title
    /// just started, the user wants messages, and Unisson is not the window
    /// in front.
    pub fn playback_changed(&self, app: &AppHandle, snapshot: &Snapshot) {
        // The title is remembered even when no message is shown. Otherwise
        // an old title would be announced when the window loses the
        // keyboard, or when the setting is switched on.
        let announcement = match self.announcer.lock() {
            Ok(mut announcer) => announcer.observe(snapshot),
            Err(_) => return,
        };
        let Some(announcement) = announcement else {
            return;
        };
        if !app.state::<AppState>().settings().notifications || has_focus(app) {
            return;
        }
        // This only fails when the thread is gone: nobody would show it.
        let _ = self.messages.send(announcement);
    }
}

/// Returns whether a window that shows what plays has the keyboard.
///
/// The quick launcher does not count. It is on its way out when the title
/// it started begins, and the user is back in another application.
fn has_focus(app: &AppHandle) -> bool {
    [windows::MAIN, windows::MINI].iter().any(|label| {
        app.get_webview_window(label)
            .is_some_and(|window| window.is_focused().unwrap_or(false))
    })
}

/// Shows each message as it arrives, until the application ends.
fn show_all(inbox: &Receiver<Announcement>) {
    // Asked once: whether the desktop reads the second line as markup.
    let mut markup: Option<bool> = None;
    // The number the desktop gave to the last message, to replace it.
    let mut last_id: Option<u32> = None;
    let mut failed = false;

    for mut announcement in inbox {
        // Only the newest title matters when several are waiting.
        while let Ok(newer) = inbox.try_recv() {
            announcement = newer;
        }
        let markup = *markup.get_or_insert_with(|| {
            notify_rust::get_capabilities()
                .is_ok_and(|capabilities| capabilities.iter().any(|name| name == "body-markup"))
        });
        let body = if markup {
            escape_markup(&announcement.body)
        } else {
            announcement.body
        };

        let mut notification = Notification::new();
        notification
            .appname(APP_NAME)
            .summary(&announcement.summary)
            .body(&body)
            .icon(DESKTOP_ENTRY)
            .hint(Hint::DesktopEntry(DESKTOP_ENTRY.into()))
            // Not kept in the list of the desktop once it left the screen.
            .hint(Hint::Transient(true))
            // The music is the sound.
            .hint(Hint::SuppressSound(true))
            .timeout(Timeout::Milliseconds(TIMEOUT_MS));
        if let Some(id) = last_id {
            notification.id(id);
        }
        match notification.show() {
            Ok(handle) => {
                last_id = Some(handle.id());
                failed = false;
            }
            // One line is enough: every later message would say the same.
            Err(error) if !failed => {
                eprintln!("unisson: cannot show a notification: {error}");
                failed = true;
            }
            Err(_) => {}
        }
    }
}

/// Makes a text safe for a desktop that reads the message as markup.
///
/// Such a desktop understands a few tags such as `<b>`. Without this, a
/// name like `Simon & Garfunkel` or `<3` could be cut or shown wrongly.
fn escape_markup(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use unisson_core::{Media, SourceKind};

    use super::*;

    fn track(path: &str, title: &str, artist: &str) -> Media {
        Media::track(MediaId::new(SourceKind::Local, path), title).with_artist(artist)
    }

    fn state(media: &Media, status: Status) -> Snapshot {
        Snapshot {
            status,
            current: Some(media.clone()),
            ..Snapshot::default()
        }
    }

    #[test]
    fn a_new_title_is_announced_once_it_plays() {
        let so_what = track("/m/a.flac", "So What", "Miles Davis");
        let mut announcer = Announcer::default();

        assert_eq!(announcer.observe(&Snapshot::default()), None);
        assert_eq!(
            announcer.observe(&state(&so_what, Status::Loading)),
            None,
            "a media that is loading may still fail"
        );
        assert_eq!(
            announcer.observe(&state(&so_what, Status::Playing)),
            Some(Announcement {
                summary: "So What".into(),
                body: "Miles Davis".into(),
            })
        );
    }

    #[test]
    fn a_pause_or_a_volume_change_is_not_news() {
        let so_what = track("/m/a.flac", "So What", "Miles Davis");
        let mut announcer = Announcer::default();
        assert!(
            announcer
                .observe(&state(&so_what, Status::Playing))
                .is_some()
        );

        assert_eq!(announcer.observe(&state(&so_what, Status::Paused)), None);
        assert_eq!(announcer.observe(&state(&so_what, Status::Playing)), None);
        let louder = Snapshot {
            volume: 0.2,
            ..state(&so_what, Status::Playing)
        };
        assert_eq!(announcer.observe(&louder), None);
    }

    #[test]
    fn the_next_title_is_announced_and_so_is_a_replay_after_a_stop() {
        let first = track("/m/a.flac", "So What", "Miles Davis");
        let second = track("/m/b.flac", "Blue in Green", "Miles Davis");
        let mut announcer = Announcer::default();
        assert!(announcer.observe(&state(&first, Status::Playing)).is_some());

        let next = announcer.observe(&state(&second, Status::Playing));
        assert_eq!(next.unwrap().summary, "Blue in Green");

        // The queue ran out, then the same track is chosen again.
        assert_eq!(announcer.observe(&Snapshot::default()), None);
        assert!(
            announcer
                .observe(&state(&second, Status::Playing))
                .is_some()
        );
    }

    #[test]
    fn a_station_is_announced_again_for_each_title_on_air() {
        let fip = Media::station(MediaId::new(SourceKind::Radio, "fip"), "FIP");
        let mut announcer = Announcer::default();
        let mut on_air = state(&fip, Status::Playing);

        let start = announcer.observe(&on_air).unwrap();
        assert_eq!((start.summary.as_str(), start.body.as_str()), ("FIP", ""));

        on_air.stream_title = Some("Nina Simone - Sinnerman".into());
        let song = announcer.observe(&on_air).unwrap();
        assert_eq!(song.summary, "Nina Simone - Sinnerman");
        assert_eq!(song.body, "FIP");
        assert_eq!(announcer.observe(&on_air), None);

        on_air.stream_title = Some("Air - La femme d'argent".into());
        assert!(announcer.observe(&on_air).is_some());
    }

    #[test]
    fn markup_characters_are_escaped() {
        assert_eq!(
            escape_markup("Simon & Garfunkel <live>"),
            "Simon &amp; Garfunkel &lt;live&gt;"
        );
        assert_eq!(escape_markup("Björk"), "Björk");
    }
}
