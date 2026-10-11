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
      settingRow({
        key: "resume-radio",
        label: "Quand la file est vide, reprendre la dernière radio",
        checked: settings.resumeRadio,
        onChange: async (checked) => {
          await api.queueSetResumeRadio(checked);
          return api.settingsGet();
        },
      }),
      settingRow({
        key: "keep-in-tray",
        label: "À la fermeture de la fenêtre, continuer la lecture",
        detail: "L'icône de la barre du haut rouvre la fenêtre, et son menu permet de quitter.",
        checked: settings.keepInTray,
        onChange: (checked) => api.settingsSetKeepInTray(checked),
      }),
    ]),
    section("Lanceur rapide", shortcutSettings(settings.shortcut)),
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
 * Builds the settings of the keyboard shortcut that opens the launcher.
 *
 * @param {import("../api.js").Settings["shortcut"]} shortcut
 * @returns {Node[]}
 */
function shortcutSettings(shortcut) {
  const intro = element("p", { class: "hint" }, [
    "Le lanceur est une petite fenêtre de recherche qui s'ouvre par-dessus votre travail : " +
      "quelques lettres, Entrée, et la lecture démarre.",
  ]);
  const open = element("button", { class: "text-button", type: "button", "data-focus-key": "open-launcher" }, [
    "Ouvrir le lanceur",
  ]);
  open.addEventListener("click", () => api.windowShowLauncher());

  if (!shortcut.supported) {
    // Not GNOME: the user binds the command in the settings of their desktop.
    return [
      intro,
      element("p", { class: "station-detail" }, [
        "Pour l'ouvrir au clavier, créez dans les réglages de votre bureau un raccourci qui lance cette commande :",
      ]),
      element("p", { class: "path" }, [shortcut.command]),
      element("div", { class: "inline-form" }, [open]),
    ];
  }

  const select = /** @type {HTMLSelectElement} */ (
    element("select", { class: "field", "aria-label": "Raccourci clavier", "data-focus-key": "shortcut-choice" },
      shortcut.choices.map(([binding, name]) => element("option", { value: binding }, [name])))
  );
  if (shortcut.binding && shortcut.choices.some(([binding]) => binding === shortcut.binding)) {
    select.value = shortcut.binding;
  }
  const install = element(
    "button",
    { class: "text-button text-button-strong", type: "button", "data-focus-key": "install-shortcut" },
    [shortcut.binding ? "Changer le raccourci" : "Installer le raccourci"],
  );
  install.addEventListener("click", () => apply(() => api.settingsInstallShortcut(select.value)));
  const controls = [select, install];
  if (shortcut.binding) {
    const remove = element("button", { class: "text-button", type: "button", "data-focus-key": "remove-shortcut" }, [
      "Retirer",
    ]);
    remove.addEventListener("click", () => apply(api.settingsRemoveShortcut));
    controls.push(remove);
  }
  controls.push(open);

  const name = shortcut.choices.find(([binding]) => binding === shortcut.binding)?.[1] ?? shortcut.binding;
  return [
    intro,
    element("p", { class: "station-detail" }, [
      shortcut.binding
        ? `Raccourci en place : ${name}. Il apparaît aussi dans les réglages de GNOME, sous Clavier.`
        : "Aucun raccourci pour l'instant. Unisson peut l'ajouter aux raccourcis personnalisés de GNOME.",
    ]),
    element("div", { class: "inline-form" }, controls),
  ];
}

/**
 * Builds one line of the behavior settings: a label and its switch.
 *
 * @param {object} options
 * @param {string} options.key
 * @param {string} options.label
 * @param {string} [options.detail] A second line, to explain.
 * @param {boolean} options.checked
 * @param {(checked: boolean) => Promise<import("../api.js").Settings>} options.onChange
 */
function settingRow({ key, label, detail, checked, onChange }) {
  return element("div", { class: "setting-row" }, [
    element("span", { class: "station-text" }, [
      element("span", { class: "setting-label", id: `${key}-label` }, [label]),
      detail ? element("span", { class: "station-detail" }, [detail]) : null,
    ]),
    toggle({ key, checked, labelledBy: `${key}-label`, onChange }),
  ]);
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
  const missingTool = source.id === "youtube" && !settings.ytDlpVersion;
  const status = !source.enabled
    ? "Coupée. Ses résultats n'apparaissent pas dans la recherche."
    : missingTool
      ? "Active, mais le programme yt-dlp est introuvable."
      : "Active. Aucun compte requis.";
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
    source.id === "youtube" ? youtubeDetails(settings) : null,
  ]);
}

/** @param {import("../api.js").Settings} settings */
function youtubeDetails(settings) {
  const check = element(
    "button",
    { class: "text-button", type: "button", "data-focus-key": "check-yt-dlp" },
    ["Vérifier à nouveau"],
  );
  check.addEventListener("click", () => apply(api.settingsCheckYtDlp));
  const note = element("p", { class: "station-detail" }, [
    "Le son est extrait des vidéos par yt-dlp. Cette méthode n'est pas prévue par les conditions " +
      "d'utilisation de YouTube, et elle peut cesser de marcher tant que yt-dlp n'est pas mis à jour.",
  ]);
  if (settings.ytDlpVersion) {
    return element("div", { class: "source-card-details" }, [
      element("p", {}, [
        element("span", { class: "station-detail" }, ["Programme : "]),
        element("span", { class: "path" }, [`yt-dlp ${settings.ytDlpVersion}`]),
      ]),
      note,
    ]);
  }
  return element("div", { class: "source-card-details" }, [
    element("p", { class: "search-error", role: "alert" }, [
      "YouTube ne peut pas être utilisé sans yt-dlp. Installez-le avec cette commande, dans un terminal :",
    ]),
    element("p", { class: "path" }, ['pipx install "yt-dlp[default]"']),
    element("div", { class: "inline-form" }, [check]),
    note,
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
