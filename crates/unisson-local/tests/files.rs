//! Integration tests on real files.
//!
//! The audio files are written by the tests themselves: a WAV file is a
//! short header followed by silence, and an MP3 file is a run of empty
//! frames. Tags are then added with the same library the crate reads them
//! with.

use std::path::{Path, PathBuf};
use std::time::Duration;

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureType};
use lofty::tag::{Accessor, Tag, TagExt, TagType};
use unisson_core::{MediaId, SourceKind};
use unisson_local::{LocalError, LocalTrack, is_audio_file, read_cover, scan};

/// Creates an empty folder for one test, and returns its path.
fn test_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir()
        .join(format!("unisson-local-tests-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// Writes `seconds` of silence as a WAV file (8000 Hz, mono, 16 bits).
fn write_wav(path: &Path, seconds: u32) {
    const RATE: u32 = 8000;
    let data_size = seconds * RATE * 2;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes()); // size of the format block
    bytes.extend_from_slice(&1_u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1_u16.to_le_bytes()); // one channel
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes()); // bytes per second
    bytes.extend_from_slice(&2_u16.to_le_bytes()); // bytes per sample
    bytes.extend_from_slice(&16_u16.to_le_bytes()); // bits per sample
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    bytes.resize(bytes.len() + data_size as usize, 0);
    std::fs::write(path, bytes).unwrap();
}

/// Writes about one second of MP3: 38 empty frames of 26 ms each.
fn write_mp3(path: &Path) {
    // MPEG-1 Layer III, 128 kbit/s, 44100 Hz: each frame is 417 bytes long.
    const HEADER: [u8; 4] = [0xFF, 0xFB, 0x90, 0x64];
    let mut bytes = Vec::new();
    for _ in 0..38 {
        bytes.extend_from_slice(&HEADER);
        bytes.resize(bytes.len() + 417 - HEADER.len(), 0);
    }
    std::fs::write(path, bytes).unwrap();
}

const PNG: &[u8] = b"\x89PNG\r\n\x1a\n (not a real picture, but enough for a test)";

/// Writes an MP3 file with a full set of tags and a front cover.
fn write_tagged_mp3(path: &Path, title: &str) {
    write_mp3(path);
    let mut tag = Tag::new(TagType::Id3v2);
    tag.set_title(title.to_owned());
    tag.set_artist("  Miles Davis ".to_owned());
    tag.set_album("Kind of Blue".to_owned());
    tag.set_track(3);
    tag.push_picture(
        Picture::unchecked(b"back cover".to_vec())
            .pic_type(PictureType::CoverBack)
            .mime_type(MimeType::Jpeg)
            .build(),
    );
    tag.push_picture(
        Picture::unchecked(PNG.to_vec())
            .pic_type(PictureType::CoverFront)
            .mime_type(MimeType::Png)
            .build(),
    );
    tag.save_to_path(path, WriteOptions::default()).unwrap();
}

#[test]
fn reads_the_tags_of_an_mp3_file() {
    let path = test_folder("tagged").join("03 blue in green.mp3");
    write_tagged_mp3(&path, "Blue in Green");

    let track = LocalTrack::read(&path).unwrap();

    assert_eq!(track.path, path);
    assert_eq!(track.title, "Blue in Green");
    assert_eq!(
        track.artist.as_deref(),
        Some("Miles Davis"),
        "values are trimmed"
    );
    assert_eq!(track.album.as_deref(), Some("Kind of Blue"));
    assert_eq!(track.track_number, Some(3));
    assert!(track.has_cover);
    let duration = track.duration.expect("the duration is known");
    assert!(
        duration.abs_diff(Duration::from_secs(1)) < Duration::from_millis(100),
        "{duration:?}"
    );
}

#[test]
fn a_file_without_tags_is_named_after_its_file() {
    let path = test_folder("untagged").join("Recording 12.wav");
    write_wav(&path, 2);

    let track = LocalTrack::read(&path).unwrap();

    assert_eq!(track.title, "Recording 12");
    assert_eq!(track.artist, None);
    assert_eq!(track.album, None);
    assert_eq!(track.track_number, None);
    assert_eq!(track.duration, Some(Duration::from_secs(2)));
    assert!(!track.has_cover);
    assert_eq!(read_cover(&path), Ok(None));
}

