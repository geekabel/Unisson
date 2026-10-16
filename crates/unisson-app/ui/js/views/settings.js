/**
 * The "Settings" view: how the player behaves, how it looks, and the
 * keyboard.
 */

import * as api from "../api.js";
import { section } from "../components.js";
import { element, icon } from "../dom.js";
import { t } from "../i18n.js";
import { apply, settingRow, toggle } from "../setting-controls.js";
import { openShortcuts } from "../shortcut-dialog.js";
import { currentSettings, onSettingsChange } from "../settings.js";

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
  return [element("h1", { class: "view-title" }, [t("settings.title")]), area];
}

/** @param {() => Promise<import("../api.js").Settings>} change */
const save = (change) => apply(change, t("settings.settingNotSaved"));

function draw() {
  const settings = currentSettings();
  if (!settings) {
    return;
  }
  const focused = document.activeElement?.dataset?.focusKey;
  const switchRow = (key, label, detail, checked, change) =>
    settingRow({
      key,
      label,
      detail,
      control: toggle({ key, checked, labelledBy: `${key}-label`, onChange: (value) => save(() => change(value)) }),
    });
  area.replaceChildren(
    section(t("settings.sectionPlayback"), [
      element("div", { class: "boxed-list" }, [
        switchRow("resume-radio", t("settings.resumeRadio"), null, settings.resumeRadio, async (checked) => {
          await api.queueSetResumeRadio(checked);
          return api.settingsGet();
        }),
        switchRow("keep-in-tray", t("settings.keepInTray"), t("settings.keepInTrayDetail"), settings.keepInTray, api.settingsSetKeepInTray),
        switchRow(
          "notifications",
          t("settings.notifications"),
          t("settings.notificationsDetail"),
          settings.notifications,
          api.settingsSetNotifications,
        ),
      ]),
    ]),
    section(t("settings.sectionAppearance"), [
      element("div", { class: "boxed-list" }, [themeSetting(settings.theme), languageSetting(settings.language)]),
    ]),
    section(t("settings.sectionKeyboard"), [
      element("div", { class: "boxed-list" }, [launcherSetting(settings.shortcut), shortcutsRow()]),
    ]),
    settings.configPath
      ? element("p", { class: "section-note" }, [
        t("settings.savedIn"),
        element("span", { class: "path" }, [settings.configPath]),
      ])
      : element("p", { class: "section-note" }, [t("settings.settingsNotSavable")]),
  );
  if (focused) {
    area.querySelector(`[data-focus-key="${focused}"]`)?.focus();
  }
}

/** The themes offered, with their name for the user. */
const THEMES = [
  ["system", t("settings.sameAsSystem")],
  ["dark", t("settings.themeDark")],
  ["light", t("settings.themeLight")],
];

/**
 * Builds the choice of the colors: three buttons, one click each.
 *
 * @param {string} theme The current choice.
 */
function themeSetting(theme) {
  const current = THEMES.some(([id]) => id === theme) ? theme : "system";
  const buttons = THEMES.map(([id, name]) => {
    const button = element(
      "button",
      {
        class: "segment",
        type: "button",
        role: "radio",
        "aria-checked": String(id === current),
        // Only the chosen one takes Tab; the arrows move between them.
        tabindex: id === current ? "0" : "-1",
        "data-focus-key": `theme-${id}`,
      },
      [name],
    );
    button.addEventListener("click", () => save(() => api.settingsSetTheme(id)));
    return button;
  });
  const group = element(
    "div",
    { class: "segmented", role: "radiogroup", "aria-labelledby": "theme-label" },
    buttons,
  );
  group.addEventListener("keydown", (event) => {
    const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];
    if (!step) {
      return;
    }
    event.preventDefault();
    const index = THEMES.findIndex(([id]) => id === current);
    const next = THEMES[(index + step + THEMES.length) % THEMES.length][0];
    save(() => api.settingsSetTheme(next)).then(() => area.querySelector(`[data-focus-key="theme-${next}"]`)?.focus());
  });
  return settingRow({ key: "theme", label: t("settings.theme"), detail: t("settings.themeDetail"), control: group });
}

