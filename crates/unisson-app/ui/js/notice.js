/**
 * The message strip above the player: errors, reconnections, short hints.
 *
 * Only one message shows at a time. Each message has an owner, so that a
 * part of the interface can remove its own message without erasing the
 * message of another part.
 */

import { byId } from "./dom.js";

const strip = byId("notice");
const text = byId("notice-text");
const actionButton = byId("notice-action");
const closeButton = byId("notice-close");

let owner = null;
let onAction = null;
let timer = null;

/**
 * Shows a message.
 *
 * @param {object} options
 * @param {string} options.owner Who shows it: `player`, `search`...
 * @param {string} options.text
 * @param {"info"|"error"} [options.tone]
 * @param {string} [options.detail] A technical reason, shown as a tooltip.
 * @param {{label: string, run: () => void}} [options.action] An optional button.
 * @param {number} [options.duration] Hides the message after that many
 *   milliseconds. Without it, the message stays.
 */
export function showNotice({ owner: newOwner, text: message, tone = "info", detail, action, duration }) {
  clearTimeout(timer);
  owner = newOwner;
  text.textContent = message;
  text.title = detail ?? "";
  strip.dataset.tone = tone;
  // An error interrupts a screen reader. Anything else waits for its turn.
  strip.setAttribute("role", tone === "error" ? "alert" : "status");
  onAction = action?.run ?? null;
  actionButton.hidden = !action;
  actionButton.textContent = action?.label ?? "";
  strip.hidden = false;
  if (duration) {
    timer = setTimeout(() => hideNotice(newOwner), duration);
  }
}

/**
 * Hides the message, if it belongs to `fromOwner`.
 *
 * @param {string} [fromOwner] Without it, any message is hidden.
 */
export function hideNotice(fromOwner) {
  if (fromOwner && fromOwner !== owner) {
    return;
  }
  clearTimeout(timer);
  owner = null;
  onAction = null;
  strip.hidden = true;
}

actionButton.addEventListener("click", () => onAction?.());
closeButton.addEventListener("click", () => hideNotice());
