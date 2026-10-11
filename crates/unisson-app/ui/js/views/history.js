/**
 * The history view: what was played, the latest first.
 */

import * as api from "../api.js";
import { confirmButton, mediaActions, mediaRow, section } from "../components.js";
import { element } from "../dom.js";
import { t } from "../i18n.js";

/**
 * Builds the view.
 *
 * @returns {Promise<Node[]>}
 */
export async function render() {
  const history = await api.historyList();
  const title = element("h1", { class: "view-title" }, [t("home.history")]);
  if (history.length === 0) {
    return [
      title,
      element("p", { class: "hint" }, [t("home.historyEmpty")]),
    ];
  }
  return [
    title,
    section(
      t("home.recent"),
      [
        element("div", { class: "rows" }, history.map((media) => {
          return mediaRow(media, { onPlay: () => api.playNow(media), actions: mediaActions(media) });
        })),
      ],
      confirmButton({
        label: t("home.clearHistory"),
        confirmLabel: t("home.clearHistoryConfirm"),
        onConfirm: () => api.historyClear(),
      }),
    ),
  ];
}
