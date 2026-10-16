/**
 * The settings, as the interface knows them.
 *
 * The backend owns them. This module keeps a copy, and shows in the
 * sidebar how many sources are switched on.
 */

import * as api from "./api.js";
import { byId } from "./dom.js";
import { plural } from "./i18n.js";

const sourcesCount = byId("sources-count");

/** @type {?import("./api.js").Settings} */
let settings = null;
const listeners = new Set();

/** Fetches the settings again from the backend, and tells the listeners. */
export async function loadSettings() {
  useSettings(await api.settingsGet());
}

/**
 * Takes settings just returned by the backend as the current ones.
 *
 * @param {import("./api.js").Settings} fresh
 */
export function useSettings(fresh) {
  settings = fresh;
  const active = fresh.sources.filter((source) => source.enabled).length;
  sourcesCount.textContent = plural(active, "shell.activeSources");
  for (const listener of listeners) {
    listener();
  }
}

/**
 * Takes a new state of the Spotify connection, sent by the backend when
 * where Spotify plays changes.
 *
 * @param {import("./api.js").Spotify} spotify
 */
export function useSpotify(spotify) {
  if (settings) {
    useSettings({ ...settings, spotify });
  }
}

/** Returns the settings, or `null` before they were first loaded. */
export function currentSettings() {
  return settings;
}

/**
 * Calls `listener` every time the settings change.
 *
 * @param {() => void} listener
 */
export function onSettingsChange(listener) {
  listeners.add(listener);
}
