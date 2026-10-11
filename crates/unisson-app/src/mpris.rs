//! MPRIS: how the desktop controls the player and shows what plays.
//!
//! # Design
//!
//! [MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/) is the
//! D-Bus interface every Linux desktop uses for media players. Through it,
//! the media keys of the keyboard work from any application, and GNOME
//! shows the title with its play, previous and next buttons.
//!
//! The `mpris-server` crate implements the interface. Its `Player` cannot
//! leave the thread it was created on, so this module gives it a thread of
//! its own, with a small executor for its asynchronous code. Two flows meet
//! there:
//!
//! - the desktop asks for something (play, next, a position): the request
//!   is passed on to the application state;
//! - the playback state changes: the application sends an update
//!   through a channel, and the thread publishes it on D-Bus.

use std::time::Duration;

use async_channel::{Receiver, Sender};
use mpris_server::{Metadata, PlaybackStatus, Player, Time};
use tauri::{AppHandle, Manager};
use unisson_playback::{Snapshot, Status};

use crate::now_playing::NowPlaying;
use crate::state::AppState;
use crate::windows;

/// The last part of the bus name: `org.mpris.MediaPlayer2.unisson`.
const BUS_NAME_SUFFIX: &str = "unisson";

/// A jump in the position larger than this is announced as a seek. Smaller
/// steps are playback simply moving on.
const SEEK_THRESHOLD: Duration = Duration::from_secs(3);

/// Something to publish on D-Bus.
#[derive(Debug)]
enum Update {
    /// The playback state changed. The flags say whether "next" and
    /// "previous" lead somewhere.
    State {
        snapshot: Box<Snapshot>,
        has_next: bool,
        has_previous: bool,
    },
    /// Playback moved on.
    Position(Duration),
}

/// Handle to the MPRIS thread.
///
/// Sending never blocks and never fails: without a D-Bus session, the
/// updates are simply dropped.
#[derive(Debug, Clone)]
pub struct Mpris {
    updates: Sender<Update>,
}

impl Mpris {
    /// Starts the MPRIS thread.
    ///
    /// A desktop without D-Bus is not an error: the player then works
    /// without media keys, and a line in the log says why.
    pub fn start(app: AppHandle) -> Self {
        let (updates, inbox) = async_channel::unbounded();
        let spawned = std::thread::Builder::new()
            .name("unisson-mpris".into())
            .spawn(move || {
                let executor = async_executor::LocalExecutor::new();
                let served =
                    futures_lite::future::block_on(executor.run(serve(&executor, app, inbox)));
                if let Err(error) = served {
                    eprintln!("unisson: the media keys are not available: {error}");
                }
            });
        if let Err(error) = spawned {
            eprintln!("unisson: cannot start the MPRIS thread: {error}");
        }
        Self { updates }
    }

    /// Publishes a new playback state.
    pub fn state_changed(&self, snapshot: &Snapshot, has_next: bool, has_previous: bool) {
        // The channel has no limit, so this cannot block. It only fails
        // when the thread is gone, and then nobody is listening anyway.
        let _ = self.updates.send_blocking(Update::State {
            snapshot: Box::new(snapshot.clone()),
            has_next,
            has_previous,
        });
    }

    /// Publishes a new playback position.
    pub fn position_changed(&self, position: Duration) {
        let _ = self.updates.send_blocking(Update::Position(position));
    }
}

/// Runs the MPRIS player until the application ends.
async fn serve(
    executor: &async_executor::LocalExecutor<'_>,
    app: AppHandle,
    updates: Receiver<Update>,
) -> mpris_server::zbus::Result<()> {
    let player = Player::builder(BUS_NAME_SUFFIX)
        .identity("Unisson")
        // The name of the desktop file, which gives the desktop the icon.
        // The package installs it as `Unisson.desktop`.
        .desktop_entry("Unisson")
        .can_raise(true)
        .can_quit(true)
        .can_play(false)
        .can_pause(false)
        .can_seek(false)
        .can_go_next(false)
        .can_go_previous(false)
        .build()
        .await?;
    connect_requests(&player, &app);
    executor.spawn(player.run()).detach();

    // What was last published, to publish changes only.
    let mut published: Option<NowPlaying> = None;
    let mut track_number: u64 = 0;
    let mut last_position = Duration::ZERO;

    while let Ok(update) = updates.recv().await {
        match update {
            Update::State {
                snapshot,
                has_next,
                has_previous,
            } => {
                let now = NowPlaying::of(&snapshot);
                let selected = now.is_some();
                let can_seek = now.as_ref().is_some_and(|now| !now.live);

                // The metadata is sent again only when what plays changed.
                let same_media = match (&published, &now) {
                    (Some(old), Some(new)) => {
                        (
                            &old.title,
                            &old.artist,
                            &old.album,
                            &old.artwork,
                            old.length,
                        ) == (
                            &new.title,
                            &new.artist,
                            &new.album,
                            &new.artwork,
                            new.length,
                        )
                    }
                    (None, None) => true,
                    _ => false,
                };
                if !same_media {
                    track_number += 1;
                    let metadata = now
                        .as_ref()
                        .map_or_else(Metadata::new, |now| metadata_of(now, track_number));
                    player.set_metadata(metadata).await?;
                }
                published = now;

                player
                    .set_playback_status(playback_status(snapshot.status))
                    .await?;
                player.set_can_play(selected).await?;
                player.set_can_pause(selected).await?;
                player.set_can_seek(can_seek).await?;
                player.set_can_go_next(has_next).await?;
                // "Previous" also restarts the current track.
                player.set_can_go_previous(has_previous || can_seek).await?;
                player.set_volume(snapshot.volume).await?;
            }
            Update::Position(position) => {
                player.set_position(to_time(position));
                if position.abs_diff(last_position) > SEEK_THRESHOLD {
                    player.seeked(to_time(position)).await?;
                }
                last_position = position;
            }
        }
    }
    Ok(())
}

