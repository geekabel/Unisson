/**
 * The mini-player: a small bar that stays above the other windows.
 *
 * It is a window of its own, with its own page. It shows the same playback
 * state as the main window, since the backend sends its events to every
 * window, and it offers the few orders needed while working elsewhere.
 */

import * as api from "./api.js";
import { byId } from "./dom.js";
import { fillPercent, mediaSubtitle, playbackErrorMessage, sourceLabel } from "./format.js";
import { t } from "./i18n.js";

const cover = byId("mini-cover");
const coverIcon = byId("mini-cover-icon");
const coverImage = byId("mini-cover-image");
const title = byId("mini-title");
const subtitle = byId("mini-subtitle");
const source = byId("mini-source");
const previousButton = byId("mini-previous");
const toggleButton = byId("mini-toggle");
const toggleIcon = byId("mini-toggle-icon");
const nextButton = byId("mini-next");
const progress = byId("mini-progress");
const progressFill = byId("mini-progress-fill");

const EMPTY_TITLE = t("mini.nothingPlaying");
const EMPTY_SUBTITLE = t("mini.nothingPlayingHint");

/** @param {import("./api.js").Snapshot} snapshot */
function render(snapshot) {
  const media = snapshot.current;
  const isLive = media?.kind === "station";

  title.textContent = media ? media.title : EMPTY_TITLE;
  // There is no room for a message strip here: a problem takes the place
  // of the second line.
  const problem = Boolean(media && snapshot.error);
  if (problem) {
    subtitle.textContent = playbackErrorMessage(snapshot.error, media);
  } else if (media && snapshot.status === "reconnecting") {
    subtitle.textContent = t("mini.reconnecting");
  } else {
    subtitle.textContent = media ? mediaSubtitle(media, snapshot.streamTitle) : EMPTY_SUBTITLE;
  }
  subtitle.dataset.tone = problem ? "error" : "info";
  title.title = media ? media.title : "";
  subtitle.title = media ? subtitle.textContent : "";
  source.hidden = !media;
  source.textContent = media ? sourceLabel(media.source) : "";

  cover.dataset.source = media?.source ?? "none";
  coverIcon.setAttribute("href", isLive ? "#icon-radio" : "#icon-note");
  const picture = /^https?:\/\//.test(media?.artwork ?? "") ? media.artwork : null;
  if (!picture) {
    coverImage.hidden = true;
    coverImage.removeAttribute("src");
  } else if (coverImage.getAttribute("src") !== picture) {
    coverImage.hidden = true;
    coverImage.src = picture;
  }

  const busy = snapshot.status === "loading";
  const sounding = snapshot.status === "playing" || snapshot.status === "reconnecting";
  toggleButton.disabled = !media;
  toggleButton.setAttribute("aria-busy", String(busy));
  toggleButton.setAttribute(
    "aria-label",
    busy ? t("mini.loading") : sounding ? t("mini.pause") : t("mini.play"),
  );
  toggleIcon.setAttribute("href", sounding ? "#icon-pause" : "#icon-play");
  previousButton.disabled = !media || (isLive && !snapshot.hasPrevious);
  nextButton.disabled = !snapshot.hasNext;

  progress.dataset.live = String(isLive);
  renderPosition(snapshot);
}

/** @param {import("./api.js").Position} position */
function renderPosition({ positionMs, durationMs }) {
  const fill = fillPercent(positionMs, durationMs ?? 0);
  progressFill.style.setProperty("--fill", fill);
  progress.setAttribute("aria-valuenow", String(Math.round(parseFloat(fill))));
}

coverImage.addEventListener("load", () => {
  coverImage.hidden = false;
});
coverImage.addEventListener("error", () => {
  coverImage.hidden = true;
});

toggleButton.addEventListener("click", () => api.toggle());
previousButton.addEventListener("click", () => api.previous());
nextButton.addEventListener("click", () => api.next());
byId("mini-expand").addEventListener("click", () => api.windowShowMain());
byId("mini-search").addEventListener("click", () => api.windowShowLauncher());

document.addEventListener("keydown", (event) => {
  if (event.key === " " && !(event.target instanceof HTMLButtonElement)) {
    event.preventDefault();
    api.toggle();
  } else if (event.key === "Escape") {
    api.windowShowMain();
  }
});

async function start() {
  // Listen first, ask second: no change can slip in between.
  await Promise.all([api.onPlayerChanged(render), api.onPlayerPosition(renderPosition)]);
  render(await api.playerState());
}

start().catch((error) => console.error(error));
