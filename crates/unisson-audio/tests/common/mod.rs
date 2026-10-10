//! Helpers shared by the integration tests.
//!
//! No sound card is needed: audio goes to a GStreamer `fakesink`, and the
//! test files are generated on the fly with `audiotestsrc`.

// Each test file compiles this module on its own and uses a different subset.
#![allow(dead_code)]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
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

/// Runs a GStreamer pipeline that writes a file, until the file is complete.
fn write_with_pipeline(description: &str) {
    let pipeline = gst::parse::launch(description).unwrap();
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
}

/// Returns `seconds` of a test tone encoded as Ogg Vorbis.
///
/// Unlike WAV, this format can be streamed over HTTP without a known length,
/// like a real radio stream.
pub fn ogg_clip(name: &str, seconds: u32) -> Vec<u8> {
    std::fs::read(ogg_file(name, seconds, None)).unwrap()
}

/// Writes `seconds` of a test tone as an Ogg Vorbis file, and returns its path.
///
/// With `title`, the file carries that title in its tags.
pub fn ogg_file(name: &str, seconds: u32, title: Option<&str>) -> PathBuf {
    gst::init().unwrap();
    let dir = std::env::temp_dir().join(format!("unisson-audio-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    // `taginject` adds tags to the stream, and the encoder writes them out.
    let tagger = match title {
        Some(title) => format!("! taginject tags=\"title=\\\"{title}\\\"\""),
        None => String::new(),
    };
    write_with_pipeline(&format!(
        "audiotestsrc num-buffers={} samplesperbuffer=4410 \
         ! audio/x-raw,rate=44100,channels=1 ! audioconvert {tagger} ! vorbisenc ! oggmux \
         ! filesink location=\"{}\"",
        seconds * 10,
        path.display()
    ));
    path
}

/// A local HTTP server that behaves like an unreliable radio stream.
///
/// Each connection receives the whole clip, then is closed, as if the
/// network dropped.
pub struct StreamServer {
    /// Address of the stream, such as `http://127.0.0.1:40123/stream`.
    pub url: String,
    connections: Arc<AtomicUsize>,
}

impl StreamServer {
    /// Starts the server on a free local port.
    ///
    /// After `max_connections` connections, the server shuts down for good
    /// and further connections are refused. `None` means no limit.
    pub fn start(clip: Vec<u8>, max_connections: Option<usize>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/stream", listener.local_addr().unwrap());
        let connections = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&connections);
        // Unless limited, the thread lives until the test process exits.
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                counter.fetch_add(1, Ordering::SeqCst);
                let mut reader = BufReader::new(stream);
                // Skip the request: every request gets the same answer.
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|read| read > 0) && line.trim() != "" {
                    line.clear();
                }
                // No length is announced: the stream ends when the connection closes.
                let header = "HTTP/1.0 200 OK\r\nContent-Type: application/ogg\r\n\r\n";
                let stream = reader.get_mut();
                let _ = stream
                    .write_all(header.as_bytes())
                    .and_then(|()| stream.write_all(&clip));
                if max_connections.is_some_and(|max| counter.load(Ordering::SeqCst) >= max) {
                    // Dropping the listener makes the port refuse connections.
                    break;
                }
            }
        });
        Self { url, connections }
    }

    /// Returns how many connections the server received so far.
    pub fn connections(&self) -> usize {
        self.connections.load(Ordering::SeqCst)
    }
}

/// Returns the address of a local port where nothing listens.
pub fn closed_port_url() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/stream", listener.local_addr().unwrap());
    drop(listener);
    url
}

/// Returns `seconds` of stereo noise encoded as Ogg Vorbis.
///
/// Noise compresses badly, so the clip is large (about 20 kB per second),
/// like a real radio stream. A test tone would fit in a few kilobytes.
pub fn noisy_ogg_clip(name: &str, seconds: u32) -> Vec<u8> {
    gst::init().unwrap();
    let dir = std::env::temp_dir().join(format!("unisson-audio-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    write_with_pipeline(&format!(
        "audiotestsrc wave=pink-noise num-buffers={} samplesperbuffer=4410 \
         ! audio/x-raw,rate=44100,channels=2 ! audioconvert ! vorbisenc quality=0.6 ! oggmux \
         ! filesink location=\"{}\"",
        seconds * 10,
        path.display()
    ));
    std::fs::read(path).unwrap()
}

/// A local HTTP server that behaves like a radio announcing its songs.
///
/// It speaks the ICY protocol used by internet radios: the audio is cut in
/// blocks of [`IcyServer::BLOCK`] bytes, and a small text block announcing the
/// current title is inserted between them.
pub struct IcyServer {
    /// Address of the stream, such as `http://127.0.0.1:40123/stream`.
    pub url: String,
}

impl IcyServer {
    /// Number of audio bytes between two title blocks.
    ///
    /// This is a value real radios commonly use. It also matters for the
    /// tests: GStreamer misses a title that comes within the very first
    /// kilobytes, before it has worked out the audio format.
    pub const BLOCK: usize = 16000;

    /// Starts the server on a free local port.
    ///
    /// Each connection receives the whole clip. The titles are spread evenly
    /// over it, in order.
    pub fn start(clip: Vec<u8>, titles: &[&str]) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/stream", listener.local_addr().unwrap());
        let body = Self::interleave(&clip, titles);
        // The thread lives until the test process exits.
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|read| read > 0) && line.trim() != "" {
                    line.clear();
                }
                let header = format!(
                    "HTTP/1.0 200 OK\r\nContent-Type: application/ogg\r\n\
                     icy-name: Test Radio\r\nicy-metaint: {}\r\n\r\n",
                    Self::BLOCK
                );
                let stream = reader.get_mut();
                let _ = stream
                    .write_all(header.as_bytes())
                    .and_then(|()| stream.write_all(&body));
            }
        });
        Self { url }
    }

    /// Inserts the title blocks between the audio blocks.
    fn interleave(clip: &[u8], titles: &[&str]) -> Vec<u8> {
        let blocks: Vec<&[u8]> = clip.chunks(Self::BLOCK).collect();
        let mut body = Vec::new();
        let mut announced = None;
        for (index, block) in blocks.iter().enumerate() {
            body.extend_from_slice(block);
            if block.len() < Self::BLOCK {
                // A title block only follows a complete audio block.
                break;
            }
            let title = titles.get(index * titles.len() / blocks.len()).copied();
            if title == announced || title.is_none() {
                // A zero length means "no change".
                body.push(0);
                continue;
            }
            announced = title;
            // The block is a length byte counting 16-byte units, then the
            // text, padded with zeros.
            let mut text = format!("StreamTitle='{}';", title.unwrap_or_default()).into_bytes();
            text.resize(text.len().div_ceil(16) * 16, 0);
            body.push((text.len() / 16) as u8);
            body.extend_from_slice(&text);
        }
        body
    }
}
