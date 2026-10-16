//! Where the sound of librespot goes: a small GStreamer pipeline.
//!
//! librespot decodes Spotify into samples and hands them to a *sink*. This
//! one pushes them into GStreamer, the audio engine Unisson already uses,
//! through an `appsrc` element:
//!
//! ```text
//! appsrc ! audioconvert ! audioresample ! volume ! autoaudiosink
//! ```
//!
//! The `appsrc` blocks when its small buffer is full, which paces librespot
//! to the speed of the sound card. The `volume` element carries the volume
//! of Unisson: librespot itself plays at full volume.
//!
//! The shape follows the GStreamer backend of librespot, with errors
//! reported instead of ending the program.

use std::sync::{Arc, Mutex, MutexGuard};

use gst::prelude::*;
use librespot_playback::audio_backend::{Sink, SinkError, SinkResult};
use librespot_playback::convert::Converter;
use librespot_playback::decoder::AudioPacket;
use librespot_playback::{NUM_CHANNELS, SAMPLE_RATE};

/// Samples pushed at once, per channel: about 46 ms.
const CHUNK_FRAMES: usize = 2048;

/// The volume of the built-in playback, shared between Unisson and the
/// pipeline that plays.
#[derive(Debug, Clone)]
pub struct Volume {
    shared: Arc<Mutex<VolumeState>>,
}

#[derive(Debug)]
struct VolumeState {
    level: f64,
    element: Option<gst::Element>,
}

impl Default for Volume {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(VolumeState {
                level: 1.0,
                element: None,
            })),
        }
    }
}

impl Volume {
    fn lock(&self) -> MutexGuard<'_, VolumeState> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Sets the volume, from `0.0` to `1.0`. It applies at once to the
    /// pipeline that plays, and to the next one.
    pub fn set(&self, level: f64) {
        let level = if level.is_nan() {
            1.0
        } else {
            level.clamp(0.0, 1.0)
        };
        let mut state = self.lock();
        state.level = level;
        if let Some(element) = &state.element {
            element.set_property("volume", level);
        }
    }

    /// Returns the volume.
    pub fn get(&self) -> f64 {
        self.lock().level
    }

    fn attach(&self, element: gst::Element) {
        let mut state = self.lock();
        element.set_property("volume", state.level);
        state.element = Some(element);
    }
}

/// The sink librespot writes to. See the notes at the top of this file.
pub struct GstSink {
    inner: Result<Pipeline, String>,
}

struct Pipeline {
    pipeline: gst::Pipeline,
    appsrc: gst_app::AppSrc,
    /// The last error GStreamer reported, if any.
    failure: Arc<Mutex<Option<String>>>,
}

impl GstSink {
    /// Builds the pipeline. A failure is kept and reported when librespot
    /// starts to play, since a sink cannot fail to be made.
    pub fn open(volume: &Volume) -> Self {
        Self {
            inner: Pipeline::build(volume),
        }
    }

    fn pipeline(&mut self) -> SinkResult<&mut Pipeline> {
        self.inner
            .as_mut()
            .map_err(|error| SinkError::ConnectionRefused(error.clone()))
    }
}

