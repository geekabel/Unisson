//! Commands of the queue panel.

use tauri::{AppHandle, Emitter, State};

use crate::dto::QueueDto;
use crate::events::QUEUE_CHANGED;
use crate::state::AppState;

/// Returns the queue, and what plays when it runs out.
#[tauri::command]
pub fn queue_state(state: State<'_, AppState>) -> QueueDto {
    QueueDto::new(
        &state.session.queue(),
        state.last_radio().as_ref(),
        state.resume_radio(),
    )
}

/// Jumps to an upcoming media, and plays it. `index` counts from the next
/// media, starting at 0.
#[tauri::command]
pub fn queue_jump(state: State<'_, AppState>, index: usize) {
    state.session.jump_to(index);
}

/// Removes an upcoming media.
#[tauri::command]
pub fn queue_remove(state: State<'_, AppState>, index: usize) {
    state.session.remove(index);
}

/// Moves an upcoming media to another position.
#[tauri::command]
pub fn queue_move(state: State<'_, AppState>, from: usize, to: usize) {
    state.session.move_item(from, to);
}

/// Removes every upcoming media.
#[tauri::command]
pub fn queue_clear(state: State<'_, AppState>) {
    state.session.clear_queue();
}

/// Chooses whether the last radio station takes over when the queue runs
/// out.
#[tauri::command]
pub fn queue_set_resume_radio(app: AppHandle, state: State<'_, AppState>, enabled: bool) {
    state.set_resume_radio(enabled);
    // The session only reports a change when a station is known: tell the
    // interface in every case, so the switch shows what was chosen.
    let _ = app.emit(QUEUE_CHANGED, ());
}
