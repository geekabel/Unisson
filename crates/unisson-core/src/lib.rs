//! Cœur d'Unisson.
//!
//! Cette bibliothèque ne dépend d'aucune interface graphique. Elle contient
//! le modèle commun ([`Media`], [`MediaId`]), le contrat que chaque source
//! respecte ([`Source`]) et le registre qui interroge toutes les sources
//! ([`SourceRegistry`]). L'application graphique vit dans une autre crate.
//!
//! Convention : les noms dans le code sont en anglais, les commentaires en
//! français.

pub mod error;
pub mod model;
pub mod source;

pub use error::SourceError;
pub use model::{Media, MediaId, MediaKind, SourceKind, StreamUri};
pub use source::{SearchQuery, Source, SourceRegistry, SourceResults};

/// Version de la bibliothèque, lue dans `Cargo.toml` à la compilation.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Renvoie le nom et la version du cœur, par exemple `unisson-core 0.1.0`.
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
