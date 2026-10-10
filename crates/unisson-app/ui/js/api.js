/**
 * Bridge to the Rust side of the application.
 *
 * Every call to the backend goes through this module, so the rest of the
 * interface never touches the Tauri API directly. Tauri exposes it as
 * `window.__TAURI__` because `app.withGlobalTauri` is set in
 * `tauri.conf.json`.
 *
 * @typedef {object} Media
 * @property {"radio"|"local"|"youtube"} source
 * @property {string} key Identifier of the media inside its source.
 * @property {"track"|"station"} kind
 * @property {string} title
 * @property {?string} artist
 * @property {?string} album
 * @property {?number} durationMs
 * @property {?string} artwork
 *
 * @typedef {object} Snapshot
 * @property {"stopped"|"loading"|"playing"|"paused"|"reconnecting"} status
 * @property {?{attempt: number, maxAttempts: number}} reconnect
 * @property {?Media} current
 * @property {?string} streamTitle
 * @property {number} positionMs
 * @property {?number} durationMs
 * @property {number} volume From 0 to 1.
 * @property {boolean} hasPrevious
 * @property {boolean} hasNext
 * @property {?{kind: string, detail: string}} error
 *
 * @typedef {object} Position
 * @property {number} positionMs
 * @property {?number} durationMs
 */

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

/**
 * Subscribes to an event of the backend.
 *
 * @template T
 * @param {string} name
 * @param {(payload: T) => void} handler
 * @returns {Promise<() => void>} Resolves to a function that unsubscribes.
 */
function on(name, handler) {
  return listen(name, (event) => handler(event.payload));
}

/** @returns {Promise<Snapshot>} */
export const playerState = () => invoke("player_state");

/**
 * Replaces the queue with `items` and plays `items[start]`.
 *
 * @param {Media[]} items
 * @param {number} start
 */
export const playList = (items, start) => invoke("player_play_list", { items, start });

/**
 * Plays a stream address or the full path of a file.
 *
 * @param {string} target
 */
export const open = (target) => invoke("player_open", { target });

export const toggle = () => invoke("player_toggle");
export const next = () => invoke("player_next");
export const previous = () => invoke("player_previous");

/** @param {number} positionMs */
export const seek = (positionMs) => invoke("player_seek", { positionMs: Math.round(positionMs) });

/** @param {number} volume From 0 to 1. */
export const setVolume = (volume) => invoke("player_set_volume", { volume });

/** @returns {Promise<Media[]>} */
export const libraryTracks = () => invoke("library_tracks");

/** @returns {Promise<Media[]>} */
export const favoriteStations = () => invoke("favorite_stations");

/** @param {(snapshot: Snapshot) => void} handler */
export const onPlayerChanged = (handler) => on("player:changed", handler);

/** @param {(position: Position) => void} handler */
export const onPlayerPosition = (handler) => on("player:position", handler);

/** @param {() => void} handler */
export const onLibraryChanged = (handler) => on("library:changed", handler);
