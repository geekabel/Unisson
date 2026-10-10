/**
 * Building blocks shared by several views.
 */

import { element, icon } from "./dom.js";
import { formatTime, mediaKey, sourceLabel } from "./format.js";

/**
 * Builds a titled section.
 *
 * @param {string} title
 * @param {(Node | string)[]} content
 * @param {Node} [aside] Something to show at the end of the title line.
 */
export function section(title, content, aside) {
  return element("section", { class: "section" }, [
    element("div", { class: "section-head" }, [
      element("h2", { class: "section-title" }, [title]),
      aside,
    ]),
    ...content,
  ]);
}

/**
 * Builds a small source label.
 *
 * @param {string} source
 */
export function sourceBadge(source) {
  return element("span", { class: "badge" }, [sourceLabel(source)]);
}

/**
 * Builds one line of a media list. The main part of the line is one
 * button: a single click plays the media.
 *
 * @param {import("./api.js").Media} media
 * @param {object} options
 * @param {() => void} options.onPlay
 * @param {Node[]} [options.actions] Buttons shown at the end of the line.
 * @param {string[]} [options.sources] Sources to show as labels. By
 *   default, the source of the media.
 */
export function mediaRow(media, { onPlay, actions = [], sources = [media.source] }) {
  const isStation = media.kind === "station";
  const detail = isStation ? "En direct" : media.artist ?? "";
  const main = element(
    "button",
    {
      class: "row-main",
      type: "button",
      "aria-label": `Lire ${media.title}${detail ? `, ${detail}` : ""}`,
    },
    [
      element("span", { class: "row-play" }, [icon("play", { filled: true })]),
      element("span", { class: "row-title truncate" }, [media.title]),
      element("span", { class: "row-artist truncate" }, [detail]),
      element("span", { class: "row-badges" }, sources.map(sourceBadge)),
      element("span", { class: "row-duration" }, [formatTime(media.durationMs)]),
    ],
  );
  main.addEventListener("click", onPlay);
  return element("div", { class: "row", "data-media-key": mediaKey(media) }, [main, ...actions]);
}

/**
 * Builds a round button that only shows an icon.
 *
 * @param {object} options
 * @param {string} options.icon
 * @param {string} options.label What the button does, for screen readers
 *   and as a tooltip.
 * @param {() => void} options.onClick
 * @param {boolean} [options.filled]
 * @param {boolean} [options.pressed] For a button that stays on or off.
 */
export function iconButton({ icon: name, label, onClick, filled = false, pressed }) {
  const button = element(
    "button",
    {
      class: "icon-button",
      type: "button",
      "aria-label": label,
      title: label,
      "aria-pressed": pressed == null ? null : String(pressed),
    },
    [icon(name, { filled })],
  );
  button.addEventListener("click", (event) => {
    event.stopPropagation();
    onClick();
  });
  return button;
}
