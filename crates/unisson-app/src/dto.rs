//! The shapes exchanged with the web interface.
//!
//! # Design
//!
//! The interface is written in JavaScript, so everything it sends or
//! receives travels as JSON. The types of the other crates stay free of
//! that concern: this module mirrors them in a JSON-friendly way.
//!
//! - Field names are in camelCase, the JavaScript convention.
//! - Durations are whole milliseconds.
//! - Enumerations are short lowercase words.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use unisson_core::{Media, MediaId, MediaKind, SourceKind};
use unisson_playback::{ErrorKind, PlaybackError, QueueView, Snapshot, Status};
use unisson_radio::Station;

fn to_milliseconds(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// A [`Media`], as the interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDto {
    /// The source, as [`SourceKind::id`]: `radio`, `local`, `youtube`.
    pub source: String,
    /// The identifier of the media inside its source.
    pub key: String,
    /// The kind, as [`MediaKind::id`]: `track` or `station`.
    pub kind: String,
    /// The title to display.
    pub title: String,
    /// The artist, when it is known.
    pub artist: Option<String>,
    /// The album, when it is known.
    pub album: Option<String>,
    /// The duration in milliseconds. Stations have none.
    pub duration_ms: Option<u64>,
    /// The address of a picture, when there is one.
    pub artwork: Option<String>,
}

impl From<&Media> for MediaDto {
    fn from(media: &Media) -> Self {
        Self {
            source: media.id.source.id().to_owned(),
            key: media.id.key.clone(),
            kind: media.kind.id().to_owned(),
            title: media.title.clone(),
            artist: media.artist.clone(),
            album: media.album.clone(),
            duration_ms: media.duration.map(to_milliseconds),
            artwork: media.artwork.clone(),
        }
    }
}

impl TryFrom<MediaDto> for Media {
    type Error = CommandError;

    /// Rebuilds a media sent back by the interface.
    ///
    /// # Errors
    ///
    /// Returns an `invalid` error if the source or the kind is unknown.
    fn try_from(dto: MediaDto) -> Result<Self, CommandError> {
        let source = SourceKind::from_id(&dto.source)
            .ok_or_else(|| CommandError::invalid(format!("unknown source: {}", dto.source)))?;
        let kind = MediaKind::from_id(&dto.kind)
            .ok_or_else(|| CommandError::invalid(format!("unknown media kind: {}", dto.kind)))?;
        let id = MediaId::new(source, dto.key);
        let mut media = match kind {
            MediaKind::Track => Media::track(id, dto.title),
            MediaKind::Station => Media::station(id, dto.title),
        };
        media.artist = dto.artist;
        media.album = dto.album;
        media.artwork = dto.artwork;
        if let Some(milliseconds) = dto.duration_ms {
            media = media.with_duration(Duration::from_millis(milliseconds));
        }
        Ok(media)
    }
}

/// A radio [`Station`], as the interface sees it.
///
/// It carries more than a [`MediaDto`]: what the radio screen displays, and
/// what is needed to save the station as a favorite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationDto {
    /// The identifier of the station in the directory.
    pub uuid: String,
    /// The name to display.
    pub name: String,
    /// The address of the audio stream.
    pub stream_url: String,
    /// The web site of the station.
    pub homepage: Option<String>,
    /// The address of its logo.
    pub favicon: Option<String>,
    /// Its tags, such as `jazz`.
    pub tags: Vec<String>,
    /// Its country, as a two-letter code.
    pub country_code: Option<String>,
    /// The audio format, such as `MP3`.
    pub codec: Option<String>,
    /// The bitrate, in kilobits per second.
    pub bitrate: Option<u32>,
    /// Whether the stream uses the HLS protocol.
    pub hls: bool,
}

impl From<&Station> for StationDto {
    fn from(station: &Station) -> Self {
        Self {
            uuid: station.uuid.clone(),
            name: station.name.clone(),
            stream_url: station.stream_url.clone(),
            homepage: station.homepage.clone(),
            favicon: station.favicon.clone(),
            tags: station.tags.clone(),
            country_code: station.country_code.clone(),
            codec: station.codec.clone(),
            bitrate: station.bitrate,
            hls: station.hls,
        }
    }
}

impl From<StationDto> for Station {
    fn from(dto: StationDto) -> Self {
        Self {
            uuid: dto.uuid,
            name: dto.name,
            stream_url: dto.stream_url,
            homepage: dto.homepage,
            favicon: dto.favicon,
            tags: dto.tags,
            country_code: dto.country_code,
            codec: dto.codec,
            bitrate: dto.bitrate,
            hls: dto.hls,
        }
    }
}

/// A reconnection in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconnectDto {
    /// Number of this attempt, starting at 1.
    pub attempt: usize,
    /// Number of attempts made before giving up.
    pub max_attempts: usize,
}

/// A [`PlaybackError`], as the interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackErrorDto {
    /// `notFound`, `network`, `unavailable` or `playback`.
    pub kind: &'static str,
    /// The technical message, in English.
    pub detail: String,
}

