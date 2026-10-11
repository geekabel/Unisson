//! The icon in the top bar of the desktop, and its menu.
//!
//! # Design
//!
//! On Linux, a tray icon cannot tell the application that it was clicked:
//! it can only open a menu. Everything therefore goes through the menu: what
//! plays, play and pause, next and previous, the favorite stations, and the
//! way back to the window.
//!
//! The menu shows the playback state, so it is built again when that state
//! changes. It is only rebuilt when something it shows changed: the
//! position moving on must not rebuild a menu four times per second.

use std::sync::{Mutex, MutexGuard};

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager};
use unisson_playback::Snapshot;
use unisson_radio::Station;

use crate::now_playing::NowPlaying;
use crate::state::AppState;
use crate::windows;

/// Most favorite stations listed in the menu.
const STATION_LIMIT: usize = 12;
/// Longest text of a menu entry, in characters.
const LABEL_LIMIT: usize = 60;

/// Prefix of the identifiers of the station entries: `station:3`.
const STATION_PREFIX: &str = "station:";

/// What the menu shows. Two equal values give the same menu.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MenuFacts {
    /// The line that says what plays.
    now: Option<String>,
    playing: bool,
    has_next: bool,
    has_previous: bool,
    /// The names of the favorite stations listed.
    stations: Vec<String>,
}

impl MenuFacts {
    fn of(snapshot: &Snapshot, has_next: bool, has_previous: bool, stations: &[Station]) -> Self {
        let now = NowPlaying::of(snapshot);
        Self {
            playing: now.as_ref().is_some_and(|now| now.playing),
            now: now.map(|now| shortened(&now.line())),
            has_next,
            has_previous,
            stations: stations
                .iter()
                .take(STATION_LIMIT)
                .map(|station| shortened(&station.name))
                .collect(),
        }
    }
}

/// Cuts a text that would make the menu too wide.
fn shortened(text: &str) -> String {
    if text.chars().count() <= LABEL_LIMIT {
        return text.to_owned();
    }
    let mut short: String = text.chars().take(LABEL_LIMIT - 1).collect();
    short.push('…');
    short
}

/// The tray icon of the application.
pub struct Tray {
    icon: TrayIcon,
    /// What the menu currently shows.
    shown: Mutex<Option<MenuFacts>>,
}

impl Tray {
    /// Creates the icon and its first menu.
    ///
    /// # Errors
    ///
    /// Returns an error if the desktop refuses the icon. The application
    /// then runs without it.
    pub fn create(app: &AppHandle) -> tauri::Result<Self> {
        let mut builder = TrayIconBuilder::with_id("unisson")
            .tooltip("Unisson")
            .on_menu_event(|app, event| on_menu_event(app, event.id().as_ref()));
        if let Some(icon) = app.default_window_icon() {
            builder = builder.icon(icon.clone());
        }
        let tray = Self {
            icon: builder.build(app)?,
            shown: Mutex::new(None),
        };
        tray.refresh(app);
        Ok(tray)
    }

    /// Brings the menu up to date with the playback state and the favorite
    /// stations. Does nothing if the menu already shows them.
    pub fn refresh(&self, app: &AppHandle) {
        let state = app.state::<AppState>();
        let snapshot = state.session.snapshot();
        let dto = state.snapshot_dto(&snapshot);
        let stations = state.stations().stations().to_vec();
        let facts = MenuFacts::of(&snapshot, dto.has_next, dto.has_previous, &stations);

        let mut shown = self.lock_shown();
        if shown.as_ref() == Some(&facts) {
            return;
        }
        match build_menu(app, &facts).and_then(|menu| self.icon.set_menu(Some(menu))) {
            Ok(()) => *shown = Some(facts),
            Err(error) => eprintln!("unisson: cannot update the tray menu: {error}"),
        }
    }

