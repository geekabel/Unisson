/**
 * Bridge to the Rust side of the application.
 *
 * Every call to the backend goes through this module, so the rest of the
 * interface never touches the Tauri API directly. Tauri exposes it as
 * `window.__TAURI__` because `app.withGlobalTauri` is set in
 * `tauri.conf.json`.
 *
 * @typedef {object} Media
 * @property {"radio"|"local"|"spotify"|"youtube"} source
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
 * @property {boolean} keepInTray Whether closing the window keeps the application running.
 * @property {boolean} notifications Whether a desktop notification tells each new title.
 * @property {"system"|"dark"|"light"} theme The colors of the interface.
 * @property {"system"|"fr"|"en"} language The language of the interface.
 * @property {?string} configPath
 * @property {?string} ytDlpVersion The yt-dlp program YouTube needs, if installed.
 * @property {{supported: boolean, binding: ?string, choices: [string, string][], command: string}} shortcut
 *   The keyboard shortcut of the quick launcher.
 * @property {Spotify} spotify The connection to Spotify.
 *
 * @typedef {object} Spotify
 * @property {boolean} signedIn Whether playback inside Unisson is signed in.
 * @property {{ready: boolean, clientId: ?string, autoSwitch: boolean, redirectUri: string}} backup
 *   The backup app the user registered, which drives the official application.
 * @property {{route: ?("builtin"|"app"), problem: ?("limited"|"accountRefused"|"premiumRequired"|"offline"), retryAtMs: ?number}} status
 *   Where Spotify plays now, and why not inside Unisson.
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
 * Plays a YouTube link, a stream address or the full path of a file.
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

/** Shows the main window, and puts the mini-player away. */
export const windowShowMain = () => invoke("window_show_main");

/**
 * Shows the main window with the keyboard in its search box.
 *
 * @param {?string} [text] A text to put in the box and to search.
 */
export const windowShowSearch = (text = null) => invoke("window_show_search", { text });

/**
 * @param {(text: ?string) => void} handler Called when the search box must
 *   take the keyboard, with a text to search or nothing.
 */
export const onFocusSearch = (handler) => on("ui:focus-search", handler);

/** Opens the quick launcher above the other windows. */
export const windowShowLauncher = () => invoke("window_show_launcher");

/** Puts the quick launcher away. */
export const windowHideLauncher = () => invoke("window_hide_launcher");

/** @param {() => void} handler Called each time the quick launcher opens. */
export const onLauncherOpened = (handler) => on("launcher:opened", handler);

/**
 * Registers the keyboard shortcut of the quick launcher in the desktop.
 *
 * @param {string} binding The key combination, as GNOME writes it.
 * @returns {Promise<Settings>}
 */
export const settingsInstallShortcut = (binding) => invoke("settings_install_shortcut", { binding });

/** @returns {Promise<Settings>} */
export const settingsRemoveShortcut = () => invoke("settings_remove_shortcut");

/** Replaces the main window with the mini-player. */
export const windowShowMini = () => invoke("window_show_mini");

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

/**
 * Chooses whether closing the window keeps the application running.
 *
 * @param {boolean} enabled
 * @returns {Promise<Settings>}
 */
export const settingsSetKeepInTray = (enabled) => invoke("settings_set_keep_in_tray", { enabled });

/**
 * Chooses the language of the interface. Every window then reloads in
 * that language.
 *
 * @param {"system"|"fr"|"en"} language
 * @returns {Promise<Settings>}
 */
export const settingsSetLanguage = (language) => invoke("settings_set_language", { language });

/**
 * Returns the colors the user chose for the interface.
 *
 * @returns {Promise<"system"|"dark"|"light">}
 */
export const settingsTheme = () => invoke("settings_theme");

/**
 * Chooses the colors of the interface.
 *
 * @param {"system"|"dark"|"light"} theme
 * @returns {Promise<Settings>}
 */
export const settingsSetTheme = (theme) => invoke("settings_set_theme", { theme });

/**
 * Chooses whether a desktop notification tells each new title.
 *
 * @param {boolean} enabled
 * @returns {Promise<Settings>}
 */
export const settingsSetNotifications = (enabled) => invoke("settings_set_notifications", { enabled });

/**
 * Looks again for the yt-dlp program, after the user installed it.
 *
 * @returns {Promise<Settings>}
 */
export const settingsCheckYtDlp = () => invoke("settings_check_yt_dlp");

/**
 * Signs in to Spotify, to play inside Unisson. The consent page opens in
 * the browser, and the promise ends once the user accepted, at most five
 * minutes later.
 *
 * @returns {Promise<Settings>}
 */
export const spotifySignIn = () => invoke("spotify_sign_in");

/** Stops a Spotify sign-in that waits for the browser. */
export const spotifyCancelSignIn = () => invoke("spotify_cancel_sign_in");

/** @returns {Promise<Settings>} */
export const spotifySignOut = () => invoke("spotify_sign_out");

/**
 * Sets up the backup app, which drives the official Spotify application.
 * Like the sign-in, it waits for the consent in the browser.
 *
 * @param {string} clientId The client ID of the app the user registered.
 * @returns {Promise<Settings>}
 */
export const spotifySetUpBackup = (clientId) => invoke("spotify_set_up_backup", { clientId });

/** @returns {Promise<Settings>} */
export const spotifyRemoveBackup = () => invoke("spotify_remove_backup");

/**
 * @param {boolean} enabled Whether playback moves to the backup on its own.
 * @returns {Promise<Settings>}
 */
export const spotifySetAutoSwitch = (enabled) => invoke("spotify_set_auto_switch", { enabled });

/**
 * Tries playback inside Unisson again, from the next track.
 *
 * @returns {Promise<Settings>}
 */
export const spotifyRetry = () => invoke("spotify_retry");

/** Opens the Spotify developer site in the browser. */
export const spotifyOpenDashboard = () => invoke("spotify_open_dashboard");

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

/**
 * @param {(skipped: {media: Media, error: {kind: string, detail: string}}) => void} handler
 *   Called when a media could not start and the next one was chosen.
 */
export const onPlayerSkipped = (handler) => on("player:skipped", handler);

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

/** @param {(spotify: Spotify) => void} handler */
export const onSpotifyChanged = (handler) => on("spotify:changed", handler);

/** @param {(address: string) => void} handler Gets the address of the Spotify sign-in page. */
export const onSpotifySignInLink = (handler) => on("spotify:sign-in-link", handler);
