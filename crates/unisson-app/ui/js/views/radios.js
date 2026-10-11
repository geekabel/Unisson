/**
 * The radio view: the station on air, the favorite stations, and the
 * directory to explore.
 *
 * The view keeps its state between two visits (criteria and results), so
 * coming back to it is instant. Its three areas are redrawn separately:
 * a new title on air must not reset what the user is typing.
 */

import * as api from "../api.js";
import { iconButton, section } from "../components.js";
import { element, icon, replaceKeepingFocus } from "../dom.js";
import {
  countryName,
  mediaKey,
  stationDescription,
  stationMedia,
  stationQuality,
} from "../format.js";
import { showNotice } from "../notice.js";
import { favoriteStations, isFavorite, onStationsChange } from "../stations.js";

/** Tags offered as shortcuts: the label shown, and the tag searched. */
const TAGS = [
  ["Jazz", "jazz"],
  ["Classique", "classical"],
  ["Info", "news"],
  ["Électro", "electronic"],
  ["Rock", "rock"],
  ["Pop", "pop"],
  ["Hip-hop", "hip hop"],
  ["Reggae", "reggae"],
  ["Musiques du monde", "world music"],
  ["Ambient", "ambient"],
];

/** Countries offered in the list, as two-letter codes. */
const COUNTRY_CODES = [
  "DZ", "DE", "AR", "AU", "AT", "BE", "BJ", "BR", "BF", "CM", "CA", "CL", "CN", "CO", "CG", "CD",
  "KR", "CI", "CU", "DK", "EG", "ES", "US", "FI", "FR", "GA", "GH", "GR", "GN", "HT", "IN", "IE",
  "IL", "IT", "JM", "JP", "KE", "LB", "LU", "MG", "ML", "MA", "MX", "NE", "NG", "NO", "NL", "PL",
  "PT", "CZ", "RO", "GB", "RU", "SN", "SE", "CH", "TG", "TN", "TR", "UA",
];

const SEARCH_ERRORS = {
  network: "L'annuaire ne répond pas. Vérifiez votre connexion.",
  unavailable: "L'annuaire est indisponible pour le moment.",
};

const state = {
  name: "",
  /** @type {?string} */
  tag: null,
  countryCode: defaultCountry(),
  /** `idle` until the first search, then `loading`, `done` or `error`. */
  status: "idle",
  /** @type {import("../api.js").Station[]} */
  results: [],
  error: "",
  formOpen: false,
  /** @type {?import("../api.js").Snapshot} */
  snapshot: null,
};

/** Stations met so far, by identifier: favorites and search results. */
const known = new Map();
/** Counts the searches, so that a slow answer never replaces a newer one. */
let searchCount = 0;
let searchTimer = null;

/** The three areas of the view, kept to redraw them separately. */
let liveArea = element("div");
let favoritesArea = element("div");
let resultsArea = element("div");

/** The country of the user's language settings, when it is in the list. */
function defaultCountry() {
  const region = (navigator.language ?? "").split("-")[1]?.toUpperCase() ?? "";
  return COUNTRY_CODES.includes(region) ? region : "";
}

/**
 * Builds the view.
 *
 * @returns {Promise<Node[]>}
 */
export async function render() {
  liveArea = element("div", { class: "live-area" });
  favoritesArea = element("div", { class: "stack" });
  resultsArea = element("div", { class: "stack", "aria-live": "polite" });
  drawLive();
  drawFavorites();
  drawResults();
  if (state.status === "idle") {
    search();
  }

  const addButton = element("button", { class: "text-button", type: "button" }, [
    "Ajouter par adresse de flux",
  ]);
  addButton.addEventListener("click", () => {
    state.formOpen = !state.formOpen;
    drawFavorites();
    favoritesArea.querySelector("input")?.focus();
  });

  return [
    liveArea,
    section("Mes stations", [favoritesArea], addButton),
    section(
      "Explorer l'annuaire",
      [criteriaForm(), resultsArea],
      element("span", { class: "section-note" }, ["Annuaire libre Radio Browser"]),
    ),
  ];
}

/**
 * Receives the playback state while the view is on screen.
 *
 * @param {import("../api.js").Snapshot} snapshot
 */
export function onPlayerChanged(snapshot) {
  state.snapshot = snapshot;
  drawLive();
  markPlaying();
}

/** Returns the station that plays, when the view knows it. */
function liveStation() {
  const media = state.snapshot?.current;
  return media?.kind === "station" ? { media, station: known.get(media.key) ?? null } : null;
}

