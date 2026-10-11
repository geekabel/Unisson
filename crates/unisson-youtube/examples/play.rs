//! Plays the sound of a YouTube video from the command line.
//!
//! ```sh
//! cargo run -p unisson-youtube --example play -- "miles davis so what"
//! cargo run -p unisson-youtube --example play -- https://www.youtube.com/watch?v=VIDEO
//! ```
//!
//! Words are searched, and the first result is played. A link is played
//! directly. This needs the `yt-dlp` program: see the README.
//!
//! Then type a command and press Enter:
//! `p` pause or resume, `i` position, `q` quit.

use std::error::Error;
use std::io::BufRead;
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use unisson_audio::{Engine, EngineEvent, PlaybackState};
use unisson_core::{Media, SearchQuery, Source};
use unisson_youtube::{YouTubeSource, is_youtube_url};

fn main() -> Result<(), Box<dyn Error>> {
    let target = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    if target.trim().is_empty() {
        eprintln!("usage: play <words to search, or a YouTube link>");
        std::process::exit(2);
    }

    let youtube = YouTubeSource::new();
    println!("Using yt-dlp {}", youtube.tool().version()?);

    let start = Instant::now();
    let found = if is_youtube_url(&target) {
        youtube.tool().lookup(&target)?
    } else {
        youtube.search(&SearchQuery::new(&target).with_limit(5))?
    };
    println!("{} video(s) found in {:.1?}:", found.len(), start.elapsed());
    for media in &found {
        println!("  {}", describe(media));
    }
    let media = found.first().ok_or("nothing was found")?;

    let start = Instant::now();
    let address = youtube.resolve(&media.id)?;
    println!("Audio address found in {:.1?}", start.elapsed());

    let (engine, events) = Engine::spawn()?;
    println!("Playing {}", describe(media));
    println!("Commands: p = pause/resume, i = position, q = quit");
    if media.is_live() {
        engine.play_live(address)?;
    } else {
        engine.play_uri(address)?;
    }

    // Read the keyboard on another thread so the main loop never blocks on it.
    let (sender, keyboard) = mpsc::channel();
    thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if sender.send(line.trim().to_owned()).is_err() {
                break;
            }
        }
    });

    let mut state = PlaybackState::Stopped;
    let mut position = Duration::ZERO;
    let mut duration = None;
    loop {
        match events.recv_timeout(Duration::from_millis(50)) {
            Ok(EngineEvent::StateChanged(new_state)) => {
                state = new_state;
                println!("State: {state:?}");
            }
            Ok(EngineEvent::Position {
                position: at,
                duration: total,
            }) => {
                position = at;
                duration = total;
            }
            Ok(EngineEvent::EndOfStream) => {
                println!("End of the video");
                return Ok(());
            }
            Ok(EngineEvent::Error(message)) => return Err(message.into()),
            Ok(_) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err("the audio engine stopped".into()),
        }

        match keyboard.try_recv() {
            Ok(command) => match command.as_str() {
                "p" if state == PlaybackState::Playing => engine.pause()?,
                "p" => engine.play()?,
                "i" => match duration {
                    Some(total) => println!("Position: {position:.1?} of {total:.1?}"),
                    None => println!("Position: {position:.1?} (live)"),
                },
                "q" => return Ok(()),
                other => println!("Unknown command: {other}"),
            },
            // No command typed, or the keyboard is closed (Ctrl+D): keep playing.
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => {}
        }
    }
}

fn describe(media: &Media) -> String {
    let artist = media.artist.as_deref().unwrap_or("unknown channel");
    let length = match media.duration {
        Some(duration) => format!("{}:{:02}", duration.as_secs() / 60, duration.as_secs() % 60),
        None => "live".to_owned(),
    };
    format!("{artist} - {} ({length})", media.title)
}
