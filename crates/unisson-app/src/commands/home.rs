//! Commands of the home screen.

use tauri::State;

use crate::dto::CommandError;
use crate::home::{self, HomeDto};
use crate::state::AppState;

/// Returns what the home screen shows.
#[tauri::command(async)]
pub fn home(state: State<'_, AppState>) -> Result<HomeDto, CommandError> {
    // Copy the favorites, so the lock is not held while the database works.
    let favorites = state.stations().clone();
    Ok(home::build(&state.store, &favorites)?)
}
