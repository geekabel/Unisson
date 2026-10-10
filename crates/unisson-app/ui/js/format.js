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
