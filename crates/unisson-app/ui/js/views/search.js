/**
 * The search view: one list for every source.
 *
 * The search box at the top of the window drives this view: it sets the
 * text, moves the selection and plays the selected line. The view itself
 * shows the results as the sources answer, and filters them.
 */

import * as api from "../api.js";
import { iconButton, listButton, mediaRow, sourceBadge } from "../components.js";
import { element, icon, replaceKeepingFocus } from "../dom.js";
import {
  countLabel,
  matchesText,
  mediaKey,
  mediaSubtitle,
  searchSummary,
  sourceLabel,
} from "../format.js";
import { showNotice } from "../notice.js";
import { allPlaylists } from "../playlists.js";
import { showView } from "../views.js";

/** Shortest text that is searched. */
export const MIN_LENGTH = 2;

/**
 * Longest wait for the slow sources when Enter was pressed before the
 * results, in milliseconds. After it, the best result so far is used.
 */
const READY_PATIENCE = 1500;

const KINDS = [
  ["all", "Tout"],
  ["track", "Titres"],
  ["station", "Radios"],
  ["playlist", "Listes"],
];

const SOURCE_ERRORS = {
  network: "pas de réponse pour l'instant",
  unavailable: "indisponible pour l'instant",
};

const state = {
  text: "",
  /** @type {?{hits: Hit[], pending: string[], errors: {source: string, kind: string}[], done: boolean}} */
  update: null,
  /** `all`, `track`, `station` or `playlist`. */
  kind: "all",
  /** Sources the user switched off. */
  hiddenSources: new Set(),
  /** Index of the selected line, among the lines on screen. */
  selected: 0,
  /** What to do with the best result once it arrives, if Enter came first. */
  whenReady: null,
};

/**
 * @typedef {object} Hit
 * @property {import("../api.js").Media} media
 * @property {string[]} sources
 */

/** Counts the searches, so that a late answer never replaces a newer one. */
let searchCount = 0;
let readyTimer = null;
/** When the action that waits for results was asked. */
let readyAskedAt = 0;

function patienceOver() {
  return performance.now() - readyAskedAt >= READY_PATIENCE;
}
let summaryArea = element("div");
let filtersArea = element("div");
let resultsArea = element("div");

/**
 * Builds the view.
 *
 * @returns {Promise<Node[]>}
 */
export async function render() {
  summaryArea = element("div", { class: "search-summary", role: "status" });
  filtersArea = element("div", { class: "search-filters" });
  resultsArea = element("div", { class: "stack" });
  draw();
  return [
    element("div", { class: "search-head" }, [summaryArea, filtersArea]),
    resultsArea,
    legend(),
  ];
}

/**
 * Starts a search. The results show as the sources answer.
 *
 * @param {string} text
 */
export function setQuery(text) {
  text = text.trim();
  if (text === state.text && state.update) {
    return;
  }
  state.text = text;
  state.update = null;
  state.selected = 0;
  const count = ++searchCount;
  if (text.length < MIN_LENGTH) {
    state.whenReady = null;
    draw();
    return;
  }
  draw();
  api
    .search(text, (update) => {
      if (count !== searchCount) {
        return;
      }
      state.update = update;
      draw();
      // Once the patience has run out, any result will do.
      runWhenReady({ patient: !patienceOver() });
    })
    .catch(() => {
      if (count !== searchCount) {
        return;
      }
      state.update = { hits: [], pending: [], errors: [{ source: "", kind: "unavailable" }], done: true };
      draw();
    });
}

/** Asks every source again for the same text. */
function retrySearch() {
  const text = state.text;
  state.text = "";
  setQuery(text);
  // The button is about to disappear: the keyboard goes back to the box.
  document.getElementById("search-input")?.focus();
}

/** Forgets the search. */
export function clearQuery() {
  searchCount += 1;
  state.text = "";
  state.update = null;
  state.selected = 0;
  state.whenReady = null;
}