impl Pipeline {
    fn build(volume: &Volume) -> Result<Self, String> {
        gst::init().map_err(|error| error.to_string())?;
        let format = if cfg!(target_endian = "little") {
            "F32LE"
        } else {
            "F32BE"
        };
        let caps = gst::Caps::builder("audio/x-raw")
            .field("format", format)
            .field("layout", "interleaved")
            .field("rate", i32::try_from(SAMPLE_RATE).unwrap_or(44_100))
            .field("channels", i32::from(NUM_CHANNELS))
            .build();
        let appsrc = gst_app::AppSrc::builder()
            .caps(&caps)
            .is_live(false)
            .block(true)
            .max_bytes(chunk_bytes() as u64 * 4)
            .build();
        // librespot already dithers: no need to do it twice.
        let output = gst::parse::bin_from_description(
            "audioconvert dithering=none ! audioresample ! volume name=volume ! autoaudiosink",
            true,
        )
        .map_err(|error| format!("cannot build the audio output: {error}"))?;
        let element = output
            .by_name("volume")
            .ok_or("the audio output has no volume")?;
        volume.attach(element);

        let pipeline = gst::Pipeline::new();
        pipeline
            .add_many([appsrc.upcast_ref::<gst::Element>(), output.upcast_ref()])
            .map_err(|error| error.to_string())?;
        appsrc.link(&output).map_err(|error| error.to_string())?;

        let failure = Arc::new(Mutex::new(None));
        let noted = Arc::clone(&failure);
        let bus = pipeline.bus().ok_or("the audio output has no bus")?;
        bus.set_sync_handler(move |_, message| {
            if let gst::MessageView::Error(error) = message.view() {
                *noted
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) =
                    Some(error.error().to_string());
            }
            gst::BusSyncReply::Drop
        });
        pipeline
            .set_state(gst::State::Ready)
            .map_err(|error| format!("the audio output does not start: {error}"))?;
        Ok(Self {
            pipeline,
            appsrc,
            failure,
        })
    }

    fn failure(&self) -> Option<String> {
        self.failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn clear_failure(&self) {
        *self
            .failure
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

/// Bytes in one chunk of 32-bit samples.
fn chunk_bytes() -> usize {
    usize::from(NUM_CHANNELS) * CHUNK_FRAMES * std::mem::size_of::<f32>()
}

impl Sink for GstSink {
    fn start(&mut self) -> SinkResult<()> {
        let output = self.pipeline()?;
        output.clear_failure();
        output.appsrc.send_event(gst::event::FlushStop::new(true));
        output
            .pipeline
            .set_state(gst::State::Playing)
            .map_err(|error| SinkError::StateChange(error.to_string()))?;
        Ok(())
    }

    fn stop(&mut self) -> SinkResult<()> {
        let output = self.pipeline()?;
        // Drop what is queued: a pause must be heard at once.
        output.appsrc.send_event(gst::event::FlushStart::new());
        output
            .pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| SinkError::StateChange(error.to_string()))?;
        Ok(())
    }

    fn write(&mut self, packet: AudioPacket, converter: &mut Converter) -> SinkResult<()> {
        let output = self.pipeline()?;
        if let Some(failure) = output.failure() {
            return Err(SinkError::OnWrite(failure));
        }
        let AudioPacket::Samples(samples) = packet else {
            return Err(SinkError::InvalidParams(
                "only decoded samples can be played".into(),
            ));
        };
        let samples = converter.f64_to_f32(&samples);
        let bytes: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_ne_bytes())
            .collect();
        let buffer = gst::Buffer::from_mut_slice(bytes);
        match output.appsrc.push_buffer(buffer) {
            // While flushing for a pause, the samples are dropped on purpose.
            Ok(_) | Err(gst::FlowError::Flushing) => Ok(()),
            Err(error) => Err(SinkError::OnWrite(error.to_string())),
        }
    }
}

impl Drop for GstSink {
    fn drop(&mut self) {
        if let Ok(output) = &self.inner {
            let _ = output.pipeline.set_state(gst::State::Null);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_volume_stays_between_zero_and_one() {
        let volume = Volume::default();
        assert_eq!(volume.get(), 1.0);
        volume.set(1.7);
        assert_eq!(volume.get(), 1.0);
        volume.set(-1.0);
        assert_eq!(volume.get(), 0.0);
        volume.set(f64::NAN);
        assert_eq!(volume.get(), 1.0);
    }

    #[test]
    fn the_output_plays_samples_and_follows_the_volume() {
        // Without a sound card, `autoaudiosink` falls back to a fake sink:
        // the pipeline still runs.
        let volume = Volume::default();
        volume.set(0.25);
        let mut sink = GstSink::open(&volume);
        let Ok(output) = &sink.inner else {
            panic!("no output: {:?}", sink.inner.as_ref().err());
        };
        let element = output.pipeline.by_name("volume").unwrap();
        assert_eq!(element.property::<f64>("volume"), 0.25);

        sink.start().unwrap();
        let mut converter = Converter::new(None);
        for _ in 0..4 {
            sink.write(AudioPacket::Samples(vec![0.0; 2 * 1024]), &mut converter)
                .unwrap();
        }
        volume.set(0.5);
        assert_eq!(element.property::<f64>("volume"), 0.5);
        sink.stop().unwrap();
        sink.start().unwrap();
        sink.stop().unwrap();
    }
}
