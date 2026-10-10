//! The play queue: one ordered list for every source.
//!
//! # Design
//!
//! The queue has three parts, in listening order:
//!
//! ```text
//! history (already played) | current | upcoming (what plays next)
//! ```
//!
//! [`PlayQueue::advance`] moves everything one step to the left, and
//! [`PlayQueue::go_back`] one step to the right. Going back never loses the
//! media that was playing: it becomes the first upcoming item again.
//!
//! The queue only orders [`Media`]. It does not play anything: the
//! application reads [`PlayQueue::current`] and hands it to the audio engine.

use std::collections::VecDeque;

use crate::model::Media;

/// A queue of media to play, mixing every source.
///
/// # Examples
///
/// ```
/// use unisson_core::{Media, MediaId, PlayQueue, SourceKind};
///
/// let track = |title: &str| Media::track(MediaId::new(SourceKind::Local, title), title);
///
/// let mut queue = PlayQueue::new();
/// queue.play_now(track("So What"));
/// queue.push_back(track("Blue in Green"));
/// queue.push_next(track("Take Five"));
///
/// // "Take Five" was asked to play next, so it jumps ahead.
/// assert_eq!(queue.advance().map(|media| media.title.as_str()), Some("Take Five"));
/// assert_eq!(queue.advance().map(|media| media.title.as_str()), Some("Blue in Green"));
/// // Nothing is left to play.
/// assert!(queue.advance().is_none());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayQueue {
    /// Media already played, oldest first.
    history: Vec<Media>,
    /// The media being played, if any.
    current: Option<Media>,
    /// Media waiting to be played, next one first.
    upcoming: VecDeque<Media>,
}

impl PlayQueue {
    /// Maximum number of played media kept for [`go_back`](Self::go_back).
    ///
    /// Older entries are dropped, so a long listening session cannot grow
    /// the queue without bound.
    pub const HISTORY_LIMIT: usize = 200;

    /// Creates an empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the media being played, if any.
    pub fn current(&self) -> Option<&Media> {
        self.current.as_ref()
    }

    /// Returns the media waiting to be played, next one first.
    pub fn upcoming(&self) -> impl ExactSizeIterator<Item = &Media> + DoubleEndedIterator {
        self.upcoming.iter()
    }

    /// Returns the media already played, oldest first.
    pub fn history(&self) -> &[Media] {
        &self.history
    }

    /// Returns the number of media waiting to be played.
    ///
    /// The current media is not counted.
    pub fn len(&self) -> usize {
        self.upcoming.len()
    }

    /// Returns `true` if no media is waiting to be played.
    pub fn is_empty(&self) -> bool {
        self.upcoming.is_empty()
    }

    /// Adds a media at the end of the queue ("add to queue").
    pub fn push_back(&mut self, media: Media) {
        self.upcoming.push_back(media);
    }

    /// Adds a media right after the current one ("play next").
    pub fn push_next(&mut self, media: Media) {
        self.upcoming.push_front(media);
    }

    /// Makes a media the current one right away ("play now").
    ///
    /// The media that was playing goes to the history. The upcoming media
    /// are kept, and play after the new one.
    pub fn play_now(&mut self, media: Media) -> &Media {
        self.retire_current();
        self.current.insert(media)
    }

    /// Moves to the next media, and returns it.
    ///
    /// The media that was playing goes to the history. Returns `None` when
    /// nothing is left to play: the queue then has no current media.
    pub fn advance(&mut self) -> Option<&Media> {
        self.retire_current();
        self.current = self.upcoming.pop_front();
        self.current.as_ref()
    }

    /// Moves back to the previously played media, and returns it.
    ///
    /// The media that was playing becomes the first upcoming one again.
    /// Returns `None`, and changes nothing, when the history is empty.
    pub fn go_back(&mut self) -> Option<&Media> {
        let previous = self.history.pop()?;
        if let Some(current) = self.current.replace(previous) {
            self.upcoming.push_front(current);
        }
        self.current.as_ref()
    }

