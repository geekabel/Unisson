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
 * @property {boolean} currentIsFavorite
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
 * @typedef {object} Queue
 * @property {?Media} current
 * @property {Media[]} upcoming The first media waiting, the next one first.
 * @property {number} upcomingTotal May exceed the length of `upcoming`.
 * @property {?Media} lastRadio The radio listened to most recently.
 * @property {boolean} resumeRadio Whether it takes over when the queue runs out.
 *
 * @typedef {object} Playlist
 * @property {number} id
 * @property {string} name
 * @property {number} len Number of media it holds.
 *
 * @typedef {object} Settings
 * @property {{id: string, enabled: boolean}[]} sources The preferred one first.
 * @property {?string} musicFolder
 * @property {boolean} musicFolderIsDefault
 * @property {boolean} musicFolderExists
 * @property {number} libraryTotal
 * @property {boolean} resumeRadio
 * @property {?string} configPath
 *
 * @typedef {object} SearchUpdate
 * @property {{media: Media, sources: string[]}[]} hits The best match first.
 * @property {string[]} pending Sources still searching.
 * @property {{source: string, kind: string}[]} errors Sources that failed.
 * @property {boolean} done Whether every source has answered.
 *
 * @typedef {object} Position
 * @property {number} positionMs
 * @property {?number} durationMs
 */

const { invoke, Channel } = window.__TAURI__.core;
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
 * Plays a media right away, and keeps the rest of the queue.
 *
 * @param {Media} media
 */
export const playNow = (media) => invoke("player_play_now", { media });

/**
 * Adds a media right after the current one.
 *
 * @param {Media} media
 */
export const playNext = (media) => invoke("player_play_next", { media });

/**
 * Adds a media at the end of the queue.
 *
 * @param {Media} media
 */
export const enqueue = (media) => invoke("player_enqueue", { media });

/**
 * Searches every active source.
 *
 * @param {string} text
 * @param {(update: SearchUpdate) => void} onUpdate Called once at the
 *   start, then each time a source answers, with the whole list so far.
 * @returns {Promise<void>} Resolves when every source has answered.
 */
export function search(text, onUpdate) {
  const channel = new Channel();
  channel.onmessage = onUpdate;
  return invoke("search", { text, onUpdate: channel });
}

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

/** @returns {Promise<Queue>} */
export const queueState = () => invoke("queue_state");

/**
 * Jumps to an upcoming media and plays it.
 *
 * @param {number} index Counted from the next media, starting at 0.
 */
export const queueJump = (index) => invoke("queue_jump", { index });

/** @param {number} index Counted from the next media, starting at 0. */
export const queueRemove = (index) => invoke("queue_remove", { index });

/**
 * Moves an upcoming media to another position.
 *
 * @param {number} from
 * @param {number} to
 */
export const queueMove = (from, to) => invoke("queue_move", { from, to });

/** Removes every upcoming media. */
export const queueClear = () => invoke("queue_clear");

/**
 * Chooses whether the last radio takes over when the queue runs out.
 *
 * @param {boolean} enabled
 */
export const queueSetResumeRadio = (enabled) => invoke("queue_set_resume_radio", { enabled });

/** @returns {Promise<Media[]>} The favorite media. Stations are apart. */
export const favoritesList = () => invoke("favorites_list");

/**
 * Adds a media to the favorites, or removes it. A radio station joins the
 * favorite stations.
 *
 * @param {Media} media
 * @param {boolean} favorite
 */
export const favoriteSet = (media, favorite) => invoke("favorite_set", { media, favorite });

/** @returns {Promise<Media[]>} The media played most recently, the latest first. */
export const historyList = () => invoke("history_list");

/** Forgets the whole listening history. */
export const historyClear = () => invoke("history_clear");

/** @returns {Promise<Playlist[]>} */
export const playlistsList = () => invoke("playlists_list");

/**
 * @param {string} name
 * @returns {Promise<Playlist>}
 */
export const playlistCreate = (name) => invoke("playlist_create", { name });

/**
 * @param {number} id
 * @param {string} name
 */
export const playlistRename = (id, name) => invoke("playlist_rename", { id, name });

/** @param {number} id */
export const playlistDelete = (id) => invoke("playlist_delete", { id });

/**
 * @param {number} id
 * @returns {Promise<Media[]>}
 */
export const playlistItems = (id) => invoke("playlist_items", { id });

/**
 * Adds a media at the end of a playlist.
 *
 * @param {number} id
 * @param {Media} media
 */
export const playlistAdd = (id, media) => invoke("playlist_add", { id, media });

/**
 * Replaces the content of a playlist, to remove or reorder its media.
 *
 * @param {number} id
 * @param {Media[]} items
 */
export const playlistSetItems = (id, items) => invoke("playlist_set_items", { id, items });

/** @returns {Promise<Settings>} */
export const settingsGet = () => invoke("settings_get");

/**
 * Switches a source on or off.
 *
 * @param {string} sourceId
 * @param {boolean} enabled
 * @returns {Promise<Settings>}
 */
export const settingsSetSourceEnabled = (sourceId, enabled) =>
  invoke("settings_set_source_enabled", { sourceId, enabled });

/**
 * Sets the preferred order of the sources.
 *
 * @param {string[]} priority Source identifiers, the preferred one first.
 * @returns {Promise<Settings>}
 */
export const settingsSetPriority = (priority) => invoke("settings_set_priority", { priority });

/**
 * Opens the system dialog to choose the music folder.
 *
 * @returns {Promise<Settings>} Unchanged if the dialog was closed.
 */
export const settingsPickMusicFolder = () => invoke("settings_pick_music_folder");

/** @returns {Promise<Settings>} */
export const settingsResetMusicFolder = () => invoke("settings_reset_music_folder");

/** Reads the music folder again, from scratch. */
export const settingsRescanLibrary = () => invoke("settings_rescan_library");

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
export const onFavoritesChanged = (handler) => on("favorites:changed", handler);

/** @param {() => void} handler */
export const onPlaylistsChanged = (handler) => on("playlists:changed", handler);

/** @param {() => void} handler */
export const onSettingsChanged = (handler) => on("settings:changed", handler);

/** @param {() => void} handler */
export const onQueueChanged = (handler) => on("queue:changed", handler);

/** @param {() => void} handler */
export const onStationsChanged = (handler) => on("stations:changed", handler);

/** @param {() => void} handler */
export const onHistoryChanged = (handler) => on("history:changed", handler);