/**
 * Moves the selection up or down.
 *
 * @param {number} delta `1` for the next line, `-1` for the previous one.
 */
export function moveSelection(delta) {
  const count = visibleEntries().length;
  if (count === 0) {
    return;
  }
  state.selected = (state.selected + delta + count) % count;
  drawResults();
  resultsArea.querySelector("[data-selected]")?.scrollIntoView({ block: "nearest" });
}

/**
 * Acts on the selected line.
 *
 * If the results are not there yet, the action waits for them: typing a
 * few letters and pressing Enter right away plays the best result.
 *
 * @param {"now"|"next"|"enqueue"} mode
 */
export function activateSelection(mode) {
  const entry = visibleEntries()[state.selected];
  if (entry) {
    actOn(entry, mode);
  } else if (!state.update?.done && state.text.length >= MIN_LENGTH) {
    state.whenReady = mode;
    readyAskedAt = performance.now();
    clearTimeout(readyTimer);
    readyTimer = setTimeout(() => runWhenReady({ patient: false }), READY_PATIENCE);
  }
}

/**
 * Carries out the action that waits for results, if it is time.
 *
 * The best result is only known for sure once every source has answered.
 * A slow source must not hold the user back for long, though.
 *
 * @param {{patient?: boolean}} [options] Whether to keep waiting for the
 *   sources that have not answered.
 */
function runWhenReady({ patient = true } = {}) {
  if (!state.whenReady || (patient && !state.update?.done)) {
    return;
  }
  const entry = visibleEntries()[0];
  if (!entry && !state.update?.done) {
    // Nothing yet: the next answer may bring something.
    return;
  }
  const mode = state.whenReady;
  state.whenReady = null;
  clearTimeout(readyTimer);
  if (entry) {
    actOn(entry, mode);
  }
}

/**
 * Acts on a line of the results: a media or a playlist.
 *
 * @param {{hit?: Hit, playlist?: import("../api.js").Playlist}} entry
 * @param {"now"|"next"|"enqueue"} mode
 */
function actOn(entry, mode) {
  if (entry.hit) {
    act(entry.hit, mode);
  } else {
    actOnPlaylist(entry.playlist, mode);
  }
}

/**
 * Plays a playlist, or adds its media to the queue.
 *
 * @param {import("../api.js").Playlist} playlist
 * @param {"now"|"next"|"enqueue"} mode
 */
async function actOnPlaylist(playlist, mode) {
  const items = await api.playlistItems(playlist.id);
  if (items.length === 0) {
    showNotice({ owner: "search", text: `La liste « ${playlist.name} » est vide.`, duration: 3000 });
  } else if (mode === "now") {
    api.playList(items, 0);
  } else {
    for (const media of items) {
      await api.enqueue(media);
    }
    showNotice({ owner: "search", text: `Ajouté à la file : la liste « ${playlist.name} »`, duration: 2500 });
  }
}

/**
 * @param {Hit} hit
 * @param {"now"|"next"|"enqueue"} mode
 */
function act(hit, mode) {
  if (mode === "now") {
    api.playNow(hit.media);
    return;
  }
  if (mode === "next") {
    api.playNext(hit.media);
  } else {
    api.enqueue(hit.media);
  }
  showNotice({
    owner: "search",
    text: `${mode === "next" ? "À lire juste après" : "Ajouté à la file"} : ${hit.media.title}`,
    duration: 2500,
  });
}

/** Returns the playlists whose name matches the text, if they are shown. */
function visiblePlaylists() {
  if (state.text.length < MIN_LENGTH || (state.kind !== "all" && state.kind !== "playlist")) {
    return [];
  }
  return allPlaylists().filter((playlist) => matchesText(playlist.name, state.text));
}

/** Returns every line on screen, in display order: media, then playlists. */
function visibleEntries() {
  return [
    ...visibleHits().map((hit) => ({ hit })),
    ...visiblePlaylists().map((playlist) => ({ playlist })),
  ];
}

