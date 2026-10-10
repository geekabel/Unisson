/**
 * Entry point of the interface: wires the parts together.
 */

import * as api from "./api.js";
import { showNotice } from "./notice.js";
import { renderPlayer, renderPosition } from "./player-bar.js";
import "./search-box.js";
import { loadStations } from "./stations.js";
import { refreshView, setPlayerState, showView } from "./views.js";

/** @param {import("./api.js").Snapshot} snapshot */
function onPlayerChanged(snapshot) {
  renderPlayer(snapshot);
  setPlayerState(snapshot);
}

document.addEventListener("keydown", (event) => {
  const typing = event.target instanceof HTMLInputElement && event.target.type !== "range";
  const onControl = event.target instanceof HTMLButtonElement || event.target instanceof HTMLSelectElement;
  if (event.key === " " && !typing && !onControl) {
    // Space plays or pauses, unless it is typed or activates a control.
    event.preventDefault();
    api.toggle();
  }
});

async function start() {
  // Listen first, ask second: no change can slip in between.
  await Promise.all([
    api.onPlayerChanged(onPlayerChanged),
    api.onPlayerPosition(renderPosition),
    api.onLibraryChanged(() => refreshView("home")),
    api.onHistoryChanged(() => refreshView("home")),
    api.onStationsChanged(async () => {
      await loadStations();
      refreshView("home");
    }),
  ]);
  await loadStations();
  onPlayerChanged(await api.playerState());
  await showView("home");
}

start().catch((error) => {
  console.error(error);
  showNotice({
    owner: "startup",
    tone: "error",
    text: "L'interface n'a pas pu démarrer. Relancez Unisson.",
  });
});
