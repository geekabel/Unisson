/**
 * The keyboard shortcuts that work anywhere in the main window.
 *
 * Everything in the interface can be reached with Tab. These shortcuts are
 * the fast lane: the transport, the volume, the screens, and a way to jump
 * from one zone of the window to the next without walking through every
 * button. `shortcut-list.js` holds their description for the user.
 */

import * as api from "./api.js";
import { byId } from "./dom.js";
import { nudgeVolume } from "./player-bar.js";
import { openShortcuts } from "./shortcut-dialog.js";
import { showView } from "./views.js";

/** The screens of Alt+1 to Alt+6, in the order of the sidebar. */
const VIEWS = ["home", "radios", "favorites", "history", "sources", "settings"];
/** One step of the volume shortcuts, in percent. */
const VOLUME_STEP = 5;

const searchInput = byId("search-input");
const view = byId("view");

/**
 * The zones of the window, in reading order. Each one tells where the
 * keyboard lands when F6 reaches it.
 *
 * @type {{root: () => ?Element, target: () => ?HTMLElement}[]}
 */
const ZONES = [
  {
    root: () => document.querySelector(".sidebar"),
    target: () => document.querySelector('.nav-item[aria-current="page"]') ?? document.querySelector(".nav-item"),
  },
  { root: () => byId("search-form"), target: () => searchInput },
  { root: () => view, target: () => firstControl(view) ?? view },
  { root: () => byId("queue"), target: () => firstControl(byId("queue")) },
  { root: () => document.querySelector(".player"), target: () => byId("player-toggle") },
];

/** Returns the first control of `root` that can take the keyboard. */
function firstControl(root) {
  const controls = root.querySelectorAll("button, input, select, a[href]");
  return [...controls].find((control) => !control.disabled && control.tabIndex >= 0 && isShown(control)) ?? null;
}

function isShown(node) {
  return node.getClientRects().length > 0;
}

/** Moves the keyboard to the next zone, or to the previous one. */
function moveZone(backwards) {
  const zones = ZONES.filter(({ root, target }) => {
    const node = root();
    return node && isShown(node) && target();
  });
  if (zones.length === 0) {
    return;
  }
  const current = zones.findIndex(({ root }) => root().contains(document.activeElement));
  const step = backwards ? -1 : 1;
  // From nowhere, F6 starts at the first zone and Shift+F6 at the last.
  const next = current === -1 ? (backwards ? zones.length - 1 : 0) : (current + step + zones.length) % zones.length;
  zones[next].target().focus();
}

/** Tells whether keys typed on `target` are text. */
function isTyping(target) {
  if (target instanceof HTMLTextAreaElement) {
    return true;
  }
  return target instanceof HTMLInputElement && target.type !== "range";
}

document.addEventListener("keydown", (event) => {
  const typing = isTyping(event.target);
  const command = event.ctrlKey || event.metaKey;

  if (event.key === "F6") {
    event.preventDefault();
    moveZone(event.shiftKey);
    return;
  }
  if (event.key === "F1") {
    event.preventDefault();
    openShortcuts();
    return;
  }
  if (event.altKey && !command && !event.shiftKey && /^[1-6]$/.test(event.key)) {
    event.preventDefault();
    // The keyboard follows: Tab then walks the new screen.
    showView(VIEWS[Number(event.key) - 1]).then(() => view.focus());
    return;
  }
  if (typing || event.altKey) {
    return;
  }
  if (command && !event.shiftKey) {
    const action = {
      ArrowRight: () => api.next(),
      ArrowLeft: () => api.previous(),
      ArrowUp: () => nudgeVolume(VOLUME_STEP),
      ArrowDown: () => nudgeVolume(-VOLUME_STEP),
    }[event.key];
    if (action) {
      // Also keeps a slider that has the keyboard from moving.
      event.preventDefault();
      action();
    }
    return;
  }
  const onControl = event.target instanceof HTMLButtonElement || event.target instanceof HTMLSelectElement;
  if (event.key === " " && !onControl) {
    // Space plays or pauses, unless it activates a control.
    event.preventDefault();
    api.toggle();
  } else if (event.key === "/" && !onControl) {
    event.preventDefault();
    searchInput.focus();
  }
});
