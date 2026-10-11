//! The user's favorite stations.

use crate::station::Station;

/// An ordered list of favorite stations, without duplicates.
///
/// The order is the user's own. It drives the "previous station" and "next
/// station" buttons, which go round the list like the presets of a car
/// radio.
///
/// The list lives in memory only. Saving it is the job of the storage
/// crate.
///
/// # Examples
///
/// ```
/// use unisson_radio::{Favorites, Station};
///
/// let fip = Station::custom("FIP", "https://example.org/fip")?;
/// let nova = Station::custom("Nova", "https://example.org/nova")?;
///
/// let mut favorites = Favorites::new();
/// favorites.add(fip.clone());
/// favorites.add(nova.clone());
///
/// assert_eq!(favorites.next_after(&fip.uuid), Some(&nova));
/// // After the last station comes the first one again.
/// assert_eq!(favorites.next_after(&nova.uuid), Some(&fip));
/// # Ok::<(), unisson_core::SourceError>(())
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Favorites {
    stations: Vec<Station>,
}

impl Favorites {
    /// Creates an empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a list from stations, in the given order.
    ///
    /// If a station appears twice, only its first occurrence is kept.
    pub fn from_stations(stations: impl IntoIterator<Item = Station>) -> Self {
        let mut favorites = Self::new();
        for station in stations {
            if !favorites.contains(&station.uuid) {
                favorites.stations.push(station);
            }
        }
        favorites
    }

    /// Returns the stations, in the user's order.
    pub fn stations(&self) -> &[Station] {
        &self.stations
    }

    /// Returns the number of stations.
    pub fn len(&self) -> usize {
        self.stations.len()
    }

    /// Returns `true` if there is no station.
    pub fn is_empty(&self) -> bool {
        self.stations.is_empty()
    }

    /// Returns `true` if the station with this identifier is a favorite.
    pub fn contains(&self, uuid: &str) -> bool {
        self.position(uuid).is_some()
    }

    /// Returns the favorite station with this identifier, if any.
    pub fn get(&self, uuid: &str) -> Option<&Station> {
        self.stations.iter().find(|station| station.uuid == uuid)
    }

    /// Adds a station at the end of the list.
    ///
    /// Returns `false` if the station was already a favorite. Its details
    /// are then refreshed, and it keeps its place.
    pub fn add(&mut self, station: Station) -> bool {
        match self.position(&station.uuid) {
            Some(index) => {
                self.stations[index] = station;
                false
            }
            None => {
                self.stations.push(station);
                true
            }
        }
    }

    /// Removes a station, and returns it.
    ///
    /// Returns `None` if the station was not a favorite.
    pub fn remove(&mut self, uuid: &str) -> Option<Station> {
        self.position(uuid).map(|index| self.stations.remove(index))
    }

