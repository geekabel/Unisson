/**
 * The player bar at the bottom of the window.
 *
 * It shows the playback state sent by the backend, and turns clicks into
 * orders. It keeps no state of its own about playback: after an order, it
 * waits for the backend to say what happened.
 */

import * as api from "./api.js";
import { byId } from "./dom.js";
import {
  fillPercent,
  formatTime,
  mediaSubtitle,
  playbackProblem,
  sourceLabel,
} from "./format.js";
import { t } from "./i18n.js";
import { hideNotice, showNotice } from "./notice.js";
import { showView } from "./views.js";

const cover = byId("player-cover");
const coverIcon = byId("player-cover-icon");
const coverImage = byId("player-cover-image");
const title = byId("player-title");
const subtitle = byId("player-subtitle");
const source = byId("player-source");
const previousButton = byId("player-previous");
const toggleButton = byId("player-toggle");
const toggleIcon = byId("player-toggle-icon");
const nextButton = byId("player-next");
const progress = byId("player-progress");
const positionLabel = byId("player-position");
const durationLabel = byId("player-duration");
const seekSlider = /** @type {HTMLInputElement} */ (byId("player-seek"));
const live = byId("player-live");
const favoriteButton = /** @type {HTMLButtonElement} */ (byId("player-favorite"));
const muteButton = byId("player-mute");
const muteIcon = byId("player-mute-icon");
const volumeSlider = /** @type {HTMLInputElement} */ (byId("player-volume"));

const EMPTY_TITLE = t("shell.nothingPlaying");
const EMPTY_SUBTITLE = t("shell.nothingPlayingHint");

/** Whether the user is dragging the position slider. */
let seeking = false;
/** Whether the user is dragging the volume slider. */
let settingVolume = false;
/** The volume to come back to after "mute". */
let volumeBeforeMute = 1;
/** The media and the favorite state the heart was last drawn for. */
let favoriteOf = null;
let isFavorite = false;
/** The error already shown, so that closing its message is final. */
let shownError = null;

/**
 * Shows a playback state.
 *
 * @param {import("./api.js").Snapshot} snapshot
 */
export function renderPlayer(snapshot) {
  const media = snapshot.current;
  const isLive = media?.kind === "station";

  title.textContent = media ? media.title : EMPTY_TITLE;
  subtitle.textContent = media ? mediaSubtitle(media, snapshot.streamTitle) : EMPTY_SUBTITLE;
  title.title = media ? media.title : "";
  subtitle.title = media ? subtitle.textContent : "";
  source.hidden = !media;
  source.textContent = media ? sourceLabel(media.source) : "";
  renderCover(media);

  const busy = snapshot.status === "loading";
  const sounding = snapshot.status === "playing" || snapshot.status === "reconnecting";
  toggleButton.disabled = !media;
  toggleButton.setAttribute("aria-busy", String(busy));
  toggleButton.setAttribute(
    "aria-label",
    busy ? t("shell.loading") : sounding ? t("shell.pause") : t("shell.play"),
  );
  toggleIcon.setAttribute("href", sounding ? "#icon-pause" : "#icon-play");

  // "Previous" also restarts the current track, so it works without history.
  previousButton.disabled = !media || (isLive && !snapshot.hasPrevious);
  nextButton.disabled = !snapshot.hasNext;

  progress.hidden = isLive;
  live.hidden = !isLive;
  const started = snapshot.status === "playing" || snapshot.status === "paused";
  seekSlider.disabled = !media || isLive || !started;
  renderPosition(snapshot);

  favoriteOf = media;
  isFavorite = snapshot.currentIsFavorite;
  favoriteButton.disabled = !media;
  favoriteButton.setAttribute("aria-pressed", String(Boolean(media) && isFavorite));
  favoriteButton.title = isFavorite && media ? t("shell.removeFromFavorites") : t("shell.addToFavorites");
  favoriteButton.setAttribute("aria-label", favoriteButton.title);

  if (!settingVolume) {
    renderVolume(snapshot.volume);
  }
  renderNotice(snapshot);
}

/**
 * Shows a playback position.
 *
 * @param {import("./api.js").Position} position
 */
export function renderPosition({ positionMs, durationMs }) {
  durationLabel.textContent = formatTime(durationMs) || "0:00";
  seekSlider.max = String(durationMs ?? 0);
  // While the user drags the slider, the slider shows where they point.
  if (seeking) {
    return;
  }
  seekSlider.value = String(positionMs);
  showSeekValue(positionMs);
}

