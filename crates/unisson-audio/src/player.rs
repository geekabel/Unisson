//! The [`Player`]: a thin, audio-only wrapper around GStreamer's `playbin`.

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use gst::prelude::*;
use unisson_core::StreamUri;

use crate::error::AudioError;

/// Name of the GStreamer element that does the actual playback.
///
/// `playbin` is used rather than `playbin3` because Ubuntu 22.04 ships
/// GStreamer 1.20, and parts of `playbin3` only exist since 1.22.
const PLAYBIN: &str = "playbin";

/// `playbin` flags: decode audio only, and let `playbin` apply the volume
/// itself. Leaving video out prevents a window from opening for files that
/// embed a cover picture.
const AUDIO_ONLY_FLAGS: &str = "audio+soft-volume";

/// What the player is currently doing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackState {
    /// Nothing is playing, and the audio device is released.
    Stopped,
    /// Playback is suspended and can resume from the same position.
    Paused,
    /// Audio is playing, or about to play.
    Playing,
}

/// Something that happened during playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerEvent {
    /// The current media reached its end.
    EndOfStream,
    /// Playback failed. The message comes from GStreamer.
    Error(String),
    /// The media announced its title.
    ///
    /// A radio stream sends one each time a new song starts, usually as
    /// `Artist - Song`. A file sends the title stored in its tags, if any.
    Title(String),
}

/// Plays one address at a time.
///
/// The player starts in the [`PlaybackState::Stopped`] state. Call
/// [`load`](Self::load), then [`play`](Self::play). Dropping the player stops
/// playback and releases the audio device.
#[derive(Debug)]
pub struct Player {
    playbin: gst::Element,
    bus: gst::Bus,
    /// The loaded address. `playbin` forgets its `uri` property once playback
    /// starts, so the player keeps its own copy.
    loaded: Mutex<Option<StreamUri>>,
}

impl Player {
    /// Creates a player that sends audio to the default output device.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::Init`] if GStreamer cannot start, or
    /// [`AudioError::MissingElement`] if the playback plugins are not
    /// installed.
    pub fn new() -> Result<Self, AudioError> {
        Self::build(None)
    }

    /// Creates a player that sends audio to the given GStreamer sink.
    ///
    /// Mostly useful in tests, with a `fakesink` that needs no sound card.
    /// GStreamer must already be initialized to create the sink.
    ///
    /// # Errors
    ///
    /// Same as [`Player::new`].
    pub fn with_audio_sink(sink: gst::Element) -> Result<Self, AudioError> {
        Self::build(Some(sink))
    }

    fn build(audio_sink: Option<gst::Element>) -> Result<Self, AudioError> {
        gst::init().map_err(|error| AudioError::Init(error.to_string()))?;
        let playbin = gst::ElementFactory::make(PLAYBIN)
            .build()
            .map_err(|_| AudioError::MissingElement(PLAYBIN))?;
        playbin.set_property_from_str("flags", AUDIO_ONLY_FLAGS);
        if let Some(sink) = audio_sink {
            playbin.set_property("audio-sink", &sink);
        }
        // An element always has a bus once it is a pipeline, and `playbin` is one.
        let bus = playbin.bus().ok_or(AudioError::MissingElement("bus"))?;
        Ok(Self {
            playbin,
            bus,
            loaded: Mutex::new(None),
        })
    }

    /// Stops any current playback and selects the address to play next.
    ///
    /// Nothing is played until [`play`](Self::play) is called.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::StateChange`] if the previous playback cannot be
    /// stopped.
    pub fn load(&self, uri: &StreamUri) -> Result<(), AudioError> {
        self.set_state(gst::State::Null)?;
        // Drop events that belong to the previous media.
        self.bus.set_flushing(true);
        self.bus.set_flushing(false);
        *self.loaded() = Some(uri.clone());
        Ok(())
    }

