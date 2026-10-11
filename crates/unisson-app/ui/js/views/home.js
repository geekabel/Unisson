/**
 * The home view: what the user is most likely to want, one click away.
 */

import * as api from "../api.js";
import { mediaActions, mediaRow, section, stationChip } from "../components.js";
import { element, icon } from "../dom.js";
import { mediaKey, stationMedia, tileText } from "../format.js";
import { plural, t } from "../i18n.js";

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
      section(t("home.resume"), [
        element("div", { class: "tiles" }, home.resume.map(resumeTile)),
      ]),
    );
  }

  if (home.stations.length > 0) {
    const media = home.stations.map(stationMedia);
    const all = element("button", { class: "link-button", type: "button" }, [t("home.allStations")]);
    all.addEventListener("click", () => showView("radios"));
    sections.push(
      section(
        t("home.favoriteStations"),
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
      section(t("home.recent"), [
        element("div", { class: "rows" }, home.recent.map((media) => {
          return mediaRow(media, { onPlay: () => api.playNow(media), actions: mediaActions(media) });
        })),
      ]),
    );
  }

  if (home.library.length > 0) {
    const shown = home.library.length < home.libraryTotal
      ? t("home.libraryFirst", { shown: home.library.length, total: home.libraryTotal })
      : plural(home.libraryTotal, "home.tracks");
    sections.push(
      section(
        t("home.library"),
        [
          element("div", { class: "rows" }, home.library.map((media, index) => {
            return mediaRow(media, {
              onPlay: () => api.playList(home.library, index),
              actions: mediaActions(media),
            });
          })),
        ],
        element("span", { class: "section-note" }, [shown]),
      ),
    );
  }

  if (sections.length === 0) {
    return [
      element("h1", { class: "view-title" }, [t("home.welcome")]),
      element("p", { class: "hint" }, [t("home.welcomeHint")]),
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
      "aria-label": t("home.playTile", { title: tile.title, subtitle }),
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
