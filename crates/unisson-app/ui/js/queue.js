/**
 * The queue panel, on the right of the window.
 *
 * It lists what plays next, lets the user reorder, remove or jump, and
 * shows what happens when the queue runs out.
 */

import * as api from "./api.js";
import { iconButton, sourceBadge } from "./components.js";
import { byId, element, icon } from "./dom.js";
import { countLabel, mediaSubtitle } from "./format.js";

/** Where the choice to show or hide the panel is remembered. */
const STORAGE_KEY = "unisson.queue.open";
/** From this window width, the panel is shown on the first run. */
const WIDE_WINDOW = 1100;

const panel = byId("queue");
const body = byId("queue-body");
const fallback = byId("queue-fallback");
const clearButton = /** @type {HTMLButtonElement} */ (byId("queue-clear"));
const toggleButton = byId("player-queue");

let refreshTimer = null;
/** The action button to focus again after the list is redrawn. */
let focusAfterDraw = null;

/** Reads the remembered choice. Storage may be unavailable: then guess. */
function wasOpen() {
  try {
    const saved = localStorage.getItem(STORAGE_KEY);
    if (saved !== null) {
      return saved === "true";
    }
  } catch {
    // Fall through to the default.
  }
  return window.innerWidth >= WIDE_WINDOW;
}

/**
 * Shows or hides the panel.
 *
 * @param {boolean} open
 */
function setOpen(open) {
  panel.hidden = !open;
  toggleButton.setAttribute("aria-pressed", String(open));
  toggleButton.title = open ? "Masquer la file d'attente" : "Afficher la file d'attente";
  toggleButton.setAttribute("aria-label", toggleButton.title);
  try {
    localStorage.setItem(STORAGE_KEY, String(open));
  } catch {
    // The choice is simply not remembered.
  }
  if (open) {
    draw();
  }
}

/** Draws the panel again after the queue changed, if it is shown. */
export function refreshQueue() {
  if (panel.hidden) {
    return;
  }
  // Several changes in a row give a single redraw.
  clearTimeout(refreshTimer);
  refreshTimer = setTimeout(draw, 30);
}

async function draw() {
  const queue = await api.queueState();
  if (panel.hidden) {
    return;
  }
  clearButton.disabled = queue.upcomingTotal === 0;

  const content = [element("h3", { class: "group-title" }, ["En cours"])];
  content.push(
    queue.current
      ? currentItem(queue.current)
      : element("p", { class: "hint" }, ["Rien en lecture."]),
  );

  content.push(element("h3", { class: "group-title" }, ["Ensuite"]));
  if (queue.upcoming.length === 0) {
    content.push(
      element("p", { class: "hint" }, [
        "Rien ensuite. Ajoutez un titre avec le bouton + de la recherche, ou avec Maj Entrée.",
      ]),
    );
  } else {
    content.push(
      element("ol", { class: "queue-list" }, queue.upcoming.map((media, index) => {
        return upcomingItem(media, index, queue.upcoming.length);
      })),
    );
    const hidden = queue.upcomingTotal - queue.upcoming.length;
    if (hidden > 0) {
      content.push(
        element("p", { class: "section-note" }, [`et ${countLabel(hidden, "autre titre", "autres titres")}`]),
      );
    }
  }
  body.replaceChildren(...content);
  drawFallback(queue);

  if (focusAfterDraw) {
    const { index, action } = focusAfterDraw;
    focusAfterDraw = null;
    const rows = body.querySelectorAll(".queue-row");
    const row = rows[Math.min(index, rows.length - 1)];
    (row?.querySelector(`[data-action="${action}"]:not(:disabled)`) ?? row?.querySelector("button"))?.focus();
  }
}

/** @param {import("./api.js").Media} media */
function itemText(media) {
  return element("span", { class: "station-text" }, [
    element("span", { class: "queue-title truncate" }, [media.title]),
    element("span", { class: "station-detail truncate" }, [mediaSubtitle(media)]),
  ]);
}

/** @param {import("./api.js").Media} media */
function currentItem(media) {
  return element("div", { class: "queue-current" }, [
    icon("bars"),
    itemText(media),
    sourceBadge(media.source),
  ]);
}

/**
 * @param {import("./api.js").Media} media
 * @param {number} index Position among the upcoming media.
 * @param {number} count Number of upcoming media listed.
 */
function upcomingItem(media, index, count) {
  const main = element(
    "button",
    { class: "queue-main", type: "button", "aria-label": `Passer à ${media.title}`, title: "Lire maintenant" },
    [element("span", { class: "queue-number" }, [String(index + 1)]), itemText(media)],
  );
  main.addEventListener("click", () => api.queueJump(index));

  const action = (name, label, iconName, run, disabled = false) => {
    const button = iconButton({ icon: iconName, label, onClick: run });
    button.dataset.action = name;
    button.disabled = disabled;
    return button;
  };
  const move = (to) => () => {
    focusAfterDraw = { index: to, action: to < index ? "up" : "down" };
    api.queueMove(index, to);
  };
  return element("li", { class: "queue-row" }, [
    main,
    element("span", { class: "queue-actions" }, [
      action("up", `Monter ${media.title}`, "up", move(index - 1), index === 0),
      action("down", `Descendre ${media.title}`, "down", move(index + 1), index === count - 1),
      action("remove", `Retirer ${media.title} de la file`, "close", () => {
        focusAfterDraw = { index, action: "remove" };
        api.queueRemove(index);
      }),
    ]),
  ]);
}

/** @param {import("./api.js").Queue} queue */
function drawFallback(queue) {
  const radio = queue.lastRadio;
  const text = element("span", { class: "station-text" }, [
    element("span", { class: "station-detail" }, ["Quand la file est vide"]),
    element("span", { class: "queue-title" }, [
      !radio
        ? "La lecture s'arrête. Écoutez une radio pour qu'elle reprenne ici."
        : queue.resumeRadio
          ? `Reprendre la radio ${radio.title}`
          : "La lecture s'arrête",
    ]),
  ]);
  const content = [icon("radio"), text];
  if (radio) {
    const toggle = element(
      "button",
      {
        class: "switch",
        type: "button",
        role: "switch",
        "aria-checked": String(queue.resumeRadio),
        "aria-label": `Reprendre la radio ${radio.title} quand la file est vide`,
      },
      [element("span", { class: "switch-knob" })],
    );
    toggle.addEventListener("click", () => api.queueSetResumeRadio(!queue.resumeRadio));
    content.push(toggle);
  }
  fallback.dataset.active = String(Boolean(radio) && queue.resumeRadio);
  fallback.replaceChildren(...content);
}

toggleButton.addEventListener("click", () => setOpen(panel.hidden));
clearButton.addEventListener("click", () => api.queueClear());

setOpen(wasOpen());
