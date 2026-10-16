# Changelog

All notable changes to Unisson are written here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and the project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
Until version 1.0.0, a minor version (0.x.0) may change how things work, and a
patch version (0.x.y) only fixes.

## [Unreleased]

### Added

- **Spotify.** Sign in once with a Spotify Premium account, and its tracks
  play inside Unisson, through librespot, in the same queue as the other
  sources. Pasted Spotify track links play too.
- **Spotify backup.** An optional app registered on the Spotify developer
  site lets Unisson drive the official Spotify application. When Spotify
  limits or refuses the playback inside Unisson, the track moves there on its
  own, and Unisson tells you. Search then goes through your own app.
- **Settings screen** (Alt+6), apart from *Sources*. The theme is three
  buttons. The keyboard shortcuts open with F1.

### Changed

- *Sources* holds only the sources and their order. The order can also be
  changed with Alt+Up and Alt+Down.
- Upcoming tracks in the queue name their source.

## [0.1.0] - 2026-10-11

The first public version.

### Added

- **Sources.** Internet radio through the Radio Browser directory, local audio
  files from the music folder, and YouTube through yt-dlp. Each source can be
  switched off, and their order decides which one plays a track found in
  several.
- **Search.** One field for every source, with results as they arrive. A
  pasted stream address, YouTube link, playlist link or file path plays
  directly.
- **Playback.** One queue for every source: play now, play next, add, reorder,
  remove. The last station can take over when the queue runs out. A lost radio
  stream reconnects by itself. A track that cannot start is skipped, with a
  message.
- **Collection.** Favorite stations and tracks, listening history, playlists.
- **Desktop integration on Linux.** Media keys and GNOME media controls
  (MPRIS), tray icon with playback in the background, mini-player, quick
  launcher with a GNOME keyboard shortcut, notifications.
- **Interface.** Dark and light themes that can follow the desktop. English
  and French, following the language of the desktop. Keyboard shortcuts for
  the transport, the volume and the screens. A display that can be enlarged.
- **Packages.** A `.deb` package for Ubuntu 22.04 and newer, and an
  experimental installer for Windows.

### Known limits

- The Windows installer has not been tested on a real machine. It needs the
  GStreamer runtime in the `PATH`. Media keys and the launcher shortcut helper
  are not available on Windows.
- Under Wayland, the desktop decides where the launcher and the mini-player
  appear, and whether the mini-player stays on top.
- Covers of local files are not shown yet.

[Unreleased]: https://github.com/geekabel/Unisson/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/geekabel/Unisson/releases/tag/v0.1.0
