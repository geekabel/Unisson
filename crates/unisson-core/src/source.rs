//! Le contrat d'une source, et le registre qui les interroge toutes.
//!
//! Les méthodes de [`Source`] sont bloquantes, et non `async`. La référence
//! Rust dit qu'une méthode `async fn` ne peut pas être appelée à travers un
//! objet-trait (`dyn Source`). Or le registre garde les sources dans un
//! `Vec<Box<dyn Source>>`. Chaque source est donc appelée dans son propre
//! fil d'exécution : une source lente ne bloque pas les autres.

use std::thread;

use crate::error::SourceError;
use crate::model::{Media, MediaId, SourceKind, StreamUri};

/// Une recherche tapée par l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchQuery {
    text: String,
    limit: usize,
}

impl SearchQuery {
    /// Nombre de résultats demandés à chaque source par défaut.
    pub const DEFAULT_LIMIT: usize = 20;

    /// Crée une recherche. Les espaces autour du texte sont retirés.
    pub fn new(text: impl AsRef<str>) -> Self {
        Self {
            text: text.as_ref().trim().to_owned(),
            limit: Self::DEFAULT_LIMIT,
        }
    }

    /// Change le nombre de résultats demandés à chaque source (au moins 1).
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit.max(1);
        self
    }

    /// Le texte recherché.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Le nombre maximal de résultats par source.
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Vrai si le texte est vide.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }
}

/// Ce que chaque source (radio, fichiers locaux, YouTube) sait faire.
///
/// `Send + Sync` permet d'appeler une source depuis plusieurs fils.
pub trait Source: Send + Sync {
    /// Quelle source est-ce ?
    fn kind(&self) -> SourceKind;

    /// Cherche des médias. Appel bloquant, possiblement lent (réseau).
    fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError>;

    /// Donne l'adresse à lire pour un média. Appelée juste avant la lecture,
    /// car certaines adresses expirent.
    fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError>;
}

/// La réponse d'une source à une recherche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceResults {
    /// La source qui répond.
    pub source: SourceKind,
    /// Ses médias, ou son erreur.
    pub result: Result<Vec<Media>, SourceError>,
}

/// L'ensemble des sources actives. Une seule source par [`SourceKind`].
#[derive(Default)]
pub struct SourceRegistry {
    sources: Vec<Box<dyn Source>>,
}

impl SourceRegistry {
    /// Crée un registre vide.
    pub fn new() -> Self {
        Self::default()
    }

    /// Ajoute une source. Si une source du même genre existe, elle est remplacée
    /// et garde sa place dans l'ordre.
    pub fn register(&mut self, source: Box<dyn Source>) {
        let kind = source.kind();
        match self
            .sources
            .iter_mut()
            .find(|existing| existing.kind() == kind)
        {
            Some(slot) => *slot = source,
            None => self.sources.push(source),
        }
    }

    /// Retire une source. Renvoie vrai si elle était présente.
    pub fn unregister(&mut self, kind: SourceKind) -> bool {
        let before = self.sources.len();
        self.sources.retain(|source| source.kind() != kind);
        self.sources.len() != before
    }

    /// Les sources actives, dans l'ordre d'enregistrement.
    pub fn kinds(&self) -> Vec<SourceKind> {
        self.sources.iter().map(|source| source.kind()).collect()
    }

    /// Interroge toutes les sources en même temps.
    ///
    /// Il y a une réponse par source, dans l'ordre d'enregistrement. Une
    /// source en erreur, ou qui plante, n'empêche pas les autres de répondre.
    /// Une recherche vide ne contacte aucune source.
    pub fn search_all(&self, query: &SearchQuery) -> Vec<SourceResults> {
        if query.is_empty() {
            return Vec::new();
        }
        thread::scope(|scope| {
            let handles: Vec<_> = self
                .sources
                .iter()
                .map(|source| (source.kind(), scope.spawn(|| source.search(query))))
                .collect();
            handles
                .into_iter()
                .map(|(source, handle)| {
                    // `join` renvoie `Err` si le fil a planté. On le transforme
                    // en erreur ordinaire pour cette seule source.
                    let result = handle.join().unwrap_or_else(|_| {
                        Err(SourceError::Unavailable("la source a planté".into()))
                    });
                    SourceResults { source, result }
                })
                .collect()
        })
    }

