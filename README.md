# Unisson

A unified audio player for Linux, written in Rust.
One search box and one play queue for internet radio, local files and YouTube.

The project is under construction. Current target: Ubuntu 22.04.

## Repository layout

- `crates/unisson-core`: the core library, with no user interface
  (common model, source contract, source registry, play queue, merge of the
  search results of several sources).
- `crates/unisson-audio`: the audio engine, built on GStreamer. `Player` plays
  one address (load, play, pause, stop, seek, volume). `Engine` runs it on its
  own thread and reports events (state, position, title on air, end of stream,
  errors). It also reconnects live streams that drop.
- `crates/unisson-radio`: the internet radio source, backed by the free
  [Radio Browser](https://www.radio-browser.info) directory.
- `crates/unisson-local`: local audio files. It scans a music folder, reads
  the tags of each file (title, artist, album, duration, cover), and watches
  the folder for files that appear, change or disappear.
- `crates/unisson-store`: the local storage, an SQLite database kept in
  `~/.local/share/unisson/`. It holds the favorite stations, the library
  (tags of the local files, with a fast search), the listening history, the
  favorite media and the playlists.
- `crates/unisson-playback`: the playback session. It drives the audio engine
  from the play queue, asks the sources for addresses in the background, and
  reports one state (what plays, where, how loud) through events.
- `crates/unisson-app`: the desktop application, built with
  [Tauri 2](https://v2.tauri.app). The Rust side (`src`) exposes commands and
  events. The interface (`ui`) is plain HTML, CSS and JavaScript modules, with
  no build step, embedded in the program at build time.

## Requirements

- Rust 1.90 or newer (install it with [rustup](https://www.rust-lang.org/tools/install)).
- GStreamer 1.14 or newer with its plugins. On Ubuntu:

  ```sh
  sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
      gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
      gstreamer1.0-plugins-bad gstreamer1.0-plugins-ugly gstreamer1.0-libav
  ```

- For the desktop application, the
  [system libraries of Tauri](https://v2.tauri.app/start/prerequisites/).
  On Ubuntu:

  ```sh
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
      libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```

## Run the application

```sh
cargo run -p unisson-app
```

The first build takes a few minutes. What the window offers so far:

- **Home**: tiles to resume what was played last, the favorite stations, the
  recently played media and the local library. One click plays.
- **Radios**: the station on air, the favorite stations, and the Radio Browser
  directory, searched by name, genre and country.
- **Search**: type in the field at the top (Ctrl+K). Every source answers in
  one list. Up and Down move, Enter plays, Shift+Enter adds to the queue,
  Ctrl+Enter plays next, Escape clears. A stream address or the full path of a
  file is played with Enter.
- **Queue**: the panel on the right lists what plays next. Items can be moved,
  removed or jumped to. When the queue runs out, the last radio station takes
  over, unless the switch at the bottom of the panel is off.
- **Player bar**: play, pause, previous, next, seek and volume. Space pauses
  and resumes. On a radio station, previous and next walk the favorites.

No Node.js is needed to build or run. It is only used to test the pure
JavaScript helpers of the interface:

```sh
node --test crates/unisson-app/ui-tests/*.test.mjs
```

## Try the audio engine

```sh
cargo run -p unisson-audio --example play -- ~/Music/song.mp3
```

Then type a command and press Enter:
`p` pause or resume, `s` stop, `+` or `-` volume, `i` position, `q` quit.
An `http(s)://` address works too. Add `--live` before a radio stream address:
the engine then reconnects by itself when the connection is lost.

## Try the radio directory

```sh
cargo run -p unisson-radio --example search -- fip
cargo run -p unisson-radio --example search -- --tag jazz --country FR
```

Each result shows a stream address, which the `play` example above can play
with `--live`.

## Useful commands

```sh
cargo fmt --all                                         # format the code
cargo clippy --workspace --all-targets -- -D warnings   # lint
cargo test --workspace                                  # run the tests
cargo doc --no-deps --workspace --open                  # read the API docs
```

## Try the music folder scan

```sh
cargo run -p unisson-local --example scan
cargo run -p unisson-local --example scan -- ~/Music/Jazz
```

Without a folder, the user's music folder is scanned.

## Try the library

```sh
cargo run -p unisson-store --example library -- scan
cargo run -p unisson-store --example library -- search miles blue
cargo run -p unisson-store --example library -- list
cargo run -p unisson-store --example library -- watch
```

`scan` saves the tags in the database. `search` then answers from the
database: it ignores case and accents, and accepts incomplete words.

`watch` scans the music folder, then follows it until Ctrl+C: copy, rename or
delete an audio file, and the library changes about one second later.

## Try the favorite stations

```sh
cargo run -p unisson-store --example favorites -- add fip
cargo run -p unisson-store --example favorites -- add-url "My radio" https://example.org/stream
cargo run -p unisson-store --example favorites -- list
```

Run it without arguments to see every command. The favorites are saved in the
real database, so they are still there on the next run.

## Branches

- `main`: stable state.
- `develop`: integration branch, and the only work branch on the remote.
  Each task is done on a local `feature/...` branch, checked locally, then
  merged into `develop`. The CI runs on every push to `develop`.
- `develop` is merged into `main` when a coherent set of tasks is ready.

## Working method

1. For the language, rely on the official Rust documentation.
2. For every library, read its documentation before writing code.
3. If a problem resists, look at GitHub issues and forums.
4. One task at a time, tracked on the project Kanban board.

## Conventions

- Code, comments, documentation and commit messages are written in English.
- Public items are documented with rustdoc comments: a one-line summary,
  then details, then `# Errors`, `# Panics` and `# Examples` sections when relevant.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org/).