function drawLive() {
  const live = liveStation();
  if (!live) {
    liveArea.replaceChildren();
    return;
  }
  const { status, streamTitle } = state.snapshot;
  const sounding = status === "playing" || status === "reconnecting";
  const label = sounding ? "En direct" : status === "loading" ? "Connexion" : "À l'arrêt";
  const onAir = !sounding
    ? ""
    : streamTitle
      ? `À l'antenne : ${streamTitle}`
      : "Le titre à l'antenne n'est pas annoncé.";
  replaceKeepingFocus(liveArea,
    element("div", { class: "live-card" }, [
      element("div", { class: "live-card-cover" }, [icon("radio")]),
      element("div", { class: "live-card-text" }, [
        element("span", { class: sounding ? "live" : "live live-off" }, [
          element("span", { class: "live-dot", "aria-hidden": "true" }),
          label,
        ]),
        element("h1", { class: "live-card-title truncate" }, [live.media.title]),
        onAir ? element("p", { class: "live-card-air truncate" }, [onAir]) : null,
        sounding && streamTitle ? findTitleButton(streamTitle) : null,
      ]),
      live.station ? favoriteButton(live.station) : null,
    ]),
  );
}

/** Builds the link that searches the title on air in every source. */
function findTitleButton(title) {
  const button = element("button", { class: "link-button", type: "button" }, ["Retrouver ce titre"]);
  button.addEventListener("click", () => {
    document.dispatchEvent(new CustomEvent("unisson:search", { detail: title }));
  });
  return button;
}

function drawFavorites() {
  const stations = favoriteStations();
  for (const station of stations) {
    known.set(station.uuid, station);
  }
  const content = [];
  if (state.formOpen) {
    content.push(customStationForm());
  }
  if (stations.length > 0) {
    content.push(
      element("div", { class: "station-grid" }, stations.map((station, index) => {
        return stationRow(station, () => api.stationsPlay(stations, index));
      })),
    );
  } else if (!state.formOpen) {
    content.push(
      element("p", { class: "hint" }, [
        "Aucune station favorite pour l'instant. Cherchez une station ci-dessous, " +
          "puis cliquez sur le cœur pour la garder ici.",
      ]),
    );
  }
  replaceKeepingFocus(favoritesArea, ...content);
  markPlaying();
}

function drawResults() {
  for (const station of state.results) {
    known.set(station.uuid, station);
  }
  let content;
  if (state.status === "loading" || state.status === "idle") {
    content = [element("p", { class: "hint" }, ["Recherche en cours…"])];
  } else if (state.status === "error") {
    const retry = element("button", { class: "text-button", type: "button" }, ["Réessayer"]);
    retry.addEventListener("click", search);
    content = [element("p", { class: "hint", role: "alert" }, [state.error]), retry];
  } else if (state.results.length === 0) {
    content = [element("p", { class: "hint" }, ["Aucune station ne correspond à cette recherche."])];
  } else {
    content = [
      element("div", { class: "rows" }, state.results.map((station) => {
        return stationRow(station, () => api.stationsPlay([station], 0), { detailed: true });
      })),
    ];
  }
  replaceKeepingFocus(resultsArea, ...content);
  markPlaying();
}

/** Highlights the station that plays, in both lists. */
function markPlaying() {
  const key = mediaKey(state.snapshot?.current ?? null);
  for (const area of [favoritesArea, resultsArea]) {
    for (const row of area.querySelectorAll("[data-media-key]")) {
      if (row.dataset.mediaKey === key) {
        row.setAttribute("aria-current", "true");
      } else {
        row.removeAttribute("aria-current");
      }
    }
  }
}

/** Updates every heart on screen, without rebuilding the lists. */
function syncFavoriteButtons() {
  for (const button of document.querySelectorAll("[data-favorite-uuid]")) {
    setFavoriteLook(button, known.get(button.dataset.favoriteUuid));
  }
}

function setFavoriteLook(button, station) {
  const favorite = isFavorite(station.uuid);
  const label = favorite
    ? `Retirer ${station.name} des favoris`
    : `Ajouter ${station.name} aux favoris`;
  button.setAttribute("aria-pressed", String(favorite));
  button.setAttribute("aria-label", label);
  button.title = label;
}

/** @param {import("../api.js").Station} station */
function favoriteButton(station) {
  known.set(station.uuid, station);
  const button = iconButton({
    icon: "heart",
    label: "",
    onClick: async () => {
      try {
        await api.stationSetFavorite(station, !isFavorite(station.uuid));
      } catch {
        showNotice({
          owner: "radios",
          tone: "error",
          text: "Les favoris n'ont pas pu être enregistrés.",
          duration: 8000,
        });
      }
    },
  });
  button.classList.add("favorite-button");
  button.dataset.favoriteUuid = station.uuid;
  setFavoriteLook(button, station);
  return button;
}

