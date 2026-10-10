//! Helpers shared by the integration tests.

// Each test file uses its own subset of the helpers.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Creates an empty folder for one test, and returns its path.
pub fn test_folder(name: &str) -> PathBuf {
    let folder = std::env::temp_dir()
        .join(format!("unisson-local-tests-{}", std::process::id()))
        .join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// Writes `seconds` of silence as a WAV file (8000 Hz, mono, 16 bits).
pub fn write_wav(path: &Path, seconds: u32) {
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
