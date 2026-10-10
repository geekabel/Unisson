//! The commands the web interface can call.
//!
//! Each function here is one entry of the `invoke` API of Tauri. They are
//! thin on purpose: they translate, and let the other crates do the work.
//!
//! A command that only passes an order to the playback session returns at
//! once, and runs on the main thread. A command that reads the database or
//! a file is marked `async`, so that Tauri runs it on another thread and
//! the window never freezes.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::State;
use unisson_core::Media;
use unisson_local::{LocalError, LocalTrack};
use unisson_radio::Station;

use crate::dto::{CommandError, MediaDto, SnapshotDto};
use crate::state::AppState;

/// Returns the current playback state.
#[tauri::command]
pub fn player_state(state: State<'_, AppState>) -> SnapshotDto {
    SnapshotDto::from(&state.session.snapshot())
}

/// Replaces the queue with `items`, and plays `items[start]`.
#[tauri::command]
pub fn player_play_list(
    state: State<'_, AppState>,
    items: Vec<MediaDto>,
    start: usize,
) -> Result<(), CommandError> {
    let items = items
        .into_iter()
        .map(Media::try_from)
        .collect::<Result<Vec<_>, _>>()?;
    if start >= items.len() {
        return Err(CommandError::invalid("the start index is out of range"));
    }
    state.session.play_list(items, start);
    Ok(())
}

/// Plays what the user typed or pasted: a stream address or a file path.
#[tauri::command(async)]
pub fn player_open(state: State<'_, AppState>, target: String) -> Result<(), CommandError> {
    let media = match Target::parse(&target)? {
        Target::Stream(address) => {
            let station = Station::custom(&stream_name(&address), &address)
                .map_err(|error| CommandError::invalid(error.to_string()))?;
            // The directory does not know this station: teach its address.
            state.radio.remember(&station);
            station.to_media()
        }
        Target::File(path) => LocalTrack::read(&path)
            .map_err(|error| match error {
                LocalError::Io(detail) => CommandError::new("notFound", detail),
                other => CommandError::new("unreadable", other.to_string()),
            })?
            .to_media(),
    };
    state.session.play_list(vec![media], 0);
    Ok(())
}

/// Pauses if something plays, resumes otherwise.
#[tauri::command]
pub fn player_toggle(state: State<'_, AppState>) {
    state.session.toggle();
}

/// Plays the next media of the queue.
#[tauri::command]
pub fn player_next(state: State<'_, AppState>) {
    state.session.next();
}

/// Restarts the current track, or goes back to the previous media.
#[tauri::command]
pub fn player_previous(state: State<'_, AppState>) {
    state.session.previous();
}

/// Moves playback to a position, in milliseconds.
#[tauri::command]
pub fn player_seek(state: State<'_, AppState>, position_ms: u64) {
    state.session.seek(Duration::from_millis(position_ms));
}

/// Sets the volume, from `0.0` to `1.0`.
#[tauri::command]
pub fn player_set_volume(state: State<'_, AppState>, volume: f64) {
    state.session.set_volume(volume);
}

/// Returns every track of the local library.
#[tauri::command(async)]
pub fn library_tracks(state: State<'_, AppState>) -> Result<Vec<MediaDto>, CommandError> {
    let tracks = state.store.library_tracks()?;
    Ok(tracks
        .iter()
        .map(|track| MediaDto::from(&track.to_media()))
        .collect())
}

/// Returns the favorite radio stations, in the order chosen by the user.
#[tauri::command(async)]
pub fn favorite_stations(state: State<'_, AppState>) -> Result<Vec<MediaDto>, CommandError> {
    let stations = state.store.favorite_stations()?;
    Ok(stations
        .iter()
        .map(|station| MediaDto::from(&station.to_media()))
        .collect())
}

/// What the user asked to open.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    /// A network address.
    Stream(String),
    /// A file on this computer.
    File(PathBuf),
}

impl Target {
    /// Tells a stream address from a file path.
    ///
    /// A path may start with `~/`, for the home folder.
    fn parse(text: &str) -> Result<Self, CommandError> {
        let text = text.trim();
        if text.starts_with("http://") || text.starts_with("https://") {
            return Ok(Self::Stream(text.to_owned()));
        }
        let path = match text.strip_prefix("~/") {
            Some(rest) => std::env::var_os("HOME").map(|home| Path::new(&home).join(rest)),
            None => Some(PathBuf::from(text)),
        };
        match path {
            Some(path) if path.is_absolute() => Ok(Self::File(path)),
            _ => Err(CommandError::invalid(
                "expected a stream address or the full path of a file",
            )),
        }
    }
}

/// Names a stream after its host: `https://icecast.radiofrance.fr/fip.mp3`
/// becomes `icecast.radiofrance.fr`.
fn stream_name(address: &str) -> String {
    let host = address
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split(['/', '?', '#']).next())
        .map(|authority| authority.rsplit('@').next().unwrap_or(authority))
        .filter(|host| !host.is_empty());
    host.unwrap_or(address).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_network_address_is_a_stream() {
        for address in ["http://a.fr/x.mp3", "  https://a.fr/live?x=1 "] {
            assert_eq!(
                Target::parse(address),
                Ok(Target::Stream(address.trim().to_owned()))
            );
        }
    }

    #[test]
    fn a_full_path_is_a_file() {
        assert_eq!(
            Target::parse("/home/me/Music/a b.mp3"),
            Ok(Target::File(PathBuf::from("/home/me/Music/a b.mp3")))
        );
        if let Some(home) = std::env::var_os("HOME") {
            assert_eq!(
                Target::parse("~/Music/a.mp3"),
                Ok(Target::File(Path::new(&home).join("Music/a.mp3")))
            );
        }
    }

    #[test]
    fn anything_else_is_refused() {
        for text in ["", "   ", "miles davis", "Music/a.mp3", "ftp://a.fr/x"] {
            assert_eq!(Target::parse(text).unwrap_err().kind, "invalid", "{text}");
        }
    }

    #[test]
    fn a_stream_is_named_after_its_host() {
        assert_eq!(
            stream_name("https://icecast.radiofrance.fr/fip-hifi.aac?id=1"),
            "icecast.radiofrance.fr"
        );
        assert_eq!(
            stream_name("http://user:pw@example.org:8000/"),
            "example.org:8000"
        );
        assert_eq!(stream_name("http://example.org"), "example.org");
    }
}
