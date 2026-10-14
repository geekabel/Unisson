//! Manages the favorite radio stations from the command line.
//!
//! ```sh
//! cargo run -p unisson-store --example favorites -- add fip
//! cargo run -p unisson-store --example favorites -- add-url "My radio" https://example.org/stream
//! cargo run -p unisson-store --example favorites -- list
//! cargo run -p unisson-store --example favorites -- around 1
//! cargo run -p unisson-store --example favorites -- move 3 1
//! cargo run -p unisson-store --example favorites -- remove 2
//! ```
//!
//! The favorites are saved in the real database of the application, so they
//! are still there the next time the command runs.

use std::error::Error;

use unisson_radio::{Favorites, RadioBrowser, Station, StationQuery};
use unisson_store::{Store, default_database_path};

const USAGE: &str = "usage: favorites <command>
  list                    show the favorites
  add <name>              add the most listened station with that name
  add-url <name> <url>    add a station by its stream address
  remove <number>         remove a favorite
  move <from> <to>        move a favorite to another position
  around <number>         show the previous and the next favorite";

fn main() -> Result<(), Box<dyn Error>> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();

    let store = Store::open_default()?;
    let mut favorites = Favorites::from_stations(store.favorite_stations()?);

    match arguments.as_slice() {
        ["list"] => {}
        ["add", name] => {
            let found = RadioBrowser::new().search(&StationQuery::by_name(name).limit(1))?;
            let station = found.into_iter().next().ok_or("no station has that name")?;
            report_added(favorites.add(station.clone()), &station);
        }
        ["add-url", name, url] => {
            let station = Station::custom(name, url)?;
            report_added(favorites.add(station.clone()), &station);
        }
        ["remove", number] => {
            let uuid = station_at(&favorites, number)?.uuid.clone();
            if let Some(removed) = favorites.remove(&uuid) {
                println!("Removed {}", removed.name);
            }
        }
        ["move", from, to] => {
            let (from, to) = (index(from)?, index(to)?);
            if !favorites.move_item(from, to) {
                return Err("there is no favorite with that number".into());
            }
        }
        ["around", number] => {
            let station = station_at(&favorites, number)?;
            let previous = favorites.previous_before(&station.uuid);
            let next = favorites.next_after(&station.uuid);
            println!("Current:  {}", station.name);
            println!("Previous: {}", name_of(previous));
            println!("Next:     {}", name_of(next));
            return Ok(());
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    }

    store.save_favorite_stations(favorites.stations())?;
    println!(
        "Favorites, saved in {}:",
        default_database_path()?.display()
    );
    if favorites.is_empty() {
        println!("  (none)");
    }
    for (position, station) in favorites.stations().iter().enumerate() {
        println!(
            "  {}. {}  {}",
            position + 1,
            station.name,
            station.stream_url
        );
    }
    Ok(())
}

fn name_of(station: Option<&Station>) -> &str {
    station.map_or("-", |station| station.name.as_str())
}

fn report_added(added: bool, station: &Station) {
    if added {
        println!("Added {}", station.name);
    } else {
        println!("{} was already a favorite", station.name);
    }
}

/// Turns a number typed by the user, starting at 1, into a list index.
fn index(number: &str) -> Result<usize, Box<dyn Error>> {
    let number: usize = number
        .parse()
        .map_err(|_| format!("not a number: {number}"))?;
    number
        .checked_sub(1)
        .ok_or_else(|| "numbers start at 1".into())
}

fn station_at<'a>(favorites: &'a Favorites, number: &str) -> Result<&'a Station, Box<dyn Error>> {
    favorites
        .stations()
        .get(index(number)?)
        .ok_or_else(|| "there is no favorite with that number".into())
}
