//! What the unified search sends to the interface.
//!
//! The sources do not answer at the same speed: the local library is
//! instant, a web service is not. The interface therefore receives one
//! [`SearchUpdateDto`] each time a source answers. Each update holds the
//! whole merged list so far, so the interface only has to show the last one.

use serde::Serialize;
use unisson_core::{SourceError, SourceKind, SourceResults, merge_results};

use crate::dto::MediaDto;

/// Number of hits each source is asked for.
pub const LIMIT_PER_SOURCE: usize = 20;

/// One line of the search results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HitDto {
    /// The media to play.
    pub media: MediaDto,
    /// Every source that has it, as [`SourceKind::id`]. The first one is
    /// the source of `media`.
    pub sources: Vec<String>,
}

/// A source that could not answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchErrorDto {
    /// The source, as [`SourceKind::id`].
    pub source: String,
    /// `network`, or `unavailable` for anything else.
    pub kind: &'static str,
}

/// The state of a search, sent each time a source answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchUpdateDto {
    /// The merged results so far, the best match first.
    pub hits: Vec<HitDto>,
    /// The sources still searching, as [`SourceKind::id`].
    pub pending: Vec<String>,
    /// The sources that failed.
    pub errors: Vec<SearchErrorDto>,
    /// Whether every source has answered.
    pub done: bool,
}

impl SearchUpdateDto {
    /// Describes a search from the answers received so far.
    ///
    /// `text` is what the user typed. `asked` lists every source that was
    /// asked, from the most to the least preferred: when several sources
    /// have a track, the first one of the list is played.
    pub fn new(answers: &[SourceResults], text: &str, asked: &[SourceKind]) -> Self {
        let hits = merge_results(answers, text, asked)
            .iter()
            .map(|hit| HitDto {
                media: MediaDto::from(&hit.media),
                sources: hit.sources().map(|source| source.id().to_owned()).collect(),
            })
            .collect();
        let pending: Vec<String> = asked
            .iter()
            .filter(|source| answers.iter().all(|answer| answer.source != **source))
            .map(|source| source.id().to_owned())
            .collect();
        let errors = answers
            .iter()
            .filter_map(|answer| {
                let error = answer.result.as_ref().err()?;
                Some(SearchErrorDto {
                    source: answer.source.id().to_owned(),
                    kind: match error {
                        SourceError::Network(_) => "network",
                        _ => "unavailable",
                    },
                })
            })
            .collect();
        Self {
            hits,
            done: pending.is_empty(),
            pending,
            errors,
        }
    }
}

#[cfg(test)]
mod tests {
    use unisson_core::{Media, MediaId};

    use super::*;

    const ASKED: [SourceKind; 2] = [SourceKind::Local, SourceKind::Radio];

    fn local_answer() -> SourceResults {
        let track = Media::track(MediaId::new(SourceKind::Local, "/m/fip.mp3"), "Fip Song")
            .with_artist("Someone");
        SourceResults {
            source: SourceKind::Local,
            result: Ok(vec![track]),
        }
    }

    #[test]
    fn a_search_with_no_answer_yet_is_pending_for_every_source() {
        let update = SearchUpdateDto::new(&[], "fip", &ASKED);
        assert!(update.hits.is_empty());
        assert_eq!(update.pending, ["local", "radio"]);
        assert!(!update.done);
    }

    #[test]
    fn each_answer_shrinks_the_pending_list() {
        let update = SearchUpdateDto::new(&[local_answer()], "fip", &ASKED);
        assert_eq!(update.hits.len(), 1);
        assert_eq!(update.hits[0].sources, ["local"]);
        assert_eq!(update.pending, ["radio"]);
        assert!(!update.done);
    }

    #[test]
    fn the_last_answer_ends_the_search_and_the_best_match_leads() {
        let radio = SourceResults {
            source: SourceKind::Radio,
            result: Ok(vec![Media::station(
                MediaId::new(SourceKind::Radio, "fip"),
                "FIP",
            )]),
        };
        let update = SearchUpdateDto::new(&[local_answer(), radio], "fip", &ASKED);
        assert!(update.done);
        assert!(update.pending.is_empty());
        assert!(update.errors.is_empty());
        let titles: Vec<&str> = update
            .hits
            .iter()
            .map(|hit| hit.media.title.as_str())
            .collect();
        assert_eq!(titles, ["FIP", "Fip Song"]);
    }

    #[test]
    fn a_failed_source_is_reported_and_does_not_hide_the_others() {
        let failures = [
            SourceResults {
                source: SourceKind::Radio,
                result: Err(SourceError::Network("down".into())),
            },
            SourceResults {
                source: SourceKind::YouTube,
                result: Err(SourceError::Unavailable("no yt-dlp".into())),
            },
        ];
        let mut answers = vec![local_answer()];
        answers.extend(failures);
        let asked = [SourceKind::Local, SourceKind::Radio, SourceKind::YouTube];

        let update = SearchUpdateDto::new(&answers, "fip", &asked);

        assert!(update.done);
        assert_eq!(update.hits.len(), 1);
        let errors: Vec<(&str, &str)> = update
            .errors
            .iter()
            .map(|error| (error.source.as_str(), error.kind))
            .collect();
        assert_eq!(errors, [("radio", "network"), ("youtube", "unavailable")]);
    }
}
