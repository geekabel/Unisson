/**
 * The favorites view: the media and the stations the user kept.
 */

import * as api from "../api.js";
import { iconButton, mediaActions, mediaRow, section, stationChip } from "../components.js";
import { element } from "../dom.js";
import { stationMedia } from "../format.js";
import { plural, t } from "../i18n.js";
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
  const content = [element("h1", { class: "view-title" }, [t("home.favorites")])];

  if (favorites.length === 0 && stations.length === 0) {
    content.push(
      element("p", { class: "hint" }, [t("home.favoritesEmpty")]),
    );
    return content;
  }

  if (favorites.length > 0) {
    const playAll = element("button", { class: "text-button", type: "button" }, [t("home.playAll")]);
    playAll.addEventListener("click", () => api.playList(favorites, 0));
    content.push(
      section(
        plural(favorites.length, "home.favoriteTracks"),
        [
          element("div", { class: "rows" }, favorites.map((media, index) => {
            return mediaRow(media, {
              onPlay: () => api.playList(favorites, index),
              actions: [
                ...mediaActions(media),
                iconButton({
                  icon: "close",
                  label: t("home.removeFavorite", { title: media.title }),
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
    const all = element("button", { class: "link-button", type: "button" }, [t("home.manageStations")]);
    all.addEventListener("click", () => showView("radios"));
    content.push(
      section(
        t("home.stations"),
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
