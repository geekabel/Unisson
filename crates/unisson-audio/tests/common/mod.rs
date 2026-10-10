//! Helpers shared by the integration tests.
//!
//! No sound card is needed: audio goes to a GStreamer `fakesink`, and the
//! test files are generated on the fly with `audiotestsrc`.

// Each test file compiles this module on its own and uses a different subset.
#![allow(dead_code)]

use std::path::PathBuf;
use std::time::Duration;

use unisson_audio::Player;
use unisson_audio::gst;
use unisson_audio::gst::prelude::*;

/// Longest time a test waits for something to happen.
pub const WAIT: Duration = Duration::from_secs(10);

/// Writes a WAV file holding `seconds` of a test tone, and returns its path.
pub fn make_wav(name: &str, seconds: u32) -> PathBuf {
    gst::init().unwrap();
    let dir = std::env::temp_dir().join(format!("unisson-audio-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    // 10 buffers of 4410 samples make one second at 44100 Hz.
    let description = format!(
        "audiotestsrc num-buffers={} samplesperbuffer=4410 \
         ! audio/x-raw,rate=44100,channels=1 ! wavenc ! filesink location=\"{}\"",
        seconds * 10,
        path.display()
    );
    let pipeline = gst::parse::launch(&description).unwrap();
    pipeline.set_state(gst::State::Playing).unwrap();
    let bus = pipeline.bus().unwrap();
    let message = bus
        .timed_pop_filtered(
            gst::ClockTime::from_seconds(10),
            &[gst::MessageType::Eos, gst::MessageType::Error],
        )
        .expect("the test file was not written in time");
    assert_eq!(
        message.type_(),
        gst::MessageType::Eos,
        "cannot write the test file"
    );
    pipeline.set_state(gst::State::Null).unwrap();
    path
}

/// Builds a player whose output is discarded.
///
/// With `real_time`, the sink consumes audio at normal speed, like a sound
/// card. Without it, the whole file is consumed as fast as possible.
pub fn silent_player(real_time: bool) -> Player {
    gst::init().unwrap();
    let sink = gst::ElementFactory::make("fakesink")
        .property("sync", real_time)
        .build()
        .unwrap();
    Player::with_audio_sink(sink).unwrap()
}
