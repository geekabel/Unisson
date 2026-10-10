//! Erreurs qu'une source peut renvoyer.

use std::error::Error;
use std::fmt;

/// Ce qui peut mal se passer quand on interroge une source.
///
/// Les messages sont écrits pour être montrés à l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// Le réseau ou le serveur distant n'a pas répondu correctement.
    Network(String),
    /// Le média demandé n'existe pas, ou n'existe plus, dans la source.
    NotFound,
    /// La source ne peut pas répondre (outil manquant, source non enregistrée, panne).
    Unavailable(String),
    /// La demande ou la réponse est mal formée.
    Invalid(String),
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(detail) => write!(f, "problème de réseau : {detail}"),
            Self::NotFound => write!(f, "média introuvable"),
            Self::Unavailable(detail) => write!(f, "source indisponible : {detail}"),
            Self::Invalid(detail) => write!(f, "donnée invalide : {detail}"),
        }
    }
}

impl Error for SourceError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_readable() {
        let error = SourceError::Network("délai dépassé".into());
        assert_eq!(error.to_string(), "problème de réseau : délai dépassé");
        assert_eq!(SourceError::NotFound.to_string(), "média introuvable");
    }

    #[test]
    fn works_as_a_standard_error() {
        let boxed: Box<dyn Error> = Box::new(SourceError::NotFound);
        assert_eq!(boxed.to_string(), "média introuvable");
    }
}
