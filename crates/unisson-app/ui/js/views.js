/**
 * The central area of the window: one view per entry of the sidebar.
 *
 * Each view is a module of the `views` folder with a `render` function
 * that returns the elements to show. This module switches between them.
 */

import { byId, element, replaceKeepingFocus } from "./dom.js";
import { mediaKey } from "./format.js";
import * as favorites from "./views/favorites.js";
import * as history from "./views/history.js";
import * as home from "./views/home.js";
import * as playlist from "./views/playlist.js";
import * as radios from "./views/radios.js";
import * as search from "./views/search.js";
import * as sources from "./views/sources.js";

const VIEWS = { home, radios, search, sources, favorites, history, playlist };

const container = byId("view");
const navItems = [...document.querySelectorAll(".nav-item[data-view]")];

let currentView = "home";
/** What the view shows, for the views that show one thing among several:
 *  the identifier of a playlist. */
let currentParam = null;
/** The options the current view was opened with. */
let currentOptions = {};
/** Key of the media selected in the player, to highlight it in lists. */
let currentKey = "";
/** The last playback state received, for the views that follow it. */
let lastSnapshot = null;
/** Counts the renders, so that a slow answer never overwrites a newer view. */
let renderCount = 0;
let refreshTimer = null;
const viewListeners = new Set();

/**
 * Calls `listener` with the name of the view, and what it shows, each time
 * the view changes.
 *
 * @param {(name: string, param: unknown) => void} listener
 */
export function onViewChange(listener) {
  viewListeners.add(listener);
}

/**
 * Shows a view.
 *
 * @param {string} name The name of a view, such as the `data-view` values
 *   of the sidebar.
 * @param {{keepScroll?: boolean, param?: unknown}} [options] `param` says
 *   what the view shows. Other options are handed to the view.
 */
export async function showView(name, options = {}) {
  const { keepScroll = false, param = null } = options;
  currentOptions = options;
  if (name !== currentView || param !== currentParam) {
    currentView = name;
    currentParam = param;
    for (const listener of viewListeners) {
      listener(name, param);
    }
  }
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
    content = await VIEWS[name].render({ showView, param, options });
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
  replaceKeepingFocus(container, ...content);
  container.scrollTop = keepScroll ? scroll : 0;
  highlightCurrent();
  if (lastSnapshot) {
    VIEWS[name].onPlayerChanged?.(lastSnapshot);
  }
}

/** Returns the name of the view on screen. */
export function activeView() {
  return currentView;
}

/** Returns what the view on screen shows, if it shows one thing among several. */
export function activeParam() {
  return currentParam;
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
  refreshTimer = setTimeout(() => {
    showView(currentView, { ...currentOptions, renaming: false, keepScroll: true });
  }, 150);
}

/**
 * Tells the views what the player is doing.
 *
 * @param {import("./api.js").Snapshot} snapshot
 */
export function setPlayerState(snapshot) {
  lastSnapshot = snapshot;
  const key = mediaKey(snapshot.current);
  if (key !== currentKey) {
    currentKey = key;
    highlightCurrent();
  }
  VIEWS[currentView].onPlayerChanged?.(snapshot);
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
