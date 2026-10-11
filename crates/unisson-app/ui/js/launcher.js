/**
 * The quick launcher: a search box that opens above the current work.
 *
 * Its goal is three keys: the shortcut that opens it, a few letters, and
 * Enter. It then closes by itself. With an empty box, it lists the
 * favorite stations and what was played last, so that Enter alone is
 * enough to resume.
 */

import * as api from "./api.js";
import { byId, element } from "./dom.js";
import { launcherGroups, mediaSubtitle, sourceLabel, stationMedia } from "./format.js";

/** Pause in the typing after which the sources are asked. */
const TYPING_PAUSE = 200;
/** Shortest text that is searched. */
const MIN_LENGTH = 2;
/** Most results shown: the launcher is for the obvious choice. */
const RESULT_LIMIT = 7;
/** Most stations and recent media shown while the box is empty. */
const SUGGESTION_LIMIT = 4;
/** Longest wait for slow sources when Enter was pressed before the results. */
const READY_PATIENCE = 1500;

const input = /** @type {HTMLInputElement} */ (byId("launcher-input"));
const results = byId("launcher-results");
const status = byId("launcher-status");
const now = byId("launcher-now");
const nowText = byId("launcher-now-text");
const toggleButton = byId("launcher-toggle");
const toggleIcon = byId("launcher-toggle-icon");
const nextButton = /** @type {HTMLButtonElement} */ (byId("launcher-next"));

/**
 * One line that can be chosen: a media, or the way to the full results.
 *
 * @typedef {{group: string, media?: import("./api.js").Media, more?: string}} Entry
 */

/** @type {Entry[]} */
let entries = [];
let selected = 0;
/** Whether every source has answered for the text in the box. */
let complete = true;
/** What to do with the first result once it arrives, if Enter came first. */
let whenReady = null;
/** Counts the searches, so that a late answer never replaces a newer one. */
let searchCount = 0;
let typingTimer = null;
let readyTimer = null;
/** Whether the box holds nothing, so that Space pauses instead of typing. */
const boxIsEmpty = () => input.value.trim() === "";

function draw() {
  selected = Math.min(selected, Math.max(0, entries.length - 1));
  const content = [];
  let group = null;
  entries.forEach((entry, index) => {
    if (entry.group !== group) {
      group = entry.group;
      content.push(element("div", { class: "group-title" }, [group]));
    }
    content.push(entryRow(entry, index));
  });
  results.replaceChildren(...content);
  results.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
}

/**
 * @param {Entry} entry
 * @param {number} index
 */
function entryRow(entry, index) {
  const media = entry.media;
  const row = element(
    "button",
    {
      class: "launcher-entry",
      type: "button",
      // The keyboard stays in the box: the lines are chosen with the arrows.
      tabindex: "-1",
      "aria-selected": String(index === selected),
    },
    media
      ? [
          element("span", { class: "station-text" }, [
            element("span", { class: "queue-title truncate" }, [media.title]),
            element("span", { class: "station-detail truncate" }, [mediaSubtitle(media)]),
          ]),
          element("span", { class: "badge" }, [sourceLabel(media.source)]),
          element("kbd", {}, ["Entrée"]),
        ]
      : [
          element("span", { class: "station-text" }, [
            element("span", { class: "queue-title truncate" }, [`Voir tous les résultats pour « ${entry.more} »`]),
            element("span", { class: "station-detail" }, ["Ouvre la fenêtre principale"]),
          ]),
          element("kbd", {}, ["Entrée"]),
        ],
  );
  row.addEventListener("click", () => act(entry, "now"));
  return row;
}

/** Lists what to resume, while the box is empty. */
async function showSuggestions() {
  const count = ++searchCount;
  complete = true;
  status.textContent = "";
  const home = await api.home();
  if (count !== searchCount) {
    return;
  }
  entries = [
    ...home.stations.slice(0, SUGGESTION_LIMIT).map((station) => {
      return { group: "Radios favorites", media: stationMedia(station) };
    }),
    ...home.recent.slice(0, SUGGESTION_LIMIT).map((media) => ({ group: "Écoutés récemment", media })),
  ];
  selected = 0;
  draw();
  if (entries.length === 0) {
    status.textContent = "Tapez quelques lettres pour chercher un titre ou une radio.";
  }
}

