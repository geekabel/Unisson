/**
 * The colors of the interface: dark or light.
 *
 * Every window loads this module first. It sets the `data-theme`
 * attribute the style sheet reads, from the choice of the user ("like the
 * desktop", dark or light) and from the theme of the desktop, and keeps it
 * right when either changes.
 */

import * as api from "./api.js";
import { resolveTheme } from "./format.js";

/** Where the last choice is kept, to apply it before the backend answers. */
const STORAGE_KEY = "unisson-theme";

const desktop = window.matchMedia("(prefers-color-scheme: light)");
let choice = remembered() ?? "system";

function remembered() {
  try {
    return window.localStorage.getItem(STORAGE_KEY);
  } catch {
    // Storage can be refused: the backend answers a moment later anyway.
    return null;
  }
}

function apply() {
  document.documentElement.dataset.theme = resolveTheme(choice, desktop.matches);
}

async function load() {
  choice = await api.settingsTheme();
  try {
    window.localStorage.setItem(STORAGE_KEY, choice);
  } catch {
    // Only a comfort: without it, the first paint may use the other theme.
  }
  apply();
}

// At once, with what is known: the page must not flash in the wrong colors.
apply();
desktop.addEventListener("change", apply);
api.onSettingsChanged(load);
load().catch((error) => console.error(error));