/** The languages offered. Each language keeps its own name. */
const LANGUAGES = [
  ["system", t("settings.sameAsSystem")],
  ["fr", "Français"],
  ["en", "English"],
];

/** @param {string} language The current choice. */
function languageSetting(language) {
  const select = /** @type {HTMLSelectElement} */ (
    element(
      "select",
      { class: "field field-compact", "aria-labelledby": "language-label", "data-focus-key": "language" },
      LANGUAGES.map(([id, name]) => element("option", { value: id }, [name])),
    )
  );
  select.value = LANGUAGES.some(([id]) => id === language) ? language : "system";
  select.addEventListener("change", () => save(() => api.settingsSetLanguage(select.value)));
  return settingRow({ key: "language", label: t("settings.language"), detail: t("settings.languageDetail"), control: select });
}

/**
 * Builds the line of the shortcut that opens the quick launcher.
 *
 * @param {import("../api.js").Settings["shortcut"]} shortcut
 */
function launcherSetting(shortcut) {
  const button = (key, label, run, strong = false) => {
    const node = element(
      "button",
      { class: strong ? "text-button text-button-strong" : "text-button", type: "button", "data-focus-key": key },
      [label],
    );
    node.addEventListener("click", run);
    return node;
  };
  const tryIt = button("open-launcher", t("settings.openLauncher"), () => api.windowShowLauncher());
  if (!shortcut.supported) {
    // Not GNOME: the user binds the command in the settings of their desktop.
    return element("div", { class: "setting-row setting-row-stack" }, [
      element("span", { class: "station-text" }, [
        element("span", { class: "setting-label" }, [t("settings.launcher")]),
        element("span", { class: "station-detail" }, [t("settings.shortcutManual")]),
        element("span", { class: "path" }, [shortcut.command]),
      ]),
      element("div", { class: "inline-form" }, [tryIt]),
    ]);
  }
  const select = /** @type {HTMLSelectElement} */ (
    element(
      "select",
      { class: "field field-compact", "aria-label": t("settings.shortcutChoice"), "data-focus-key": "shortcut-choice" },
      shortcut.choices.map(([binding, name]) => element("option", { value: binding }, [name])),
    )
  );
  if (shortcut.binding && shortcut.choices.some(([binding]) => binding === shortcut.binding)) {
    select.value = shortcut.binding;
  }
  const name = shortcut.choices.find(([binding]) => binding === shortcut.binding)?.[1] ?? shortcut.binding;
  const controls = [
    select,
    button(
      "install-shortcut",
      shortcut.binding ? t("settings.changeShortcut") : t("settings.installShortcut"),
      () => save(() => api.settingsInstallShortcut(select.value)),
      !shortcut.binding,
    ),
    shortcut.binding ? button("remove-shortcut", t("settings.removeShortcut"), () => save(api.settingsRemoveShortcut)) : null,
    tryIt,
  ];
  return element("div", { class: "setting-row setting-row-stack" }, [
    element("span", { class: "station-text" }, [
      element("span", { class: "setting-label" }, [t("settings.launcher")]),
      element("span", { class: "station-detail" }, [
        t("settings.launcherIntro"),
        " ",
        shortcut.binding ? t("settings.shortcutInPlace", { name }) : t("settings.noShortcut"),
      ]),
    ]),
    element("div", { class: "inline-form" }, controls),
  ]);
}

/** The line that opens the list of the keyboard shortcuts. */
function shortcutsRow() {
  const open = element("button", { class: "text-button", type: "button", "data-focus-key": "show-shortcuts" }, [
    icon("keyboard"),
    t("settings.showShortcuts"),
  ]);
  open.addEventListener("click", openShortcuts);
  return settingRow({
    key: "all-shortcuts",
    label: t("settings.allShortcuts"),
    detail: t("settings.allShortcutsDetail"),
    control: open,
  });
}

onSettingsChange(() => {
  if (area.isConnected) {
    draw();
  }
});
