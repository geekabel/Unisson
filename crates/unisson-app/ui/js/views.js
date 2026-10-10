/**
 * The central area of the window: one view per entry of the sidebar.
 *
 * Only the shell exists for now. The home view lists what can already be
 * played, and the other views say when they arrive.
 */

import * as api from "./api.js";
import { byId, element, icon } from "./dom.js";
import { formatTime, mediaKey, sourceLabel } from "./format.js";

/** How many tracks the temporary home list shows at most. */
const HOME_TRACK_LIMIT = 200;

const COMING_VIEWS = {
  radios: { title: "Radios", text: "La recherche de radios par nom, genre et pays arrive avec la tâche T17." },
  favorites: { title: "Favoris", text: "Vos titres favoris arrivent avec la tâche T21." },
  history: { title: "Historique", text: "L'historique d'écoute arrive avec la tâche T21. Il s'enregistre déjà." },
  sources: { title: "Sources", text: "Le réglage des sources arrive avec la tâche T20." },
};

const container = byId("view");
const navItems = [...document.querySelectorAll(".nav-item[data-view]")];

let currentView = "home";
/** Key of the media selected in the player, to highlight it in lists. */
let currentKey = "";
/** Counts the renders, so that a slow answer never overwrites a newer view. */
let renderCount = 0;

/**
 * Shows a view.
 *
 * @param {string} name One of the `data-view` values of the sidebar.
 */
export async function showView(name) {
  currentView = name;
  for (const item of navItems) {
    if (item.dataset.view === name) {
      item.setAttribute("aria-current", "page");
    } else {
      item.removeAttribute("aria-current");
    }
  }
  const render = ++renderCount;
  const content = name === "home" ? await homeView() : comingView(COMING_VIEWS[name]);
  if (render === renderCount) {
    container.replaceChildren(...content);
    highlightCurrent();
  }
}

/** Draws the current view again, after its data changed. */
export function refreshView() {
  return showView(currentView);
}

/**
 * Tells the views which media is selected in the player.
 *
 * @param {?import("./api.js").Media} media
 */
export function setCurrentMedia(media) {
  const key = mediaKey(media);
  if (key !== currentKey) {
    currentKey = key;
    highlightCurrent();
  }
}

function highlightCurrent() {
  for (const node of container.querySelectorAll("[data-media-key]")) {
    if (node.dataset.mediaKey === currentKey) {
      node.setAttribute("aria-current", "true");
    } else {
      node.removeAttribute("aria-current");
    }
  }
}

function comingView({ title, text }) {
  return [
    element("h1", { class: "view-title" }, [title]),
    element("p", { class: "hint" }, [text]),
  ];
}

async function homeView() {
  const [stations, tracks] = await Promise.all([api.favoriteStations(), api.libraryTracks()]);
  const sections = [];

  if (stations.length > 0) {
    sections.push(
      element("section", { class: "section" }, [
        element("h2", { class: "section-title" }, ["Radios favorites"]),
        element("div", { class: "chips" }, stations.map((station, index) => {
          const chip = element(
            "button",
            { class: "chip truncate", type: "button", "data-media-key": mediaKey(station) },
            [station.title],
          );
          chip.addEventListener("click", () => api.playList(stations, index));
          return chip;
        })),
      ]),
    );
  }

  if (tracks.length > 0) {
    const shown = tracks.slice(0, HOME_TRACK_LIMIT);
    const count = tracks.length > shown.length
      ? `${shown.length} premiers titres sur ${tracks.length}`
      : `${tracks.length} titre${tracks.length > 1 ? "s" : ""}`;
    sections.push(
      element("section", { class: "section" }, [
        element("h2", { class: "section-title" }, [`Bibliothèque locale, ${count}`]),
        element("div", { class: "rows" }, shown.map((track, index) => trackRow(track, () => {
          api.playList(shown, index);
        }))),
      ]),
    );
  }

  if (sections.length === 0) {
    sections.push(
      element("h1", { class: "view-title" }, ["Bienvenue dans Unisson"]),
      element("p", { class: "hint" }, [
        "Votre dossier Musique est vide, et vous n'avez pas encore de radio favorite. " +
          "Collez l'adresse d'un flux radio ou le chemin complet d'un fichier audio " +
          "dans le champ ci-dessus, puis appuyez sur Entrée.",
      ]),
    );
  }
  return sections;
}

/**
 * Builds one line of a track list. The whole line is one button: a single
 * click plays the track.
 *
 * @param {import("./api.js").Media} track
 * @param {() => void} play
 */
function trackRow(track, play) {
  const row = element(
    "button",
    {
      class: "row",
      type: "button",
      "data-media-key": mediaKey(track),
      "aria-label": `Lire ${track.title}${track.artist ? `, ${track.artist}` : ""}`,
    },
    [
      element("span", { class: "row-play" }, [icon("play", { filled: true })]),
      element("span", { class: "row-title truncate" }, [track.title]),
      element("span", { class: "row-artist truncate" }, [track.artist ?? ""]),
      element("span", { class: "badge" }, [sourceLabel(track.source)]),
      element("span", { class: "row-duration" }, [formatTime(track.durationMs)]),
    ],
  );
  row.addEventListener("click", play);
  return row;
}

for (const item of navItems) {
  item.addEventListener("click", () => showView(item.dataset.view));
}
