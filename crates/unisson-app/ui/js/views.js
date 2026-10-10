/**
 * The central area of the window: one view per entry of the sidebar.
 *
 * Each view is a module of the `views` folder with a `render` function
 * that returns the elements to show. This module switches between them.
 */

import { byId, element } from "./dom.js";
import { mediaKey } from "./format.js";
import * as home from "./views/home.js";

/** A view that is not built yet: it only says when it arrives. */
function coming(title, text) {
  return {
    render: async () => [
      element("h1", { class: "view-title" }, [title]),
      element("p", { class: "hint" }, [text]),
    ],
  };
}

const VIEWS = {
  home,
  radios: coming("Radios", "La recherche de radios par nom, genre et pays arrive avec la tâche T17."),
  favorites: coming("Favoris", "Vos titres favoris arrivent avec la tâche T21."),
  history: coming("Historique", "L'historique d'écoute arrive avec la tâche T21. Il s'enregistre déjà."),
  sources: coming("Sources", "Le réglage des sources arrive avec la tâche T20."),
};

const container = byId("view");
const navItems = [...document.querySelectorAll(".nav-item[data-view]")];

let currentView = "home";
/** Key of the media selected in the player, to highlight it in lists. */
let currentKey = "";
/** Counts the renders, so that a slow answer never overwrites a newer view. */
let renderCount = 0;
let refreshTimer = null;

/**
 * Shows a view.
 *
 * @param {string} name One of the `data-view` values of the sidebar.
 * @param {{keepScroll?: boolean}} [options]
 */
export async function showView(name, { keepScroll = false } = {}) {
  currentView = name;
  for (const item of navItems) {
    if (item.dataset.view === name) {
      item.setAttribute("aria-current", "page");
    } else {
      item.removeAttribute("aria-current");
    }
  }
  const render = ++renderCount;
  let content;
  try {
    content = await VIEWS[name].render({ showView });
  } catch (error) {
    console.error(error);
    content = [
      element("p", { class: "hint" }, ["Cette page n'a pas pu s'afficher. Réessayez dans un instant."]),
    ];
  }
  if (render !== renderCount) {
    return;
  }
  const scroll = container.scrollTop;
  container.replaceChildren(...content);
  container.scrollTop = keepScroll ? scroll : 0;
  highlightCurrent();
}

/** Returns the name of the view on screen. */
export function activeView() {
  return currentView;
}

/**
 * Draws the current view again after its data changed, if it is one of
 * `names`. Several calls in a row give a single redraw.
 *
 * @param {...string} names
 */
export function refreshView(...names) {
  if (!names.includes(currentView)) {
    return;
  }
  clearTimeout(refreshTimer);
  refreshTimer = setTimeout(() => showView(currentView, { keepScroll: true }), 150);
}

/**
 * Tells the views which media is selected in the player.
 *
 * @param {?import("./api.js").Media} media
 */
export function setCurrentMedia(media) {
  const key = mediaKey(media);
  if (key !== currentKey) {
    currentKey = key;
    highlightCurrent();
  }
}

function highlightCurrent() {
  for (const node of container.querySelectorAll("[data-media-key]")) {
    if (node.dataset.mediaKey === currentKey) {
      node.setAttribute("aria-current", "true");
    } else {
      node.removeAttribute("aria-current");
    }
  }
}

for (const item of navItems) {
  item.addEventListener("click", () => showView(item.dataset.view));
}
