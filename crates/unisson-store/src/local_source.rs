//! The library as a [`Source`] of the application.

use std::path::Path;
use std::sync::Arc;

use unisson_core::{Media, MediaId, SearchQuery, Source, SourceError, SourceKind, StreamUri};

use crate::store::Store;

/// The local audio files, searched in the library kept by the [`Store`].
///
/// The source never reads the music folder itself: it answers from the
/// library, which must be filled first with
/// [`Store::replace_library`].
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use unisson_core::{SearchQuery, SourceRegistry};
/// use unisson_store::{LocalSource, Store};
///
/// let store = Arc::new(Store::open_in_memory()?);
/// let mut registry = SourceRegistry::new();
/// registry.register(Box::new(LocalSource::new(Arc::clone(&store))));
/// let results = registry.search_all(&SearchQuery::new("jazz"));
/// # Ok::<(), unisson_store::StoreError>(())
/// ```
#[derive(Debug)]
pub struct LocalSource {
    store: Arc<Store>,
}

impl LocalSource {
    /// Creates the source on top of a store.
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }
}

impl Source for LocalSource {
    fn kind(&self) -> SourceKind {
        SourceKind::Local
    }

    /// Searches the library by title, artist and album.
    fn search(&self, query: &SearchQuery) -> Result<Vec<Media>, SourceError> {
        let tracks = self
            .store
            .search_library(query.text(), query.limit())
            .map_err(|error| SourceError::Unavailable(error.to_string()))?;
        Ok(tracks.iter().map(|track| track.to_media()).collect())
    }

    /// Returns the `file://` address of a track.
    ///
    /// The file is checked on disk, so a track whose file was moved or
    /// deleted is reported as [`SourceError::NotFound`] instead of failing
    /// later, during playback.
    fn resolve(&self, id: &MediaId) -> Result<StreamUri, SourceError> {
        if id.source != SourceKind::Local {
            return Err(SourceError::Invalid(format!(
                "not a local file: {}",
                id.source
            )));
        }
        let path = Path::new(&id.key);
        if !path.is_file() {
            return Err(SourceError::NotFound);
        }
        StreamUri::from_file_path(path)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use unisson_core::SourceRegistry;
    use unisson_local::LocalTrack;

    use super::*;

    fn track(path: &Path, title: &str) -> LocalTrack {
        LocalTrack {
            path: path.to_owned(),
            title: title.into(),
            artist: Some("Miles Davis".into()),
            album: None,
            track_number: None,
            duration: None,
            has_cover: false,
        }
    }

    /// Creates an empty file in a folder of its own, and returns its path.
    fn real_file(name: &str) -> PathBuf {
        let folder = std::env::temp_dir()
            .join(format!("unisson-store-tests-{}", std::process::id()))
            .join("local source");
        std::fs::create_dir_all(&folder).unwrap();
        let path = folder.join(name);
        std::fs::write(&path, "").unwrap();
        path
    }

    #[test]
    fn search_answers_from_the_library() {
        let store = Arc::new(Store::open_in_memory().unwrap());
        store
            .replace_library(&[track(Path::new("/m/so what.flac"), "So What")])
            .unwrap();
        let source = LocalSource::new(Arc::clone(&store));

        let media = source.search(&SearchQuery::new("so what miles")).unwrap();

        assert_eq!(media.len(), 1);
        assert_eq!(
            media[0].id,
            MediaId::new(SourceKind::Local, "/m/so what.flac")
        );
        assert_eq!(media[0].title, "So What");
        assert_eq!(media[0].artist.as_deref(), Some("Miles Davis"));
    }

    #[test]
    fn resolve_gives_the_address_of_an_existing_file() {
        let path = real_file("so what & more.flac");
        let source = LocalSource::new(Arc::new(Store::open_in_memory().unwrap()));
        let id = track(&path, "So What").to_media().id;

        let uri = source.resolve(&id).unwrap();

        assert!(uri.as_str().starts_with("file:///"), "{uri}");
        assert!(
            uri.as_str().ends_with("/so%20what%20%26%20more.flac"),
            "{uri}"
        );
    }

    #[test]
    fn resolve_reports_a_file_that_is_gone() {
        let source = LocalSource::new(Arc::new(Store::open_in_memory().unwrap()));
        let gone = MediaId::new(SourceKind::Local, "/nonexistent/unisson/a.flac");
        assert_eq!(source.resolve(&gone), Err(SourceError::NotFound));

        let other = MediaId::new(SourceKind::Radio, "uuid");
        assert!(matches!(
            source.resolve(&other),
            Err(SourceError::Invalid(_))
        ));
    }

    #[test]
    fn the_source_works_through_the_registry() {
        let path = real_file("take five.mp3");
        let store = Arc::new(Store::open_in_memory().unwrap());
        store.replace_library(&[track(&path, "Take Five")]).unwrap();
        let mut registry = SourceRegistry::new();
        registry.register(Box::new(LocalSource::new(store)));

        let results = registry.search_all(&SearchQuery::new("take"));

        assert_eq!(results[0].source, SourceKind::Local);
        let media = results[0].result.as_ref().unwrap();
        assert!(registry.resolve(&media[0].id).is_ok());
    }
}