    /// Jumps to an upcoming media, and returns it.
    ///
    /// `index` counts from the next media, starting at 0. The media before it
    /// are skipped and removed from the queue. Returns `None`, and changes
    /// nothing, if `index` is out of range.
    pub fn jump_to(&mut self, index: usize) -> Option<&Media> {
        if index >= self.upcoming.len() {
            return None;
        }
        self.upcoming.drain(..index);
        self.advance()
    }

    /// Removes an upcoming media, and returns it.
    ///
    /// `index` counts from the next media, starting at 0. Returns `None` if
    /// `index` is out of range.
    pub fn remove(&mut self, index: usize) -> Option<Media> {
        self.upcoming.remove(index)
    }

    /// Moves an upcoming media to another position.
    ///
    /// Both indexes count from the next media, starting at 0. Returns
    /// `false`, and changes nothing, if either index is out of range.
    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.upcoming.len() || to >= self.upcoming.len() {
            return false;
        }
        if let Some(media) = self.upcoming.remove(from) {
            self.upcoming.insert(to, media);
        }
        true
    }

    /// Removes every upcoming media ("clear the queue").
    ///
    /// The current media and the history are kept.
    pub fn clear(&mut self) {
        self.upcoming.clear();
    }

    /// Moves the current media, if any, to the history.
    fn retire_current(&mut self) {
        if let Some(media) = self.current.take() {
            if self.history.len() == Self::HISTORY_LIMIT {
                self.history.remove(0);
            }
            self.history.push(media);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{MediaId, SourceKind};

    fn track(source: SourceKind, title: &str) -> Media {
        Media::track(MediaId::new(source, title), title)
    }

    fn local(title: &str) -> Media {
        track(SourceKind::Local, title)
    }

    fn titles<'a>(media: impl Iterator<Item = &'a Media>) -> Vec<&'a str> {
        media.map(|media| media.title.as_str()).collect()
    }

    fn title(media: Option<&Media>) -> Option<&str> {
        media.map(|media| media.title.as_str())
    }

    /// Builds a queue playing "a", with "b", "c" and "d" waiting.
    fn sample() -> PlayQueue {
        let mut queue = PlayQueue::new();
        queue.play_now(local("a"));
        for name in ["b", "c", "d"] {
            queue.push_back(local(name));
        }
        queue
    }

    #[test]
    fn a_new_queue_is_empty() {
        let queue = PlayQueue::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
        assert_eq!(queue.current(), None);
        assert!(queue.history().is_empty());
    }

    #[test]
    fn push_back_adds_at_the_end() {
        let queue = sample();
        assert_eq!(title(queue.current()), Some("a"));
        assert_eq!(titles(queue.upcoming()), ["b", "c", "d"]);
        assert_eq!(queue.len(), 3);
    }

    #[test]
    fn push_next_adds_right_after_the_current_media() {
        let mut queue = sample();
        queue.push_next(local("x"));
        assert_eq!(title(queue.current()), Some("a"));
        assert_eq!(titles(queue.upcoming()), ["x", "b", "c", "d"]);
    }

    #[test]
    fn play_now_keeps_the_upcoming_media() {
        let mut queue = sample();
        assert_eq!(queue.play_now(local("x")).title, "x");
        assert_eq!(titles(queue.history().iter()), ["a"]);
        assert_eq!(titles(queue.upcoming()), ["b", "c", "d"]);
    }

    #[test]
    fn advance_walks_the_queue_then_stops() {
        let mut queue = sample();
        assert_eq!(title(queue.advance()), Some("b"));
        assert_eq!(title(queue.advance()), Some("c"));
        assert_eq!(title(queue.advance()), Some("d"));
        assert_eq!(queue.advance(), None);
        assert_eq!(queue.current(), None);
        assert_eq!(titles(queue.history().iter()), ["a", "b", "c", "d"]);
        // Advancing an exhausted queue changes nothing.
        assert_eq!(queue.advance(), None);
        assert_eq!(queue.history().len(), 4);
    }

    #[test]
    fn advance_starts_a_queue_that_has_no_current_media() {
        let mut queue = PlayQueue::new();
        queue.push_back(local("a"));
        assert_eq!(title(queue.advance()), Some("a"));
        assert!(queue.history().is_empty());
    }

    #[test]
    fn go_back_returns_to_the_previous_media_without_losing_the_current_one() {
        let mut queue = sample();
        queue.advance();
        queue.advance();
        assert_eq!(title(queue.go_back()), Some("b"));
        assert_eq!(titles(queue.upcoming()), ["c", "d"]);
        assert_eq!(titles(queue.history().iter()), ["a"]);
        // Going forward again plays the same media in the same order.
        assert_eq!(title(queue.advance()), Some("c"));
    }

    #[test]
    fn go_back_with_no_history_changes_nothing() {
        let mut queue = sample();
        let before = queue.clone();
        assert_eq!(queue.go_back(), None);
        assert_eq!(queue, before);
    }

    #[test]
    fn go_back_works_after_the_queue_ran_out() {
        let mut queue = PlayQueue::new();
        queue.play_now(local("a"));
        assert_eq!(queue.advance(), None);
        assert_eq!(title(queue.go_back()), Some("a"));
        assert!(queue.is_empty());
    }

    #[test]
    fn jump_to_skips_the_media_before_the_target() {
        let mut queue = sample();
        assert_eq!(title(queue.jump_to(1)), Some("c"));
        assert_eq!(titles(queue.upcoming()), ["d"]);
        assert_eq!(titles(queue.history().iter()), ["a"]);
    }

    #[test]
    fn jump_to_an_unknown_position_changes_nothing() {
        let mut queue = sample();
        let before = queue.clone();
        assert_eq!(queue.jump_to(3), None);
        assert_eq!(queue, before);
    }

    #[test]
    fn remove_takes_one_upcoming_media_out() {
        let mut queue = sample();
        assert_eq!(queue.remove(1).map(|media| media.title), Some("c".into()));
        assert_eq!(titles(queue.upcoming()), ["b", "d"]);
        assert_eq!(queue.remove(9), None);
    }

    #[test]
    fn move_item_reorders_the_upcoming_media() {
        let mut queue = sample();
        assert!(queue.move_item(0, 2));
        assert_eq!(titles(queue.upcoming()), ["c", "d", "b"]);
        assert!(queue.move_item(2, 0));
        assert_eq!(titles(queue.upcoming()), ["b", "c", "d"]);
        assert!(queue.move_item(1, 1));
        assert!(!queue.move_item(0, 3));
        assert!(!queue.move_item(3, 0));
        assert_eq!(titles(queue.upcoming()), ["b", "c", "d"]);
    }

    #[test]
    fn clear_keeps_the_current_media_and_the_history() {
        let mut queue = sample();
        queue.advance();
        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(title(queue.current()), Some("b"));
        assert_eq!(titles(queue.history().iter()), ["a"]);
    }

    #[test]
    fn one_queue_mixes_every_source() {
        let mut queue = PlayQueue::new();
        queue.play_now(Media::station(
            MediaId::new(SourceKind::Radio, "fip"),
            "FIP",
        ));
        queue.push_back(track(SourceKind::YouTube, "Weightless"));
        queue.push_back(track(SourceKind::Local, "Take Five"));

        let mut sources = vec![queue.current().unwrap().id.source];
        while let Some(media) = queue.advance() {
            sources.push(media.id.source);
        }
        assert_eq!(
            sources,
            [SourceKind::Radio, SourceKind::YouTube, SourceKind::Local]
        );
    }

    #[test]
    fn the_history_is_bounded() {
        let mut queue = PlayQueue::new();
        for index in 0..PlayQueue::HISTORY_LIMIT + 5 {
            queue.play_now(local(&index.to_string()));
        }
        // The last media is current, so the history holds the ones before it.
        assert_eq!(queue.history().len(), PlayQueue::HISTORY_LIMIT);
        assert_eq!(queue.history()[0].title, "4");
        assert_eq!(title(queue.current()), Some("204"));
    }
}
