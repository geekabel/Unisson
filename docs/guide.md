# Unisson in five minutes

*[Lire ce guide en français](guide.fr.md)*

Unisson plays four things from one window: **radio stations**, **your music
files**, **Spotify** and **YouTube**. This page shows the few things to know.

## 1. Install

Download the installer from the
[releases page](https://github.com/geekabel/Unisson/releases).

On Ubuntu, open a terminal in your downloads folder and type:

```sh
sudo apt install ./Unisson_0.1.0_amd64.deb
```

Then open the application menu and start **Unisson**.

Radio and your own files work at once. YouTube needs one more small program:
the [README](../README.md#youtube) says how to add it. Spotify needs a Premium
account: open **Sources** and choose **Sign in with Spotify**. The
[README](../README.md#spotify) tells more, and how to set up a backup.

## 2. Play something

Click in the field at the top, or press **Ctrl+K**. Type a few letters: the
name of a song, an artist or a station. Press **Enter**. That is all.

- **Enter** plays now.
- **Shift+Enter** adds to the queue, to play later.
- **Ctrl+Enter** plays right after the current track.

You can also paste a link there: a YouTube or Spotify address, or the address
of a radio stream.

## 3. Your music files

Unisson shows the music of your **Music** folder on the home screen. Copy a
file there and it appears a few seconds later.

Is your music somewhere else? Open **Sources**, then choose **Change folder**.

## 4. Radio

Open **Radios**. Search a station by name, or pick a genre and a country.
Click a station to listen.

Click the **heart** at the bottom of the window to keep a station. Your
stations then wait for you on the home screen.

## 5. The queue

The panel on the right lists what plays next. You can move a track up or down,
remove it, or click it to play it now.

When the queue is empty, your last station starts again by itself. The switch
at the bottom of the panel turns this off.

## 6. Keep what you like

- The **heart** keeps a track or a station in **Favorites**.
- **History** lists what you played.
- **New playlist**, in the menu on the left, creates a playlist. The button
  with a small plus, on each line, adds a track to it.

## 7. Listen while you work

- **Close the window**: the music goes on. The Unisson icon stays in the top
  bar of the screen. Click it to pause, to change station, or to quit.
- **The keys of your keyboard**: play, next and previous work from any
  application.
- **The mini-player**: the button at the bottom right shrinks Unisson to a
  small bar.
- **The quick launcher**: a small search window that opens over your work. In
  **Settings**, choose a shortcut and click **Install**. Then, from
  anywhere: the shortcut, a few letters, Enter.

## 8. Make it yours

In **Settings**:

- **Theme**: dark, light, or like your system.
- **Language**: English, French, or like your system.
- Turn the notifications off.

In **Sources**, turn a source off if you do not use it, or change their order.

Is the text too small? Press **Ctrl** and **+**. **Ctrl+0** comes back to the
normal size.

## Useful keys

| Keys | Action |
| --- | --- |
| Space | Play or pause |
| Ctrl+K | Search |
| Ctrl+Right, Ctrl+Left | Next, previous |
| Ctrl+Up, Ctrl+Down | Louder, quieter |
| Alt+1 to Alt+6 | Home, Radios, Favorites, History, Sources, Settings |
| F1 | All the keyboard shortcuts |

## If something does not work

- **A station does not start.** The station may be closed or busy. Click
  **Try again**, or pick another one.
- **"The network does not answer."** Check your connection, then click
  **Try again**.
- **YouTube gives no result.** Open **Sources** and look at the YouTube card.
  If its component is missing or old, the [README](../README.md#youtube) says
  what to do.
- **A Spotify track does not play.** Open **Sources** and look at the Spotify
  card: it says why. Spotify needs a Premium account. When Spotify refuses to
  play inside Unisson, the backup through the Spotify application solves it.
- **A song of my folder is missing.** Open **Sources** and click
  **Scan again**.
- **The launcher shortcut does nothing.** Another program may use the same
  keys. Choose another shortcut in **Settings**.

Still stuck? [Tell us](https://github.com/geekabel/Unisson/issues). Leave the
pointer on the message that appeared: a small box shows a technical detail
that helps a lot.