impl From<&PlaybackError> for PlaybackErrorDto {
    fn from(error: &PlaybackError) -> Self {
        let kind = match error.kind {
            ErrorKind::NotFound => "notFound",
            ErrorKind::Network => "network",
            ErrorKind::Unavailable => "unavailable",
            ErrorKind::Playback => "playback",
        };
        Self {
            kind,
            detail: error.detail.clone(),
        }
    }
}

/// A [`Snapshot`] of the playback session, as the interface sees it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDto {
    /// `stopped`, `loading`, `playing`, `paused` or `reconnecting`.
    pub status: &'static str,
    /// The attempt in progress, while the status is `reconnecting`.
    pub reconnect: Option<ReconnectDto>,
    /// The selected media.
    pub current: Option<MediaDto>,
    /// What a radio station announces as being on air.
    pub stream_title: Option<String>,
    /// The playback position, in milliseconds.
    pub position_ms: u64,
    /// The total duration in milliseconds, when it is known.
    pub duration_ms: Option<u64>,
    /// The volume, from `0.0` to `1.0`.
    pub volume: f64,
    /// Whether "previous" can go to another media.
    pub has_previous: bool,
    /// Whether "next" can go to another media.
    pub has_next: bool,
    /// Why the last playback failed.
    pub error: Option<PlaybackErrorDto>,
}

impl From<&Snapshot> for SnapshotDto {
    fn from(snapshot: &Snapshot) -> Self {
        let (status, reconnect) = match snapshot.status {
            Status::Stopped => ("stopped", None),
            Status::Loading => ("loading", None),
            Status::Playing => ("playing", None),
            Status::Paused => ("paused", None),
            Status::Reconnecting {
                attempt,
                max_attempts,
            } => (
                "reconnecting",
                Some(ReconnectDto {
                    attempt,
                    max_attempts,
                }),
            ),
        };
        Self {
            status,
            reconnect,
            current: snapshot.current.as_ref().map(MediaDto::from),
            stream_title: snapshot.stream_title.clone(),
            position_ms: to_milliseconds(snapshot.position),
            duration_ms: snapshot.duration.map(to_milliseconds),
            volume: snapshot.volume,
            has_previous: snapshot.has_previous,
            has_next: snapshot.has_next,
            error: snapshot.error.as_ref().map(PlaybackErrorDto::from),
        }
    }
}

/// The play queue, as the interface sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueDto {
    /// The selected media.
    pub current: Option<MediaDto>,
    /// The first media waiting to be played, the next one first.
    pub upcoming: Vec<MediaDto>,
    /// The number of media waiting, which may exceed what `upcoming` holds.
    pub upcoming_total: usize,
    /// The radio station listened to most recently.
    pub last_radio: Option<MediaDto>,
    /// Whether that station takes over when the queue runs out.
    pub resume_radio: bool,
}

impl QueueDto {
    /// Number of waiting media sent to the interface at most.
    pub const UPCOMING_LIMIT: usize = 200;

    /// Describes the queue for the interface.
    pub fn new(queue: &QueueView, last_radio: Option<&Media>, resume_radio: bool) -> Self {
        Self {
            current: queue.current.as_ref().map(MediaDto::from),
            upcoming: queue
                .upcoming
                .iter()
                .take(Self::UPCOMING_LIMIT)
                .map(MediaDto::from)
                .collect(),
            upcoming_total: queue.upcoming.len(),
            last_radio: last_radio.map(MediaDto::from),
            resume_radio,
        }
    }
}

/// The payload of the `player:position` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionDto {
    /// The playback position, in milliseconds.
    pub position_ms: u64,
    /// The total duration in milliseconds, when it is known.
    pub duration_ms: Option<u64>,
}

impl PositionDto {
    /// Builds the payload from the values of a session event.
    pub fn new(position: Duration, duration: Option<Duration>) -> Self {
        Self {
            position_ms: to_milliseconds(position),
            duration_ms: duration.map(to_milliseconds),
        }
    }
}

/// The error of a command, received by the interface as a rejected promise.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    /// `invalid`, `notFound`, `unreadable` or `storage`.
    pub kind: &'static str,
    /// The technical message, in English.
    pub detail: String,
}

impl CommandError {
    /// The interface sent something that makes no sense.
    pub fn invalid(detail: impl Into<String>) -> Self {
        Self::new("invalid", detail)
    }