    /// Demande l'adresse de lecture à la source du média.
    pub fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
        self.sources
            .iter()
            .find(|source| source.kind() == id.source)
            .ok_or_else(|| SourceError::Unavailable(format!("{} n'est pas activée", id.source)))?
            .resolve(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fausse source pour les tests : elle renvoie ce qu'on lui a donné.
    struct FakeSource {
        kind: SourceKind,
        behaviour: Behaviour,
    }

    enum Behaviour {
        Titles(Vec<&'static str>),
        Fails(SourceError),
        Panics,
    }

    impl FakeSource {
        fn boxed(kind: SourceKind, behaviour: Behaviour) -> Box<dyn Source> {
            Box::new(Self { kind, behaviour })
        }
    }

    impl Source for FakeSource {
        fn kind(&self) -> SourceKind {
            self.kind
        }

        fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError> {
            match &self.behaviour {
                Behaviour::Titles(titles) => Ok(titles
                    .iter()
                    .filter(|title| title.to_lowercase().contains(&query.text().to_lowercase()))
                    .take(query.limit())
                    .map(|title| Media::track(MediaId::new(self.kind, *title), *title))
                    .collect()),
                Behaviour::Fails(error) => Err(error.clone()),
                Behaviour::Panics => panic!("panne volontaire du test"),
            }
        }

        fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
            match &self.behaviour {
                Behaviour::Titles(titles) if titles.contains(&id.key.as_str()) => {
                    StreamUri::new(format!("https://exemple.org/{}", id.key.replace(' ', "-")))
                }
                Behaviour::Titles(_) => Err(SourceError::NotFound),
                Behaviour::Fails(error) => Err(error.clone()),
                Behaviour::Panics => panic!("panne volontaire du test"),
            }
        }
    }

    fn titles(source: &SourceResults) -> Vec<&str> {
        source
            .result
            .as_ref()
            .unwrap()
            .iter()
            .map(|media| media.title.as_str())
            .collect()
    }

    #[test]
    fn query_is_trimmed_and_limited() {
        let query = SearchQuery::new("  jazz ");
        assert_eq!(query.text(), "jazz");
        assert_eq!(query.limit(), SearchQuery::DEFAULT_LIMIT);
        assert_eq!(query.with_limit(0).limit(), 1);
        assert!(SearchQuery::new("   ").is_empty());
    }

    #[test]
    fn every_source_answers_in_registration_order() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Radio,
            Behaviour::Titles(vec!["TSF Jazz", "FIP"]),
        ));
        registry.register(FakeSource::boxed(
            SourceKind::YouTube,
            Behaviour::Titles(vec!["Jazz pour travailler", "Lofi", "Smooth jazz"]),
        ));

        let results = registry.search_all(&SearchQuery::new("jazz"));

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, SourceKind::Radio);
        assert_eq!(titles(&results[0]), ["TSF Jazz"]);
        assert_eq!(results[1].source, SourceKind::YouTube);
        assert_eq!(titles(&results[1]), ["Jazz pour travailler", "Smooth jazz"]);
    }

    #[test]
    fn the_limit_applies_to_each_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["a1", "a2", "a3"]),
        ));
        let results = registry.search_all(&SearchQuery::new("a").with_limit(2));
        assert_eq!(titles(&results[0]), ["a1", "a2"]);
    }

    #[test]
    fn a_failing_or_crashing_source_does_not_break_the_others() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Radio,
            Behaviour::Fails(SourceError::Network("hors ligne".into())),
        ));
        registry.register(FakeSource::boxed(SourceKind::YouTube, Behaviour::Panics));
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["So What"]),
        ));

        let results = registry.search_all(&SearchQuery::new("so"));

        assert_eq!(
            results[0].result,
            Err(SourceError::Network("hors ligne".into()))
        );
        assert!(matches!(
            results[1].result,
            Err(SourceError::Unavailable(_))
        ));
        assert_eq!(titles(&results[2]), ["So What"]);
    }

    #[test]
    fn an_empty_query_contacts_no_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(SourceKind::YouTube, Behaviour::Panics));
        assert!(registry.search_all(&SearchQuery::new("  ")).is_empty());
    }

    #[test]
    fn registering_the_same_kind_replaces_the_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Radio,
            Behaviour::Titles(vec!["ancienne"]),
        ));
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec![]),
        ));
        registry.register(FakeSource::boxed(
            SourceKind::Radio,
            Behaviour::Titles(vec!["nouvelle"]),
        ));

        assert_eq!(registry.kinds(), [SourceKind::Radio, SourceKind::Local]);
        let results = registry.search_all(&SearchQuery::new("n"));
        assert_eq!(titles(&results[0]), ["nouvelle"]);

        assert!(registry.unregister(SourceKind::Radio));
        assert!(!registry.unregister(SourceKind::Radio));
        assert_eq!(registry.kinds(), [SourceKind::Local]);
    }

    #[test]
    fn resolve_goes_to_the_right_source() {
        let mut registry = SourceRegistry::new();
        registry.register(FakeSource::boxed(
            SourceKind::Local,
            Behaviour::Titles(vec!["So What"]),
        ));

        let found = registry.resolve(&MediaId::new(SourceKind::Local, "So What"));
        assert_eq!(found.unwrap().as_str(), "https://exemple.org/So-What");

        let missing = registry.resolve(&MediaId::new(SourceKind::Local, "inconnu"));
        assert_eq!(missing, Err(SourceError::NotFound));

        let inactive = registry.resolve(&MediaId::new(SourceKind::YouTube, "abc"));
        assert_eq!(
            inactive,
            Err(SourceError::Unavailable("YouTube n'est pas activée".into()))
        );
    }
}