/** Returns the media on screen, in display order. */
function visibleHits() {
  const hits = (state.update?.hits ?? []).filter((hit) => {
    return (
      (state.kind === "all" || hit.media.kind === state.kind) &&
      !state.hiddenSources.has(hit.media.source)
    );
  });
  // The best result leads, then the tracks, then the stations.
  const [best, ...others] = hits;
  return best
    ? [
        best,
        ...others.filter((hit) => hit.media.kind === "track"),
        ...others.filter((hit) => hit.media.kind === "station"),
      ]
    : [];
}

function draw() {
  drawFilters();
  drawResults();
}

function drawFilters() {
  const kinds = KINDS.map(([kind, label]) => {
    const chip = element(
      "button",
      { class: "filter-chip", type: "button", "aria-pressed": String(state.kind === kind) },
      [label],
    );
    chip.addEventListener("click", () => {
      state.kind = kind;
      state.selected = 0;
      draw();
    });
    return chip;
  });

  const sources = [...new Set((state.update?.hits ?? []).map((hit) => hit.media.source))].sort();
  const toggles = sources.map((source) => {
    const shown = !state.hiddenSources.has(source);
    const chip = element(
      "button",
      { class: "filter-chip filter-chip-source", type: "button", "aria-pressed": String(shown) },
      [sourceLabel(source)],
    );
    chip.addEventListener("click", () => {
      if (shown) {
        state.hiddenSources.add(source);
      } else {
        state.hiddenSources.delete(source);
      }
      state.selected = 0;
      draw();
    });
    return chip;
  });

  const groups = [
    element("div", { class: "filter-chips", role: "group", "aria-label": "Type de résultat" }, kinds),
  ];
  // Choosing between sources only makes sense when several answered.
  if (toggles.length > 1) {
    groups.push(
      element("div", { class: "filter-chips", role: "group", "aria-label": "Sources" }, [
        element("span", { class: "section-note" }, ["Sources"]),
        ...toggles,
      ]),
    );
  }
  replaceKeepingFocus(filtersArea, ...groups);
}

function drawResults() {
  const update = state.update;
  const hits = visibleHits();
  const playlists = visiblePlaylists();
  const count = hits.length + playlists.length;
  state.selected = Math.min(state.selected, Math.max(0, count - 1));
  summaryArea.textContent = searchSummary(state.text, update, count, MIN_LENGTH);

  const content = [];
  const errors = update?.errors ?? [];
  if (errors.length > 0) {
    // A source that did not answer is a side note, not an alarm: the other
    // sources still show their results. The technical reason is a tooltip.
    const notes = errors.map((error) => {
      const reason = SOURCE_ERRORS[error.kind] ?? SOURCE_ERRORS.unavailable;
      return element("span", { class: "search-note", title: error.detail || null }, [
        error.source ? `${sourceLabel(error.source)} : ${reason}.` : "La recherche a échoué.",
      ]);
    });
    const retry = element("button", { class: "link-button", type: "button" }, ["Réessayer"]);
    retry.addEventListener("click", retrySearch);
    const ways = [retry];
    // YouTube is "unavailable" when its helper program is missing or too
    // old: the Sources screen says which, and how to repair it.
    if (errors.some((error) => error.source === "youtube" && error.kind === "unavailable")) {
      const repair = element("button", { class: "link-button", type: "button" }, ["Vérifier yt-dlp"]);
      repair.addEventListener("click", () => showView("sources"));
      ways.push(repair);
    }
    content.push(element("p", { class: "search-notes" }, [...notes, ...ways]));
  }

  if (hits.length > 0) {
    const [best, ...others] = hits;
    const tracks = others.filter((hit) => hit.media.kind === "track");
    const stations = others.filter((hit) => hit.media.kind === "station");
    content.push(group("Meilleur résultat", [bestCard(best, state.selected === 0)]));
    if (tracks.length > 0) {
      content.push(group("Titres", tracks.map((hit, index) => hitRow(hit, 1 + index))));
    }
    if (stations.length > 0) {
      content.push(
        group("Radios", stations.map((hit, index) => hitRow(hit, 1 + tracks.length + index))),
      );
    }
  }
  if (playlists.length > 0) {
    content.push(
      group("Listes", playlists.map((playlist, index) => playlistRow(playlist, hits.length + index))),
    );
  }
  replaceKeepingFocus(resultsArea, ...content);
}

