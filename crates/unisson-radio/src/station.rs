//! A radio station, as described by the Radio Browser directory.

use serde::Deserialize;
use unisson_core::{Media, MediaId, SourceKind};

/// A radio station.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Station {
    /// Identifier of the station in the directory.
    pub uuid: String,
    /// Name of the station.
    pub name: String,
    /// Address of the audio stream.
    ///
    /// This is the address resolved by the directory when there is one:
    /// playlists and redirects are already followed.
    pub stream_url: String,
    /// Web site of the station.
    pub homepage: Option<String>,
    /// Address of the station logo.
    pub favicon: Option<String>,
    /// Free-form tags, usually genres such as `jazz`.
    pub tags: Vec<String>,
    /// Two-letter country code (ISO 3166-1 alpha-2), such as `FR`.
    pub country_code: Option<String>,
    /// Audio codec of the stream, such as `MP3` or `AAC`.
    pub codec: Option<String>,
    /// Bitrate of the stream, in kbit/s.
    pub bitrate: Option<u32>,
    /// Whether the stream uses HLS (HTTP Live Streaming).
    pub hls: bool,
}

impl Station {
    /// Converts the station into the common [`Media`] model.
    ///
    /// The media is a live station from the radio source. Its key is the
    /// station [`uuid`](Self::uuid).
    pub fn to_media(&self) -> Media {
        let media = Media::station(MediaId::new(SourceKind::Radio, &self.uuid), &self.name);
        match &self.favicon {
            Some(favicon) => media.with_artwork(favicon),
            None => media,
        }
    }
}

/// A station exactly as the directory sends it.
///
/// Every field is optional, so a missing or `null` value never makes the
/// whole answer unreadable. Fields the application does not use are ignored.
#[derive(Debug, Default, Deserialize)]
pub(crate) struct RawStation {
    #[serde(default)]
    stationuuid: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    url_resolved: Option<String>,
    #[serde(default)]
    homepage: Option<String>,
    #[serde(default)]
    favicon: Option<String>,
    #[serde(default)]
    tags: Option<String>,
    #[serde(default)]
    countrycode: Option<String>,
    #[serde(default)]
    codec: Option<String>,
    #[serde(default)]
    bitrate: Option<u32>,
    #[serde(default)]
    hls: Option<u8>,
}

impl RawStation {
    /// Cleans the raw station up.
    ///
    /// Returns `None` if the station cannot be used: no identifier, no name
    /// or no stream address.
    pub(crate) fn into_station(self) -> Option<Station> {
        let stream_url = non_empty(self.url_resolved).or_else(|| non_empty(self.url))?;
        Some(Station {
            uuid: non_empty(self.stationuuid)?,
            name: non_empty(self.name)?,
            stream_url,
            homepage: non_empty(self.homepage),
            favicon: non_empty(self.favicon),
            tags: self
                .tags
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(str::to_owned)
                .collect(),
            country_code: non_empty(self.countrycode).map(|code| code.to_uppercase()),
            codec: non_empty(self.codec),
            // The directory sends 0 when it does not know the bitrate.
            bitrate: self.bitrate.filter(|bitrate| *bitrate > 0),
            hls: self.hls.unwrap_or(0) != 0,
        })
    }
}

/// Trims a value, and turns an empty one into `None`.
fn non_empty(value: Option<String>) -> Option<String> {
    let value = value?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(uuid: &str, name: &str, url: &str) -> RawStation {
        RawStation {
            stationuuid: Some(uuid.into()),
            name: Some(name.into()),
            url: Some(url.into()),
            ..RawStation::default()
        }
    }

    #[test]
    fn the_resolved_address_is_preferred() {
        let mut station = raw("u1", "FIP", "http://example.org/fip.pls");
        station.url_resolved = Some("http://stream.example.org/fip.mp3".into());
        assert_eq!(
            station.into_station().unwrap().stream_url,
            "http://stream.example.org/fip.mp3"
        );

        let mut station = raw("u1", "FIP", "http://example.org/fip.mp3");
        station.url_resolved = Some("  ".into());
        assert_eq!(
            station.into_station().unwrap().stream_url,
            "http://example.org/fip.mp3"
        );
    }

    #[test]
    fn values_are_cleaned_up() {
        let mut station = raw(" u1 ", "\tFIP \n", "http://example.org/a");
        station.tags = Some("jazz, ,eclectic,".into());
        station.countrycode = Some("fr".into());
        station.codec = Some("".into());
        station.bitrate = Some(0);
        station.hls = Some(1);
        let station = station.into_station().unwrap();
        assert_eq!(station.uuid, "u1");
        assert_eq!(station.name, "FIP");
        assert_eq!(station.tags, ["jazz", "eclectic"]);
        assert_eq!(station.country_code.as_deref(), Some("FR"));
        assert_eq!(station.codec, None);
        assert_eq!(station.bitrate, None);
        assert!(station.hls);
    }

    #[test]
    fn unusable_stations_are_dropped() {
        assert!(
            raw("", "FIP", "http://example.org/a")
                .into_station()
                .is_none()
        );
        assert!(
            raw("u1", " ", "http://example.org/a")
                .into_station()
                .is_none()
        );
        assert!(raw("u1", "FIP", "").into_station().is_none());
    }

    #[test]
    fn a_station_becomes_a_live_radio_media() {
        let mut station = raw("u1", "FIP", "http://example.org/a");
        station.favicon = Some("http://example.org/logo.png".into());
        let media = station.into_station().unwrap().to_media();
        assert_eq!(media.id, MediaId::new(SourceKind::Radio, "u1"));
        assert_eq!(media.title, "FIP");
        assert!(media.is_live());
        assert_eq!(
            media.artwork.as_deref(),
            Some("http://example.org/logo.png")
        );
    }
}