#[test]
fn the_front_cover_is_preferred() {
    let path = test_folder("cover").join("a.mp3");
    write_tagged_mp3(&path, "A");

    let cover = read_cover(&path).unwrap().expect("the file has a cover");

    assert_eq!(cover.mime_type.as_deref(), Some("image/png"));
    assert_eq!(cover.data, PNG);
}

#[test]
fn a_missing_file_and_a_damaged_file_are_told_apart() {
    let folder = test_folder("errors");
    let missing = LocalTrack::read(folder.join("nothing.mp3"));
    assert!(matches!(missing, Err(LocalError::Io(_))), "{missing:?}");

    let damaged = folder.join("damaged.flac");
    std::fs::write(&damaged, "this is only text").unwrap();
    let result = LocalTrack::read(&damaged);
    assert!(
        matches!(result, Err(LocalError::Unreadable(_))),
        "{result:?}"
    );
    assert!(matches!(
        read_cover(&damaged),
        Err(LocalError::Unreadable(_))
    ));
}

#[test]
fn a_track_becomes_a_local_media() {
    let path = test_folder("media").join("a.mp3");
    write_tagged_mp3(&path, "So What");

    let media = LocalTrack::read(&path).unwrap().to_media();

    assert_eq!(
        media.id,
        MediaId::new(SourceKind::Local, path.to_string_lossy())
    );
    assert_eq!(media.title, "So What");
    assert_eq!(media.artist.as_deref(), Some("Miles Davis"));
    assert_eq!(media.album.as_deref(), Some("Kind of Blue"));
    assert!(media.duration.is_some());
    assert!(!media.is_live());
}

#[test]
fn audio_files_are_recognized_by_their_extension() {
    for audio in [
        "a.mp3",
        "a.FLAC",
        "dir.with.dots/a.Ogg",
        "a.m4a",
        "a.opus",
        "a.wav",
    ] {
        assert!(is_audio_file(audio), "{audio}");
    }
    for other in ["cover.jpg", "notes.txt", "mp3", "a.mp3.part", "folder/"] {
        assert!(!is_audio_file(other), "{other}");
    }
}

#[test]
fn scan_finds_audio_files_in_sub_folders_and_reports_problems() {
    let root = test_folder("scan");
    let album = root.join("Miles Davis").join("Kind of Blue");
    std::fs::create_dir_all(&album).unwrap();
    std::fs::create_dir_all(root.join(".hidden")).unwrap();

    write_tagged_mp3(&album.join("02 freddie.mp3"), "Freddie Freeloader");
    write_tagged_mp3(&album.join("01 so what.mp3"), "So What");
    write_wav(&root.join("voice memo.WAV"), 1);
    // None of these must show up as a track.
    std::fs::write(album.join("cover.jpg"), "picture").unwrap();
    std::fs::write(root.join("damaged.flac"), "this is only text").unwrap();
    write_wav(&root.join(".hidden").join("secret.wav"), 1);
    write_wav(&root.join(".hidden.wav"), 1);

    let report = scan(&root).unwrap();

    let titles: Vec<&str> = report
        .tracks
        .iter()
        .map(|track| track.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["So What", "Freddie Freeloader", "voice memo"],
        "sorted by path"
    );
    assert_eq!(report.unreadable.len(), 1);
    assert_eq!(report.unreadable[0].0, root.join("damaged.flac"));
    assert!(matches!(report.unreadable[0].1, LocalError::Unreadable(_)));
}

#[test]
fn scan_survives_links_that_loop() {
    let root = test_folder("loop");
    let inner = root.join("inner");
    std::fs::create_dir_all(&inner).unwrap();
    write_wav(&inner.join("a.wav"), 1);
    // A link back to the top: following it blindly would never end.
    std::os::unix::fs::symlink(&root, inner.join("back to the top")).unwrap();
    // A link to nothing must simply be ignored.
    std::os::unix::fs::symlink(root.join("nowhere"), root.join("broken.wav")).unwrap();

    let report = scan(&root).unwrap();

    assert_eq!(report.tracks.len(), 1, "each file is found exactly once");
    assert!(report.unreadable.is_empty(), "{:?}", report.unreadable);
}

#[test]
fn scan_of_a_missing_folder_is_an_error() {
    let missing = test_folder("missing").join("does not exist");
    assert!(matches!(scan(&missing), Err(LocalError::Io(_))));
}

#[test]
fn scan_of_an_empty_folder_finds_nothing() {
    let report = scan(test_folder("empty")).unwrap();
    assert!(report.tracks.is_empty());
    assert!(report.unreadable.is_empty());
}
