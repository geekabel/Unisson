//! The command of the unified search.

use tauri::State;
use tauri::ipc::Channel;
use unisson_core::SearchQuery;

use crate::search::{LIMIT_PER_SOURCE, SearchUpdateDto};
use crate::state::AppState;

/// Searches every active source for `text`.
///
/// The results arrive through `on_update`, once at the start and then each
/// time a source answers. The last update has `done` set. The command
/// itself returns when the search is over.
#[tauri::command(async)]
pub fn search(state: State<'_, AppState>, text: String, on_update: Channel<SearchUpdateDto>) {
    let query = SearchQuery::new(&text).with_limit(LIMIT_PER_SOURCE);
    // An empty text asks no source: the search is over at once.
    let asked = if query.is_empty() {
        Vec::new()
    } else {
        state.sources.kinds()
    };
    let mut answers = Vec::new();
    // A failed send means the interface stopped listening: carry on, the
    // sources that are still working cannot be interrupted anyway.
    let _ = on_update.send(SearchUpdateDto::new(&answers, &text, &asked));
    state.sources.search_each(&query, |results| {
        answers.push(results);
        let _ = on_update.send(SearchUpdateDto::new(&answers, &text, &asked));
    });
}
