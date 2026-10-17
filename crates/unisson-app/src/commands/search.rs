//! The command of the unified search.

use tauri::State;
use tauri::ipc::Channel;
use unisson_core::{SearchQuery, SourceError, SourceKind, SourceResults};
use unisson_youtube::Status;

use crate::search::{LIMIT_PER_SOURCE, SearchUpdateDto};
use crate::state::AppState;

/// Searches every source that is switched on for `text`.
///
/// The results arrive through `on_update`, once at the start and then each
/// time a source answers. The last update has `done` set. The command
/// itself returns when the search is over.
#[tauri::command(async)]
pub fn search(state: State<'_, AppState>, text: String, on_update: Channel<SearchUpdateDto>) {
    let query = SearchQuery::new(&text).with_limit(LIMIT_PER_SOURCE);
    let mut answers = Vec::new();
    let mut preparing = Vec::new();
    // An empty text asks no source: the search is over at once.
    let asked = if query.is_empty() {
        Vec::new()
    } else {
        // In order of preference, and only the sources that exist.
        let registered = state.sources.kinds();
        let mut asked = state.settings().enabled_sources();
        asked.retain(|source| registered.contains(source));
        if asked.contains(&SourceKind::YouTube) {
            // The first use downloads the component, in the background.
            state.youtube.prepare();
            // Without yt-dlp, YouTube would only answer with an error each
            // time: it is left out, and the interface says why.
            if state.yt_dlp_version(false).is_none() {
                asked.retain(|source| *source != SourceKind::YouTube);
                let missing = match state
                    .youtube
                    .component()
                    .map(|component| component.status())
                {
                    Some(Status::Absent | Status::Installing { .. }) => {
                        preparing.push(SourceKind::YouTube);
                        None
                    }
                    Some(Status::Failed(reason)) => Some(reason),
                    Some(Status::Ready) => Some("the YouTube component does not start".into()),
                    // Installed by hand: the "Sources" screen explains how.
                    None => None,
                };
                if let Some(reason) = missing {
                    answers.push(SourceResults {
                        source: SourceKind::YouTube,
                        result: Err(SourceError::Unavailable(reason)),
                    });
                }
            }
        }
        asked
    };
    let update = |answers: &[SourceResults]| {
        SearchUpdateDto::new(answers, &text, &asked).with_preparing(&preparing)
    };
    // A failed send means the interface stopped listening: carry on, the
    // sources that are still working cannot be interrupted anyway.
    let _ = on_update.send(update(&answers));
    state.sources.search_each_in(&query, &asked, |results| {
        if let Err(error) = &results.result {
            eprintln!("unisson: the {} search failed: {error}", results.source);
        }
        answers.push(results);
        let _ = on_update.send(update(&answers));
    });
}
