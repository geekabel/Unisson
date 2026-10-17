//! YouTube source of Unisson.
//!
//! YouTube has no public way to get the audio of a video. This crate relies
//! on [yt-dlp](https://github.com/yt-dlp/yt-dlp), a separate program: it
//! searches YouTube, and it finds the address of the audio stream of a
//! video. The audio engine then plays that address like any other.
//!
//! - [`Component`] downloads yt-dlp, and Deno when needed, the first time
//!   YouTube is used, checks them, and keeps yt-dlp up to date.
//! - [`YtDlp`] runs the program and reads its answers.
//! - [`YouTubeSource`] plugs it into the application through the
//!   [`Source`](unisson_core::Source) trait.
//! - [`is_youtube_url`] recognizes a YouTube link, to play what the user
//!   pastes.
//!
//! # What to know
//!
//! This way of listening is not covered by the terms of service of
//! YouTube, and it stops working whenever YouTube changes something, until
//! yt-dlp is updated. The [`Component`] looks for a new yt-dlp each day.
//!
//! # Examples
//!
//! ```no_run
//! use unisson_core::{SearchQuery, Source};
//! use unisson_youtube::YouTubeSource;
//!
//! let youtube = YouTubeSource::new();
//! let found = youtube.search(&SearchQuery::new("miles davis so what").with_limit(5))?;
//! let address = youtube.resolve(&found[0].id)?;
//! println!("{} plays from {address}", found[0].title);
//! # Ok::<(), unisson_core::SourceError>(())
//! ```

mod component;
mod source;
mod title;
mod url;
mod ytdlp;

pub use component::{Component, Fetch, Platform, Status, Web};
pub use source::YouTubeSource;
pub use title::split_title;
pub use url::{is_youtube_url, watch_url};
pub use ytdlp::YtDlp;
