/**
 * The home view: what the user is most likely to want, one click away.
 */

import * as api from "../api.js";
import { mediaRow, section } from "../components.js";
import { element, icon } from "../dom.js";
import {
  countLabel,
  mediaKey,
  stationDescription,
  stationMedia,
  tileText,
} from "../format.js";

/**
 * Builds the view.
 *
 * @param {{showView: (name: string) => void}} context
 * @returns {Promise<Node[]>}
 */
export async function render({ showView }) {
  const home = await api.home();
  const sections = [];

  if (home.resume.length > 0) {
    sections.push(
      section("Reprendre en un clic", [
        element("div", { class: "tiles" }, home.resume.map(resumeTile)),
      ]),
    );
  }

  if (home.stations.length > 0) {
    const media = home.stations.map(stationMedia);
    const all = element("button", { class: "link-button", type: "button" }, ["Toutes les radios"]);
    all.addEventListener("click", () => showView("radios"));
    sections.push(
      section(
        "Radios favorites",
        [
          element("div", { class: "chips" }, home.stations.map((station, index) => {
            return stationChip(station, () => api.playList(media, index));
          })),
        ],
        all,
      ),
    );
  }

  if (home.recent.length > 0) {
    sections.push(
      section("Écoutés récemment", [
        element("div", { class: "rows" }, home.recent.map((media) => {
          return mediaRow(media, { onPlay: () => api.playList([media], 0) });
        })),
      ]),
    );
  }

  if (home.library.length > 0) {
    const shown = home.library.length < home.libraryTotal
      ? `${home.library.length} premiers titres sur ${home.libraryTotal}`
      : countLabel(home.libraryTotal, "titre");
    sections.push(
      section(
        "Bibliothèque locale",
        [
          element("div", { class: "rows" }, home.library.map((media, index) => {
            return mediaRow(media, { onPlay: () => api.playList(home.library, index) });
          })),
        ],
        element("span", { class: "section-note" }, [shown]),
      ),
    );
  }

  if (sections.length === 0) {
    return [
      element("h1", { class: "view-title" }, ["Bienvenue dans Unisson"]),
      element("p", { class: "hint" }, [
        "Votre dossier Musique est vide, et vous n'avez pas encore de radio favorite. " +
          "Ouvrez Radios pour choisir une station, ou collez l'adresse d'un flux " +
          "dans le champ ci-dessus, puis appuyez sur Entrée.",
      ]),
    ];
  }
  return sections;
}

/** @param {import("../api.js").Tile} tile */
function resumeTile(tile) {
  const { badge, subtitle } = tileText(tile);
  const button = element(
    "button",
    {
      class: "tile",
      type: "button",
      "data-source": tile.source,
      "data-media-key": mediaKey(tile.items[tile.start]),
      "aria-label": `Lire ${tile.title}, ${subtitle}`,
    },
    [
      element("span", { class: "tile-band" }, [
        element("span", { class: "tile-badge" }, [badge]),
        element("span", { class: "tile-play" }, [icon("play", { filled: true })]),
      ]),
      element("span", { class: "tile-text" }, [
        element("span", { class: "tile-title truncate" }, [tile.title]),
        element("span", { class: "tile-subtitle truncate" }, [subtitle]),
      ]),
    ],
  );
  button.addEventListener("click", () => api.playList(tile.items, tile.start));
  return button;
}

/**
 * @param {import("../api.js").Station} station
 * @param {() => void} onPlay
 */
function stationChip(station, onPlay) {
  const description = stationDescription(station);
  const chip = element(
    "button",
    {
      class: "chip",
      type: "button",
      "data-media-key": mediaKey(stationMedia(station)),
      "aria-label": `Écouter ${station.name}`,
    },
    [
      element("span", { class: "chip-title truncate" }, [station.name]),
      description ? element("span", { class: "chip-subtitle truncate" }, [description]) : null,
    ],
  );
  chip.addEventListener("click", onPlay);
  return chip;
}
