//! Core library of Unisson, a unified audio player for Linux.
//!
//! This crate has no dependency on any user interface. It provides:
//!
//! - the common model shared by every source ([`Media`], [`MediaId`]);
//! - the contract each source implements ([`Source`]);
//! - the registry that queries all sources at once ([`SourceRegistry`]);
//! - the merge of their answers into one ranked list ([`merge_results`]);
//! - the play queue that orders media from every source ([`PlayQueue`]).
//!
//! The desktop application lives in a separate crate and calls into this one.
//!
//! # Examples
//!
//! Searching every registered source:
//!
//! ```
//! use unisson_core::{SearchQuery, SourceRegistry};
//!
//! let registry = SourceRegistry::new();
//! let results = registry.search_all(&SearchQuery::new("jazz"));
//! // No source is registered yet, so there is nothing to report.
//! assert!(results.is_empty());
//! ```

pub mod error;
pub mod model;
pub mod queue;
pub mod search;
pub mod source;
pub mod text;

pub use error::SourceError;
pub use model::{Media, MediaId, MediaKind, SourceKind, StreamUri};
pub use queue::PlayQueue;
pub use search::{SearchHit, merge_results};
pub use source::{SearchQuery, Source, SourceRegistry, SourceResults};
pub use text::search_key;

/// Version of this crate, read from `Cargo.toml` at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns the crate name followed by its version, such as `unisson-core 0.1.0`.
///
/// # Examples
///
/// ```
/// assert!(unisson_core::identity().starts_with("unisson-core "));
/// ```
pub fn identity() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_contains_the_version() {
        assert_eq!(identity(), format!("unisson-core {VERSION}"));
    }
}