    /// Starts playback, or resumes it after a pause.
    ///
    /// After [`stop`](Self::stop), playback restarts from the beginning.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::NothingLoaded`] if no address was loaded, or
    /// [`AudioError::StateChange`] if GStreamer refuses to start, for
    /// instance because the file does not exist. The player is then back in
    /// the [`PlaybackState::Stopped`] state.
    pub fn play(&self) -> Result<(), AudioError> {
        let uri = self.loaded_uri()?;
        if self.state() == PlaybackState::Stopped {
            // Starting from scratch: tell `playbin` what to play.
            self.playbin.set_property("uri", uri.as_str());
        }
        self.set_state(gst::State::Playing)
    }

    /// Suspends playback, keeping the current position.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::NothingLoaded`] if no address was loaded, or
    /// [`AudioError::StateChange`] if GStreamer refuses to pause.
    pub fn pause(&self) -> Result<(), AudioError> {
        let uri = self.loaded_uri()?;
        if self.state() == PlaybackState::Stopped {
            self.playbin.set_property("uri", uri.as_str());
        }
        self.set_state(gst::State::Paused)
    }

    /// Stops playback and releases the audio device.
    ///
    /// The loaded address is kept, so [`play`](Self::play) starts it again.
    ///
    /// # Errors
    ///
    /// Returns [`AudioError::StateChange`] if GStreamer refuses to stop.
    pub fn stop(&self) -> Result<(), AudioError> {
        self.set_state(gst::State::Null)
    }

    /// Sets the volume, from `0.0` (silent) to `1.0` (full).
    ///
    /// Values outside that range are clamped. The scale is linear.
    pub fn set_volume(&self, volume: f64) {
        let volume = if volume.is_nan() {
            0.0
        } else {
            volume.clamp(0.0, 1.0)
        };
        self.playbin.set_property("volume", volume);
    }

    /// Returns the volume, from `0.0` to `1.0`.
    pub fn volume(&self) -> f64 {
        self.playbin.property("volume")
    }

    /// Returns what the player is doing.
    ///
    /// While GStreamer is still switching state, this reports the state it is
    /// heading to. For instance, it returns [`PlaybackState::Playing`] right
    /// after a successful [`play`](Self::play).
    pub fn state(&self) -> PlaybackState {
        let (_, current, pending) = self.playbin.state(gst::ClockTime::ZERO);
        let target = if pending == gst::State::VoidPending {
            current
        } else {
            pending
        };
        match target {
            gst::State::Playing => PlaybackState::Playing,
            gst::State::Paused => PlaybackState::Paused,
            _ => PlaybackState::Stopped,
        }
    }

    /// Returns the playback position, when it is known.
    pub fn position(&self) -> Option<Duration> {
        self.playbin
            .query_position::<gst::ClockTime>()
            .map(Duration::from)
    }

    /// Returns the total duration of the media, when it is known.
    ///
    /// Live streams have no duration.
    pub fn duration(&self) -> Option<Duration> {
        self.playbin
            .query_duration::<gst::ClockTime>()
            .map(Duration::from)
    }

    /// Waits up to `timeout` for the next playback event.
    ///
    /// Returns `None` if nothing happened in that time. A zero timeout checks
    /// for a pending event without waiting. After a
    /// [`PlayerEvent::EndOfStream`] or a [`PlayerEvent::Error`], the player is
    /// back in the [`PlaybackState::Stopped`] state.
    pub fn poll_event(&self, timeout: Duration) -> Option<PlayerEvent> {
        // `None` stands for a deadline too far away to be represented.
        let deadline = Instant::now().checked_add(timeout);
        loop {
            let remaining = match deadline {
                Some(deadline) => deadline.saturating_duration_since(Instant::now()),
                None => Duration::MAX,
            };
            let message = self.pop_message(remaining)?;
            // Many messages carry nothing of interest: keep reading.
            if let Some(event) = self.event_from(&message) {
                return Some(event);
            }
        }
    }

