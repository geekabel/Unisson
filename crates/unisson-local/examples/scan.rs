//! Scans a music folder from the command line.
//!
//! ```sh
//! cargo run -p unisson-local --example scan
//! cargo run -p unisson-local --example scan -- ~/Musique/Jazz
//! ```
//!
//! Without a folder, the user's music folder is scanned.

use std::error::Error;
use std::path::PathBuf;
use std::time::Instant;

use unisson_local::{music_directory, scan};

/// How many tracks and problems are shown at most.
const SHOWN: usize = 15;

fn main() -> Result<(), Box<dyn Error>> {
    let folder = match std::env::args().nth(1) {
        Some(folder) => PathBuf::from(folder),
        None => music_directory().ok_or("no music folder is known: give one as argument")?,
    };
    println!("Scanning {}", folder.display());

    let start = Instant::now();
    let report = scan(&folder)?;
    let elapsed = start.elapsed();

    for track in report.tracks.iter().take(SHOWN) {
        let artist = track.artist.as_deref().unwrap_or("unknown artist");
        let album = track.album.as_deref().unwrap_or("unknown album");
        let duration = track.duration.map_or("?:??".to_owned(), |duration| {
            format!("{}:{:02}", duration.as_secs() / 60, duration.as_secs() % 60)
        });
        let cover = if track.has_cover { "cover" } else { "no cover" };
        println!(
            "  {} - {} [{album}] {duration}, {cover}",
            artist, track.title
        );
    }
    if report.tracks.len() > SHOWN {
        println!("  ... and {} more", report.tracks.len() - SHOWN);
    }

    println!("{} track(s) found in {elapsed:.1?}", report.tracks.len());
    let with_cover = report.tracks.iter().filter(|track| track.has_cover).count();
    println!("{with_cover} track(s) have a cover picture");
    println!(
        "{} file(s) or folder(s) could not be read",
        report.unreadable.len()
    );
    for (path, error) in report.unreadable.iter().take(SHOWN) {
        println!("  {}: {error}", path.display());
    }
    Ok(())
}
