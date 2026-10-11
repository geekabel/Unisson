//! Searches the Radio Browser directory from the command line.
//!
//! ```sh
//! cargo run -p unisson-radio --example search -- fip
//! cargo run -p unisson-radio --example search -- --tag jazz --country FR
//! ```
//!
//! Each line shows a station and its stream address. The address can be
//! played with the `play` example of the `unisson-audio` crate.

use std::error::Error;

use unisson_radio::{RadioBrowser, StationQuery};

fn main() -> Result<(), Box<dyn Error>> {
    let mut query = StationQuery::new().limit(10);
    let mut arguments = std::env::args().skip(1);
    let mut has_criteria = false;
    while let Some(argument) = arguments.next() {
        query = match argument.as_str() {
            "--tag" => query.tag(arguments.next().ok_or("--tag needs a value")?),
            "--country" => query.country_code(arguments.next().ok_or("--country needs a value")?),
            name => query.name(name),
        };
        has_criteria = true;
    }
    if !has_criteria {
        eprintln!("usage: search [name] [--tag <genre>] [--country <two-letter code>]");
        std::process::exit(2);
    }

    let stations = RadioBrowser::new().search(&query)?;
    if stations.is_empty() {
        println!("No station found.");
    }
    for station in stations {
        let country = station.country_code.as_deref().unwrap_or("??");
        let codec = station.codec.as_deref().unwrap_or("?");
        let bitrate = station
            .bitrate
            .map_or("?".to_owned(), |bitrate| bitrate.to_string());
        println!("{} [{country}, {codec} {bitrate} kbit/s]", station.name);
        println!("    {}", station.stream_url);
    }
    Ok(())
}
