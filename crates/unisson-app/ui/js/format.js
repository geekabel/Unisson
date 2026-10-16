/**
 * Pure helpers that turn data into text. They touch neither the page nor
 * the backend, so they can be tested with `node --test`.
 */

import { getLanguage, plural, t } from "./i18n.js";

/**
 * Formats a duration for display: `3:41`, or `1:02:03` from one hour.
 *
 * @param {?number} milliseconds
 * @returns {string} An empty string when the duration is unknown.
 */
export function formatTime(milliseconds) {
  if (milliseconds == null || !Number.isFinite(milliseconds) || milliseconds < 0) {
    return "";
  }
  const total = Math.floor(milliseconds / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = String(total % 60).padStart(2, "0");
  return hours > 0
    ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}`
    : `${minutes}:${seconds}`;
}

// Tables of keys, not of texts: the language can change while the module lives.
const SOURCE_LABELS = {
  radio: "format.sourceRadio",
  local: "format.sourceLocal",
  youtube: "format.sourceYoutube",
  spotify: "format.sourceSpotify",
};
const SOURCE_NAMES = {
  radio: "format.sourceRadio",
  local: "format.sourceLocalFiles",
  youtube: "format.sourceYoutube",
  spotify: "format.sourceSpotify",
};

/**
 * Returns the full name of a source, for the settings.
 *
 * @param {string} source
 */
export function sourceName(source) {
  const key = SOURCE_NAMES[source];
  return key ? t(key) : source;
}

/**
 * Shortens a path for display: the home folder becomes `~`.
 *
 * @param {?string} path
 * @param {?string} [home] The home folder, when it is known.
 */
export function displayPath(path, home = null) {
  if (!path) {
    return "";
  }
  if (home && (path === home || path.startsWith(`${home}/`))) {
    return `~${path.slice(home.length)}`;
  }
  // Without the home folder, recognize the usual place of homes on Linux.
  return path.replace(/^\/home\/[^/]+(?=\/|$)/, "~");
}

/**
 * Moves an item of a list one step up or down, and returns the new list.
 * The list is returned unchanged when the item cannot move.
 *
 * @template T
 * @param {T[]} list
 * @param {number} index
 * @param {-1|1} step
 * @returns {T[]}
 */
export function moved(list, index, step) {
  const target = index + step;
  if (index < 0 || index >= list.length || target < 0 || target >= list.length) {
    return list;
  }
  const copy = [...list];
  [copy[index], copy[target]] = [copy[target], copy[index]];
  return copy;
}

/**
 * Returns the display name of a source.
 *
 * @param {string} source
 */
export function sourceLabel(source) {
  const key = SOURCE_LABELS[source];
  return key ? t(key) : source;
}

/**
 * Returns the line shown under the title of a media.
 *
 * @param {import("./api.js").Media} media
 * @param {?string} streamTitle What a station announces as being on air.
 */
export function mediaSubtitle(media, streamTitle = null) {
  if (media.kind === "station") {
    return streamTitle || t("format.live");
  }
  return [media.artist, media.album].filter(Boolean).join(", ") || t("format.unknownArtist");
}

/**
 * Turns a station into the media the player understands.
 *
 * @param {import("./api.js").Station} station
 * @returns {import("./api.js").Media}
 */
export function stationMedia(station) {
  return {
    source: "radio",
    key: station.uuid,
    kind: "station",
    title: station.name,
    artist: null,
    album: null,
    durationMs: null,
    artwork: station.favicon ?? null,
  };
}

/** @type {Map<string, ?Intl.DisplayNames>} The names of the countries, per language. */
const COUNTRY_NAMES = new Map();

/** @returns {?Intl.DisplayNames} The names of the countries in the language in use. */
function countryNames() {
  const language = getLanguage();
  if (!COUNTRY_NAMES.has(language)) {
    // Missing from old web engines: country names are then left out.
    try {
      COUNTRY_NAMES.set(language, new Intl.DisplayNames([language], { type: "region" }));
    } catch {
      COUNTRY_NAMES.set(language, null);
    }
  }
  return COUNTRY_NAMES.get(language);
}

/**
 * Returns the name of a country, in the language in use, from its
 * two-letter code.
 *
 * @param {?string} code
 * @returns {string} An empty string when the code is missing or unknown.
 */
export function countryName(code) {
  if (!/^[A-Za-z]{2}$/.test(code ?? "")) {
    return "";
  }
  try {
    const name = countryNames()?.of(code.toUpperCase());
    return name && name !== code.toUpperCase() ? name : "";
  } catch {
    return "";
  }
}

/**
 * Returns the short description of a station: its first tags and its
 * country, such as `Jazz, France`.
 *
 * @param {import("./api.js").Station} station
 * @param {number} [tagCount] How many tags to show.
 */
export function stationDescription(station, tagCount = 1) {
  const tags = station.tags
    .filter((tag) => tag.trim())
    .slice(0, tagCount)
    .map((tag) => capitalize(tag.trim()));
  return [...tags, countryName(station.countryCode)].filter(Boolean).join(", ");
}

/**
 * Returns the audio quality of a station, such as `MP3 128 kb/s`.
 *
 * @param {import("./api.js").Station} station
 * @returns {string} An empty string when nothing is known.
 */
export function stationQuality(station) {
  const bitrate = station.bitrate > 0 ? t("format.bitrate", { bitrate: station.bitrate }) : "";
  return [station.codec, bitrate].filter(Boolean).join(" ");
}

/**
 * Puts the first letter of a text in upper case.
 *
 * @param {string} text
 */
export function capitalize(text) {
  return text.charAt(0).toLocaleUpperCase(getLanguage()) + text.slice(1);
}

/**
 * Returns the two lines of a "resume" tile.
 *
 * @param {import("./api.js").Tile} tile
 * @returns {{badge: string, subtitle: string}}
 */
export function tileText(tile) {
  switch (tile.kind) {
    case "station":
      return { badge: t("format.sourceRadio"), subtitle: t("format.live") };
    case "album":
      return {
        badge: t("format.album"),
        subtitle: [t("format.album"), tile.artist].filter(Boolean).join(", "),
      };
    default:
      return { badge: sourceLabel(tile.source), subtitle: tile.artist || t("format.track") };
  }
}

/**
 * Simplifies a text for comparison: lower case, no accents.
 *
 * @param {string} text
 */
function simplified(text) {
  return text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

/**
 * Tells whether `text` contains every word of `wanted`, ignoring case and
 * accents.
 *
 * @param {string} text
 * @param {string} wanted
 */
export function matchesText(text, wanted) {
  const haystack = simplified(text);
  const words = simplified(wanted).split(/\s+/).filter(Boolean);
  return words.length > 0 && words.every((word) => haystack.includes(word));
}

/**
 * Writes the line shown above the search results.
 *
 * @param {string} text What the user typed.
 * @param {?import("./api.js").SearchUpdate} update The last update, if any.
 * @param {number} shown Number of results on screen, after the filters.
 * @param {number} minLength Shortest text that is searched.
 */
export function searchSummary(text, update, shown, minLength) {
  if (text.length < minLength) {
    return t("format.searchTooShort", { min: minLength });
  }
  if (!update) {
    return t("format.searching");
  }
  const sources = new Set(update.hits.map((hit) => hit.media.source)).size;
  if (shown === 0) {
    return update.done ? t("format.noResult", { text }) : t("format.searching");
  }
  const results = plural(shown, "format.results");
  // Playlists are results too, and they come from no source.
  const summary = sources > 0
    ? t("format.resultsFromSources", { results, sources: plural(sources, "format.sources") })
    : t("format.resultsAlone", { results });
  return update.done ? summary : t("format.resultsSoFar", { summary });
}

/**
 * Groups the best search hits for the quick launcher: tracks and stations
 * apart, the group of the best hit first.
 *
 * @param {{media: import("./api.js").Media}[]} hits The best match first.
 * @param {number} limit Most hits kept.
 * @returns {{group: string, media: import("./api.js").Media}[]}
 */
export function launcherGroups(hits, limit) {
  const kept = hits.slice(0, limit);
  const group = (media) => (media.kind === "station" ? t("format.groupStations") : t("format.groupTracks"));
  const first = kept.length > 0 ? group(kept[0].media) : null;
  const entries = kept.map((hit) => ({ group: group(hit.media), media: hit.media }));
  // A stable sort: inside a group, the ranking of the search is kept.
  return entries.sort((a, b) => (b.group === first) - (a.group === first));
}

/**
 * Tells whether a text is something to open rather than words to search:
 * a network address, or the full path of a file.
 *
 * @param {string} text
 */
export function looksLikeTarget(text) {
  return /^(https?:\/\/|\/|~\/)\S/.test(text.trim());
}

/**
 * Returns a key that identifies a media across every source.
 *
 * @param {?import("./api.js").Media} media
 */
export function mediaKey(media) {
  return media ? `${media.source}\u0000${media.key}` : "";
}

/**
 * Converts a value inside a range to a percentage, for a slider fill.
 *
 * @param {number} value
 * @param {number} max
 * @returns {string} From `0%` to `100%`.
 */
export function fillPercent(value, max) {
  if (!(max > 0)) {
    return "0%";
  }
  return `${Math.min(100, Math.max(0, (value / max) * 100)).toFixed(2)}%`;
}

/**
 * Tells which colors to use, from the choice of the user and from the
 * theme of the desktop.
 *
 * @param {string} choice `system`, `dark` or `light`. Anything else counts
 *   as `system`.
 * @param {boolean} desktopIsLight Whether the desktop uses a light theme.
 * @returns {"dark"|"light"}
 */
export function resolveTheme(choice, desktopIsLight) {
  if (choice === "dark" || choice === "light") {
    return choice;
  }
  return desktopIsLight ? "light" : "dark";
}

/**
 * What to tell the user when a playback fails: a sentence, and the way out
 * that fits best.
 *
 * The sentence depends on the kind of failure and on what was playing: a
 * file that is gone and a station that closed are both "not found", but
 * they do not call for the same words.
 *
 * @param {{kind: string}} error
 * @param {?{source?: string, kind?: string}} [media] What was playing.
 * @returns {{text: string, action: "retry"|"sources"}} `retry` offers to
 *   try again, `sources` leads to the screen where a source is repaired.
 */
export function playbackProblem(error, media) {
  const station = media?.kind === "station";
  const youtube = media?.source === "youtube";
  if (media?.source === "spotify") {
    // Spotify plays in its own application: most failures mean it is not
    // open, or the account cannot be driven.
    switch (error?.kind) {
      case "network":
        return { text: t("format.networkDown"), action: "retry" };
      case "unavailable":
        return { text: t("format.spotifyUnavailable"), action: "sources" };
      case "notFound":
        return { text: t("format.trackNotFound"), action: "retry" };
      default:
        return { text: t("format.spotifyPlaybackFailed"), action: "retry" };
    }
  }
  switch (error?.kind) {
    case "notFound":
      if (station) {
        return { text: t("format.stationGone"), action: "retry" };
      }
      if (youtube) {
        return { text: t("format.videoNotFound"), action: "retry" };
      }
      if (media?.source === "local") {
        return { text: t("format.fileNotFound"), action: "retry" };
      }
      return { text: t("format.trackNotFound"), action: "retry" };
    case "network":
      return {
        text: station ? t("format.stationConnectionLost") : t("format.networkDown"),
        action: "retry",
      };
    case "unavailable":
      if (youtube) {
        return { text: t("format.youtubeUnavailable"), action: "sources" };
      }
      return { text: t("format.sourceUnavailable"), action: "retry" };
    case "unsupported":
      return {
        text: station ? t("format.streamUnsupported") : t("format.audioUnsupported"),
        action: "retry",
      };
    default:
      return {
        text: station ? t("format.stationPlaybackFailed") : t("format.playbackFailed"),
        action: "retry",
      };
  }
}

/**
 * Returns the sentence shown when a playback fails.
 *
 * @param {{kind: string}} error
 * @param {?{source?: string, kind?: string}} [media] What was playing.
 */
export function playbackErrorMessage(error, media) {
  return playbackProblem(error, media).text;
}

const SKIP_REASONS = {
  notFound: "format.skipNotFound",
  unavailable: "format.skipUnavailable",
  unsupported: "format.skipUnsupported",
};

/**
 * Returns the sentence shown when a media could not start and the next one
 * was chosen instead.
 *
 * @param {{title: string}} media
 * @param {{kind: string}} error
 */
export function skippedMessage(media, error) {
  const reason = t(SKIP_REASONS[error?.kind] ?? "format.skipFailed");
  return t("format.skipped", { title: media.title, reason });
}

const OPEN_ERRORS = {
  invalid: "format.openInvalid",
  notFound: "format.openNotFound",
  unreadable: "format.openUnreadable",
  network: "format.networkDown",
  unavailable: "format.openUnavailable",
};

/**
 * Returns the sentence shown when a pasted link or path cannot be opened.
 *
 * @param {{kind?: string}} error
 */
export function openErrorMessage(error) {
  return t(OPEN_ERRORS[error?.kind] ?? "format.openFailed");
}
