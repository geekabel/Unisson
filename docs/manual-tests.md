# Manual test plan

This list is run by hand on a real desktop, once under Wayland and once under
X11. The automated tests cover the logic. This list covers what only a real
session can show: sound, the desktop of GNOME, the keyboard, the network.

Target: Ubuntu 22.04 with GNOME.

The texts quoted below are those of the French interface. In English the
labels differ, the behavior does not.

## How to use it

1. Find the session in use:

   ```sh
   echo $XDG_SESSION_TYPE
   ```

   It prints `wayland` or `x11`. To change, log out, select your name, then
   click the gear at the bottom right of the login screen. Choose *Ubuntu* for
   Wayland, or *Ubuntu on Xorg* for X11.

2. Run every step in the first session. Put `ok` or `KO` in its column. For a
   `KO`, add a note at the end of this file with what happened.

3. Log out, change session, and run the list again for the other column.

Steps that need the network are marked **net**. Steps that need yt-dlp are
marked **yt**. Steps that need a Spotify Premium account, with an app
registered as the README says, are marked **sp**.

## 1. Installation and start

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 1.1 | Install the package: `sudo apt install ./Unisson_0.1.0_amd64.deb` | No error. The GStreamer packages are installed with it. | | |
| 1.2 | Open the application menu and type "unisson" | Unisson appears with its amber icon. Typing "musique" or "radio" finds it too. | | |
| 1.3 | Start it from the menu | The window opens. The dock shows the amber icon, not a generic one. | | |
| 1.4 | Start it a second time from the menu | No second window. The first one comes to the front. | | |
| 1.5 | Right-click the icon in the dock | The menu offers "Lanceur rapide". It opens the launcher. | | |

## 2. Local files

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 2.1 | Look at the home screen | The tracks of `~/Music` are listed. | | |
| 2.2 | Click a track | Sound starts. The player bar shows the title. The progress bar moves. | | |
| 2.3 | Press Space, twice | Pause, then play again from the same place. | | |
| 2.4 | Click the middle of the progress bar | Playback jumps there. | | |
| 2.5 | Move the volume slider, then click the speaker | The volume follows. The speaker mutes and unmutes. | | |
| 2.6 | Copy a new audio file into `~/Music` | It appears in the library within a few seconds. | | |
| 2.7 | Let a track play to its end | The next one starts by itself. | | |

## 3. Radio

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 3.1 | **net** Open *Radios*, search "fip" | Stations are listed. | | |
| 3.2 | **net** Click a station | Sound starts within a few seconds. "En direct" is shown. | | |
| 3.3 | **net** Wait one minute | The title on air appears, if the station announces it. | | |
| 3.4 | **net** Click the heart of the player bar | The station is kept. It appears on the home screen. | | |
| 3.5 | **net** With several favorite stations, press Next | The next favorite station plays. | | |
| 3.6 | **net** Paste a stream address in the search field, press Enter | The stream plays. | | |

## 4. YouTube

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 4.1 | Open *Sources* | The YouTube card says that its component is installed, with a version, or that it is missing. | | |
| 4.2 | **net yt** Search a song by name | Results from YouTube appear with the others, a moment later. | | |
| 4.3 | **net yt** Press Enter on a YouTube result | Sound starts. The duration is right. | | |
| 4.4 | **net yt** Paste a YouTube link in the search field, press Enter | The video plays. | | |
| 4.5 | **net yt** Paste the link of a YouTube playlist | Its videos fill the queue. | | |

## 5. Search and queue

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 5.1 | Press Ctrl+K, type two letters | Results appear while typing. Local results come first. | | |
| 5.2 | Use Down and Up, then Enter | The chosen result plays. | | |
| 5.3 | Press Shift+Enter on a result | It is added to the queue. A short message says so. | | |
| 5.4 | Press Ctrl+Enter on a result | It is placed right after the current track. | | |
| 5.5 | In the queue panel, move an item up and remove another | The queue follows. | | |
| 5.6 | Click an item of the queue | Playback jumps to it. | | |
| 5.7 | **net** Listen to a station, then play one local track to its end | The station takes over again. The switch of the queue panel turns this off. | | |

## 6. Favorites, history, playlists

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 6.1 | Click the heart while a track plays, open *Favoris* | The track is listed. | | |
| 6.2 | Open *Historique* | What was played is listed, the last one first. | | |
| 6.3 | Create a playlist, add two tracks with the "add to a list" button | The playlist shows them. | | |
| 6.4 | Rename the playlist, reorder it, then play it | All three work. | | |
| 6.5 | Quit Unisson and start it again | Favorites, history and playlists are still there. | | |

## 7. Desktop integration

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 7.1 | Play a track, click in another application, press the Play/Pause key | Playback pauses, then resumes. | | |
| 7.2 | Press the Next and Previous keys | The track changes. | | |
| 7.3 | Open the clock menu of GNOME | The title is shown with its buttons. They work. | | |
| 7.4 | Close the window with its cross | The music goes on. The icon stays in the top bar. | | |
| 7.5 | Click the icon in the top bar | A menu opens: what plays, transport, favorite stations, open, quit. | | |
| 7.6 | Choose "Ouvrir Unisson" in that menu | The window comes back. | | |
| 7.7 | Click the mini-player button of the player bar | A small window replaces the large one. Its buttons work. It can be dragged. | | |
| 7.8 | Check that the mini-player stays above other windows | Under X11, yes. Under Wayland, GNOME decides: click the mini-player, press Alt+Space, then choose "Always on Top". | | |
| 7.9 | In *Sources*, install the launcher shortcut. Click in another application, press the shortcut | The launcher opens and the cursor blinks in its field. | | |
| 7.10 | Type a few letters, press Enter | Playback starts. The launcher closes. The other application has the keyboard again. | | |
| 7.11 | Repeat 7.9 and 7.10 ten times, with a few seconds between each | It works every time. | | |
| 7.12 | Open the launcher, click outside it | It closes. | | |
| 7.13 | Change the shortcut for another one in *Sources* | The new keys open the launcher. The old keys do nothing. | | |
| 7.14 | With another application in front, let the track change | A notification shows the new title, without sound. | | |
| 7.15 | With Unisson in front, let the track change | No notification. | | |
| 7.16 | Turn notifications off in *Sources*, repeat 7.14 | No notification. | | |