function showSeekValue(positionMs) {
  positionLabel.textContent = formatTime(positionMs) || "0:00";
  seekSlider.style.setProperty("--fill", fillPercent(positionMs, Number(seekSlider.max)));
  seekSlider.setAttribute(
    "aria-valuetext",
    t("shell.seekValue", { position: positionLabel.textContent, duration: durationLabel.textContent }),
  );
}

function renderCover(media) {
  cover.dataset.source = media?.source ?? "none";
  coverIcon.setAttribute("href", media?.kind === "station" ? "#icon-radio" : "#icon-note");
  // Only pictures from the web for now: covers of local files come later.
  const picture = /^https?:\/\//.test(media?.artwork ?? "") ? media.artwork : null;
  if (!picture) {
    coverImage.hidden = true;
    coverImage.removeAttribute("src");
  } else if (coverImage.getAttribute("src") !== picture) {
    coverImage.hidden = true;
    coverImage.src = picture;
  }
}

function renderVolume(volume) {
  const percent = Math.round(volume * 100);
  volumeSlider.value = String(percent);
  volumeSlider.style.setProperty("--fill", `${percent}%`);
  volumeSlider.setAttribute("aria-valuetext", t("shell.volumeValue", { percent }));
  const muted = percent === 0;
  muteButton.setAttribute("aria-pressed", String(muted));
  muteButton.setAttribute("aria-label", muted ? t("shell.unmute") : t("shell.mute"));
  muteIcon.setAttribute("href", muted ? "#icon-muted" : "#icon-volume");
  if (!muted) {
    volumeBeforeMute = volume;
  }
}

function renderNotice(snapshot) {
  if (snapshot.status === "reconnecting" && snapshot.reconnect) {
    const { attempt, maxAttempts } = snapshot.reconnect;
    showNotice({
      owner: "player",
      text: t("shell.reconnecting", { attempt, maxAttempts }),
    });
    return;
  }
  const error = snapshot.error;
  if (!error) {
    shownError = null;
    hideNotice("player");
    return;
  }
  const identity = `${error.kind}:${error.detail}`;
  if (identity === shownError) {
    return;
  }
  shownError = identity;
  const problem = playbackProblem(error, snapshot.current);
  showNotice({
    owner: "player",
    tone: "error",
    text: problem.text,
    // The technical reason, for a bug report.
    detail: error.detail,
    action:
      problem.action === "sources"
        ? { label: t("shell.showSources"), run: () => showView("sources") }
        : { label: t("common.tryAgain"), run: () => api.toggle() },
  });
}

// A picture that cannot be loaded leaves the colored square in place.
coverImage.addEventListener("load", () => {
  coverImage.hidden = false;
});
coverImage.addEventListener("error", () => {
  coverImage.hidden = true;
});

favoriteButton.addEventListener("click", async () => {
  if (!favoriteOf) {
    return;
  }
  try {
    await api.favoriteSet(favoriteOf, !isFavorite);
  } catch (error) {
    showNotice({
      owner: "favorites",
      tone: "error",
      text:
        error?.kind === "notFound"
          ? t("shell.stationNotSaved")
          : t("shell.favoritesNotSaved"),
      duration: 8000,
    });
  }
});

byId("player-mini").addEventListener("click", () => api.windowShowMini());
toggleButton.addEventListener("click", () => api.toggle());
previousButton.addEventListener("click", () => api.previous());
nextButton.addEventListener("click", () => api.next());

seekSlider.addEventListener("input", () => {
  seeking = true;
  showSeekValue(Number(seekSlider.value));
});
seekSlider.addEventListener("change", () => {
  seeking = false;
  api.seek(Number(seekSlider.value));
});

volumeSlider.addEventListener("input", () => {
  settingVolume = true;
  const volume = Number(volumeSlider.value) / 100;
  renderVolume(volume);
  api.setVolume(volume);
});
volumeSlider.addEventListener("change", () => {
  settingVolume = false;
});

/**
 * Raises or lowers the volume, for the keyboard shortcuts.
 *
 * @param {number} step In percent, negative to lower.
 */
export function nudgeVolume(step) {
  const percent = Math.min(100, Math.max(0, Number(volumeSlider.value) + step));
  renderVolume(percent / 100);
  api.setVolume(percent / 100);
}

muteButton.addEventListener("click", () => {
  const muted = Number(volumeSlider.value) === 0;
  api.setVolume(muted ? volumeBeforeMute || 1 : 0);
});