    /// Moves a station to another position, both counted from 0.
    ///
    /// Returns `false`, and changes nothing, if either index is out of range.
    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.stations.len() || to >= self.stations.len() {
            return false;
        }
        let station = self.stations.remove(from);
        self.stations.insert(to, station);
        true
    }

    /// Returns the favorite that follows the given station.
    ///
    /// After the last favorite comes the first one. If the given station is
    /// not a favorite, the first favorite is returned. Returns `None` only
    /// when the list is empty.
    pub fn next_after(&self, uuid: &str) -> Option<&Station> {
        let index = match self.position(uuid) {
            Some(index) => (index + 1) % self.stations.len(),
            None => 0,
        };
        self.stations.get(index)
    }

    /// Returns the favorite that precedes the given station.
    ///
    /// Before the first favorite comes the last one. If the given station is
    /// not a favorite, the last favorite is returned. Returns `None` only
    /// when the list is empty.
    pub fn previous_before(&self, uuid: &str) -> Option<&Station> {
        match self.position(uuid) {
            Some(0) | None => self.stations.last(),
            Some(index) => self.stations.get(index - 1),
        }
    }

    fn position(&self, uuid: &str) -> Option<usize> {
        self.stations
            .iter()
            .position(|station| station.uuid == uuid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(name: &str) -> Station {
        Station::custom(name, &format!("https://example.org/{name}")).unwrap()
    }

    fn names(favorites: &Favorites) -> Vec<&str> {
        favorites
            .stations()
            .iter()
            .map(|station| station.name.as_str())
            .collect()
    }

    /// Builds the favorites "a", "b" and "c", and returns them with the list.
    fn sample() -> (Favorites, [Station; 3]) {
        let stations = ["a", "b", "c"].map(station);
        (Favorites::from_stations(stations.clone()), stations)
    }

    #[test]
    fn a_new_list_is_empty() {
        let favorites = Favorites::new();
        assert!(favorites.is_empty());
        assert_eq!(favorites.len(), 0);
        assert_eq!(favorites.next_after("anything"), None);
        assert_eq!(favorites.previous_before("anything"), None);
    }

    #[test]
    fn add_appends_and_refuses_duplicates() {
        let (mut favorites, [a, _, _]) = sample();
        assert!(favorites.add(station("d")));
        assert_eq!(names(&favorites), ["a", "b", "c", "d"]);

        // Adding "a" again only refreshes its details, in place.
        let mut renamed = a.clone();
        renamed.name = "A (renamed)".into();
        assert!(!favorites.add(renamed));
        assert_eq!(names(&favorites), ["A (renamed)", "b", "c", "d"]);
        assert!(favorites.contains(&a.uuid));
        assert_eq!(
            favorites.get(&a.uuid).map(|station| station.name.as_str()),
            Some("A (renamed)")
        );
    }

    #[test]
    fn from_stations_keeps_the_first_occurrence() {
        let favorites = Favorites::from_stations([station("a"), station("b"), station("a")]);
        assert_eq!(names(&favorites), ["a", "b"]);
    }

    #[test]
    fn remove_takes_a_station_out() {
        let (mut favorites, [_, b, _]) = sample();
        assert_eq!(favorites.remove(&b.uuid), Some(b.clone()));
        assert_eq!(names(&favorites), ["a", "c"]);
        assert_eq!(favorites.remove(&b.uuid), None);
        assert!(!favorites.contains(&b.uuid));
    }

    #[test]
    fn move_item_reorders_the_list() {
        let (mut favorites, _) = sample();
        assert!(favorites.move_item(0, 2));
        assert_eq!(names(&favorites), ["b", "c", "a"]);
        assert!(!favorites.move_item(0, 3));
        assert!(!favorites.move_item(3, 0));
        assert_eq!(names(&favorites), ["b", "c", "a"]);
    }

    #[test]
    fn next_and_previous_go_round_the_list() {
        let (favorites, [a, b, c]) = sample();
        assert_eq!(favorites.next_after(&a.uuid), Some(&b));
        assert_eq!(favorites.next_after(&b.uuid), Some(&c));
        assert_eq!(favorites.next_after(&c.uuid), Some(&a));
        assert_eq!(favorites.previous_before(&c.uuid), Some(&b));
        assert_eq!(favorites.previous_before(&b.uuid), Some(&a));
        assert_eq!(favorites.previous_before(&a.uuid), Some(&c));
    }

    #[test]
    fn from_a_station_outside_the_list_next_is_the_first_and_previous_the_last() {
        let (favorites, [a, _, c]) = sample();
        assert_eq!(favorites.next_after("not-a-favorite"), Some(&a));
        assert_eq!(favorites.previous_before("not-a-favorite"), Some(&c));
    }

    #[test]
    fn with_a_single_favorite_next_and_previous_stay_on_it() {
        let only = station("a");
        let favorites = Favorites::from_stations([only.clone()]);
        assert_eq!(favorites.next_after(&only.uuid), Some(&only));
        assert_eq!(favorites.previous_before(&only.uuid), Some(&only));
    }
}
