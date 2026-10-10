/**
 * Entry point of the interface: wires the parts together.
 */

import * as api from "./api.js";
import { byId } from "./dom.js";
import { looksLikeTarget, openErrorMessage } from "./format.js";
import { hideNotice, showNotice } from "./notice.js";
import { renderPlayer, renderPosition } from "./player-bar.js";
import { loadStations } from "./stations.js";
import { refreshView, setPlayerState, showView } from "./views.js";

const searchForm = byId("search-form");
const searchInput = /** @type {HTMLInputElement} */ (byId("search-input"));

/** @param {import("./api.js").Snapshot} snapshot */
function onPlayerChanged(snapshot) {
  renderPlayer(snapshot);
  setPlayerState(snapshot);
}

searchForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  const text = searchInput.value.trim();
  if (!text) {
    return;
  }
  if (!looksLikeTarget(text)) {
    showNotice({
      owner: "search",
      text: "La recherche par mots arrive bientôt. Pour l'instant, collez un lien ou le chemin complet d'un fichier.",
      duration: 8000,
    });
    return;
  }
  try {
    await api.open(text);
    hideNotice("search");
    searchInput.value = "";
    searchInput.blur();
  } catch (error) {
    showNotice({ owner: "search", tone: "error", text: openErrorMessage(error), duration: 8000 });
  }
});

document.addEventListener("keydown", (event) => {
  const typing = event.target instanceof HTMLInputElement && event.target.type !== "range";

  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    searchInput.focus();
    searchInput.select();
  } else if (event.key === "Escape" && event.target === searchInput) {
    searchInput.blur();
  } else if (event.key === " " && !typing && !(event.target instanceof HTMLButtonElement)) {
    // Space plays or pauses, unless it is typed or activates a button.
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
