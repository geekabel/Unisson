# Development notes

How to run, test and find your way in the code of Unisson. The
[README](../README.md#build-from-source) lists what to install first.

## Run the application

```sh
cargo run -p unisson-app
```

The interface is in `crates/unisson-app/ui`: plain HTML, CSS and JavaScript
modules, embedded in the program at build time. There is no bundler and no
Node.js step. Node.js is only used to run the unit tests of the interface.

## Checks

```sh
cargo fmt --all                                         # format the code
cargo clippy --workspace --all-targets -- -D warnings   # lint
cargo test --workspace                                  # run the Rust tests
cargo doc --no-deps --workspace --open                  # read the API docs
node --test crates/unisson-app/ui-tests/*.test.mjs      # test the interface helpers
```

The tests of the interface include two guards: `contrast.test.mjs` checks the
colors of both themes against WCAG 2.1 (level AA), and `i18n.test.mjs` checks
that English and French hold the same texts.

[`manual-tests.md`](manual-tests.md) lists what to check by hand on a real
desktop, once under Wayland and once under X11.

## The crates

- `crates/unisson-core`: the core library, with no user interface: common
  model, source contract, source registry, play queue, merge of the search
  results of several sources.
- `crates/unisson-audio`: the audio engine, built on GStreamer. `Player` plays
  one address. `Engine` runs it on its own thread and reports events (state,
  position, title on air, end of stream, failures with their kind). It also
  reconnects live streams that drop.
- `crates/unisson-radio`: the internet radio source, backed by the free
  [Radio Browser](https://www.radio-browser.info) directory.
- `crates/unisson-local`: local audio files. It scans a music folder, reads
  the tags of each file, and watches the folder for changes.
- `crates/unisson-youtube`: the YouTube source. It runs
  [yt-dlp](https://github.com/yt-dlp/yt-dlp) to search and to find the audio
  address of a video. It also downloads yt-dlp, and Deno when needed, checks
  them, and keeps yt-dlp up to date. `cargo run -p unisson-youtube --example
  install -- /tmp/yt` does that download alone.
- `crates/unisson-store`: the local storage, an SQLite database. It holds the
  favorite stations, the library, the listening history, the favorite media
  and the playlists.
- `crates/unisson-playback`: the playback session. One thread owns the queue
  and the engine, asks the sources for addresses in the background, and
  reports one state through events.
- `crates/unisson-app`: the desktop application, built with
  [Tauri 2](https://v2.tauri.app). The Rust side (`src`) exposes commands and
  events and holds the desktop integration. The interface is in `ui`.

Where data lives:

| | Linux | Windows |
| --- | --- | --- |
| Settings | `~/.config/unisson/config.toml` | `%APPDATA%\unisson\config.toml` |
| Database | `~/.local/share/unisson/unisson.db` | `%LOCALAPPDATA%\unisson\unisson.db` |

## Try each part from the command line

### The audio engine

```sh
cargo run -p unisson-audio --example play -- ~/Music/song.mp3
```

Then type a command and press Enter:
`p` pause or resume, `s` stop, `+` or `-` volume, `i` position, `q` quit.
An `http(s)://` address works too. Add `--live` before a radio stream address:
the engine then reconnects by itself when the connection is lost.

### The radio directory

```sh
cargo run -p unisson-radio --example search -- fip
cargo run -p unisson-radio --example search -- --tag jazz --country FR
```

Each result shows a stream address, which the `play` example above can play
with `--live`.

### YouTube

```sh
cargo run -p unisson-youtube --example play -- "miles davis so what"
cargo run -p unisson-youtube --example play -- https://www.youtube.com/watch?v=VIDEO
```

Words are searched and the first result is played. A link is played directly.
Then type `p` to pause or resume, `i` for the position, `q` to quit.

### The music folder scan

```sh
cargo run -p unisson-local --example scan
cargo run -p unisson-local --example scan -- ~/Music/Jazz
```

Without a folder, the user's music folder is scanned.

### The library

```sh
cargo run -p unisson-store --example library -- scan
cargo run -p unisson-store --example library -- search miles blue
cargo run -p unisson-store --example library -- list
cargo run -p unisson-store --example library -- watch
```

`scan` saves the tags in the database. `search` then answers from the
database: it ignores case and accents, and accepts incomplete words.
`watch` scans the music folder, then follows it until Ctrl+C.

### The favorite stations

```sh
cargo run -p unisson-store --example favorites -- add fip
cargo run -p unisson-store --example favorites -- add-url "My radio" https://example.org/stream
cargo run -p unisson-store --example favorites -- list
```

Run it without arguments to see every command. The favorites are saved in the
real database, so they are still there on the next run.

## Packaging

The Debian package is built by the Tauri bundler:

```sh
cargo install tauri-cli --version "^2" --locked
cd crates/unisson-app
cargo tauri build --bundles deb
```

The package is written to `target/release/bundle/deb/`. Build it on the oldest
system it must run on: a program built on a newer Ubuntu does not start on an
older one. The `Package` workflow does this on Ubuntu 22.04 and keeps the file
with the run.

The icons are PNG files made from `icons/icon.svg` by
`packaging/make-icons.sh`. The desktop entry comes from
`packaging/unisson.desktop`. `tauri.windows.conf.json` holds what differs for
the Windows installer.

[`releasing.md`](releasing.md) explains how a version is published.

## Working method

1. For the language, rely on the official Rust documentation.
2. For every library, read its documentation before writing code.
3. If a problem resists, look at GitHub issues and forums.
4. One task at a time.
