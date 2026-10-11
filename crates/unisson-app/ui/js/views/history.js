/**
 * The history view: what was played, the latest first.
 */

import * as api from "../api.js";
import { confirmButton, mediaActions, mediaRow, section } from "../components.js";
import { element } from "../dom.js";

/**
 * Builds the view.
 *
 * @returns {Promise<Node[]>}
 */
export async function render() {
  const history = await api.historyList();
  const title = element("h1", { class: "view-title" }, ["Historique"]);
  if (history.length === 0) {
    return [
      title,
      element("p", { class: "hint" }, [
        "Rien pour l'instant. Chaque titre et chaque radio que vous écoutez s'ajoute ici.",
      ]),
    ];
  }
  return [
    title,
    section(
      "Écoutés récemment",
      [
        element("div", { class: "rows" }, history.map((media) => {
          return mediaRow(media, { onPlay: () => api.playNow(media), actions: mediaActions(media) });
        })),
      ],
      confirmButton({
        label: "Effacer l'historique",
        confirmLabel: "Cliquer encore pour effacer",
        onConfirm: () => api.historyClear(),
      }),
    ),
  ];
}