/// Passes the requests of the desktop on to the application.
fn connect_requests(player: &Player, app: &AppHandle) {
    // Each request gets its own handle to the application.
    let state_of = |app: &AppHandle| app.clone();

    let handle = state_of(app);
    player.connect_play_pause(move |_| handle.state::<AppState>().session.toggle());
    let handle = state_of(app);
    player.connect_play(move |_| handle.state::<AppState>().session.resume());
    let handle = state_of(app);
    player.connect_pause(move |_| handle.state::<AppState>().session.pause());
    // The player has no "stop" of its own: pausing is the closest.
    let handle = state_of(app);
    player.connect_stop(move |_| handle.state::<AppState>().session.pause());
    let handle = state_of(app);
    player.connect_next(move |_| handle.state::<AppState>().next());
    let handle = state_of(app);
    player.connect_previous(move |_| handle.state::<AppState>().previous());

    // "Seek" moves by an offset, which may be negative.
    let handle = state_of(app);
    player.connect_seek(move |player, offset| {
        let target = player
            .position()
            .as_micros()
            .saturating_add(offset.as_micros());
        let target = Duration::from_micros(u64::try_from(target).unwrap_or(0));
        handle.state::<AppState>().session.seek(target);
    });
    let handle = state_of(app);
    player.connect_set_position(move |_, _track, position| {
        let position = Duration::from_micros(u64::try_from(position.as_micros()).unwrap_or(0));
        handle.state::<AppState>().session.seek(position);
    });
    let handle = state_of(app);
    player.connect_set_volume(move |_, volume| {
        handle.state::<AppState>().session.set_volume(volume);
    });

    let handle = state_of(app);
    player.connect_raise(move |_| windows::show_main(&handle));
    let handle = state_of(app);
    player.connect_quit(move |_| handle.exit(0));
}

/// Translates the status of the session into the three of MPRIS.
fn playback_status(status: Status) -> PlaybackStatus {
    match status {
        Status::Paused => PlaybackStatus::Paused,
        Status::Stopped => PlaybackStatus::Stopped,
        // Loading and reconnecting are "playing" for a listener: sound is
        // on its way, and the button to show is "pause".
        Status::Playing | Status::Loading | Status::Reconnecting { .. } => PlaybackStatus::Playing,
    }
}

fn to_time(duration: Duration) -> Time {
    Time::from_micros(i64::try_from(duration.as_micros()).unwrap_or(i64::MAX))
}

/// Builds the MPRIS metadata of what plays.
///
/// `track_number` makes the track identifier unique, as the specification
/// asks: the same media played twice is two tracks.
fn metadata_of(now: &NowPlaying, track_number: u64) -> Metadata {
    let mut metadata = Metadata::builder().title(now.title.clone());
    if let Ok(track_id) =
        mpris_server::TrackId::try_from(format!("/io/github/geekabel/unisson/track/{track_number}"))
    {
        metadata = metadata.trackid(track_id);
    }
    if let Some(artist) = &now.artist {
        metadata = metadata.artist([artist.clone()]);
    }
    if let Some(album) = &now.album {
        metadata = metadata.album(album.clone());
    }
    if let Some(length) = now.length {
        metadata = metadata.length(to_time(length));
    }
    if let Some(artwork) = &now.artwork {
        metadata = metadata.art_url(artwork.clone());
    }
    metadata.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_status_maps_to_one_of_the_three_of_mpris() {
        assert_eq!(playback_status(Status::Playing), PlaybackStatus::Playing);
        assert_eq!(playback_status(Status::Loading), PlaybackStatus::Playing);
        assert_eq!(
            playback_status(Status::Reconnecting {
                attempt: 1,
                max_attempts: 5
            }),
            PlaybackStatus::Playing
        );
        assert_eq!(playback_status(Status::Paused), PlaybackStatus::Paused);
        assert_eq!(playback_status(Status::Stopped), PlaybackStatus::Stopped);
    }

    #[test]
    fn the_metadata_carries_what_is_known() {
        let now = NowPlaying {
            title: "So What".into(),
            artist: Some("Miles Davis".into()),
            album: Some("Kind of Blue".into()),
            artwork: Some("https://example.org/cover.jpg".into()),
            length: Some(Duration::from_secs(562)),
            live: false,
            playing: true,
        };

        let metadata = metadata_of(&now, 7);

        assert_eq!(metadata.title(), Some("So What"));
        assert_eq!(metadata.artist(), Some(vec!["Miles Davis".to_owned()]));
        assert_eq!(metadata.album(), Some("Kind of Blue"));
        assert_eq!(metadata.length(), Some(Time::from_secs(562)));
        assert_eq!(
            metadata.art_url().as_deref(),
            Some("https://example.org/cover.jpg")
        );
        let track_id = metadata.trackid().expect("the track has an identifier");
        assert!(track_id.as_str().ends_with("/track/7"), "{track_id:?}");
    }

    #[test]
    fn a_bare_title_is_enough() {
        let now = NowPlaying {
            title: "FIP".into(),
            artist: None,
            album: None,
            artwork: None,
            length: None,
            live: true,
            playing: true,
        };
        let metadata = metadata_of(&now, 1);
        assert_eq!(metadata.title(), Some("FIP"));
        assert_eq!(metadata.artist(), None);
        assert_eq!(metadata.length(), None);
    }
}
