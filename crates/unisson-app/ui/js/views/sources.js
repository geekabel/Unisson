/**
 * The "Sources and settings" view: which sources are used, in which order,
 * where the local music is, and how the player behaves.
 */

import * as api from "../api.js";
import { iconButton, section } from "../components.js";
import { element, icon } from "../dom.js";
import { countLabel, displayPath, moved, sourceName } from "../format.js";
import { showNotice } from "../notice.js";
import { currentSettings, onSettingsChange, useSettings } from "../settings.js";

const SOURCE_ICONS = { radio: "radio", local: "folder", youtube: "video" };

/** The area redrawn when the settings change. */
let area = element("div");

/**
 * Builds the view.
 *
 * @returns {Promise<Node[]>}
 */
export async function render() {
  area = element("div", { class: "settings" });
  draw();
  return [element("h1", { class: "view-title" }, ["Sources et réglages"]), area];
}

/**
 * Runs a change of settings, then shows the settings the backend returns.
 *
 * @param {() => Promise<import("../api.js").Settings>} change
 */
async function apply(change) {
  try {
    useSettings(await change());
  } catch {
    showNotice({
      owner: "settings",
      tone: "error",
      text: "Ce réglage n'a pas pu être enregistré.",
      duration: 8000,
    });
  }
}

function draw() {
  const settings = currentSettings();
  if (!settings) {
    return;
  }
  // Keep the focus on the same control across the redraw.
  const focused = document.activeElement?.dataset?.focusKey;
  area.replaceChildren(
    section("Sources", [
      element("div", { class: "source-cards" }, settings.sources.map((source) => {
        return sourceCard(source, settings);
      })),
    ]),
    section("Priorité des sources", [
      element("p", { class: "hint" }, [
        "Si un titre existe dans plusieurs sources, la première de la liste est utilisée.",
      ]),
      element("ol", { class: "priority" }, settings.sources.map((source, index) => {
        return priorityRow(source, index, settings);
      })),
    ]),
    section("Comportement", [
      element("div", { class: "setting-row" }, [
        element("span", { class: "setting-label", id: "resume-radio-label" }, [
          "Quand la file est vide, reprendre la dernière radio",
        ]),
        toggle({
          key: "resume-radio",
          checked: settings.resumeRadio,
          labelledBy: "resume-radio-label",
          onChange: async (checked) => {
            await api.queueSetResumeRadio(checked);
            return api.settingsGet();
          },
        }),
      ]),
    ]),
    settings.configPath
      ? element("p", { class: "section-note" }, [
          `Réglages enregistrés dans ${displayPath(settings.configPath)}`,
        ])
      : element("p", { class: "section-note" }, ["Les réglages ne peuvent pas être enregistrés."]),
  );
  if (focused) {
    area.querySelector(`[data-focus-key="${focused}"]`)?.focus();
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
 * @param {(checked: boolean) => Promise<import("../api.js").Settings>} options.onChange
 */
function toggle({ key, checked, label, labelledBy, onChange }) {
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
  button.addEventListener("click", () => apply(() => onChange(!checked)));
  return button;
}

/**
 * @param {{id: string, enabled: boolean}} source
 * @param {import("../api.js").Settings} settings
 */
function sourceCard(source, settings) {
  const status = source.enabled
    ? "Active. Aucun compte requis."
    : "Coupée. Ses résultats n'apparaissent pas dans la recherche.";
  return element("div", { class: "source-card", "data-enabled": String(source.enabled) }, [
    element("div", { class: "source-card-head" }, [
      element("span", { class: "cover", "data-source": source.id }, [icon(SOURCE_ICONS[source.id] ?? "note")]),
      element("span", { class: "station-text" }, [
        element("span", { class: "source-name" }, [sourceName(source.id)]),
        element("span", { class: "station-detail" }, [status]),
      ]),
      toggle({
        key: `source-${source.id}`,
        checked: source.enabled,
        label: `Activer la source ${sourceName(source.id)}`,
        onChange: (checked) => api.settingsSetSourceEnabled(source.id, checked),
      }),
    ]),
    source.id === "local" ? localDetails(settings) : null,
  ]);
}

/** @param {import("../api.js").Settings} settings */
function localDetails(settings) {
  const button = (key, label, run) => {
    const node = element("button", { class: "text-button", type: "button", "data-focus-key": key }, [label]);
    node.addEventListener("click", run);
    return node;
  };
  const folder = settings.musicFolder
    ? `${displayPath(settings.musicFolder)}${settings.musicFolderIsDefault ? " (dossier Musique du bureau)" : ""}`
    : "aucun dossier connu";
  return element("div", { class: "source-card-details" }, [
    element("p", {}, [
      element("span", { class: "station-detail" }, ["Dossier : "]),
      element("span", { class: "path" }, [folder]),
    ]),
    settings.musicFolder && !settings.musicFolderExists
      ? element("p", { class: "search-error", role: "alert" }, ["Ce dossier est introuvable."])
      : element("p", { class: "station-detail" }, [
          `${countLabel(settings.libraryTotal, "titre")} dans la bibliothèque.`,
        ]),
    element("div", { class: "inline-form" }, [
      button("pick-folder", "Changer de dossier", () => apply(api.settingsPickMusicFolder)),
      settings.musicFolderIsDefault
        ? null
        : button("reset-folder", "Revenir au dossier Musique", () => apply(api.settingsResetMusicFolder)),
      button("rescan", "Analyser à nouveau", async () => {
        await api.settingsRescanLibrary();
        showNotice({ owner: "settings", text: "Analyse du dossier en cours.", duration: 3000 });
      }),
    ]),
  ]);
}

/**
 * @param {{id: string, enabled: boolean}} source
 * @param {number} index
 * @param {import("../api.js").Settings} settings
 */
function priorityRow(source, index, settings) {
  const ids = settings.sources.map((other) => other.id);
  const move = (step, label, iconName) => {
    const button = iconButton({
      icon: iconName,
      label: `${label} ${sourceName(source.id)}`,
      onClick: () => apply(() => api.settingsSetPriority(moved(ids, index, step))),
    });
    button.dataset.focusKey = `priority-${source.id}-${step}`;
    button.disabled = moved(ids, index, step) === ids;
    return button;
  };
  return element("li", { class: "priority-row", "data-enabled": String(source.enabled) }, [
    element("span", { class: "queue-number" }, [String(index + 1)]),
    element("span", { class: "priority-name" }, [
      sourceName(source.id),
      source.enabled ? null : element("span", { class: "station-detail" }, [" (coupée)"]),
    ]),
    move(-1, "Monter", "up"),
    move(1, "Descendre", "down"),
  ]);
}

onSettingsChange(() => {
  if (area.isConnected) {
    draw();
  }
});
