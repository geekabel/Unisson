//! Modèle commun à toutes les sources.
//!
//! Un [`Media`] décrit ce qu'on peut écouter : un titre ou une station de
//! radio. Il ne contient pas l'adresse du flux. Cette adresse ([`StreamUri`])
//! est demandée à la source au moment de lire, car elle peut expirer.

use std::fmt;
use std::time::Duration;

use crate::error::SourceError;

/// Les sources connues d'Unisson.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourceKind {
    /// Radios en ligne.
    Radio,
    /// Fichiers audio du disque.
    Local,
    /// YouTube.
    YouTube,
}

impl SourceKind {
    /// Nom court affiché dans l'interface (la pastille de source).
    pub fn label(self) -> &'static str {
        match self {
            Self::Radio => "Radio",
            Self::Local => "Local",
            Self::YouTube => "YouTube",
        }
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// Identifiant d'un média : sa source, plus une clé propre à cette source.
///
/// La clé est par exemple l'identifiant d'une station, le chemin d'un
/// fichier ou l'identifiant d'une vidéo.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MediaId {
    /// La source qui connaît ce média.
    pub source: SourceKind,
    /// La clé du média dans cette source.
    pub key: String,
}

impl MediaId {
    /// Crée un identifiant.
    pub fn new(source: SourceKind, key: impl Into<String>) -> Self {
        Self {
            source,
            key: key.into(),
        }
    }
}

/// La nature d'un média.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MediaKind {
    /// Un titre avec un début et une fin.
    Track,
    /// Une station en direct, sans fin.
    Station,
}

/// Ce qu'on peut mettre dans la file d'attente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    /// Identifiant du média.
    pub id: MediaId,
    /// Titre ou station.
    pub kind: MediaKind,
    /// Titre du morceau, ou nom de la station.
    pub title: String,
    /// Artiste, si la source le connaît.
    pub artist: Option<String>,
    /// Album, si la source le connaît.
    pub album: Option<String>,
    /// Durée. Toujours absente pour une station en direct.
    pub duration: Option<Duration>,
    /// Adresse ou chemin de l'image (pochette, logo de station).
    pub artwork: Option<String>,
}

impl Media {
    /// Crée un titre. Les autres champs se règlent avec les méthodes `with_*`.
    ///
    /// ```
    /// use std::time::Duration;
    /// use unisson_core::{Media, MediaId, SourceKind};
    ///
    /// let media = Media::track(MediaId::new(SourceKind::Local, "/musique/a.flac"), "So What")
    ///     .with_artist("Miles Davis")
    ///     .with_duration(Duration::from_secs(562));
    /// assert!(!media.is_live());
    /// ```
    pub fn track(id: MediaId, title: impl Into<String>) -> Self {
        Self::new(id, MediaKind::Track, title)
    }

    /// Crée une station de radio en direct.
    pub fn station(id: MediaId, title: impl Into<String>) -> Self {
        Self::new(id, MediaKind::Station, title)
    }

    fn new(id: MediaId, kind: MediaKind, title: impl Into<String>) -> Self {
        Self {
            id,
            kind,
            title: title.into(),
            artist: None,
            album: None,
            duration: None,
            artwork: None,
        }
    }

    /// Ajoute l'artiste.
    pub fn with_artist(mut self, artist: impl Into<String>) -> Self {
        self.artist = Some(artist.into());
        self
    }

    /// Ajoute l'album.
    pub fn with_album(mut self, album: impl Into<String>) -> Self {
        self.album = Some(album.into());
        self
    }

    /// Ajoute la durée. Sans effet sur une station en direct.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        if self.kind == MediaKind::Track {
            self.duration = Some(duration);
        }
        self
    }

    /// Ajoute l'image.
    pub fn with_artwork(mut self, artwork: impl Into<String>) -> Self {
        self.artwork = Some(artwork.into());
        self
    }

    /// Vrai pour un direct : pas de durée, pas de barre de progression.
    pub fn is_live(&self) -> bool {
        self.kind == MediaKind::Station
    }
}

/// Adresse absolue qu'un moteur audio peut lire.
///
/// Seuls les schémas `file://`, `http://` et `https://` sont acceptés.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StreamUri(String);

impl StreamUri {
    const SCHEMES: [&'static str; 3] = ["file://", "http://", "https://"];

    /// Vérifie puis enveloppe une adresse.
    ///
    /// ```
    /// use unisson_core::StreamUri;
    ///
    /// assert!(StreamUri::new("https://exemple.org/flux.mp3").is_ok());
    /// assert!(StreamUri::new("flux.mp3").is_err());
    /// ```
    pub fn new(uri: impl Into<String>) -> Result<Self, SourceError> {
        let uri = uri.into();
        let known = Self::SCHEMES
            .iter()
            .any(|scheme| uri.len() > scheme.len() && uri.starts_with(scheme));
        if known {
            Ok(Self(uri))
        } else {
            Err(SourceError::Invalid(format!(
                "adresse de flux non reconnue : {uri}"
            )))
        }
    }

    /// L'adresse sous forme de texte.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StreamUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_station_is_live_and_has_no_duration() {
        let id = MediaId::new(SourceKind::Radio, "fip");
        let station = Media::station(id, "FIP").with_duration(Duration::from_secs(60));
        assert!(station.is_live());
        assert_eq!(station.duration, None);
    }

    #[test]
    fn a_track_keeps_its_details() {
        let id = MediaId::new(SourceKind::YouTube, "abc123");
        let track = Media::track(id.clone(), "Take Five")
            .with_artist("The Dave Brubeck Quartet")
            .with_album("Time Out")
            .with_duration(Duration::from_secs(324))
            .with_artwork("https://exemple.org/pochette.jpg");
        assert_eq!(track.id, id);
        assert!(!track.is_live());
        assert_eq!(track.artist.as_deref(), Some("The Dave Brubeck Quartet"));
        assert_eq!(track.album.as_deref(), Some("Time Out"));
        assert_eq!(track.duration, Some(Duration::from_secs(324)));
        assert!(track.artwork.is_some());
    }

    #[test]
    fn stream_uri_accepts_only_known_schemes() {
        for good in ["file:///home/a.mp3", "http://a.fr/x", "https://a.fr/x"] {
            assert_eq!(StreamUri::new(good).unwrap().as_str(), good);
        }
        for bad in ["", "a.mp3", "/home/a.mp3", "ftp://a.fr/x", "https://"] {
            assert!(
                matches!(StreamUri::new(bad), Err(SourceError::Invalid(_))),
                "{bad}"
            );
        }
    }

    #[test]
    fn source_labels_match_the_interface() {
        assert_eq!(SourceKind::Radio.to_string(), "Radio");
        assert_eq!(SourceKind::Local.to_string(), "Local");
        assert_eq!(SourceKind::YouTube.to_string(), "YouTube");
    }
}
