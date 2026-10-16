/**
 * Controls shared by the Sources and Settings screens: a switch, a line
 * with a label and its switch, and the way a change is sent.
 */

import { element } from "./dom.js";
import { showNotice } from "./notice.js";
import { useSettings } from "./settings.js";

/**
 * Runs a change of settings, then shows the settings the backend returns.
 *
 * @param {() => Promise<import("./api.js").Settings>} change
 * @param {string} failedText What to tell when the change fails.
 */
export async function apply(change, failedText) {
  try {
    useSettings(await change());
  } catch {
    showNotice({ owner: "settings", tone: "error", text: failedText, duration: 8000 });
  }
}

/**
 * Builds a switch.
 *
 * @param {object} options
 * @param {string} options.key Identifies the switch, to give it the focus
 *   back after a redraw.
 * @param {boolean} options.checked
 * @param {string} [options.label]
 * @param {string} [options.labelledBy] Id of the element that names it.
 * @param {boolean} [options.disabled]
 * @param {(checked: boolean) => void} options.onChange
 */
export function toggle({ key, checked, label, labelledBy, disabled = false, onChange }) {
  const button = element(
    "button",
    {
      class: "switch",
      type: "button",
      role: "switch",
      "aria-checked": String(checked),
      "aria-label": label,
      "aria-labelledby": labelledBy,
      "data-focus-key": key,
    },
    [element("span", { class: "switch-knob" })],
  );
  button.disabled = disabled;
  button.addEventListener("click", () => onChange(!checked));
  return button;
}

/**
 * Builds a line with a label, an optional second line, and a control.
 *
 * @param {object} options
 * @param {string} options.key Identifies the line; its label gets the id
 *   `<key>-label`.
 * @param {string} options.label
 * @param {string} [options.detail]
 * @param {Node} options.control
 */
export function settingRow({ key, label, detail, control }) {
  return element("div", { class: "setting-row" }, [
    element("span", { class: "station-text" }, [
      element("span", { class: "setting-label", id: `${key}-label` }, [label]),
      detail ? element("span", { class: "station-detail" }, [detail]) : null,
    ]),
    control,
  ]);
}
