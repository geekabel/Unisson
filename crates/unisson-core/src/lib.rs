//! Cœur d'Unisson.
//!
//! Cette bibliothèque ne dépend d'aucune interface graphique. Elle contiendra
//! le modèle commun (titre, source, file d'attente) et le moteur de lecture.
//! L'application graphique vit dans une autre crate et appelle celle-ci.

/// Version de la bibliothèque, lue dans `Cargo.toml` à la compilation.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Renvoie le nom et la version du cœur, par exemple `unisson-core 0.1.0`.
///
/// ```
/// assert!(unisson_core::identite().starts_with("unisson-core "));
/// ```
pub fn identite() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), VERSION)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identite_contient_la_version() {
        assert_eq!(identite(), format!("unisson-core {VERSION}"));
    }
}
