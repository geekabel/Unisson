//! Internet radio source of Unisson.
//!
//! Stations come from [Radio Browser](https://www.radio-browser.info), a free
//! community directory that needs no account. The crate exposes two levels:
//!
//! - [`RadioBrowser`] is the client of the directory: search stations by
//!   name, tag and country, fetch one station, count a click.
//! - [`RadioSource`] plugs the directory into the application through the
//!   [`Source`](unisson_core::Source) trait.
//!
//! # Examples
//!
//! Searching the most popular French jazz stations:
//!
//! ```no_run
//! use unisson_radio::{RadioBrowser, StationQuery};
//!
//! let client = RadioBrowser::new();
//! let query = StationQuery::new().tag("jazz").country_code("FR").limit(5);
//! for station in client.search(&query)? {
//!     println!("{} -> {}", station.name, station.stream_url);
//! }
//! # Ok::<(), unisson_core::SourceError>(())
//! ```

mod client;
mod source;
mod station;
#[cfg(test)]
mod test_server;

pub use client::{RadioBrowser, StationQuery};
pub use source::RadioSource;
pub use station::Station;
