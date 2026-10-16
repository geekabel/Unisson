/**
 * The keyboard shortcuts of the main window, as the user reads them.
 *
 * This list is only text: `shortcuts.js` and `search-box.js` hold the
 * behavior. It lives apart so that the help window can show it without
 * loading them.
 */

import { t } from "./i18n.js";

const SPACE = t("shell.keySpace");
const SHIFT = t("shell.keyShift");
const ENTER = t("shell.keyEnter");
const ESCAPE = t("shell.keyEscape");

/**
 * Groups of shortcuts. Each shortcut has its keys, one array per
 * alternative, and what it does.
 *
 * @type {{title: string, items: {keys: string[][], text: string}[]}[]}
 */
export const SHORTCUT_GROUPS = [
  {
    title: t("shell.shortcutsPlayback"),
    items: [
      { keys: [[SPACE]], text: t("shell.shortcutToggle") },
      { keys: [["Ctrl", "→"]], text: t("shell.nextTrack") },
      { keys: [["Ctrl", "←"]], text: t("shell.shortcutPrevious") },
      { keys: [["Ctrl", "↑"], ["Ctrl", "↓"]], text: t("shell.shortcutVolume") },
    ],
  },
  {
    title: t("shell.shortcutsMoving"),
    items: [
      { keys: [["Ctrl", "K"], ["/"]], text: t("shell.shortcutSearch") },
      { keys: [["Alt", "1"], ["…"], ["Alt", "6"]], text: t("shell.shortcutViews") },
      { keys: [["F6"], [SHIFT, "F6"]], text: t("shell.shortcutZones") },
      { keys: [["Tab"], [SHIFT, "Tab"]], text: t("shell.shortcutTab") },
      { keys: [["Ctrl", "+"], ["Ctrl", "-"], ["Ctrl", "0"]], text: t("shell.shortcutZoom") },
      { keys: [["F1"]], text: t("shell.shortcutHelp") },
    ],
  },
  {
    title: t("shell.shortcutsSearch"),
    items: [
      { keys: [["↑"], ["↓"]], text: t("shell.shortcutChoose") },
      { keys: [[ENTER]], text: t("shell.shortcutPlayNow") },
      { keys: [[SHIFT, ENTER]], text: t("shell.shortcutEnqueue") },
      { keys: [["Ctrl", ENTER]], text: t("shell.shortcutPlayNext") },
      { keys: [[ESCAPE]], text: t("shell.shortcutClear") },
    ],
  },
  {
    title: t("shell.shortcutsOrdering"),
    items: [{ keys: [["Alt", "↑"], ["Alt", "↓"]], text: t("shell.shortcutReorder") }],
  },
];