    fn lock_shown(&self) -> MutexGuard<'_, Option<MenuFacts>> {
        self.shown
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Refreshes the tray menu, if the application has a tray icon.
pub fn refresh(app: &AppHandle) {
    if let Some(tray) = app.try_state::<Tray>() {
        tray.refresh(app);
    }
}

fn build_menu(app: &AppHandle, facts: &MenuFacts) -> tauri::Result<Menu<tauri::Wry>> {
    // The first line only informs: it cannot be clicked.
    let now = MenuItemBuilder::with_id("now", facts.now.as_deref().unwrap_or("Rien en lecture"))
        .enabled(false)
        .build(app)?;
    let selected = facts.now.is_some();
    let play_pause = MenuItemBuilder::with_id(
        "play_pause",
        if facts.playing { "Pause" } else { "Lecture" },
    )
    .enabled(selected)
    .build(app)?;
    let next = MenuItemBuilder::with_id("next", "Suivant")
        .enabled(facts.has_next)
        .build(app)?;
    let previous = MenuItemBuilder::with_id("previous", "Précédent")
        .enabled(selected)
        .build(app)?;

    let mut menu = MenuBuilder::new(app)
        .item(&now)
        .separator()
        .item(&play_pause)
        .item(&next)
        .item(&previous)
        .separator();
    if !facts.stations.is_empty() {
        let mut stations = SubmenuBuilder::new(app, "Radios favorites");
        for (index, name) in facts.stations.iter().enumerate() {
            stations = stations.text(format!("{STATION_PREFIX}{index}"), name);
        }
        menu = menu.item(&stations.build()?).separator();
    }
    menu.text("open", "Ouvrir Unisson")
        .text("quit", "Quitter")
        .build()
}

/// Carries out the entry of the menu that was chosen.
fn on_menu_event(app: &AppHandle, id: &str) {
    let state = app.state::<AppState>();
    match id {
        "play_pause" => state.session.toggle(),
        "next" => state.next(),
        "previous" => state.previous(),
        "open" => windows::show_main(app),
        "quit" => app.exit(0),
        _ => {
            if let Some(index) = id
                .strip_prefix(STATION_PREFIX)
                .and_then(|index| index.parse::<usize>().ok())
            {
                state.play_favorite_station(index);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use unisson_core::{Media, MediaId, SourceKind};
    use unisson_playback::Status;

    use super::*;

    fn station(name: &str) -> Station {
        Station::custom(name, &format!("https://example.org/{}", name.len())).unwrap()
    }

    #[test]
    fn the_facts_follow_what_plays() {
        let idle = MenuFacts::of(&Snapshot::default(), false, false, &[]);
        assert_eq!(idle.now, None);
        assert!(!idle.playing);

        let track = Media::track(MediaId::new(SourceKind::Local, "/m/a.flac"), "So What")
            .with_artist("Miles Davis");
        let snapshot = Snapshot {
            status: Status::Playing,
            current: Some(track),
            ..Snapshot::default()
        };
        let playing = MenuFacts::of(&snapshot, true, false, &[station("FIP")]);
        assert_eq!(playing.now.as_deref(), Some("So What, Miles Davis"));
        assert!(playing.playing && playing.has_next && !playing.has_previous);
        assert_eq!(playing.stations, ["FIP"]);
    }

    #[test]
    fn the_position_moving_on_does_not_change_the_facts() {
        let track = Media::track(MediaId::new(SourceKind::Local, "/m/a.flac"), "So What");
        let at = |seconds| Snapshot {
            status: Status::Playing,
            current: Some(track.clone()),
            position: std::time::Duration::from_secs(seconds),
            ..Snapshot::default()
        };
        assert_eq!(
            MenuFacts::of(&at(1), false, false, &[]),
            MenuFacts::of(&at(90), false, false, &[])
        );
    }

    #[test]
    fn long_texts_and_long_lists_are_cut() {
        let long = "x".repeat(200);
        assert_eq!(shortened(&long).chars().count(), LABEL_LIMIT);
        assert!(shortened(&long).ends_with('…'));
        assert_eq!(shortened("FIP"), "FIP");
        // Cutting counts letters, not bytes.
        assert_eq!(shortened(&"é".repeat(LABEL_LIMIT)), "é".repeat(LABEL_LIMIT));

        let many: Vec<Station> = (0..30)
            .map(|index| station(&"s".repeat(index + 1)))
            .collect();
        let facts = MenuFacts::of(&Snapshot::default(), false, false, &many);
        assert_eq!(facts.stations.len(), STATION_LIMIT);
    }
}
