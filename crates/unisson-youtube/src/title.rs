//! Tidying the titles of YouTube videos.
//!
//! # Design
//!
//! A music file says "title: So What, artist: Miles Davis". A YouTube video
//! only has one line of text, often "Miles Davis - So What (Official
//! Audio)", posted by a channel that may or may not be the artist.
//!
//! Splitting that line gives cleaner lists, and lets the search show one
//! line for a track that exists both on YouTube and in the local library.
//! The rules are deliberately few: when in doubt, the title is left alone.

/// Words that mark a bracketed note as a remark about the video, not as a
/// part of the title: "(Official Video)", "[Lyrics]", "(HD)".
const NOTE_WORDS: [&str; 14] = [
    "official",
    "officiel",
    "video",
    "vidéo",
    "audio",
    "lyrics",
    "lyric",
    "paroles",
    "clip",
    "visualizer",
    "hd",
    "hq",
    "4k",
    "mv",
];

/// Splits the title of a video into a track title and an artist.
///
/// `channel` is the name of the channel that posted the video.
///
/// - A channel named "Artist - Topic" holds the official audio of that
///   artist: the title is already clean.
/// - A title shaped like "Artist - Title" is split there.
/// - Otherwise the title is kept, and the channel stands for the artist.
///
/// Notes about the video at the end of the title, such as "(Official
/// Video)", are removed in every case.
///
/// # Examples
///
/// ```
/// use unisson_youtube::split_title;
///
/// let (title, artist) = split_title("Miles Davis - So What (Official Audio)", Some("MilesDavisVEVO"));
/// assert_eq!((title.as_str(), artist.as_deref()), ("So What", Some("Miles Davis")));
/// ```
pub fn split_title(raw_title: &str, channel: Option<&str>) -> (String, Option<String>) {
    let channel = channel.map(str::trim).filter(|channel| !channel.is_empty());
    let cleaned = strip_notes(raw_title);

    if let Some(artist) = channel.and_then(|channel| channel.strip_suffix(" - Topic")) {
        return (cleaned, Some(artist.trim().to_owned()));
    }
    // Exactly one separator: with more, there is no telling which is which.
    let mut parts = cleaned.split(" - ");
    if let (Some(artist), Some(title), None) = (parts.next(), parts.next(), parts.next()) {
        let (artist, title) = (artist.trim(), title.trim());
        if !artist.is_empty() && !title.is_empty() {
            return (title.to_owned(), Some(artist.to_owned()));
        }
    }
    (cleaned, channel.map(str::to_owned))
}

/// Removes the bracketed notes about the video at the end of a title.
///
/// A title made only of such a note is kept as it is.
fn strip_notes(raw_title: &str) -> String {
    let mut title = raw_title.trim();
    while let Some(open) = last_group_start(title) {
        let note = &title[open + 1..title.len() - 1];
        let before = title[..open].trim_end();
        if before.is_empty() || !is_note(note) {
            break;
        }
        title = before;
    }
    title.to_owned()
}

/// Returns where the bracketed group that ends the title starts, if the
/// title ends with one: `(...)` or `[...]`.
fn last_group_start(title: &str) -> Option<usize> {
    let open = match title.chars().last()? {
        ')' => '(',
        ']' => '[',
        _ => return None,
    };
    title.rfind(open)
}

/// Tells whether the inside of a bracketed group is a remark about the
/// video.
fn is_note(inside: &str) -> bool {
    inside
        .split(|letter: char| !letter.is_alphanumeric())
        .any(|word| NOTE_WORDS.contains(&word.to_lowercase().as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn split(title: &str, channel: Option<&str>) -> (String, Option<String>) {
        split_title(title, channel)
    }

    fn pair(title: &str, artist: Option<&str>) -> (String, Option<String>) {
        (title.to_owned(), artist.map(str::to_owned))
    }

    #[test]
    fn an_artist_and_a_title_are_split() {
        assert_eq!(
            split("Tems - Me & U (Official Video)", Some("TemsVEVO")),
            pair("Me & U", Some("Tems"))
        );
        assert_eq!(
            split("  Dave Brubeck  -  Take Five  ", None),
            pair("Take Five", Some("Dave Brubeck"))
        );
    }

    #[test]
    fn a_topic_channel_names_the_artist() {
        assert_eq!(
            split("So What", Some("Miles Davis - Topic")),
            pair("So What", Some("Miles Davis"))
        );
        // The title of a topic channel is trusted, even with a dash in it.
        assert_eq!(
            split("Blue - Green", Some("Miles Davis - Topic")),
            pair("Blue - Green", Some("Miles Davis"))
        );
    }

    #[test]
    fn without_a_clear_shape_the_channel_stands_for_the_artist() {
        assert_eq!(
            split("lofi hip hop radio", Some("Lofi Girl")),
            pair("lofi hip hop radio", Some("Lofi Girl"))
        );
        assert_eq!(split("Untitled", None), pair("Untitled", None));
        assert_eq!(split("Untitled", Some("  ")), pair("Untitled", None));
        // Two separators: no guess.
        assert_eq!(
            split("Live - Paris - 1964", Some("Jazz Archive")),
            pair("Live - Paris - 1964", Some("Jazz Archive"))
        );
        // A hyphen inside a word is not a separator.
        assert_eq!(
            split("Jay-Z interview", Some("Channel")),
            pair("Jay-Z interview", Some("Channel"))
        );
    }

    #[test]
    fn notes_about_the_video_are_removed_from_the_end() {
        assert_eq!(strip_notes("Song (Official Video) [HD]"), "Song");
        assert_eq!(strip_notes("Song (Clip officiel)"), "Song");
        assert_eq!(strip_notes("Song [Lyrics]"), "Song");
        assert_eq!(strip_notes("Song (Official Music Video) (4K)"), "Song");
    }

    #[test]
    fn brackets_that_belong_to_the_title_are_kept() {
        assert_eq!(
            strip_notes("Song (Live at Montreux)"),
            "Song (Live at Montreux)"
        );
        assert_eq!(strip_notes("Song (feat. Someone)"), "Song (feat. Someone)");
        assert_eq!(
            strip_notes("Song (Remastered 2011)"),
            "Song (Remastered 2011)"
        );
        // Only the end of the title is looked at.
        assert_eq!(strip_notes("(Official) Song"), "(Official) Song");
        // A title cannot be emptied.
        assert_eq!(strip_notes("(Official Video)"), "(Official Video)");
        assert_eq!(strip_notes("Song (unbalanced"), "Song (unbalanced");
        assert_eq!(strip_notes(""), "");
    }
}
