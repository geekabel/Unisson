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
- `crates/unisson-youtube`: the YouTube source. It runs the
  [yt-dlp](https://github.com/yt-dlp/yt-dlp) program to search YouTube and to
  find the address of the audio stream of a video.
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

- For YouTube, the [yt-dlp](https://github.com/yt-dlp/yt-dlp) program, in a
  recent version. The one packaged by Ubuntu 22.04 is too old. Install it with
  pipx, with the extras yt-dlp needs for YouTube:

  ```sh
  sudo apt install pipx
  pipx install "yt-dlp[default]"
  ```

  yt-dlp also needs a JavaScript runtime for YouTube. It looks for
  [Deno](https://deno.com) by default: see the yt-dlp documentation. Update
  yt-dlp when YouTube stops working: `pipx upgrade yt-dlp`.

  Unisson works without yt-dlp: only YouTube is then left out. Listening to
  YouTube this way is not covered by its terms of service.

## Install the package

Unisson is packaged for Debian and Ubuntu as a `.deb` file. The *Package*
workflow builds it on Ubuntu 22.04: open the
[Actions tab](https://github.com/geekabel/Unisson/actions/workflows/package.yml),
choose the last run, and download the `unisson-deb` artifact. Unzip it, then:

```sh
sudo apt install ./Unisson_0.1.0_amd64.deb
```

`apt` installs the GStreamer plugins and the other libraries with it. Unisson
then appears in the application menu. To remove it: `sudo apt remove unisson`.
Settings and library stay in `~/.config/unisson` and `~/.local/share/unisson`.

To build the package yourself, install the Tauri command line tool once, then
build from the folder of the application:

```sh
cargo install tauri-cli --version "^2" --locked
cd crates/unisson-app
cargo tauri build --bundles deb
```

The package is written to `target/release/bundle/deb/`. Build it on the oldest
system it must run on: a program built on a newer Ubuntu does not start on an
older one.

The icons of the package are PNG files made from `icons/icon.svg` by
`packaging/make-icons.sh`. The desktop entry comes from
`packaging/unisson.desktop`.

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
- **YouTube**: its videos appear in the search like any other track, and
  play in the same queue. A pasted YouTube link plays the video, or the whole
  playlist.
- **Favorites, History, playlists**: the heart of the player bar keeps a
  track or a station. Every line has a button to add it to a playlist. The
  playlists are listed in the sidebar.
- **Sources**: switch a source on or off, order the sources, choose the music
  folder. The settings are kept in `~/.config/unisson/config.toml`.
- **Queue**: the panel on the right lists what plays next. Items can be moved,
  removed or jumped to. When the queue runs out, the last radio station takes
  over, unless the switch at the bottom of the panel is off.
- **Player bar**: play, pause, previous, next, seek and volume. Space pauses
  and resumes. On a radio station, previous and next walk the favorites.

### Desktop integration

- **Media keys**: the Play, Previous and Next keys of the keyboard work from
  any application, and GNOME shows the title with its buttons. This uses
  [MPRIS](https://specifications.freedesktop.org/mpris-spec/latest/), the
  standard D-Bus interface of media players.
- **Tray icon**: an icon in the top bar opens a menu with what plays, the
  transport buttons and the favorite stations. Closing the window keeps the
  music going. Quit from that menu, or switch this off in *Sources*. On GNOME
  the icon needs the AppIndicator extension, which Ubuntu enables by default.
- **Mini-player**: a button of the player bar shrinks the window to a small
  one that stays on top.
- **Quick launcher**: a small search window that opens over the current work.
  Type a few letters, press Enter, and playback starts. Open it with:

  ```sh
  unisson --lanceur
  ```

  When Unisson is already open, this command only talks to the running copy.
  *Sources* can bind it to a GNOME keyboard shortcut, such as Ctrl+Alt+M.
  The shortcut is a GNOME custom shortcut, so it also works under Wayland,
  and it appears in the GNOME settings under *Keyboard*.
- **Notifications**: a short message tells each new title, and each title a
  radio station announces, when Unisson is not the window in front. A switch
  in *Sources* turns them off. A playback that stops on a failure is told the
  same way.

### Keyboard, colors and text size

Every control can be reached with Tab, and shows a ring when it has the
keyboard. The shortcuts of the main window:

| Keys | Action |
| --- | --- |
| Space | Play or pause |
| Ctrl+Right, Ctrl+Left | Next track, previous track |
| Ctrl+Up, Ctrl+Down | Volume up, volume down |
| Ctrl+K, or / | Go to the search field |
| Alt+1 to Alt+5 | Home, Radios, Favorites, History, Sources |
| F6, Shift+F6 | Jump to the next or previous zone of the window |
| Ctrl++, Ctrl+-, Ctrl+0 | Larger display, smaller display, normal size |

The same list is at the bottom of the *Sources* screen.

The interface has a dark and a light theme. By default it follows the desktop.
*Sources* has a choice to keep one of them. The colors of both themes are
checked against the contrast rules of WCAG 2.1 (level AA) by
`ui-tests/contrast.test.mjs`.

No Node.js is needed to build or run. It is only used to test the pure
JavaScript helpers of the interface:

```sh
node --test crates/unisson-app/ui-tests/*.test.mjs
```

## Manual tests

[`docs/manual-tests.md`](docs/manual-tests.md) lists what to check by hand on
a real desktop, once under Wayland and once under X11: sound, the desktop
integration, the keyboard, the behavior when the network drops.

## Try YouTube from the command line

```sh
cargo run -p unisson-youtube --example play -- "miles davis so what"
cargo run -p unisson-youtube --example play -- https://www.youtube.com/watch?v=VIDEO
```

Words are searched and the first result is played. A link is played directly.
Then type `p` to pause or resume, `i` for the position, `q` to quit.

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
