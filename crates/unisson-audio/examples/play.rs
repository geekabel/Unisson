//! Plays one audio file or stream from the command line, through the engine.
//!
//! ```sh
//! cargo run -p unisson-audio --example play -- ~/Music/song.mp3
//! cargo run -p unisson-audio --example play -- https://example.org/file.mp3
//! cargo run -p unisson-audio --example play -- --live https://example.org/radio
//! ```
//!
//! With `--live`, the address is treated as a radio stream: if the connection
//! is lost, the engine connects again by itself.
//!
//! Then type a command and press Enter:
//! `p` pause or resume, `s` stop, `+` or `-` volume, `i` position, `q` quit.

use std::error::Error;
use std::io::BufRead;
use std::sync::mpsc::{self, RecvTimeoutError, TryRecvError};
use std::thread;
use std::time::Duration;

use unisson_audio::{Engine, EngineEvent, PlaybackState, file_uri};
use unisson_core::StreamUri;

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    let live = arguments
        .first()
        .is_some_and(|argument| argument == "--live");
    if live {
        arguments.remove(0);
    }
    let Some(target) = arguments.into_iter().next() else {
        eprintln!("usage: play [--live] <file path or http(s) address>");
        std::process::exit(2);
    };
    // An argument with a scheme is an address. Anything else is a file path.
    let uri = if target.contains("://") {
        StreamUri::new(target)?
    } else {
        file_uri(target)?
    };

    let (engine, events) = Engine::spawn()?;
    println!("Playing {uri}");
    println!("Commands: p = pause/resume, s = stop, + / - = volume, i = position, q = quit");
    if live {
        engine.play_live(uri)?;
    } else {
        engine.play_uri(uri)?;
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

    // What the engine last reported. The interface will keep the same data.
    let mut state = PlaybackState::Stopped;
    let mut volume = 1.0;
    let mut position = Duration::ZERO;
    let mut duration = None;

    loop {
        match events.recv_timeout(Duration::from_millis(50)) {
            Ok(EngineEvent::Loaded(uri)) => println!("Loaded {uri}"),
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
            Ok(EngineEvent::VolumeChanged(new_volume)) => {
                volume = new_volume;
                println!("Volume: {:.0} %", volume * 100.0);
            }
            Ok(EngineEvent::TitleChanged(title)) => println!("Now playing: {title}"),
            Ok(EngineEvent::Reconnecting {
                attempt,
                max_attempts,
            }) => {
                println!("Connection lost, reconnecting ({attempt}/{max_attempts})");
            }
            Ok(EngineEvent::Reconnected) => println!("Reconnected"),
            Ok(EngineEvent::EndOfStream) => {
                println!("End of stream");
                return Ok(());
            }
            Ok(EngineEvent::Error(message)) => return Err(message.into()),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return Err("the audio engine stopped".into()),
        }

        match keyboard.try_recv() {
            Ok(command) => match command.as_str() {
                "p" if state == PlaybackState::Playing => engine.pause()?,
                "p" => engine.play()?,
                "s" => engine.stop()?,
                "+" => engine.set_volume(volume + 0.1)?,
                "-" => engine.set_volume(volume - 0.1)?,
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
