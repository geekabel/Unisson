//! Fills and searches the local library from the command line.
//!
//! ```sh
//! cargo run -p unisson-store --example library -- scan
//! cargo run -p unisson-store --example library -- scan ~/Music/Jazz
//! cargo run -p unisson-store --example library -- search miles blue
//! cargo run -p unisson-store --example library -- list
//! ```
//!
//! `scan` reads the tags of the audio files and saves them in the real
//! database of the application. `search` and `list` then answer from the
//! database, without reading the files again.

use std::error::Error;
use std::path::PathBuf;
use std::time::Instant;

use unisson_local::{LocalTrack, music_directory, scan};
use unisson_store::Store;

const USAGE: &str = "usage: library <command>
  scan [folder]     read the tags of a folder (default: the music folder)
  search <words>    search the library by title, artist and album
  list              show the whole library";

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let store = Store::open_default()?;

    match arguments.as_slice() {
        ["scan", folder @ ..] if folder.len() <= 1 => {
            let folder = match folder.first() {
                Some(folder) => PathBuf::from(folder),
                None => music_directory().ok_or("no music folder is known: give one")?,
            };
            println!("Scanning {}", folder.display());
            let report = scan(&folder)?;
            store.replace_library(&report.tracks)?;
            println!(
                "{} track(s) saved in the library, {} file(s) could not be read",
                report.tracks.len(),
                report.unreadable.len()
            );
        }
        ["search", words @ ..] if !words.is_empty() => {
            let start = Instant::now();
            let tracks = store.search_library(&words.join(" "), 20)?;
            let elapsed = start.elapsed();
            show(&tracks);
            println!("{} track(s) found in {elapsed:.1?}", tracks.len());
        }
        ["list"] => {
            let tracks = store.library_tracks()?;
            show(&tracks);
            println!("{} track(s) in the library", tracks.len());
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }
    Ok(())
}

fn show(tracks: &[LocalTrack]) {
    for track in tracks {
        let artist = track.artist.as_deref().unwrap_or("unknown artist");
        let album = track.album.as_deref().unwrap_or("unknown album");
        println!("  {artist} - {} [{album}]", track.title);
    }
}
