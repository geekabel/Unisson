//! Merging the answers of several sources into one list.
//!
//! # Design
//!
//! Each source answers a search on its own. The user wants one list:
//!
//! - a track found in two sources must show once, with the source that
//!   will be used first and the other ones as alternatives;
//! - the hit that matches best what was typed must come first, whatever
//!   its source, since Enter plays it.
//!
//! Two tracks are the same when their titles and their artists are equal
//! once case and accents are ignored. Radio stations are never merged: two
//! stations with the same name are two different streams.

use crate::model::{Media, MediaKind, SourceKind};
use crate::source::SourceResults;
use crate::text::search_key;

/// One line of the merged list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    /// The media to play, from the preferred source that has it.
    pub media: Media,
    /// The same track in other sources, in order of preference.
    pub alternates: Vec<Media>,
}

impl SearchHit {
    /// Returns every source that has this hit, the one used first.
    pub fn sources(&self) -> impl Iterator<Item = SourceKind> + '_ {
        std::iter::once(&self.media)
            .chain(&self.alternates)
            .map(|media| media.id.source)
    }
}

/// Merges the answers of several sources into one ranked list.
///
/// `text` is what the user typed. `priority` lists the sources from the
/// most to the least preferred: it decides which copy of a track is played,
/// and it orders hits that match equally well. Sources missing from
/// `priority` come last. Failed sources are skipped.
///
/// # Examples
///
/// ```
/// use unisson_core::{Media, MediaId, SourceKind, SourceResults, merge_results};
///
/// let track = |source| {
///     Media::track(MediaId::new(source, "x"), "Take Five").with_artist("Dave Brubeck")
/// };
/// let results = [
///     SourceResults { source: SourceKind::YouTube, result: Ok(vec![track(SourceKind::YouTube)]) },
///     SourceResults { source: SourceKind::Local, result: Ok(vec![track(SourceKind::Local)]) },
/// ];
///
/// let hits = merge_results(&results, "take five", &[SourceKind::Local, SourceKind::YouTube]);
///
/// // One line, played from the local file, with YouTube as an alternative.
/// assert_eq!(hits.len(), 1);
/// assert_eq!(hits[0].media.id.source, SourceKind::Local);
/// assert_eq!(hits[0].alternates[0].id.source, SourceKind::YouTube);
/// ```
pub fn merge_results(
    results: &[SourceResults],
    text: &str,
    priority: &[SourceKind],
) -> Vec<SearchHit> {
    let rank = |source: SourceKind| {
        priority
            .iter()
            .position(|kind| *kind == source)
            .unwrap_or(priority.len())
    };
    let mut ordered: Vec<&SourceResults> = results.iter().collect();
    ordered.sort_by_key(|results| rank(results.source));

    // Each hit goes with the identity of its track, when it can be merged.
    let mut hits: Vec<(Option<TrackIdentity>, SearchHit)> = Vec::new();
    for media in ordered
        .iter()
        .filter_map(|results| results.result.as_ref().ok())
        .flatten()
    {
        let identity = TrackIdentity::of(media);
        let same_track = identity.as_ref().and_then(|identity| {
            hits.iter_mut().find(|(other, hit)| {
                other.as_ref() == Some(identity)
                    // Two files of one source stay two lines.
                    && hit.sources().all(|source| source != media.id.source)
            })
        });
        match same_track {
            Some((_, hit)) => hit.alternates.push(media.clone()),
            None => hits.push((
                identity,
                SearchHit {
                    media: media.clone(),
                    alternates: Vec::new(),
                },
            )),
        }
    }

    let wanted = search_key(text);
    let mut hits: Vec<SearchHit> = hits.into_iter().map(|(_, hit)| hit).collect();
    // The sort is stable: equal scores keep the order of preference.
    hits.sort_by_key(|hit| std::cmp::Reverse(score(&hit.media, &wanted)));
    hits
}

/// What makes two tracks "the same" across sources.
#[derive(Debug, PartialEq, Eq)]
struct TrackIdentity {
    title: String,
    artist: String,
}

impl TrackIdentity {
    /// Returns the identity of a track, or `None` for what is never merged:
    /// stations, and tracks whose artist is unknown.
    fn of(media: &Media) -> Option<Self> {
        if media.kind != MediaKind::Track {
            return None;
        }
        let artist = search_key(media.artist.as_deref()?);
        (!artist.is_empty()).then(|| Self {
            title: search_key(&media.title),
            artist,
        })
    }
}

/// Tells how well a media matches the typed text: the higher, the better.
///
/// `wanted` must already be a [`search_key`].
fn score(media: &Media, wanted: &str) -> u8 {
    let title = search_key(&media.title);
    let artist = media.artist.as_deref().map(search_key).unwrap_or_default();
    if wanted.is_empty() {
        0
    } else if title == wanted {
        100
    } else if title.starts_with(wanted) {
        80
    } else if words_start_with(&title, wanted) {
        60
    } else if title.contains(wanted) {
        50
    } else if artist == wanted {
        40
    } else {
        // Found through its artist, its album, or several fields together.
        30
    }
}