/** Searches every source for what is in the box. */
function searchNow() {
  clearTimeout(typingTimer);
  const text = input.value.trim();
  if (text.length < MIN_LENGTH) {
    whenReady = null;
    if (text === "") {
      showSuggestions();
    } else {
      searchCount += 1;
      entries = [];
      draw();
      status.textContent = `Tapez au moins ${MIN_LENGTH} lettres.`;
    }
    return;
  }
  const count = ++searchCount;
  complete = false;
  status.textContent = "Recherche en cours…";
  api
    .search(text, (update) => {
      if (count !== searchCount) {
        return;
      }
      complete = update.done;
      entries = [
        ...launcherGroups(update.hits, RESULT_LIMIT),
        { group: "Aller plus loin", more: text },
      ];
      selected = 0;
      draw();
      status.textContent = update.done
        ? update.hits.length === 0
          ? `Aucun résultat pour « ${text} ».`
          : ""
        : "Recherche en cours…";
      runWhenReady({ patient: true });
    })
    .catch(() => {
      if (count === searchCount) {
        complete = true;
        status.textContent = "La recherche a échoué.";
      }
    });
}

/**
 * Carries out the action that waits for results, if it is time: once every
 * source answered, or once the patience ran out and something is there.
 */
function runWhenReady({ patient }) {
  if (!whenReady || (patient && !complete)) {
    return;
  }
  const first = entries.find((entry) => entry.media);
  if (!first && !complete) {
    return;
  }
  const mode = whenReady;
  whenReady = null;
  clearTimeout(readyTimer);
  if (first) {
    act(first, mode);
  }
}

/**
 * @param {Entry} entry
 * @param {"now"|"next"|"enqueue"} mode
 */
function act(entry, mode) {
  if (!entry.media) {
    api.windowShowSearch(entry.more);
    return;
  }
  if (mode === "now") {
    api.playNow(entry.media);
    // The goal is reached: get out of the way.
    api.windowHideLauncher();
    return;
  }
  // Adding keeps the launcher open, to add several media in a row.
  if (mode === "next") {
    api.playNext(entry.media);
  } else {
    api.enqueue(entry.media);
  }
  status.textContent = `${mode === "next" ? "À lire juste après" : "Ajouté à la file"} : ${entry.media.title}`;
}

/** @param {import("./api.js").Snapshot} snapshot */
function renderNow(snapshot) {
  const media = snapshot.current;
  now.hidden = !media;
  if (!media) {
    return;
  }
  nowText.textContent = `${media.title}, ${mediaSubtitle(media, snapshot.streamTitle)}`;
  const sounding = snapshot.status === "playing" || snapshot.status === "reconnecting";
  toggleIcon.setAttribute("href", sounding ? "#icon-pause" : "#icon-play");
  toggleButton.setAttribute("aria-label", sounding ? "Mettre en pause" : "Lire");
  nextButton.disabled = !snapshot.hasNext;
}

/** Gets the launcher ready for a new use. */
function reset() {
  clearTimeout(typingTimer);
  clearTimeout(readyTimer);
  whenReady = null;
  input.value = "";
  input.focus();
  showSuggestions();
}

input.addEventListener("input", () => {
  clearTimeout(typingTimer);
  if (boxIsEmpty()) {
    searchNow();
  } else {
    typingTimer = setTimeout(searchNow, TYPING_PAUSE);
  }
});

document.addEventListener("keydown", (event) => {
  switch (event.key) {
    case "ArrowDown":
    case "ArrowUp":
      event.preventDefault();
      if (entries.length > 0) {
        const step = event.key === "ArrowDown" ? 1 : -1;
        selected = (selected + step + entries.length) % entries.length;
        draw();
      }
      break;
    case "Enter": {
      event.preventDefault();
      const mode = event.shiftKey ? "enqueue" : event.ctrlKey || event.metaKey ? "next" : "now";
      const typed = input.value.trim();
      // Enter may come before the pause that starts the search, or before
      // the answers: the action then waits for the first result.
      const pending = typed.length >= MIN_LENGTH && typingTimer !== null && !resultsMatch(typed);
      if (pending) {
        searchNow();
      }
      const entry = entries[selected];
      if (entry && !pending) {
        act(entry, mode);
      } else if (typed.length >= MIN_LENGTH) {
        whenReady = mode;
        clearTimeout(readyTimer);
        readyTimer = setTimeout(() => runWhenReady({ patient: false }), READY_PATIENCE);
      }
      break;
    }
    case "Escape":
      event.preventDefault();
      api.windowHideLauncher();
      break;
    case " ":
      if (boxIsEmpty()) {
        event.preventDefault();
        api.toggle();
      }
      break;
    default:
  }
});

/** Tells whether the results on screen are those of `text`. */
function resultsMatch(text) {
  return entries.some((entry) => entry.more === text);
}

toggleButton.addEventListener("click", () => api.toggle());
nextButton.addEventListener("click", () => api.next());

// Clicking elsewhere puts the launcher away, like any pop-up. The
// application does it, not this page: it knows whether the launcher
// really had the keyboard before losing it.
// The keyboard always goes back to the box.
window.addEventListener("focus", () => input.focus());

async function start() {
  await Promise.all([api.onPlayerChanged(renderNow), api.onLauncherOpened(reset)]);
  renderNow(await api.playerState());
  reset();
}

start().catch((error) => console.error(error));