    /// Builds an error of the given kind.
    pub fn new(kind: &'static str, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl From<unisson_store::StoreError> for CommandError {
    fn from(error: unisson_store::StoreError) -> Self {
        Self::new("storage", error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn sample_track() -> Media {
        Media::track(
            MediaId::new(SourceKind::Local, "/m/so what.flac"),
            "So What",
        )
        .with_artist("Miles Davis")
        .with_album("Kind of Blue")
        .with_duration(Duration::from_millis(562_500))
        .with_artwork("https://example.org/cover.jpg")
    }

    #[test]
    fn a_media_is_written_in_camel_case_with_milliseconds() {
        let written = serde_json::to_value(MediaDto::from(&sample_track())).unwrap();
        assert_eq!(
            written,
            json!({
                "source": "local",
                "key": "/m/so what.flac",
                "kind": "track",
                "title": "So What",
                "artist": "Miles Davis",
                "album": "Kind of Blue",
                "durationMs": 562_500,
                "artwork": "https://example.org/cover.jpg",
            })
        );
    }

    #[test]
    fn a_media_survives_the_round_trip_through_the_interface() {
        let station = Media::station(MediaId::new(SourceKind::Radio, "fip"), "FIP");
        for media in [sample_track(), station] {
            let text = serde_json::to_string(&MediaDto::from(&media)).unwrap();
            let back: MediaDto = serde_json::from_str(&text).unwrap();
            assert_eq!(Media::try_from(back), Ok(media));
        }
    }

    #[test]
    fn a_station_survives_the_round_trip_through_the_interface() {
        let mut station = Station::custom("My radio", "https://example.org/live.mp3").unwrap();
        station.tags = vec!["jazz".into(), "soul".into()];
        station.country_code = Some("FR".into());
        station.bitrate = Some(128);

        let written = serde_json::to_value(StationDto::from(&station)).unwrap();
        assert_eq!(written["streamUrl"], "https://example.org/live.mp3");
        assert_eq!(written["countryCode"], "FR");
        assert_eq!(written["tags"], json!(["jazz", "soul"]));

        let back: StationDto = serde_json::from_value(written).unwrap();
        assert_eq!(Station::from(back), station);
    }

    #[test]
    fn a_media_from_an_unknown_source_or_kind_is_refused() {
        let mut dto = MediaDto::from(&sample_track());
        dto.source = "cassette".into();
        assert_eq!(Media::try_from(dto).unwrap_err().kind, "invalid");

        let mut dto = MediaDto::from(&sample_track());
        dto.kind = "podcast".into();
        assert_eq!(Media::try_from(dto).unwrap_err().kind, "invalid");
    }

    #[test]
    fn a_snapshot_is_written_for_the_interface() {
        let snapshot = Snapshot {
            status: Status::Reconnecting {
                attempt: 2,
                max_attempts: 5,
            },
            current: Some(Media::station(
                MediaId::new(SourceKind::Radio, "fip"),
                "FIP",
            )),
            stream_title: Some("Miles Davis - So What".into()),
            position: Duration::from_millis(1_500),
            duration: None,
            volume: 0.5,
            has_previous: true,
            has_next: false,
            error: Some(PlaybackError {
                kind: ErrorKind::Network,
                detail: "no route".into(),
            }),
        };
        let written = serde_json::to_value(SnapshotDto::from(&snapshot)).unwrap();
        assert_eq!(written["status"], "reconnecting");
        assert_eq!(
            written["reconnect"],
            json!({"attempt": 2, "maxAttempts": 5})
        );
        assert_eq!(written["current"]["kind"], "station");
        assert_eq!(written["streamTitle"], "Miles Davis - So What");
        assert_eq!(written["positionMs"], 1_500);
        assert_eq!(written["durationMs"], json!(null));
        assert_eq!(written["volume"], 0.5);
        assert_eq!(written["hasPrevious"], true);
        assert_eq!(written["hasNext"], false);
        assert_eq!(
            written["error"],
            json!({"kind": "network", "detail": "no route"})
        );
    }

    #[test]
    fn a_long_queue_is_cut_but_its_length_is_kept() {
        let many: Vec<Media> = (0..QueueDto::UPCOMING_LIMIT + 50)
            .map(|index| Media::track(MediaId::new(SourceKind::Local, index.to_string()), "t"))
            .collect();
        let radio = Media::station(MediaId::new(SourceKind::Radio, "fip"), "FIP");
        let queue = QueueView {
            current: Some(sample_track()),
            upcoming: many,
            fallback: Some(radio.clone()),
        };

        let dto = QueueDto::new(&queue, Some(&radio), true);

        assert_eq!(dto.upcoming.len(), QueueDto::UPCOMING_LIMIT);
        assert_eq!(dto.upcoming_total, QueueDto::UPCOMING_LIMIT + 50);
        let written = serde_json::to_value(&dto).unwrap();
        assert_eq!(written["current"]["title"], "So What");
        assert_eq!(written["lastRadio"]["title"], "FIP");
        assert_eq!(written["resumeRadio"], true);
        assert_eq!(written["upcomingTotal"], 250);
    }

    #[test]
    fn every_status_has_a_name() {
        let name = |status| {
            SnapshotDto::from(&Snapshot {
                status,
                ..Snapshot::default()
            })
            .status
        };
        assert_eq!(name(Status::Stopped), "stopped");
        assert_eq!(name(Status::Loading), "loading");
        assert_eq!(name(Status::Playing), "playing");
        assert_eq!(name(Status::Paused), "paused");
    }
}
