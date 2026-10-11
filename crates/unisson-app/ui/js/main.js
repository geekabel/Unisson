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
import { loadSettings } from "./settings.js";
import { loadStations } from "./stations.js";
import { onViewChange, refreshView, setPlayerState, showView } from "./views.js";

/** @param {import("./api.js").Snapshot} snapshot */
function onPlayerChanged(snapshot) {
  renderPlayer(snapshot);
  setPlayerState(snapshot);
}

// The sidebar highlights the playlist on screen.
onViewChange(drawPlaylistsSidebar);

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
