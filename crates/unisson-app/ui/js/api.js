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
 * @typedef {object} Station
 * @property {string} uuid
 * @property {string} name
 * @property {string} streamUrl
 * @property {?string} homepage
 * @property {?string} favicon
 * @property {string[]} tags
 * @property {?string} countryCode
 * @property {?string} codec
 * @property {?number} bitrate
 * @property {boolean} hls
 *
 * @typedef {object} Tile
 * @property {"station"|"album"|"track"} kind
 * @property {string} title
 * @property {?string} artist
 * @property {string} source
 * @property {Media[]} items What a click plays.
 * @property {number} start Index in `items` where playback starts.
 *
 * @typedef {object} Home
 * @property {Tile[]} resume
 * @property {Station[]} stations
 * @property {Media[]} recent
 * @property {Media[]} library
 * @property {number} libraryTotal
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

/** @returns {Promise<Home>} */
export const home = () => invoke("home");

/** @returns {Promise<Station[]>} The favorite stations, in the user's order. */
export const stationsFavorites = () => invoke("stations_favorites");

/**
 * Searches the directory of stations. Every criterion is optional.
 *
 * @param {{name?: string, tag?: ?string, countryCode?: string}} criteria
 * @returns {Promise<Station[]>}
 */
export const stationsSearch = ({ name, tag, countryCode }) =>
  invoke("stations_search", { name: name || null, tag: tag || null, countryCode: countryCode || null });

/**
 * Replaces the queue with `stations` and plays `stations[start]`.
 *
 * @param {Station[]} stations
 * @param {number} start
 */
export const stationsPlay = (stations, start) => invoke("stations_play", { stations, start });

/**
 * Adds a station to the favorites, or removes it.
 *
 * @param {Station} station
 * @param {boolean} favorite
 */
export const stationSetFavorite = (station, favorite) =>
  invoke("station_set_favorite", { station, favorite });

/**
 * Adds to the favorites a station known only by the address of its stream.
 *
 * @param {string} name
 * @param {string} streamUrl
 * @returns {Promise<Station>}
 */
export const stationAddCustom = (name, streamUrl) =>
  invoke("station_add_custom", { name, streamUrl });

/** @param {(snapshot: Snapshot) => void} handler */
export const onPlayerChanged = (handler) => on("player:changed", handler);

/** @param {(position: Position) => void} handler */
export const onPlayerPosition = (handler) => on("player:position", handler);

/** @param {() => void} handler */
export const onLibraryChanged = (handler) => on("library:changed", handler);

/** @param {() => void} handler */
export const onStationsChanged = (handler) => on("stations:changed", handler);

/** @param {() => void} handler */
export const onHistoryChanged = (handler) => on("history:changed", handler);
