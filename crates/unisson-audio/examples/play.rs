//! Plays one audio file or stream from the command line.
//!
//! ```sh
//! cargo run -p unisson-audio --example play -- ~/Music/song.mp3
//! cargo run -p unisson-audio --example play -- https://example.org/stream.mp3
//! ```
//!
//! Then type a command and press Enter:
//! `p` pause or resume, `s` stop, `+` or `-` volume, `q` quit.

use std::error::Error;
use std::io::BufRead;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use unisson_audio::{PlaybackState, Player, PlayerEvent, file_uri};
use unisson_core::StreamUri;

fn main() -> Result<(), Box<dyn Error>> {
    let Some(target) = std::env::args().nth(1) else {
        eprintln!("usage: play <file path or http(s) address>");
        std::process::exit(2);
    };
    // An argument with a scheme is an address. Anything else is a file path.
    let uri = if target.contains("://") {
        StreamUri::new(target)?
    } else {
        file_uri(target)?
    };

    let player = Player::new()?;
    player.load(&uri)?;
    player.play()?;
    println!("Playing {uri}");
    println!("Commands: p = pause/resume, s = stop, + / - = volume, q = quit");

    // Read the keyboard on another thread so the main loop never blocks on it.
    let (sender, commands) = mpsc::channel();
    thread::spawn(move || {
        for line in std::io::stdin().lock().lines().map_while(Result::ok) {
            if sender.send(line.trim().to_owned()).is_err() {
                break;
            }
        }
    });

    loop {
        match player.poll_event(Duration::from_millis(100)) {
            Some(PlayerEvent::EndOfStream) => {
                println!("End of stream");
                return Ok(());
            }
            Some(PlayerEvent::Error(message)) => return Err(message.into()),
            None => {}
        }
        match commands.try_recv() {
            Ok(command) => match command.as_str() {
                "p" if player.state() == PlaybackState::Playing => {
                    player.pause()?;
                    println!("Paused at {:?}", player.position().unwrap_or_default());
                }
                "p" => {
                    player.play()?;
                    println!("Playing");
                }
                "s" => {
                    player.stop()?;
                    println!("Stopped (p starts again from the beginning)");
                }
                "+" | "-" => {
                    let step = if command == "+" { 0.1 } else { -0.1 };
                    player.set_volume(player.volume() + step);
                    println!("Volume {:.0} %", player.volume() * 100.0);
                }
                "q" => return Ok(()),
                other => println!("Unknown command: {other}"),
            },
            Err(mpsc::TryRecvError::Empty) => {}
            // The keyboard is closed (Ctrl+D): keep playing until the end.
            Err(mpsc::TryRecvError::Disconnected) => {}
        }
    }
}
