//! Commands of the radio screen: favorite stations and the directory.

use tauri::{AppHandle, Emitter, State};
use unisson_core::SourceError;
use unisson_radio::{Station, StationQuery};

use crate::dto::{CommandError, StationDto};
use crate::events::{PLAYER_CHANGED, STATIONS_CHANGED};
use crate::state::AppState;

/// Number of stations a search of the directory returns at most.
const SEARCH_LIMIT: usize = 40;

/// Returns the favorite stations, in the order chosen by the user.
#[tauri::command]
pub fn stations_favorites(state: State<'_, AppState>) -> Vec<StationDto> {
    state
        .stations()
        .stations()
        .iter()
        .map(StationDto::from)
        .collect()
}

/// Searches the directory of stations.
///
/// Every criterion is optional. Without any, the most listened stations of
/// the directory are returned.
#[tauri::command(async)]
pub fn stations_search(
    state: State<'_, AppState>,
    name: Option<String>,
    tag: Option<String>,
    country_code: Option<String>,
) -> Result<Vec<StationDto>, CommandError> {
    let query = StationQuery::new()
        .name(name.unwrap_or_default())
        .tag(tag.unwrap_or_default())
        .country_code(country_code.unwrap_or_default())
        .limit(SEARCH_LIMIT);
    let stations = state.radio.client().search(&query).map_err(source_error)?;
    Ok(stations.iter().map(StationDto::from).collect())
}

/// Replaces the queue with `stations`, and plays `stations[start]`.
#[tauri::command]
pub fn stations_play(
    state: State<'_, AppState>,
    stations: Vec<StationDto>,
    start: usize,
) -> Result<(), CommandError> {
    if start >= stations.len() {
        return Err(CommandError::invalid("the start index is out of range"));
    }
    let stations: Vec<Station> = stations.into_iter().map(Station::from).collect();
    for station in &stations {
        // Saves a request to the directory when the station starts.
        state.radio.remember(station);
    }
    let media = stations.iter().map(Station::to_media).collect();
    state.session.play_list(media, start);
    Ok(())
}

/// Adds a station to the favorites, or removes it.
#[tauri::command(async)]
pub fn station_set_favorite(
    app: AppHandle,
    state: State<'_, AppState>,
    station: StationDto,
    favorite: bool,
) -> Result<(), CommandError> {
    let station = Station::from(station);
    state.radio.remember(&station);
    state.update_stations(|stations| {
        if favorite {
            stations.add(station);
        } else {
            stations.remove(&station.uuid);
        }
    })?;
    stations_changed(&app, &state);
    Ok(())
}

/// Adds to the favorites a station the directory does not know, from the
/// address of its stream.
#[tauri::command(async)]
pub fn station_add_custom(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    stream_url: String,
) -> Result<StationDto, CommandError> {
    let station = Station::custom(&name, &stream_url).map_err(source_error)?;
    state.radio.remember(&station);
    let added = station.clone();
    state.update_stations(|stations| stations.add(added))?;
    stations_changed(&app, &state);
    Ok(StationDto::from(&station))
}

/// Tells the interface that the favorite stations changed.
fn stations_changed(app: &AppHandle, state: &AppState) {
    // A failed emit means the window is closing: nothing to do.
    let _ = app.emit(STATIONS_CHANGED, ());
    crate::tray::refresh(app);
    // "Previous" and "next" depend on the favorites while a station plays.
    let _ = app.emit(
        PLAYER_CHANGED,
        state.snapshot_dto(&state.session.snapshot()),
    );
}

fn source_error(error: SourceError) -> CommandError {
    let kind = match error {
        SourceError::Network(_) => "network",
        SourceError::NotFound => "notFound",
        SourceError::Invalid(_) => "invalid",
        _ => "unavailable",
    };
    CommandError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_errors_keep_their_kind() {
        let kind = |error| source_error(error).kind;
        assert_eq!(kind(SourceError::Network("down".into())), "network");
        assert_eq!(kind(SourceError::NotFound), "notFound");
        assert_eq!(kind(SourceError::Invalid("no name".into())), "invalid");
        assert_eq!(kind(SourceError::Unavailable("off".into())), "unavailable");
    }
}
