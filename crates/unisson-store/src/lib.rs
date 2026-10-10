//! Local storage of Unisson.
//!
//! Everything the application must remember between two runs lives in one
//! SQLite database file, opened through a [`Store`]:
//!
//! - the favorite radio stations;
//! - the library: the tags of the local audio files, so they can be searched
//!   at once without reading the files again;
//! - the listening history;
//! - the favorite media;
//! - the playlists, which can mix media from every source.
//!
//! [`LocalSource`] plugs the library into the application through the
//! [`Source`](unisson_core::Source) trait.
//!
//! # Where the data lives
//!
//! [`Store::open_default`] follows the XDG Base Directory specification: the
//! database is `unisson/unisson.db` under `$XDG_DATA_HOME`, which defaults to
//! `~/.local/share`.
//!
//! # Examples
//!
//! ```
//! use unisson_radio::{Favorites, Station};
//! use unisson_store::Store;
//!
//! let store = Store::open_in_memory()?;
//! let mut favorites = Favorites::from_stations(store.favorite_stations()?);
//! favorites.add(Station::custom("My radio", "https://example.org/stream.mp3")?);
//! store.save_favorite_stations(favorites.stations())?;
//!
//! assert_eq!(store.favorite_stations()?.len(), 1);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod error;
mod favorites;
mod history;
mod library;
mod local_source;
mod media;
mod paths;
mod playlists;
mod store;

pub use error::StoreError;
pub use local_source::LocalSource;
pub use paths::default_database_path;
pub use playlists::Playlist;
pub use store::Store;