## 8. When things go wrong

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 8.1 | **net** Play a station, then turn the Wi-Fi off | "Connexion perdue" with the attempt count, then an error message with "Réessayer". Unisson does not close. | | |
| 8.2 | **net** Turn the Wi-Fi on again, click "Réessayer" | The station plays again. | | |
| 8.3 | With the Wi-Fi off, search something | Local results appear. A quiet note says which sources did not answer. | | |
| 8.4 | Queue three local tracks. Delete the file of the second one while the first plays | The second is skipped with a short message. The third plays. | | |
| 8.5 | Paste `http://example.invalid/stream` in the search field, press Enter | A clear message. Nothing plays. Unisson does not close. | | |
| 8.6 | Rename the yt-dlp program for a moment, then search | YouTube is noted as unavailable, with "Vérifier YouTube". The other sources still answer. | | |
| 8.7 | Close the window, then make a station fail (Wi-Fi off) | A notification says that playback stopped. | | |

## 9. Keyboard, colors, text size

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 9.1 | Put the mouse aside. Press Tab many times | Every control is reached, in a logical order. A ring always shows where the keyboard is. | | |
| 9.2 | Press F6 several times | The keyboard jumps from zone to zone: menu, search, content, queue, player. | | |
| 9.3 | Press Alt+2, Alt+5, Alt+1 | Radios, Sources, Home. | | |
| 9.4 | Press Ctrl+Right, Ctrl+Left, Ctrl+Up, Ctrl+Down | Next, previous, volume up, volume down. | | |
| 9.5 | Play a track with Enter on its line, then press Tab | The keyboard is still on that line, not back at the top. | | |
| 9.6 | In *Sources*, choose the light theme, then the dark one | The whole window changes at once. Every text stays easy to read. | | |
| 9.7 | Choose "Comme le système". In the settings of Ubuntu, switch between the light and dark styles | Unisson follows. | | |
| 9.8 | Press Ctrl++ three times, then Ctrl+0 | The display grows, nothing is cut off, then it comes back. | | |

## 10. Language

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 10.1 | In *Sources*, choose English as the language | The window reloads in English. Playback is not interrupted. | | |
| 10.2 | Open the tray menu, the mini-player and the launcher | They are in English too. | | |
| 10.3 | Choose "Same as system" | The interface follows the language of the desktop again. | | |

## 11. Removal

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 11.1 | `sudo apt remove unisson` | Unisson leaves the application menu. | | |
| 11.2 | Look at `~/.config/unisson` and `~/.local/share/unisson` | Settings and library are kept, for a later installation. | | |

## 12. Spotify

Open the Spotify application before steps 12.4 to 12.10.

| # | Step | Expected | Wayland | X11 |
| --- | --- | --- | --- | --- |
| 12.1 | Open *Sources* | The Spotify card says that no account is connected, lists the steps, and shows `http://127.0.0.1/callback`. | | |
| 12.2 | Type `abc` as the Client ID, choose *Se connecter* | A message says the Client ID is not valid. No browser opens. | | |
| 12.3 | **net sp** Paste the real Client ID, choose *Se connecter*, accept in the browser | The browser page says Unisson is connected. The card says *Compte connecté*. | | |
| 12.4 | **net sp** Search a song by name | Results from Spotify appear with the others, with their cover. | | |
| 12.5 | **net sp** Press Enter on a Spotify result | The Spotify application plays it. The player bar of Unisson shows the title, the badge Spotify, and the time moving. | | |
| 12.6 | **net sp** Pause, play, seek, change the volume in Unisson | The Spotify application follows each order within a second or two. | | |
| 12.7 | **net sp** Queue a local file after a Spotify track, and let the track end | Spotify stops, then the local file plays in Unisson. | | |
| 12.8 | **net sp** During a Spotify track, choose another song in the Spotify application | Unisson stops, and leaves the Spotify application alone. | | |
| 12.9 | **net sp** Paste a link `https://open.spotify.com/track/...`, press Enter | The track plays in the Spotify application. | | |
| 12.10 | **net sp** Quit Unisson and start it again | The card still says *Compte connecté*: no new consent. | | |
| 12.11 | **net sp** Close the Spotify application, press Enter on a Spotify result | A message asks to open the Spotify application, with a way to *Sources*. | | |
| 12.12 | Choose *Se déconnecter* | The card shows the form again, with the Client ID filled in. `~/.config/unisson/spotify.json` is gone. | | |

## Known differences between the two sessions

- **Window position.** Under Wayland an application cannot place its windows.
  The launcher and the mini-player appear where GNOME puts them.
- **Always on top.** Under Wayland only the user can ask for it (step 7.8).
- **Global shortcut.** Neither session lets Unisson grab a key by itself in a
  way that works in both. The shortcut is a GNOME custom shortcut, so it
  behaves the same in both sessions.
- **Tray icon.** It needs the AppIndicator extension, which Ubuntu enables by
  default. Without it, closing the window quits Unisson.

## Notes

Write here what failed: the step number, the session, and what happened.
