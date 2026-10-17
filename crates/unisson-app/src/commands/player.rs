//! Commands of the player: what plays, and how.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::State;
use unisson_core::{Media, MediaKind, SourceError};
use unisson_local::{LocalError, LocalTrack};
use unisson_radio::{Station, is_finite_file};
use unisson_youtube::is_youtube_url;

use crate::dto::{CommandError, MediaDto, SnapshotDto};
use crate::state::AppState;

/// Returns the current playback state.
#[tauri::command]
pub fn player_state(state: State<'_, AppState>) -> SnapshotDto {
    state.snapshot_dto(&state.session.snapshot())
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

/// Plays a media right away, and keeps the rest of the queue.
#[tauri::command]
pub fn player_play_now(state: State<'_, AppState>, media: MediaDto) -> Result<(), CommandError> {
    state.session.play_now(Media::try_from(media)?);
    Ok(())
}

/// Adds a media right after the current one.
#[tauri::command]
pub fn player_play_next(state: State<'_, AppState>, media: MediaDto) -> Result<(), CommandError> {
    state.session.play_next(Media::try_from(media)?);
    Ok(())
}

/// Adds a media at the end of the queue.
#[tauri::command]
pub fn player_enqueue(state: State<'_, AppState>, media: MediaDto) -> Result<(), CommandError> {
    state.session.enqueue(Media::try_from(media)?);
    Ok(())
}

/// Plays what the user typed or pasted: a YouTube or Spotify link, the
/// address of a stream or of an audio file, or the path of a file on this
/// computer.
///
/// A link to a YouTube playlist plays the whole playlist.
#[tauri::command(async)]
pub fn player_open(state: State<'_, AppState>, target: String) -> Result<(), CommandError> {
    let media = match Target::parse(&target)? {
        Target::YouTube(link) => {
            state.youtube.prepare();
            state.youtube.tool().lookup(&link).map_err(source_error)?
        }
        Target::Spotify(link) => vec![state.spotify_source.lookup(&link).map_err(source_error)?],
        Target::Stream(address) => {
            let station = Station::custom(&stream_name(&address), &address)
                .map_err(|error| CommandError::invalid(error.to_string()))?;
            // The directory does not know this address: teach it.
            state.radio.remember(&station);
            let mut media = station.to_media();
            // A file has an end, and can be moved through: play it as a
            // track. A radio stream is kept alive when it drops.
            if is_finite_file(&address) {
                media.kind = MediaKind::Track;
            }
            vec![media]
        }
        Target::File(path) => vec![
            LocalTrack::read(&path)
                .map_err(|error| match error {
                    LocalError::Io(detail) => CommandError::new("notFound", detail),
                    other => CommandError::new("unreadable", other.to_string()),
                })?
                .to_media(),
        ],
    };
    state.session.play_list(media, 0);
    Ok(())
}

pub(crate) fn source_error(error: SourceError) -> CommandError {
    let kind = match error {
        SourceError::Network(_) => "network",
        SourceError::NotFound => "notFound",
        SourceError::Invalid(_) => "invalid",
        _ => "unavailable",
    };
    CommandError::new(kind, error.to_string())
}

/// Pauses if something plays, resumes otherwise.
#[tauri::command]
pub fn player_toggle(state: State<'_, AppState>) {
    state.session.toggle();
}

/// Plays the next media of the queue, or the next favorite station.
#[tauri::command]
pub fn player_next(state: State<'_, AppState>) {
    state.next();
}

/// Restarts the current track, or goes back to the previous media or
/// favorite station.
#[tauri::command]
pub fn player_previous(state: State<'_, AppState>) {
    state.previous();
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

/// What the user asked to open.
#[derive(Debug, PartialEq, Eq)]
enum Target {
    /// A link to YouTube: a video or a playlist.
    YouTube(String),
    /// A link to a Spotify track, or its `spotify:track:` address.
    Spotify(String),
    /// Any other network address.
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
        if is_youtube_url(text) {
            return Ok(Self::YouTube(text.to_owned()));
        }
        if unisson_spotify::track_id(text).is_some() {
            return Ok(Self::Spotify(text.to_owned()));
        }
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
    fn a_youtube_link_is_told_apart_from_other_addresses() {
        for link in [
            "https://www.youtube.com/watch?v=abc",
            " https://youtu.be/abc ",
        ] {
            assert_eq!(
                Target::parse(link),
                Ok(Target::YouTube(link.trim().to_owned()))
            );
        }
        assert_eq!(
            Target::parse("https://example.org/youtube.mp3"),
            Ok(Target::Stream("https://example.org/youtube.mp3".to_owned()))
        );
    }

    #[test]
    fn a_spotify_link_is_told_apart_from_other_addresses() {
        for link in [
            "https://open.spotify.com/track/6rqhFgbbKwnb9MLmUQDhG6?si=x",
            "spotify:track:6rqhFgbbKwnb9MLmUQDhG6",
        ] {
            assert_eq!(Target::parse(link), Ok(Target::Spotify(link.to_owned())));
        }
        assert_eq!(
            Target::parse("https://open.spotify.com/album/6rqhFgbbKwnb9MLmUQDhG6"),
            Ok(Target::Stream(
                "https://open.spotify.com/album/6rqhFgbbKwnb9MLmUQDhG6".to_owned()
            ))
        );
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
