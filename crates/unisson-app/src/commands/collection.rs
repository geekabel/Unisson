//! Commands of what the user collects: favorites, history and playlists.

use tauri::{AppHandle, Emitter, State};
use unisson_core::Media;
use unisson_radio::Station;

use crate::dto::{CommandError, MediaDto, PlaylistDto};
use crate::events::{
    FAVORITES_CHANGED, HISTORY_CHANGED, PLAYER_CHANGED, PLAYLISTS_CHANGED, STATIONS_CHANGED,
};
use crate::home::is_playable;
use crate::state::{AppState, is_station};

/// Number of media the history screen lists at most.
const HISTORY_LIMIT: usize = 300;

fn to_dtos(media: &[Media]) -> Vec<MediaDto> {
    media.iter().map(MediaDto::from).collect()
}

fn to_media(items: Vec<MediaDto>) -> Result<Vec<Media>, CommandError> {
    items.into_iter().map(Media::try_from).collect()
}

/// Returns the favorite media, the first added first. Favorite stations
/// are apart: see `stations_favorites`.
#[tauri::command(async)]
pub fn favorites_list(state: State<'_, AppState>) -> Result<Vec<MediaDto>, CommandError> {
    Ok(to_dtos(&state.store.favorites()?))
}

/// Adds a media to the favorites, or removes it.
///
/// A radio station joins the favorite stations. Anything else joins the
/// favorite media.
#[tauri::command(async)]
pub fn favorite_set(
    app: AppHandle,
    state: State<'_, AppState>,
    media: MediaDto,
    favorite: bool,
) -> Result<(), CommandError> {
    let media = Media::try_from(media)?;
    if is_station(&media) {
        if favorite {
            let station = station_of(&state, &media)?;
            state.update_stations(|stations| stations.add(station))?;
        } else {
            state.update_stations(|stations| stations.remove(&media.id.key))?;
        }
        let _ = app.emit(STATIONS_CHANGED, ());
        crate::tray::refresh(&app);
    } else {
        if favorite {
            state.store.add_favorite(&media)?;
        } else {
            state.store.remove_favorite(&media.id)?;
        }
        let _ = app.emit(FAVORITES_CHANGED, ());
    }
    // The heart of the player bar follows.
    let _ = app.emit(
        PLAYER_CHANGED,
        state.snapshot_dto(&state.session.snapshot()),
    );
    Ok(())
}

/// Rebuilds the station behind a media, to save it as a favorite.
fn station_of(state: &AppState, media: &Media) -> Result<Station, CommandError> {
    let uuid = &media.id.key;
    if let Some(station) = state.stations().get(uuid) {
        return Ok(station.clone());
    }
    // A station met in a search or played from a link: its address is known.
    let stream_url = state
        .radio
        .stream_url(uuid)
        .ok_or_else(|| CommandError::new("notFound", "the address of this station is not known"))?;
    Ok(Station {
        uuid: uuid.clone(),
        name: media.title.clone(),
        stream_url,
        homepage: None,
        favicon: media.artwork.clone(),
        tags: Vec::new(),
        country_code: None,
        codec: None,
        bitrate: None,
        hls: false,
    })
}

/// Returns the media played most recently, the latest first.
#[tauri::command(async)]
pub fn history_list(state: State<'_, AppState>) -> Result<Vec<MediaDto>, CommandError> {
    let favorites = state.stations().clone();
    let history: Vec<Media> = state
        .store
        .recently_played(HISTORY_LIMIT)?
        .into_iter()
        .filter(|media| is_playable(media, &favorites))
        .collect();
    Ok(to_dtos(&history))
}

/// Forgets the whole listening history.
#[tauri::command(async)]
pub fn history_clear(app: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError> {
    state.store.clear_history()?;
    let _ = app.emit(HISTORY_CHANGED, ());
    Ok(())
}

/// Returns the playlists, sorted by name.
#[tauri::command(async)]
pub fn playlists_list(state: State<'_, AppState>) -> Result<Vec<PlaylistDto>, CommandError> {
    Ok(state
        .store
        .playlists()?
        .iter()
        .map(PlaylistDto::from)
        .collect())
}

/// Creates an empty playlist.
#[tauri::command(async)]
pub fn playlist_create(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
) -> Result<PlaylistDto, CommandError> {
    let playlist = state.store.create_playlist(&name)?;
    let _ = app.emit(PLAYLISTS_CHANGED, ());
    Ok(PlaylistDto::from(&playlist))
}

/// Renames a playlist.
#[tauri::command(async)]
pub fn playlist_rename(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<(), CommandError> {
    found(state.store.rename_playlist(id, &name)?)?;
    let _ = app.emit(PLAYLISTS_CHANGED, ());
    Ok(())
}

/// Deletes a playlist and what it holds.
#[tauri::command(async)]
pub fn playlist_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), CommandError> {
    found(state.store.delete_playlist(id)?)?;
    let _ = app.emit(PLAYLISTS_CHANGED, ());
    Ok(())
}

/// Returns the media of a playlist, in order.
#[tauri::command(async)]
pub fn playlist_items(state: State<'_, AppState>, id: i64) -> Result<Vec<MediaDto>, CommandError> {
    Ok(to_dtos(&state.store.playlist_items(id)?))
}

/// Adds a media at the end of a playlist.
#[tauri::command(async)]
pub fn playlist_add(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    media: MediaDto,
) -> Result<(), CommandError> {
    found(state.store.add_to_playlist(id, &Media::try_from(media)?)?)?;
    let _ = app.emit(PLAYLISTS_CHANGED, ());
    Ok(())
}

/// Replaces the content of a playlist: this is how media are removed or
/// reordered.
#[tauri::command(async)]
pub fn playlist_set_items(
    app: AppHandle,
    state: State<'_, AppState>,
    id: i64,
    items: Vec<MediaDto>,
) -> Result<(), CommandError> {
    found(state.store.set_playlist_items(id, &to_media(items)?)?)?;
    let _ = app.emit(PLAYLISTS_CHANGED, ());
    Ok(())
}

/// Turns "no playlist has this identifier" into an error.
fn found(exists: bool) -> Result<(), CommandError> {
    if exists {
        Ok(())
    } else {
        Err(CommandError::new(
            "notFound",
            "this playlist no longer exists",
        ))
    }
}
