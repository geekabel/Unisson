/**
 * Pure helpers that turn data into text. They touch neither the page nor
 * the backend, so they can be tested with `node --test`.
 */

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

const SOURCE_LABELS = { radio: "Radio", local: "Local", youtube: "YouTube" };
const SOURCE_NAMES = { radio: "Radio", local: "Fichiers locaux", youtube: "YouTube" };

/**
 * Returns the full name of a source, for the settings.
 *
 * @param {string} source
 */
export function sourceName(source) {
  return SOURCE_NAMES[source] ?? source;
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
  return SOURCE_LABELS[source] ?? source;
}

/**
 * Returns the line shown under the title of a media.
 *
 * @param {import("./api.js").Media} media
 * @param {?string} streamTitle What a station announces as being on air.
 */
export function mediaSubtitle(media, streamTitle = null) {
  if (media.kind === "station") {
    return streamTitle || "En direct";
  }
  return [media.artist, media.album].filter(Boolean).join(", ") || "Artiste inconnu";
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

// Missing from old web engines: country names are then left out.
const COUNTRY_NAMES = (() => {
  try {
    return new Intl.DisplayNames(["fr"], { type: "region" });
  } catch {
    return null;
  }
})();

/**
 * Returns the French name of a country from its two-letter code.
 *
 * @param {?string} code
 * @returns {string} An empty string when the code is missing or unknown.
 */
export function countryName(code) {
  if (!/^[A-Za-z]{2}$/.test(code ?? "")) {
    return "";
  }
  try {
    const name = COUNTRY_NAMES?.of(code.toUpperCase());
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
  const bitrate = station.bitrate > 0 ? `${station.bitrate} kb/s` : "";
  return [station.codec, bitrate].filter(Boolean).join(" ");
}

/**
 * Puts the first letter of a text in upper case.
 *
 * @param {string} text
 */
export function capitalize(text) {
  return text.charAt(0).toLocaleUpperCase("fr") + text.slice(1);
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
      return { badge: "Radio", subtitle: "En direct" };
    case "album":
      return { badge: "Album", subtitle: ["Album", tile.artist].filter(Boolean).join(", ") };
    default:
      return { badge: sourceLabel(tile.source), subtitle: tile.artist || "Titre" };
  }
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
    return `Tapez au moins ${minLength} lettres.`;
  }
  if (!update) {
    return "Recherche en cours…";
  }
  const sources = new Set(update.hits.map((hit) => hit.media.source)).size;
  const waiting = update.done ? "" : " Recherche en cours…";
  if (shown === 0) {
    return update.done ? `Aucun résultat pour « ${text} ».` : "Recherche en cours…";
  }
  return `${countLabel(shown, "résultat")}, ${countLabel(sources, "source")}.${waiting}`;
}

/**
 * Writes a count with its noun: `1 titre`, `12 titres`.
 *
 * @param {number} count
 * @param {string} singular
 * @param {string} [plural]
 */
export function countLabel(count, singular, plural = `${singular}s`) {
  return `${count} ${count > 1 ? plural : singular}`;
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

const PLAYBACK_ERRORS = {
  notFound: "Ce titre est introuvable. Il a peut-être été déplacé ou supprimé.",
  network: "Le réseau ne répond pas. Vérifiez votre connexion.",
  unavailable: "Cette source est indisponible pour le moment.",
  playback: "Lecture impossible. Le flux est peut-être arrêté, ou son format n'est pas pris en charge.",
};

/**
 * Returns the sentence shown when a playback fails.
 *
 * @param {{kind: string}} error
 */
export function playbackErrorMessage(error) {
  return PLAYBACK_ERRORS[error.kind] ?? PLAYBACK_ERRORS.playback;
}

const OPEN_ERRORS = {
  invalid: "Ce lien ou ce chemin n'est pas valide.",
  notFound: "Fichier introuvable. Vérifiez le chemin.",
  unreadable: "Ce fichier n'est pas un fichier audio lisible.",
};

/**
 * Returns the sentence shown when a pasted link or path cannot be opened.
 *
 * @param {{kind?: string}} error
 */
export function openErrorMessage(error) {
  return OPEN_ERRORS[error?.kind] ?? "Impossible d'ouvrir ce lien.";
}