/// Returns `true` if each word of `wanted` begins a different word of
/// `text`, in any order: `fiv ta` matches `take five`.
fn words_start_with(text: &str, wanted: &str) -> bool {
    let mut words: Vec<&str> = text.split(' ').collect();
    wanted.split(' ').all(|prefix| {
        let found = words.iter().position(|word| word.starts_with(prefix));
        found.map(|index| words.swap_remove(index)).is_some()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SourceError;
    use crate::model::MediaId;

    const PRIORITY: [SourceKind; 3] = [SourceKind::Local, SourceKind::Radio, SourceKind::YouTube];

    fn track(source: SourceKind, title: &str, artist: Option<&str>) -> Media {
        let media = Media::track(MediaId::new(source, format!("{source}:{title}")), title);
        match artist {
            Some(artist) => media.with_artist(artist),
            None => media,
        }
    }

    fn station(name: &str) -> Media {
        Media::station(MediaId::new(SourceKind::Radio, name), name)
    }

    fn found(source: SourceKind, media: Vec<Media>) -> SourceResults {
        SourceResults {
            source,
            result: Ok(media),
        }
    }

    fn titles(hits: &[SearchHit]) -> Vec<&str> {
        hits.iter().map(|hit| hit.media.title.as_str()).collect()
    }

    fn sources(hit: &SearchHit) -> Vec<SourceKind> {
        hit.sources().collect()
    }

    #[test]
    fn a_track_found_in_two_sources_has_one_line() {
        let results = [
            found(
                SourceKind::YouTube,
                vec![
                    track(
                        SourceKind::YouTube,
                        "TAKE FIVE",
                        Some("the dave brubeck quartet"),
                    ),
                    track(
                        SourceKind::YouTube,
                        "My Favorite Things",
                        Some("John Coltrane"),
                    ),
                ],
            ),
            found(
                SourceKind::Local,
                vec![track(
                    SourceKind::Local,
                    "Take Five",
                    Some("The Dave Brubeck Quartet"),
                )],
            ),
        ];

        let hits = merge_results(&results, "take", &PRIORITY);

        assert_eq!(titles(&hits), ["Take Five", "My Favorite Things"]);
        // The preferred source is used, whatever the order of the answers.
        assert_eq!(sources(&hits[0]), [SourceKind::Local, SourceKind::YouTube]);
        assert!(hits[1].alternates.is_empty());
    }

    #[test]
    fn accents_and_case_do_not_split_a_track() {
        let results = [
            found(
                SourceKind::Local,
                vec![track(SourceKind::Local, "Für Élise", Some("Beethoven"))],
            ),
            found(
                SourceKind::YouTube,
                vec![track(SourceKind::YouTube, "fur elise", Some("BEETHOVEN"))],
            ),
        ];
        assert_eq!(merge_results(&results, "elise", &PRIORITY).len(), 1);
    }

    #[test]
    fn different_artists_or_unknown_artists_are_not_merged() {
        let results = [
            found(
                SourceKind::Local,
                vec![
                    track(SourceKind::Local, "Summertime", Some("Ella Fitzgerald")),
                    track(SourceKind::Local, "Intro", None),
                ],
            ),
            found(
                SourceKind::YouTube,
                vec![
                    track(SourceKind::YouTube, "Summertime", Some("Janis Joplin")),
                    track(SourceKind::YouTube, "Intro", None),
                ],
            ),
        ];
        assert_eq!(merge_results(&results, "x", &PRIORITY).len(), 4);
    }

    #[test]
    fn two_copies_in_one_source_stay_two_lines() {
        let copy = track(SourceKind::Local, "So What", Some("Miles Davis"));
        let results = [
            found(SourceKind::Local, vec![copy.clone(), copy.clone()]),
            found(
                SourceKind::YouTube,
                vec![track(SourceKind::YouTube, "So What", Some("Miles Davis"))],
            ),
        ];

        let hits = merge_results(&results, "so what", &PRIORITY);

        assert_eq!(hits.len(), 2);
        // The copy of the other source joins the first line only.
        assert_eq!(sources(&hits[0]), [SourceKind::Local, SourceKind::YouTube]);
        assert_eq!(sources(&hits[1]), [SourceKind::Local]);
    }

    #[test]
    fn stations_are_never_merged() {
        let results = [found(
            SourceKind::Radio,
            vec![station("FIP"), station("FIP")],
        )];
        assert_eq!(merge_results(&results, "fip", &PRIORITY).len(), 2);
    }

    #[test]
    fn the_best_match_comes_first_whatever_its_source() {
        let results = [
            found(
                SourceKind::Local,
                vec![
                    track(SourceKind::Local, "Radio Song", Some("FIP Tribute Band")),
                    track(SourceKind::Local, "Fipple Flute Blues", Some("Someone")),
                    track(SourceKind::Local, "The Last Fip", Some("Someone")),
                ],
            ),
            found(SourceKind::Radio, vec![station("FIP Jazz"), station("FIP")]),
        ];

        let hits = merge_results(&results, "FIP", &PRIORITY);

        assert_eq!(
            titles(&hits),
            [
                "FIP",                // the title is exactly what was typed
                "Fipple Flute Blues", // the title starts with it
                "FIP Jazz",           // same, from a less preferred source
                "The Last Fip",       // a word of the title starts with it
                "Radio Song",         // found through the artist
            ]
        );
    }

    #[test]
    fn words_may_be_incomplete_and_in_any_order() {
        assert!(words_start_with("take five", "fiv ta"));
        assert!(words_start_with("take five", "take"));
        assert!(!words_start_with("take five", "ake"));
        // One word of the title cannot serve twice.
        assert!(!words_start_with("take five", "ta tak"));
    }

    #[test]
    fn failed_sources_are_skipped_and_unknown_sources_come_last() {
        let results = [
            SourceResults {
                source: SourceKind::Radio,
                result: Err(SourceError::Network("down".into())),
            },
            found(
                SourceKind::YouTube,
                vec![track(SourceKind::YouTube, "Same", Some("A"))],
            ),
            found(
                SourceKind::Local,
                vec![track(SourceKind::Local, "Same", Some("B"))],
            ),
        ];

        // Only YouTube is in the priority list: it wins over Local.
        let hits = merge_results(&results, "same", &[SourceKind::YouTube]);

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].media.id.source, SourceKind::YouTube);
        assert!(merge_results(&[], "same", &PRIORITY).is_empty());
    }
}
