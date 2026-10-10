/**
 * The favorite radio stations, as the interface knows them.
 *
 * The backend owns the list. This module keeps a copy, so that any part of
 * the interface can tell at once whether a station is a favorite.
 */

import * as api from "./api.js";

/** @type {import("./api.js").Station[]} */
let favorites = [];
const listeners = new Set();

/** Fetches the list again from the backend, and tells the listeners. */
export async function loadStations() {
  favorites = await api.stationsFavorites();
  for (const listener of listeners) {
    listener();
  }
}

/** Returns the favorite stations, in the order chosen by the user. */
export function favoriteStations() {
  return favorites;
}

/**
 * Tells whether a station is a favorite.
 *
 * @param {string} uuid
 */
export function isFavorite(uuid) {
  return favorites.some((station) => station.uuid === uuid);
}

/**
 * Calls `listener` every time the list changes.
 *
 * @param {() => void} listener
 */
export function onStationsChange(listener) {
  listeners.add(listener);
}
