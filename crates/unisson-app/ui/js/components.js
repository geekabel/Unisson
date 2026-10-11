/**
 * Building blocks shared by several views.
 */

import * as api from "./api.js";
import { element, icon } from "./dom.js";
import { formatTime, mediaKey, sourceLabel, stationDescription, stationMedia } from "./format.js";
import { showNotice } from "./notice.js";
import { openAddToListMenu } from "./playlists.js";

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
 * Builds the button of a favorite station: one click plays it.
 *
 * @param {import("./api.js").Station} station
 * @param {() => void} onPlay
 */
export function stationChip(station, onPlay) {
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

/**
 * Builds the "add to the queue" button of a media.
 *
 * @param {import("./api.js").Media} media
 */
export function queueButton(media) {
  return iconButton({
    icon: "plus",
    label: `Ajouter ${media.title} à la file`,
    onClick: () => {
      api.enqueue(media);
      showNotice({ owner: "queue", text: `Ajouté à la file : ${media.title}`, duration: 2500 });
    },
  });
}

/**
 * Builds the "add to a list" button of a media.
 *
 * @param {import("./api.js").Media} media
 */
export function listButton(media) {
  const button = iconButton({
    icon: "list-add",
    label: `Ajouter ${media.title} à une liste`,
    onClick: () => openAddToListMenu(button, media),
  });
  button.setAttribute("aria-haspopup", "menu");
  button.setAttribute("aria-expanded", "false");
  return button;
}

/**
 * Builds the buttons every media line offers: add to the queue, add to a
 * list.
 *
 * @param {import("./api.js").Media} media
 */
export function mediaActions(media) {
  return [queueButton(media), listButton(media)];
}

/**
 * Builds a button that asks for a second click before it acts, for what
 * cannot be undone.
 *
 * @param {object} options
 * @param {string} options.label
 * @param {string} options.confirmLabel Shown after the first click.
 * @param {() => void} options.onConfirm
 */
export function confirmButton({ label, confirmLabel, onConfirm }) {
  const button = element("button", { class: "text-button", type: "button" }, [label]);
  let timer = null;
  const disarm = () => {
    clearTimeout(timer);
    button.textContent = label;
    button.classList.remove("text-button-danger");
    delete button.dataset.armed;
  };
  button.addEventListener("click", () => {
    if (button.dataset.armed) {
      disarm();
      onConfirm();
      return;
    }
    button.dataset.armed = "true";
    button.textContent = confirmLabel;
    button.classList.add("text-button-danger");
    // Without a second click, the button goes back to its first state.
    timer = setTimeout(disarm, 4000);
  });
  button.addEventListener("blur", disarm);
  return button;
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
