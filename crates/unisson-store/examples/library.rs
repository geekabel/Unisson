//! Fills and searches the local library from the command line.
//!
//! ```sh
//! cargo run -p unisson-store --example library -- scan
//! cargo run -p unisson-store --example library -- scan ~/Music/Jazz
//! cargo run -p unisson-store --example library -- search miles blue
//! cargo run -p unisson-store --example library -- list
//! cargo run -p unisson-store --example library -- watch
//! ```
//!
//! `scan` reads the tags of the audio files and saves them in the real
//! database of the application. `search` and `list` then answer from the
//! database, without reading the files again.
//!
//! `watch` scans the folder once, then keeps the library up to date while
//! files are added, changed or removed, until Ctrl+C is pressed.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::Instant;

use unisson_local::{LibraryChange, LibraryWatcher, LocalTrack, music_directory, scan};
use unisson_store::Store;

const USAGE: &str = "usage: library <command>
  scan [folder]     read the tags of a folder (default: the music folder)
  watch [folder]    scan a folder, then follow its changes until Ctrl+C
  search <words>    search the library by title, artist and album
  list              show the whole library";

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let store = Store::open_default()?;

    match arguments.as_slice() {
        ["scan", folder @ ..] if folder.len() <= 1 => {
            scan_into(&store, &folder_or_default(folder)?)?;
        }
        ["watch", folder @ ..] if folder.len() <= 1 => {
            let folder = folder_or_default(folder)?;
            // Watch first, scan second: a file that arrives during the
            // scan is then reported by the watcher instead of being missed.
            let (_watcher, changes) = LibraryWatcher::start(&folder)?;
            scan_into(&store, &folder)?;
            println!("Watching {} (Ctrl+C to stop)", folder.display());
            for change in changes {
                let affected = store.apply_library_change(&change)?;
                match &change {
                    LibraryChange::Updated(track) => {
                        println!("Added or updated: {}", describe(track));
                    }
                    // Removals are also reported for paths the library
                    // never knew, such as a picture: stay silent for those.
                    LibraryChange::Removed(_) if affected == 0 => continue,
                    LibraryChange::Removed(path) => {
                        println!("Removed: {} ({affected} track(s))", path.display());
                    }
                }
                println!("  {} track(s) in the library", store.library_len()?);
            }
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

/// Returns the folder given on the command line, or the music folder.
fn folder_or_default(folder: &[&str]) -> Result<PathBuf, Box<dyn Error>> {
    match folder.first() {
        Some(folder) => Ok(PathBuf::from(folder)),
        None => Ok(music_directory().ok_or("no music folder is known: give one")?),
    }
}

/// Reads the tags of a folder and makes them the library.
fn scan_into(store: &Store, folder: &Path) -> Result<(), Box<dyn Error>> {
    println!("Scanning {}", folder.display());
    let report = scan(folder)?;
    store.replace_library(&report.tracks)?;
    println!(
        "{} track(s) saved in the library, {} file(s) could not be read",
        report.tracks.len(),
        report.unreadable.len()
    );
    Ok(())
}

fn describe(track: &LocalTrack) -> String {
    let artist = track.artist.as_deref().unwrap_or("unknown artist");
    let album = track.album.as_deref().unwrap_or("unknown album");
    format!("{artist} - {} [{album}]", track.title)
}

fn show(tracks: &[LocalTrack]) {
    for track in tracks {
        println!("  {}", describe(track));
    }
}
