/**
 * The favorites view: the media and the stations the user kept.
 */

import * as api from "../api.js";
import { iconButton, mediaActions, mediaRow, section, stationChip } from "../components.js";
import { element } from "../dom.js";
import { countLabel, stationMedia } from "../format.js";
import { favoriteStations } from "../stations.js";

/**
 * Builds the view.
 *
 * @param {{showView: (name: string) => void}} context
 * @returns {Promise<Node[]>}
 */
export async function render({ showView }) {
  const favorites = await api.favoritesList();
  const stations = favoriteStations();
  const content = [element("h1", { class: "view-title" }, ["Favoris"])];

  if (favorites.length === 0 && stations.length === 0) {
    content.push(
      element("p", { class: "hint" }, [
        "Aucun favori pour l'instant. Pendant une écoute, cliquez sur le cœur de la barre de lecture " +
          "pour garder le titre ou la station ici.",
      ]),
    );
    return content;
  }

  if (favorites.length > 0) {
    const playAll = element("button", { class: "text-button", type: "button" }, ["Tout lire"]);
    playAll.addEventListener("click", () => api.playList(favorites, 0));
    content.push(
      section(
        `Titres, ${countLabel(favorites.length, "favori")}`,
        [
          element("div", { class: "rows" }, favorites.map((media, index) => {
            return mediaRow(media, {
              onPlay: () => api.playList(favorites, index),
              actions: [
                ...mediaActions(media),
                iconButton({
                  icon: "close",
                  label: `Retirer ${media.title} des favoris`,
                  onClick: () => api.favoriteSet(media, false),
                }),
              ],
            });
          })),
        ],
        playAll,
      ),
    );
  }

  if (stations.length > 0) {
    const media = stations.map(stationMedia);
    const all = element("button", { class: "link-button", type: "button" }, ["Gérer les radios"]);
    all.addEventListener("click", () => showView("radios"));
    content.push(
      section(
        "Radios",
        [
          element("div", { class: "chips" }, stations.map((station, index) => {
            return stationChip(station, () => api.playList(media, index));
          })),
        ],
        all,
      ),
    );
  }
  return content;
}
