//! Integration tests of the folder watcher, on a real folder.
//!
//! The watcher is given a short settle delay so the tests stay quick.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use unisson_local::{LibraryChange, LibraryWatcher, LocalError};

mod common;
use common::{test_folder, write_wav};

const SETTLE_DELAY: Duration = Duration::from_millis(150);

/// Long enough for a change to be reported, even on a busy machine.
const PATIENCE: Duration = Duration::from_secs(5);

/// Long enough to be confident that nothing more will be reported.
const QUIET: Duration = Duration::from_millis(700);

/// Creates a watched folder for one test.
///
/// The returned path is the real one, as the watcher reports real paths.
fn watched_folder(name: &str) -> (PathBuf, LibraryWatcher, Receiver<LibraryChange>) {
    let folder = test_folder(name).canonicalize().unwrap();
    let (watcher, changes) = LibraryWatcher::start_with_delay(&folder, SETTLE_DELAY).unwrap();
    (folder, watcher, changes)
}

/// Waits for the next change.
fn next(changes: &Receiver<LibraryChange>) -> LibraryChange {
    changes
        .recv_timeout(PATIENCE)
        .expect("a change should have been reported")
}

/// Waits for the next updated track, and returns its path.
fn next_updated(changes: &Receiver<LibraryChange>) -> PathBuf {
    match next(changes) {
        LibraryChange::Updated(track) => track.path,
        other => panic!("expected an update, got {other:?}"),
    }
}

fn assert_quiet(changes: &Receiver<LibraryChange>) {
    let change = changes.recv_timeout(QUIET);
    assert_eq!(change, Err(RecvTimeoutError::Timeout));
}

fn move_path(from: &Path, to: &Path) {
    std::fs::rename(from, to).unwrap();
}

#[test]
fn a_new_file_is_reported_once_with_its_tags() {
    let (folder, _watcher, changes) = watched_folder("watch-new");
    let path = folder.join("Recording.wav");

    write_wav(&path, 2);

    match next(&changes) {
        LibraryChange::Updated(track) => {
            assert_eq!(track.path, path);
            assert_eq!(track.title, "Recording");
            assert_eq!(track.duration, Some(Duration::from_secs(2)));
        }
        other => panic!("expected an update, got {other:?}"),
    }
    // Writing one file produces several system events: still one change.
    assert_quiet(&changes);
}

#[test]
fn a_file_written_in_several_steps_is_reported_complete() {
    let (folder, _watcher, changes) = watched_folder("watch-slow-copy");
    let path = folder.join("slow.wav");

    // Each step comes before the previous one has settled.
    for seconds in 1..=4 {
        write_wav(&path, seconds);
        std::thread::sleep(SETTLE_DELAY / 3);
    }

    match next(&changes) {
        LibraryChange::Updated(track) => {
            assert_eq!(track.duration, Some(Duration::from_secs(4)));
        }
        other => panic!("expected an update, got {other:?}"),
    }
    assert_quiet(&changes);
}

#[test]
fn a_file_in_a_new_sub_folder_is_reported() {
    let (folder, _watcher, changes) = watched_folder("watch-sub-folder");
    let album = folder.join("Artist").join("Album");

    std::fs::create_dir_all(&album).unwrap();
    // Let the watcher see the new folders before something lands in them.
    assert_quiet(&changes);
    write_wav(&album.join("song.wav"), 1);

    assert_eq!(next_updated(&changes), album.join("song.wav"));
}

#[test]
fn a_removed_file_is_reported() {
    let (folder, _watcher, changes) = watched_folder("watch-removed");
    let path = folder.join("song.wav");
    write_wav(&path, 1);
    assert_eq!(next_updated(&changes), path);

    std::fs::remove_file(&path).unwrap();

    assert_eq!(next(&changes), LibraryChange::Removed(path));
    assert_quiet(&changes);
}

#[test]
fn a_renamed_file_is_removed_then_added() {
    let (folder, _watcher, changes) = watched_folder("watch-renamed");
    let old = folder.join("old.wav");
    let new = folder.join("new.wav");
    write_wav(&old, 1);
    assert_eq!(next_updated(&changes), old);

    move_path(&old, &new);

    let mut reported = [next(&changes), next(&changes)];
    reported.sort_by_key(|change| matches!(change, LibraryChange::Updated(_)));
    assert_eq!(reported[0], LibraryChange::Removed(old));
    assert!(matches!(&reported[1], LibraryChange::Updated(track) if track.path == new));
    assert_quiet(&changes);
}

#[test]
fn a_folder_moved_in_brings_its_tracks_and_takes_them_away() {
    // The album is prepared outside the watched folder, then moved in.
    let outside = test_folder("watch-moved-outside").canonicalize().unwrap();
    let album = outside.join("Album");
    std::fs::create_dir_all(album.join("CD 2")).unwrap();
    write_wav(&album.join("one.wav"), 1);
    write_wav(&album.join("CD 2").join("two.wav"), 1);

    let (folder, _watcher, changes) = watched_folder("watch-moved-in");
    let arrived = folder.join("Album");
    move_path(&album, &arrived);

    let mut paths = vec![next_updated(&changes), next_updated(&changes)];
    paths.sort();
    assert_eq!(
        paths,
        [
            arrived.join("CD 2").join("two.wav"),
            arrived.join("one.wav")
        ]
    );
    assert_quiet(&changes);

    // Moved out again: one removal, for the folder itself.
    move_path(&arrived, &album);
    assert_eq!(next(&changes), LibraryChange::Removed(arrived));
    assert_quiet(&changes);
}

#[test]
fn paths_are_reported_under_the_root_as_it_was_given() {
    let base = test_folder("watch-link");
    let real = base.join("real");
    let link = base.join("link");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let (_watcher, changes) = LibraryWatcher::start_with_delay(&link, SETTLE_DELAY).unwrap();

    write_wav(&real.join("song.wav"), 1);
    assert_eq!(next_updated(&changes), link.join("song.wav"));

    std::fs::remove_file(real.join("song.wav")).unwrap();
    assert_eq!(
        next(&changes),
        LibraryChange::Removed(link.join("song.wav"))
    );
}

#[test]
fn hidden_files_and_other_files_are_ignored() {
    let (folder, _watcher, changes) = watched_folder("watch-ignored");

    std::fs::write(folder.join("cover.jpg"), b"picture").unwrap();
    std::fs::write(folder.join("notes.txt"), b"text").unwrap();
    write_wav(&folder.join(".hidden.wav"), 1);
    std::fs::create_dir(folder.join(".cache")).unwrap();
    write_wav(&folder.join(".cache").join("song.wav"), 1);
    // A download in progress, removed before it settles.
    std::fs::remove_file(folder.join(".hidden.wav")).unwrap();

    assert_quiet(&changes);
}

#[test]
fn a_damaged_audio_file_is_not_reported() {
    let (folder, _watcher, changes) = watched_folder("watch-damaged");

    std::fs::write(folder.join("broken.mp3"), b"this is not music").unwrap();

    assert_quiet(&changes);
}

#[test]
fn a_missing_folder_cannot_be_watched() {
    let missing = test_folder("watch-missing").join("nothing here");
    let result = LibraryWatcher::start(&missing);
    assert!(matches!(result, Err(LocalError::Io(_))), "{result:?}");
}

#[test]
fn dropping_the_watcher_ends_the_changes() {
    let (folder, watcher, changes) = watched_folder("watch-dropped");

    drop(watcher);
    write_wav(&folder.join("late.wav"), 1);

    assert_eq!(
        changes.recv_timeout(PATIENCE),
        Err(RecvTimeoutError::Disconnected)
    );
}
