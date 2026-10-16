<div align="center">

<img src="crates/unisson-app/icons/icon.svg" width="96" height="96" alt="">

# Unisson

**One search box and one play queue for internet radio, local files and YouTube.**

A desktop audio player for Linux, written in Rust.

[![CI](https://github.com/geekabel/Unisson/actions/workflows/ci.yml/badge.svg)](https://github.com/geekabel/Unisson/actions/workflows/ci.yml)
[![Latest release](https://img.shields.io/github/v/release/geekabel/Unisson?include_prereleases&sort=semver)](https://github.com/geekabel/Unisson/releases)
[![License: MIT or Apache-2.0](https://img.shields.io/badge/license-MIT%20or%20Apache--2.0-blue)](#license)

[Install](#install) · [User guide](docs/guide.md) · [Guide en français](docs/guide.fr.md) · [Build from source](#build-from-source) · [Contributing](CONTRIBUTING.md)

![The home screen of Unisson, with its queue on the right](docs/screenshots/home-dark.png)

</div>

## Why Unisson

Music is scattered: a few radio stations in a browser tab, albums in a folder,
a mix on YouTube. Unisson puts them behind one search field and plays them in
one queue, without an account and without leaving the keyboard.

## Features

- **Four sources, one search.** Type a few letters: local files, radio
  stations, Spotify and YouTube answer in the same list. Paste a stream
  address, a YouTube or Spotify link, or the path of a file to play it
  directly.
- **Internet radio.** Browse the free [Radio Browser](https://www.radio-browser.info)
  directory by name, genre and country. Keep your stations, see the title on
  air, and let Unisson reconnect when the network drops.
- **Local library.** Your music folder is scanned and watched: a file you
  copy there appears a few seconds later. Search ignores case and accents.
- **One queue.** Play now, play next or add to the queue, from any source.
  When the queue runs out, your last station can take over.
- **Favorites, history and playlists**, kept in a local database.
- **Part of the desktop.** Media keys and the GNOME media controls (MPRIS), a
  tray icon that keeps the music going when the window is closed, a
  mini-player, notifications, and a quick launcher that opens over your work.
- **Keyboard first.** Every control is reachable with Tab, with shortcuts for
  the transport, the volume and the screens.
- **Yours to read.** Dark and light themes checked against WCAG 2.1 AA
  contrast, a display you can enlarge, and an interface in English and French.
- **Private by design.** No account, no telemetry, no update check. See
  [Privacy](#privacy).

<table>
  <tr>
    <td width="50%"><img src="docs/screenshots/search-dark.png" alt="Search results from several sources"></td>
    <td width="50%"><img src="docs/screenshots/radios-light.png" alt="The Radios screen, in the light theme"></td>
  </tr>
  <tr>
    <td align="center"><em>One search, every source</em></td>
    <td align="center"><em>Stations and the directory, light theme</em></td>
  </tr>
  <tr>
    <td><img src="docs/screenshots/launcher-dark.png" alt="The quick launcher"></td>
    <td><img src="docs/screenshots/mini-dark.png" alt="The mini-player"></td>
  </tr>
  <tr>
    <td align="center"><em>The quick launcher: a shortcut, a few letters, Enter</em></td>
    <td align="center"><em>The mini-player</em></td>
  </tr>
</table>

## Install

Installers are on the [releases page](https://github.com/geekabel/Unisson/releases).
Each release lists the SHA-256 sum of its files in `SHA256SUMS`.

### Ubuntu and Debian

Unisson targets Ubuntu 22.04 and newer, with GNOME. Download the `.deb` file,
then:

```sh
sudo apt install ./Unisson_0.1.0_amd64.deb
```

`apt` installs the GStreamer plugins and the other libraries with it. Unisson
then appears in the application menu. To remove it: `sudo apt remove unisson`.
Your settings and your library stay in `~/.config/unisson` and
`~/.local/share/unisson`.

### Windows (experimental)

The Windows installer is built from the same code, but **it has not been
tested on a real Windows machine yet**. Expect rough edges, and please
[open an issue](https://github.com/geekabel/Unisson/issues) with what you find.

1. Install the [GStreamer runtime](https://gstreamer.freedesktop.org/download/)
   (MSVC 64-bit, "Complete" setup).
2. Add its `bin` folder to your `PATH`, usually
   `C:\gstreamer\1.0\msvc_x86_64\bin`. Unisson does not start without it.
3. Run `Unisson_0.1.0_x64-setup.exe`.

Not available on Windows yet: media keys, and the helper that installs the
shortcut of the quick launcher.

### YouTube

Radio and local files work out of the box. YouTube needs one more program,
[yt-dlp](https://github.com/yt-dlp/yt-dlp), which Unisson runs to search
YouTube and to find the audio of a video. Install a recent version:

```sh
# Ubuntu and Debian. The version packaged by Ubuntu 22.04 is too old.
sudo apt install pipx
pipx install "yt-dlp[default]"

# Windows
winget install yt-dlp.yt-dlp
```

Then open *Sources* in Unisson and choose *Check again*. When YouTube stops
answering, update it: `pipx upgrade yt-dlp`. yt-dlp may also ask for a
JavaScript runtime such as [Deno](https://deno.com): see its documentation.

Without yt-dlp, Unisson works the same, minus YouTube.

### Spotify

Unisson searches the Spotify catalog and plays its tracks **in the official
Spotify application**, which it drives through the
[Spotify Web API](https://developer.spotify.com/documentation/web-api). The
sound comes from the Spotify application, not from Unisson. You need:

- a **Spotify Premium** account: Spotify only lets Premium accounts be driven;
- the **Spotify application** open on this computer, or on another device of
  the same account.

Spotify asks each user of its API to register an app. It takes two minutes,
once:

1. Open the [Spotify developer dashboard](https://developer.spotify.com/dashboard),
   log in, and create an app. Any name works. Tick **Web API**.
2. Add this **Redirect URI**, exactly: `http://127.0.0.1/callback`.
3. In **User Management**, add the name and email of each Spotify account
   that will use Unisson. An app in development mode accepts up to five, and
   its owner needs Premium too.
4. Copy the **Client ID** of the app. In Unisson, open *Sources*, paste it in
   the Spotify card, and choose *Connect*. Your browser opens on the consent
   page of Spotify: accept, then come back to Unisson.

Unisson keeps a token in `~/.config/unisson/spotify.json`, readable by you
only. *Disconnect* deletes it. To revoke the access on the side of Spotify,
remove the app from [your account](https://www.spotify.com/account/apps/).

Good to know:

- A Spotify track takes its place in the queue of Unisson, among radio, files
  and YouTube. When it ends, Unisson plays what comes next.
- Choose something else in the Spotify application, and Unisson steps back.
- The volume of Unisson moves the volume of the Spotify application, when that
  device allows it.

## Use

Start Unisson, type in the field at the top, press Enter. The
[user guide](docs/guide.md) ([en français](docs/guide.fr.md)) walks through the
rest in a few minutes.

| Keys | Action |
| --- | --- |
| Space | Play or pause |
| Ctrl+K, or / | Go to the search field |
| Enter, Shift+Enter, Ctrl+Enter | Play now, add to the queue, play next |
| Ctrl+Right, Ctrl+Left | Next track, previous track |
| Ctrl+Up, Ctrl+Down | Volume up, volume down |
| Alt+1 to Alt+5 | Home, Radios, Favorites, History, Sources |
| F6, Shift+F6 | Jump to the next or previous zone of the window |
| Ctrl++, Ctrl+-, Ctrl+0 | Larger display, smaller display, normal size |

The quick launcher opens with `unisson --launcher`. *Sources* can bind that
command to a GNOME keyboard shortcut for you.

## Privacy

Unisson has no account, no telemetry and no update check. What leaves your
computer is what playing music needs:

- **Search text** goes to a Radio Browser server, to Spotify if an account is
  connected, and to YouTube through yt-dlp if YouTube is on. Turn a source off
  in *Sources* and it is no longer asked.
- **Spotify**, once connected, receives the orders of playback (play, pause,
  seek, volume) and is asked every second what plays, while one of its tracks
  plays.
- **Playing a station** tells the Radio Browser directory that the station was
  chosen, which is how the directory ranks stations. It also connects you to
  the server of the station, like any radio player.
- **Covers** of stations and videos are pictures fetched from the address the
  source gives, so that server sees your IP address.

Favorites, history, playlists and settings stay on your computer.

## Build from source

You need Rust 1.90 or newer ([rustup](https://www.rust-lang.org/tools/install)),
GStreamer, and the [system libraries of Tauri](https://v2.tauri.app/start/prerequisites/).
On Ubuntu:

```sh
sudo apt install libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
    gstreamer1.0-plugins-bad gstreamer1.0-libav \
    libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

Then:

```sh
git clone https://github.com/geekabel/Unisson.git
cd Unisson
cargo run -p unisson-app
```

The first build takes a few minutes. The interface is plain HTML, CSS and
JavaScript modules embedded in the program: there is no Node.js step.

To build the `.deb` package yourself:

```sh
cargo install tauri-cli --version "^2" --locked
cd crates/unisson-app
cargo tauri build --bundles deb
```

[`docs/development.md`](docs/development.md) covers the tests, the command
line examples of each crate, and how the code is organized.

## How it is built

Unisson is a Cargo workspace. Each crate does one thing and can be tried on
its own:

| Crate | Role |
| --- | --- |
| `unisson-core` | The common model, the contract every source fulfils, the play queue, the merge of search results |
| `unisson-audio` | The audio engine, on [GStreamer](https://gstreamer.freedesktop.org) |
| `unisson-radio` | Internet radio, backed by the Radio Browser directory |
| `unisson-local` | Local files: scan, tags, folder watching |
| `unisson-youtube` | YouTube, through yt-dlp |
| `unisson-spotify` | Spotify, through its Web API and the official application |
| `unisson-store` | Local storage, in SQLite |
| `unisson-playback` | The playback session: queue, sources and engine together |
| `unisson-app` | The desktop application, with [Tauri 2](https://v2.tauri.app) |

## Contributing

Bug reports, ideas and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md)
explains how to set up, what the checks are, and how the branches work. To
report a security problem, see [SECURITY.md](SECURITY.md). The changes of each
version are in [CHANGELOG.md](CHANGELOG.md).

## Acknowledgements

Unisson stands on the work of others: [Radio Browser](https://www.radio-browser.info)
and the people who keep its directory, [GStreamer](https://gstreamer.freedesktop.org),
[Tauri](https://tauri.app), [yt-dlp](https://github.com/yt-dlp/yt-dlp),
[lofty](https://github.com/Serial-ATA/lofty-rs) and the Rust community.

## Legal

Unisson is an independent project. It is not affiliated with, or endorsed by,
YouTube, Google, Spotify, or any radio station or directory. It hosts and distributes
no content. You are responsible for using it in line with the terms of the
services you listen to, and with the law where you live.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in this work by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms or
conditions.
