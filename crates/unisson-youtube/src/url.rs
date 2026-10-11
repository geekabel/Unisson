//! Recognizing and building YouTube addresses.

/// The host names that belong to YouTube.
const HOSTS: [&str; 5] = [
    "youtube.com",
    "youtu.be",
    "music.youtube.com",
    "m.youtube.com",
    "youtube-nocookie.com",
];

/// Tells whether a text is a link to YouTube: a video, a playlist, or
/// anything else on its sites.
///
/// # Examples
///
/// ```
/// use unisson_youtube::is_youtube_url;
///
/// assert!(is_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
/// assert!(is_youtube_url("https://youtu.be/dQw4w9WgXcQ?t=10"));
/// assert!(!is_youtube_url("https://example.org/watch?v=dQw4w9WgXcQ"));
/// ```
pub fn is_youtube_url(text: &str) -> bool {
    let text = text.trim();
    let Some(rest) = text
        .strip_prefix("https://")
        .or_else(|| text.strip_prefix("http://"))
    else {
        return false;
    };
    // The host ends at the first of these characters.
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // Drop a port, and anything before an `@`.
    let host = authority.rsplit('@').next().unwrap_or_default();
    let host = host.split(':').next().unwrap_or_default().to_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    HOSTS.contains(&host)
}

/// Returns the address of the page of a video, from its identifier.
///
/// # Examples
///
/// ```
/// use unisson_youtube::watch_url;
///
/// assert_eq!(watch_url("dQw4w9WgXcQ"), "https://www.youtube.com/watch?v=dQw4w9WgXcQ");
/// ```
pub fn watch_url(video_id: &str) -> String {
    format!("https://www.youtube.com/watch?v={video_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_to_the_sites_of_youtube_are_recognized() {
        for link in [
            "https://www.youtube.com/watch?v=abc",
            "http://youtube.com/watch?v=abc",
            "https://youtu.be/abc",
            "https://music.youtube.com/watch?v=abc&list=xyz",
            "https://m.youtube.com/watch?v=abc",
            "https://www.youtube.com/playlist?list=PL123",
            "https://www.youtube.com/shorts/abc",
            "  https://WWW.YouTube.com:443/watch?v=abc  ",
            "https://www.youtube-nocookie.com/embed/abc",
        ] {
            assert!(is_youtube_url(link), "{link}");
        }
    }

    #[test]
    fn other_links_and_plain_words_are_not() {
        for text in [
            "",
            "youtube",
            "youtube.com/watch?v=abc",
            "ftp://youtube.com/abc",
            "https://example.org/youtube.com",
            "https://notyoutube.com/watch?v=abc",
            "https://youtube.com.example.org/watch?v=abc",
            "https://youtube.com@example.org/watch",
            "https://icecast.radiofrance.fr/fip-hifi.aac",
        ] {
            assert!(!is_youtube_url(text), "{text}");
        }
    }
}