/**
 * @param {import("../api.js").Playlist} playlist
 * @param {number} index Position among the lines on screen.
 */
function playlistRow(playlist, index) {
  const main = element(
    "button",
    { class: "row-main", type: "button", "aria-label": `Lire la liste ${playlist.name}` },
    [
      element("span", { class: "row-play" }, [icon("play", { filled: true })]),
      element("span", { class: "row-title truncate" }, [playlist.name]),
      element("span", { class: "row-artist truncate" }, [`Liste, ${countLabel(playlist.len, "titre")}`]),
    ],
  );
  main.addEventListener("click", () => actOnPlaylist(playlist, "now"));
  const row = element("div", { class: "row" }, [
    main,
    iconButton({
      icon: "list",
      label: `Ouvrir la liste ${playlist.name}`,
      onClick: () => showView("playlist", { param: playlist.id }),
    }),
  ]);
  if (index === state.selected) {
    row.dataset.selected = "";
  }
  return row;
}

function group(title, rows) {
  return element("section", { class: "search-group" }, [
    element("h2", { class: "group-title" }, [title]),
    element("div", { class: "rows" }, rows),
  ]);
}

function addButton(hit) {
  return iconButton({
    icon: "plus",
    label: `Ajouter ${hit.media.title} à la file`,
    onClick: () => act(hit, "enqueue"),
  });
}

/**
 * @param {Hit} hit
 * @param {number} index Position among the lines on screen.
 */
function hitRow(hit, index) {
  const row = mediaRow(hit.media, {
    onPlay: () => act(hit, "now"),
    sources: hit.sources,
    actions: [addButton(hit), listButton(hit.media)],
  });
  if (index === state.selected) {
    row.dataset.selected = "";
  }
  return row;
}

/**
 * @param {Hit} hit
 * @param {boolean} selected
 */
function bestCard(hit, selected) {
  const media = hit.media;
  const isStation = media.kind === "station";
  const main = element(
    "button",
    { class: "best-main", type: "button", "aria-label": `Lire ${media.title}` },
    [
      element("span", { class: "cover", "data-source": media.source }, [
        icon(isStation ? "radio" : "note"),
      ]),
      element("span", { class: "station-text" }, [
        element("span", { class: "best-title truncate" }, [media.title]),
        element("span", { class: "station-detail truncate" }, [
          isStation ? "Radio en direct" : mediaSubtitle(media),
        ]),
      ]),
      element("span", { class: "row-badges" }, hit.sources.map(sourceBadge)),
      element("span", { class: "best-hint" }, [element("kbd", {}, ["Entrée"]), "Lire"]),
    ],
  );
  main.addEventListener("click", () => act(hit, "now"));
  const card = element("div", { class: "row best", "data-media-key": mediaKey(media) }, [
    main,
    addButton(hit),
    listButton(hit.media),
  ]);
  if (selected) {
    card.dataset.selected = "";
  }
  return card;
}

function legend() {
  const entry = (keys, text) =>
    element("span", { class: "legend-entry" }, [
      ...keys.map((key) => element("kbd", {}, [key])),
      text,
    ]);
  return element("div", { class: "legend", "aria-label": "Raccourcis clavier" }, [
    entry(["Haut", "Bas"], "naviguer"),
    entry(["Entrée"], "lire tout de suite"),
    entry(["Maj Entrée"], "ajouter à la file"),
    entry(["Ctrl Entrée"], "lire juste après"),
    entry(["Échap"], "effacer"),
  ]);
}
