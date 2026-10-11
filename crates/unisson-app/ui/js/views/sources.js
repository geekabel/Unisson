/**
 * The "Sources and settings" view: which sources are used, in which order,
 * where the local music is, and how the player behaves.
 */

import * as api from "../api.js";
import { iconButton, section } from "../components.js";
import { element, icon } from "../dom.js";
import { displayPath, moved, sourceName } from "../format.js";
import { plural, t } from "../i18n.js";
import { showNotice } from "../notice.js";
import { SHORTCUT_GROUPS } from "../shortcut-list.js";
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
  return [element("h1", { class: "view-title" }, [t("sources.title")]), area];
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
      text: t("sources.settingNotSaved"),
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
    section(t("sources.sectionSources"), [
      element("div", { class: "source-cards" }, settings.sources.map((source) => {
        return sourceCard(source, settings);
      })),
    ]),
    section(t("sources.sectionPriority"), [
      element("p", { class: "hint" }, [t("sources.priorityHint")]),
      element("ol", { class: "priority" }, settings.sources.map((source, index) => {
        return priorityRow(source, index, settings);
      })),
    ]),
    section(t("sources.sectionBehavior"), [
      settingRow({
        key: "resume-radio",
        label: t("sources.resumeRadio"),
        checked: settings.resumeRadio,
        onChange: async (checked) => {
          await api.queueSetResumeRadio(checked);
          return api.settingsGet();
        },
      }),
      settingRow({
        key: "keep-in-tray",
        label: t("sources.keepInTray"),
        detail: t("sources.keepInTrayDetail"),
        checked: settings.keepInTray,
        onChange: (checked) => api.settingsSetKeepInTray(checked),
      }),
      settingRow({
        key: "notifications",
        label: t("sources.notifications"),
        detail: t("sources.notificationsDetail"),
        checked: settings.notifications,
        onChange: (checked) => api.settingsSetNotifications(checked),
      }),
    ]),
    section(t("sources.sectionAppearance"), [
      themeSetting(settings.theme),
      languageSetting(settings.language),
    ]),
    section(t("sources.sectionLauncher"), shortcutSettings(settings.shortcut)),
    section(t("sources.sectionShortcuts"), shortcutHelp()),
    // Only worth a line when something is wrong.
    ...(settings.configPath
      ? []
      : [element("p", { class: "section-note" }, [t("sources.settingsNotSavable")])]),
  );
  if (focused) {
    area.querySelector(`[data-focus-key="${focused}"]`)?.focus();
  }
}

/** Builds the list of the keyboard shortcuts of the main window. */
function shortcutHelp() {
  return SHORTCUT_GROUPS.map((group) => {
    return element("div", { class: "shortcut-group" }, [
      element("h3", { class: "group-title" }, [group.title]),
      element("dl", { class: "shortcut-list" }, group.items.flatMap((item) => [
        element("dt", {}, item.keys.flatMap((keys, index) => [
          index > 0 ? " " : null,
          element("kbd", {}, [keys.join(" ")]),
        ])),
        element("dd", {}, [item.text]),
      ])),
    ]);
  });
}

/** The themes offered, with their name for the user. */
const THEMES = [
  ["system", t("sources.sameAsSystem")],
  ["dark", t("sources.themeDark")],
  ["light", t("sources.themeLight")],
];

/**
 * Builds the choice of the colors of the interface.
 *
 * @param {string} theme The current choice.
 */
function themeSetting(theme) {
  const select = /** @type {HTMLSelectElement} */ (
    element("select", { class: "field field-compact", "aria-labelledby": "theme-label", "data-focus-key": "theme" },
      THEMES.map(([id, name]) => element("option", { value: id }, [name])))
  );
  select.value = THEMES.some(([id]) => id === theme) ? theme : "system";
  select.addEventListener("change", () => apply(() => api.settingsSetTheme(select.value)));
  return element("div", { class: "setting-row" }, [
    element("span", { class: "station-text" }, [
      element("span", { class: "setting-label", id: "theme-label" }, [t("sources.theme")]),
      element("span", { class: "station-detail" }, [t("sources.themeDetail")]),
    ]),
    select,
  ]);
}

/** The languages offered. Each language keeps its own name, whatever the language in use. */
const LANGUAGES = [
  ["system", t("sources.sameAsSystem")],
  ["fr", "Français"],
  ["en", "English"],
];

/**
 * Builds the choice of the language of the interface.
 *
 * @param {string} language The current choice.
 */
function languageSetting(language) {
  const select = /** @type {HTMLSelectElement} */ (
    element(
      "select",
      { class: "field field-compact", "aria-labelledby": "language-label", "data-focus-key": "language" },
      LANGUAGES.map(([id, name]) => element("option", { value: id }, [name])),
    )
  );
  select.value = LANGUAGES.some(([id]) => id === language) ? language : "system";
  select.addEventListener("change", () => apply(() => api.settingsSetLanguage(select.value)));
  return element("div", { class: "setting-row" }, [
    element("span", { class: "station-text" }, [
      element("span", { class: "setting-label", id: "language-label" }, [t("sources.language")]),
      element("span", { class: "station-detail" }, [t("sources.languageDetail")]),
    ]),
    select,
  ]);
}

/**
 * Builds the settings of the keyboard shortcut that opens the launcher.
 *
 * @param {import("../api.js").Settings["shortcut"]} shortcut
 * @returns {Node[]}
 */
