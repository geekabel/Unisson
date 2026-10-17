/**
 * Entry point of the interface: wires the parts together.
 */

import * as api from "./api.js";
import { skippedMessage } from "./format.js";
import { t } from "./i18n.js";
import { showNotice } from "./notice.js";
import { renderPlayer, renderPosition } from "./player-bar.js";
import { drawPlaylistsSidebar, loadPlaylists } from "./playlists.js";
import { refreshQueue } from "./queue.js";
import "./search-box.js";
import "./shortcuts.js";
import { currentSettings, loadSettings, useSpotify, useYoutube } from "./settings.js";
import { loadStations } from "./stations.js";
import { onViewChange, refreshView, setPlayerState, showView } from "./views.js";

/** @param {import("./api.js").Snapshot} snapshot */
function onPlayerChanged(snapshot) {
  renderPlayer(snapshot);
  setPlayerState(snapshot);
}

// The sidebar highlights the playlist on screen.
onViewChange(drawPlaylistsSidebar);

/**
 * Takes a new state of Spotify. A move between Unisson and the official
 * application is told once, as it happens: the card and the player bar
 * keep showing where Spotify plays.
 *
 * @param {import("./api.js").Spotify} spotify
 */
function onSpotifyChanged(spotify) {
  const before = currentSettings()?.spotify?.status;
  useSpotify(spotify);
  const after = spotify.status;
  if (before?.route === "builtin" && after.route === "app" && after.problem) {
    showNotice({
      owner: "spotify",
      text: after.problem === "accountRefused" ? t("shell.spotifySwitchedRefused") : t("shell.spotifySwitched"),
      action: { label: t("shell.details"), run: () => showView("sources") },
      duration: 12000,
    });
  } else if (before?.route === "app" && before.problem && after.route === "builtin") {
    showNotice({ owner: "spotify", text: t("shell.spotifyBack"), duration: 6000 });
  }
}

async function start() {
  // Listen first, ask second: no change can slip in between.
  await Promise.all([
    api.onPlayerChanged(onPlayerChanged),
    api.onPlayerPosition(renderPosition),
    api.onPlayerSkipped(({ media, error }) => {
      showNotice({
        owner: "skipped",
        text: skippedMessage(media, error),
        detail: error.detail,
        duration: 8000,
      });
    }),
    api.onLibraryChanged(() => {
      refreshView("home");
      // The settings screen shows how many tracks the library holds.
      loadSettings();
    }),
    api.onHistoryChanged(() => refreshView("home", "history")),
    api.onFavoritesChanged(() => refreshView("favorites")),
    api.onPlaylistsChanged(async () => {
      await loadPlaylists();
      refreshView("playlist");
    }),
    api.onQueueChanged(refreshQueue),
    api.onSettingsChanged(async () => {
      await loadSettings();
      refreshQueue();
    }),
    api.onSpotifyChanged(onSpotifyChanged),
    api.onYoutubeChanged(useYoutube),
    api.onStationsChanged(async () => {
      await loadStations();
      refreshView("home", "favorites");
    }),
  ]);
  await Promise.all([loadStations(), loadSettings(), loadPlaylists()]);
  onPlayerChanged(await api.playerState());
  refreshQueue();
  await showView("home");
}

start().catch((error) => {
  console.error(error);
  showNotice({
    owner: "startup",
    tone: "error",
    text: t("shell.startFailed"),
  });
});