    /// Takes the next relevant message off the bus, waiting up to `timeout`.
    fn pop_message(&self, timeout: Duration) -> Option<gst::Message> {
        const WANTED: [gst::MessageType; 3] = [
            gst::MessageType::Eos,
            gst::MessageType::Error,
            gst::MessageType::Tag,
        ];
        // With a zero timeout, `timed_pop_filtered` gives up before looking at
        // the queue. `pop_filtered` is the non-blocking way to read it.
        if timeout.is_zero() {
            self.bus.pop_filtered(&WANTED)
        } else {
            let timeout = gst::ClockTime::try_from(timeout).unwrap_or(gst::ClockTime::MAX);
            self.bus.timed_pop_filtered(timeout, &WANTED)
        }
    }

    /// Turns a bus message into an event, if it carries one.
    fn event_from(&self, message: &gst::Message) -> Option<PlayerEvent> {
        let event = match message.view() {
            gst::MessageView::Eos(_) => PlayerEvent::EndOfStream,
            gst::MessageView::Error(error) => PlayerEvent::Error(error.error().to_string()),
            gst::MessageView::Tag(tag) => {
                let tags = tag.tags();
                let title = tags.get::<gst::tags::Title>()?;
                let title = title.get().trim();
                // Tags without a title (codec, bitrate) are of no interest here.
                return (!title.is_empty()).then(|| PlayerEvent::Title(title.to_owned()));
            }
            _ => return None,
        };
        // Playback is over. Best effort: the event is reported even if
        // stopping fails.
        let _ = self.playbin.set_state(gst::State::Null);
        Some(event)
    }

    fn loaded(&self) -> std::sync::MutexGuard<'_, Option<StreamUri>> {
        // The guarded value is a plain `Option`, always valid even if another
        // thread panicked while holding the lock.
        self.loaded
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn loaded_uri(&self) -> Result<StreamUri, AudioError> {
        self.loaded().clone().ok_or(AudioError::NothingLoaded)
    }

    fn set_state(&self, state: gst::State) -> Result<(), AudioError> {
        let Err(error) = self.playbin.set_state(state) else {
            return Ok(());
        };
        // GStreamer posts the real cause on the bus. Prefer it to the generic
        // "failed to change state" message.
        let detail = self
            .bus
            .pop_filtered(&[gst::MessageType::Error])
            .and_then(|message| match message.view() {
                gst::MessageView::Error(cause) => Some(cause.error().to_string()),
                _ => None,
            })
            .unwrap_or_else(|| error.to_string());
        // Leave the player in a clean, stopped state.
        let _ = self.playbin.set_state(gst::State::Null);
        Err(AudioError::StateChange(detail))
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // GStreamer requires elements to be in the Null state before disposal.
        let _ = self.playbin.set_state(gst::State::Null);
    }
}

/// Turns a file path into an address the [`Player`] can load.
///
/// Relative paths are resolved against the current directory, and special
/// characters are escaped.
///
/// # Errors
///
/// Returns [`AudioError::InvalidPath`] if the file does not exist or its path
/// cannot be expressed as a `file://` address.
///
/// # Examples
///
/// ```no_run
/// let uri = unisson_audio::file_uri("/home/me/Music/my song.mp3")?;
/// assert_eq!(uri.as_str(), "file:///home/me/Music/my%20song.mp3");
/// # Ok::<(), unisson_audio::AudioError>(())
/// ```
pub fn file_uri(path: impl AsRef<Path>) -> Result<StreamUri, AudioError> {
    let path = path.as_ref();
    let invalid = |detail: String| AudioError::InvalidPath(format!("{}: {detail}", path.display()));
    let absolute = path
        .canonicalize()
        .map_err(|error| invalid(error.to_string()))?;
    let uri =
        gst::glib::filename_to_uri(&absolute, None).map_err(|error| invalid(error.to_string()))?;
    StreamUri::new(uri.as_str()).map_err(|error| invalid(error.to_string()))
}