function shortcutSettings(shortcut) {
  const intro = element("p", { class: "hint" }, [t("sources.launcherIntro")]);
  const open = element("button", { class: "text-button", type: "button", "data-focus-key": "open-launcher" }, [
    t("sources.openLauncher"),
  ]);
  open.addEventListener("click", () => api.windowShowLauncher());

  if (!shortcut.supported) {
    // Not GNOME: the user binds the command in the settings of their desktop.
    return [
      intro,
      element("p", { class: "station-detail" }, [t("sources.shortcutManual")]),
      element("p", { class: "path" }, [shortcut.command]),
      element("div", { class: "inline-form" }, [open]),
    ];
  }

  const select = /** @type {HTMLSelectElement} */ (
    element(
      "select",
      { class: "field", "aria-label": t("sources.shortcutChoice"), "data-focus-key": "shortcut-choice" },
      shortcut.choices.map(([binding, name]) => element("option", { value: binding }, [name])),
    )
  );
  if (shortcut.binding && shortcut.choices.some(([binding]) => binding === shortcut.binding)) {
    select.value = shortcut.binding;
  }
  const install = element(
    "button",
    { class: "text-button text-button-strong", type: "button", "data-focus-key": "install-shortcut" },
    [shortcut.binding ? t("sources.changeShortcut") : t("sources.installShortcut")],
  );
  install.addEventListener("click", () => apply(() => api.settingsInstallShortcut(select.value)));
  const controls = [select, install];
  if (shortcut.binding) {
    const remove = element("button", { class: "text-button", type: "button", "data-focus-key": "remove-shortcut" }, [
      t("sources.removeShortcut"),
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
        ? t("sources.shortcutInPlace", { name })
        : t("sources.noShortcut"),
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
    ? t("sources.statusOff")
    : missingTool
      ? t("sources.statusMissingTool")
      : t("sources.statusOn");
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
        label: t("sources.enableSource", { name: sourceName(source.id) }),
        onChange: (checked) => api.settingsSetSourceEnabled(source.id, checked),
      }),
    ]),
    source.id === "local" ? localDetails(settings) : null,
    source.id === "youtube" ? youtubeDetails(settings) : null,
  ]);
}

/** Where the installation guide is. It is shown as text, to copy. */
const INSTALL_GUIDE = "github.com/geekabel/Unisson#youtube";

/** @param {import("../api.js").Settings} settings */
function youtubeDetails(settings) {
  const check = element(
    "button",
    { class: "text-button", type: "button", "data-focus-key": "check-yt-dlp" },
    [t("sources.checkAgain")],
  );
  check.addEventListener("click", () => apply(api.settingsCheckYtDlp));
  if (settings.ytDlpVersion) {
    return element("div", { class: "source-card-details" }, [
      element("p", { class: "station-detail" }, [
        t("sources.componentInstalled", { version: settings.ytDlpVersion }),
      ]),
    ]);
  }
  return element("div", { class: "source-card-details" }, [
    element("p", { class: "search-error", role: "alert" }, [t("sources.componentMissing")]),
    element("p", { class: "station-detail" }, [
      t("sources.installGuide"),
      element("span", { class: "path" }, [INSTALL_GUIDE]),
    ]),
    element("div", { class: "inline-form" }, [check]),
  ]);
}

/** @param {import("../api.js").Settings} settings */
function localDetails(settings) {
  const button = (key, label, run) => {
    const node = element("button", { class: "text-button", type: "button", "data-focus-key": key }, [label]);
    node.addEventListener("click", run);
    return node;
  };
  const path = settings.musicFolder ? displayPath(settings.musicFolder) : "";
  const folder = !settings.musicFolder
    ? t("sources.noFolder")
    : settings.musicFolderIsDefault
      ? t("sources.folderDefault", { path })
      : path;
  return element("div", { class: "source-card-details" }, [
    element("p", {}, [
      element("span", { class: "station-detail" }, [t("sources.folder")]),
      element("span", { class: "path" }, [folder]),
    ]),
    settings.musicFolder && !settings.musicFolderExists
      ? element("p", { class: "search-error", role: "alert" }, [t("sources.folderMissing")])
      : element("p", { class: "station-detail" }, [plural(settings.libraryTotal, "sources.tracksInLibrary")]),
    element("div", { class: "inline-form" }, [
      button("pick-folder", t("sources.changeFolder"), () => apply(api.settingsPickMusicFolder)),
      settings.musicFolderIsDefault
        ? null
        : button("reset-folder", t("sources.resetFolder"), () => apply(api.settingsResetMusicFolder)),
      button("rescan", t("sources.rescan"), async () => {
        await api.settingsRescanLibrary();
        showNotice({ owner: "settings", text: t("sources.rescanStarted"), duration: 3000 });
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
  const name = sourceName(source.id);
  const move = (step, label, iconName) => {
    const button = iconButton({
      icon: iconName,
      label,
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
      source.enabled ? null : element("span", { class: "station-detail" }, [t("sources.sourceOff")]),
    ]),
    move(-1, t("sources.moveUp", { name }), "up"),
    move(1, t("sources.moveDown", { name }), "down"),
  ]);
}

onSettingsChange(() => {
  if (area.isConnected) {
    draw();
  }
});