/**
 * Builds the line of a station. A click on it plays the station, or
 * pauses it if it is the one playing.
 *
 * @param {import("../api.js").Station} station
 * @param {() => void} play
 * @param {{detailed?: boolean}} [options] Shows more tags and the quality.
 */
function stationRow(station, play, { detailed = false } = {}) {
  const key = mediaKey(stationMedia(station));
  const description = stationDescription(station, detailed ? 3 : 1);
  const main = element(
    "button",
    { class: "row-main", type: "button", "aria-label": `Écouter ${station.name}` },
    [
      element("span", { class: "row-play" }, [icon("play", { filled: true })]),
      element("span", { class: "station-text" }, [
        element("span", { class: "station-name truncate" }, [station.name]),
        description ? element("span", { class: "station-detail truncate" }, [description]) : null,
      ]),
      detailed ? element("span", { class: "station-quality" }, [stationQuality(station)]) : null,
    ],
  );
  main.addEventListener("click", () => {
    if (mediaKey(state.snapshot?.current ?? null) === key) {
      api.toggle();
    } else {
      play();
    }
  });
  return element("div", { class: "row", "data-media-key": key }, [main, favoriteButton(station)]);
}

function customStationForm() {
  const name = element("input", {
    class: "field",
    type: "text",
    required: true,
    placeholder: "Nom de la station",
    "aria-label": "Nom de la station",
  });
  const address = element("input", {
    class: "field field-wide",
    type: "url",
    required: true,
    placeholder: "https://exemple.fr/flux.mp3",
    "aria-label": "Adresse du flux",
  });
  const cancel = element("button", { class: "text-button", type: "button" }, ["Annuler"]);
  const form = element("form", { class: "inline-form" }, [
    name,
    address,
    element("button", { class: "text-button text-button-strong", type: "submit" }, ["Ajouter"]),
    cancel,
  ]);
  cancel.addEventListener("click", () => {
    state.formOpen = false;
    drawFavorites();
  });
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    try {
      await api.stationAddCustom(name.value, address.value);
      state.formOpen = false;
      drawFavorites();
    } catch (error) {
      showNotice({
        owner: "radios",
        tone: "error",
        text:
          error?.kind === "invalid"
            ? "Donnez un nom, et une adresse qui commence par http:// ou https://."
            : "La station n'a pas pu être ajoutée.",
        duration: 8000,
      });
    }
  });
  return form;
}

function criteriaForm() {
  const name = element("input", {
    class: "field field-wide",
    type: "search",
    placeholder: "Nom d'une station",
    "aria-label": "Nom d'une station",
    autocomplete: "off",
    spellcheck: "false",
  });
  name.value = state.name;
  name.addEventListener("input", () => {
    state.name = name.value;
    // Wait for a pause in the typing before asking the directory.
    clearTimeout(searchTimer);
    searchTimer = setTimeout(search, 400);
  });

  const country = element("select", { class: "field", "aria-label": "Pays" }, [
    element("option", { value: "" }, ["Tous les pays"]),
    ...COUNTRY_CODES.map((code) => ({ code, name: countryName(code) || code }))
      .sort((a, b) => a.name.localeCompare(b.name, "fr"))
      .map(({ code, name: label }) => element("option", { value: code }, [label])),
  ]);
  country.value = state.countryCode;
  country.addEventListener("change", () => {
    state.countryCode = country.value;
    search();
  });

  const tags = element("div", { class: "filter-chips", role: "group", "aria-label": "Genre" });
  const drawTags = () => {
    replaceKeepingFocus(tags,
      ...TAGS.map(([label, tag]) => {
        const chip = element(
          "button",
          { class: "filter-chip", type: "button", "aria-pressed": String(state.tag === tag) },
          [label],
        );
        chip.addEventListener("click", () => {
          state.tag = state.tag === tag ? null : tag;
          drawTags();
          search();
        });
        return chip;
      }),
    );
  };
  drawTags();

  const form = element("form", { class: "stack", role: "search" }, [
    element("div", { class: "inline-form" }, [name, country]),
    tags,
  ]);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    search();
  });
  return form;
}

/** Asks the directory for the stations that match the criteria. */
async function search() {
  clearTimeout(searchTimer);
  const count = ++searchCount;
  state.status = "loading";
  drawResults();
  try {
    const results = await api.stationsSearch({
      name: state.name.trim(),
      tag: state.tag,
      countryCode: state.countryCode,
    });
    if (count !== searchCount) {
      return;
    }
    state.results = results;
    state.status = "done";
  } catch (error) {
    if (count !== searchCount) {
      return;
    }
    state.results = [];
    state.status = "error";
    state.error = SEARCH_ERRORS[error?.kind] ?? SEARCH_ERRORS.unavailable;
  }
  drawResults();
}

onStationsChange(() => {
  drawFavorites();
  syncFavoriteButtons();
});
