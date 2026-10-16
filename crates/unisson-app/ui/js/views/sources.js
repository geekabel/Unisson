/**
 * The "Sources" view: which sources are used, their accounts, and in which
 * order.
 *
 * The Spotify card follows where Spotify plays: inside Unisson, through the
 * official application as a backup, or not at all. Its states are drawn in
 * the design canvas "Unisson, écran Sources et bascule Spotify".
 */

import * as api from "../api.js";
import { iconButton, section } from "../components.js";
import { element, icon } from "../dom.js";
import { displayPath, moved, sourceName } from "../format.js";
import { plural, t } from "../i18n.js";
import { hideNotice, showNotice } from "../notice.js";
import { apply, settingRow, toggle } from "../setting-controls.js";
import { currentSettings, onSettingsChange, useSettings } from "../settings.js";

const SOURCE_ICONS = { radio: "radio", local: "folder", youtube: "video", spotify: "note" };

/** Where the developer site of Spotify is, shown as text too. */
const SPOTIFY_DASHBOARD = "developer.spotify.com/dashboard";

/** The area redrawn when the settings change. */
let area = element("div");
/** Where moves in the order list are announced to screen readers. */
let announcer = element("p");

/**
 * What the Spotify card remembers across redraws: a sign-in waiting for
 * the browser, the link of its page, the backup section opened or not, and
 * what was typed.
 */
const spotifyUi = {
  waiting: false,
  link: /** @type {?string} */ (null),
  backupOpen: false,
  savingBackup: false,
  clientId: "",
};

/** @param {() => Promise<import("../api.js").Settings>} change */
const save = (change) => apply(change, t("sources.settingNotSaved"));

/**
 * Builds the view.
 *
 * @returns {Promise<Node[]>}
 */
export async function render() {
  area = element("div", { class: "settings" });
  announcer = element("p", { class: "visually-hidden", role: "status", "aria-live": "polite" });
  draw();
  return [
    element("div", { class: "view-head" }, [
      element("h1", { class: "view-title" }, [t("sources.title")]),
      element("p", { class: "hint" }, [t("sources.intro")]),
    ]),
    area,
    announcer,
  ];
}

function draw() {
  const settings = currentSettings();
  if (!settings) {
    return;
  }
  // Keep the focus on the same control across the redraw.
  const focused = document.activeElement?.dataset?.focusKey;
  const parts = [
    spotifyBanner(settings),
    element("div", { class: "source-cards" }, settings.sources.map((source) => sourceCard(source, settings))),
    section(t("sources.sectionPriority"), [
      element("p", { class: "hint" }, [t("sources.priorityHint")]),
      priorityList(settings),
    ]),
  ];
  // `replaceChildren` would write a missing banner as the text "null".
  area.replaceChildren(...parts.filter(Boolean));
  if (focused) {
    area.querySelector(`[data-focus-key="${focused}"]`)?.focus();
  }
}

/**
 * @param {{id: string, enabled: boolean}} source
 * @param {import("../api.js").Settings} settings
 */
function sourceCard(source, settings) {
  return element("section", { class: "source-card", id: `source-${source.id}`, "data-enabled": String(source.enabled) }, [
    element("div", { class: "source-card-head" }, [
      element("span", { class: "cover", "data-source": source.id }, [icon(SOURCE_ICONS[source.id] ?? "note")]),
      element("span", { class: "station-text" }, [
        element("span", { class: "source-name" }, [sourceName(source.id)]),
        element("span", { class: "station-detail" }, [sourceStatus(source, settings)]),
      ]),
      toggle({
        key: `source-${source.id}`,
        checked: source.enabled,
        label: t("sources.enableSource", { name: sourceName(source.id) }),
        onChange: (checked) => save(() => api.settingsSetSourceEnabled(source.id, checked)),
      }),
    ]),
    {
      local: () => localDetails(settings),
      youtube: () => youtubeDetails(settings),
      spotify: () => spotifyDetails(settings.spotify),
    }[source.id]?.() ?? null,
  ]);
}

/**
 * The line under the name of a source.
 *
 * @param {{id: string, enabled: boolean}} source
 * @param {import("../api.js").Settings} settings
 */
function sourceStatus(source, settings) {
  if (!source.enabled) {
    return t("sources.statusOff");
  }
  if (source.id === "youtube" && !settings.ytDlpVersion) {
    return t("sources.statusMissingTool");
  }
  if (source.id === "spotify") {
    const { signedIn, backup, status } = settings.spotify;
    if (!signedIn && !backup.ready) {
      return t("sources.spotifySignedOut");
    }
    if (status.route === "builtin") {
      return t("sources.spotifyInUnisson");
    }
    if (status.route === "app") {
      return status.problem ? t("sources.spotifyViaApp") : t("sources.spotifyInApp");
    }
    return t("sources.spotifyStopped");
  }
  return t("sources.statusOn");
}

// ---------- Spotify ----------

/**
 * The banner at the top of the screen, only while Spotify cannot play at
 * all: a working backup is not worth one.
 *
 * @param {import("../api.js").Settings} settings
 */
function spotifyBanner(settings) {
  const enabled = settings.sources.find((source) => source.id === "spotify")?.enabled;
  const { status } = settings.spotify;
  const text = {
    limited: "sources.bannerStopped",
    offline: "sources.bannerStopped",
    accountRefused: "sources.bannerRefused",
    premiumRequired: "sources.bannerPremium",
  }[status.problem ?? ""];
  if (!enabled || status.route || !text) {
    return null;
  }
  const show = element("button", { class: "text-button", type: "button", "data-focus-key": "banner-spotify" }, [
    t("sources.bannerShow"),
  ]);
  show.addEventListener("click", () => {
    const card = area.querySelector("#source-spotify");
    card?.scrollIntoView({ block: "start" });
    /** @type {?HTMLElement} */ (card?.querySelector(".card-row-actions button"))?.focus();
  });
  return element("div", { class: "banner", "data-tone": "error", role: "status" }, [
    icon("alert"),
    element("span", { class: "banner-text" }, [t(text)]),
    show,
  ]);
}

/**
 * A line of the Spotify card: a label, what it says, and its actions.
 *
 * @param {string} label
 * @param {(Node|string|null)[]} value
 * @param {(Node|null)[]} [actions]
 */
function cardRow(label, value, actions = []) {
  return element("div", { class: "card-row" }, [
    element("span", { class: "card-row-label" }, [label]),
    element("span", { class: "card-row-value" }, value),
    actions.some(Boolean) ? element("span", { class: "card-row-actions" }, actions) : null,
  ]);
}

/**
 * @param {string} key
 * @param {string} label
 * @param {() => void} run
 * @param {{strong?: boolean, iconName?: string, disabled?: boolean}} [options]
 */
function button(key, label, run, { strong = false, iconName, disabled = false } = {}) {
  const node = element(
    "button",
    { class: strong ? "text-button text-button-strong" : "text-button", type: "button", "data-focus-key": key },
    [iconName ? icon(iconName) : null, label],
  );
  node.disabled = disabled;
  node.addEventListener("click", run);
  return node;
}

/**
 * A short label with a state: `ok`, `info` or `error`. The icon and the
 * words carry the meaning, not the color alone.
 *
 * @param {"ok"|"info"|"error"} tone
 * @param {string} text
 */
function chip(tone, text) {
  const iconName = { ok: "check", info: "swap", error: "alert" }[tone];
  return element("span", { class: "status-chip", "data-tone": tone }, [icon(iconName), text]);
}

/** Copies a text, and says so. */
async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    // An older engine: copy through a selected field.
    const field = element("textarea", { class: "visually-hidden" }, [text]);
    document.body.append(field);
    field.select();
    document.execCommand("copy");
    field.remove();
  }
  showNotice({ owner: "settings", text: t("sources.copied"), duration: 3000 });
}

/**
 * Tells when the built-in playback is tried again.
 *
 * @param {?number} retryAtMs
 */
function retryWhen(retryAtMs) {
  const minutes = Math.ceil(((retryAtMs ?? 0) - Date.now()) / 60_000);
  return minutes > 0 ? plural(minutes, "sources.retryIn") : t("sources.retrySoon");
}

/** Tells what went wrong, from the error of a command. */
function spotifyErrorText(error, fallback) {
  if (error?.kind === "invalid") {
    return t("sources.spotifyInvalidId");
  }
  if (error?.kind === "network") {
    return t("sources.spotifyNetwork");
  }
  return t(fallback);
}

/** A sign-in the user cancelled needs no message. */
function wasCancelled(error) {
  return String(error?.detail ?? "").includes("sign-in cancelled");
}

async function signIn() {
  hideNotice("settings");
  spotifyUi.waiting = true;
  spotifyUi.link = null;
  draw();
  try {
    const settings = await api.spotifySignIn();
    spotifyUi.waiting = false;
    useSettings(settings);
  } catch (error) {
    spotifyUi.waiting = false;
    draw();
    if (!wasCancelled(error)) {
      showNotice({ owner: "settings", tone: "error", text: spotifyErrorText(error, "sources.spotifySignInFailed"), duration: 10000 });
    }
  }
}

async function setUpBackup() {
  hideNotice("settings");
  spotifyUi.savingBackup = true;
  draw();
  try {
    const settings = await api.spotifySetUpBackup(spotifyUi.clientId.trim());
    spotifyUi.savingBackup = false;
    useSettings(settings);
  } catch (error) {
    spotifyUi.savingBackup = false;
    draw();
    if (!wasCancelled(error)) {
      showNotice({ owner: "settings", tone: "error", text: spotifyErrorText(error, "sources.backupFailed"), duration: 12000 });
    }
  }
}

/** Opens the backup section, and brings it into view. */
function openBackup() {
  spotifyUi.backupOpen = true;
  draw();
  const backup = area.querySelector("#spotify-backup");
  backup?.scrollIntoView({ block: "nearest" });
  /** @type {?HTMLElement} */ (backup?.querySelector("button"))?.focus();
}

/** @param {import("../api.js").Spotify} spotify */
function spotifyDetails(spotify) {
  const connected = spotify.signedIn || spotify.backup.ready;
  return element("div", { class: "source-card-details" }, [
    accountRow(spotify),
    connected ? playbackRow(spotify) : element("p", { class: "station-detail" }, [t("sources.spotifyAbout")]),
    connected ? backupSection(spotify) : null,
  ]);
}

/** @param {import("../api.js").Spotify} spotify */
function accountRow(spotify) {
  const label = t("sources.spotifyAccount");
  if (spotifyUi.waiting) {
    const actions = [
      spotifyUi.link
        ? button("spotify-copy-link", t("sources.spotifyCopyLink"), () => copyText(spotifyUi.link ?? ""), { iconName: "copy" })
        : null,
      button("spotify-cancel", t("sources.cancel"), () => api.spotifyCancelSignIn()),
    ];
    return cardRow(label, [
      element("span", { class: "card-row-title", role: "status" }, [
        element("span", { class: "spinner", "aria-hidden": "true" }),
        t("sources.spotifyWaiting"),
      ]),
      element("span", { class: "station-detail" }, [t("sources.spotifyWaitingHint")]),
    ], actions);
  }
  if (!spotify.signedIn) {
    return cardRow(label, [
      element("span", { class: "card-row-title" }, [t("sources.spotifyNoAccount")]),
      element("span", { class: "station-detail" }, [t("sources.spotifyPremiumNeeded")]),
    ], [button("spotify-sign-in", t("sources.spotifySignIn"), signIn, { strong: true })]);
  }
  return cardRow(label, [
    element("span", { class: "card-row-title" }, [t("sources.spotifySignedIn")]),
    element("span", { class: "station-detail" }, [t("sources.spotifySignedInHint")]),
  ], [button("spotify-sign-out", t("sources.spotifySignOut"), () => save(api.spotifySignOut))]);
}

/** @param {import("../api.js").Spotify} spotify */
function playbackRow(spotify) {
  const { status, backup } = spotify;
  const label = t("sources.spotifyPlayback");
  const retry = (key, text) => button(key, text, () => save(api.spotifyRetry));
  const setUp = backup.ready ? null : button("spotify-set-up", t("sources.spotifySetUpBackup"), openBackup, { strong: true });
  const lines = (...nodes) => nodes.map((node) => (typeof node === "string" ? element("span", { class: "card-row-text" }, [node]) : node));

  if (status.route === "builtin") {
    return cardRow(label, lines(chip("ok", t("sources.chipInUnisson")), t("sources.spotifyOkHint")));
  }
  if (status.route === "app") {
    if (status.problem === "accountRefused") {
      return cardRow(label, lines(chip("info", t("sources.chipViaApp")), t("sources.spotifyRefusedViaApp")), [
        retry("spotify-retry", t("sources.spotifyRetry")),
      ]);
    }
    if (status.problem) {
      return cardRow(
        label,
        lines(chip("info", t("sources.chipViaApp")), t("sources.spotifyLimitedViaApp", { when: retryWhen(status.retryAtMs) })),
        [retry("spotify-retry", t("sources.spotifyRetryNow"))],
      );
    }
    return cardRow(label, lines(chip("info", t("sources.chipViaApp")), t("sources.spotifyAppOnlyHint")));
  }
  if (status.problem === "accountRefused") {
    return cardRow(label, lines(chip("error", t("sources.chipRefused")), t("sources.spotifyRefusedStopped")), [
      setUp,
      retry("spotify-retry", t("sources.spotifyRetry")),
    ]);
  }
  if (status.problem === "premiumRequired") {
    return cardRow(label, lines(chip("error", t("sources.chipPremium")), t("sources.spotifyPremiumStopped")), [
      retry("spotify-retry", t("sources.spotifyRetry")),
    ]);
  }
  return cardRow(
    label,
    lines(chip("error", t("sources.chipStopped")), t("sources.spotifyLimitedStopped", { when: retryWhen(status.retryAtMs) })),
    [retry("spotify-retry", t("sources.spotifyRetry")), setUp],
  );
}

/** @param {import("../api.js").Spotify} spotify */
function backupSection(spotify) {
  const { backup, status } = spotify;
  const subtitle = !backup.ready
    ? t("sources.backupNone")
    : status.route === "app"
      ? t("sources.backupInUse")
      : t("sources.backupReady");
  const head = element(
    "button",
    {
      class: "expander-head",
      type: "button",
      "aria-expanded": String(spotifyUi.backupOpen),
      "aria-controls": "spotify-backup-body",
      "data-focus-key": "spotify-backup",
    },
    [
      icon(spotifyUi.backupOpen ? "down" : "right"),
      element("span", { class: "station-text" }, [
        element("span", { class: "setting-label" }, [t("sources.backupTitle")]),
        element("span", { class: "station-detail", "data-tone": backup.ready ? "ok" : null }, [subtitle]),
      ]),
    ],
  );
  head.addEventListener("click", () => {
    spotifyUi.backupOpen = !spotifyUi.backupOpen;
    draw();
  });
  return element("div", { class: "expander", id: "spotify-backup" }, [
    head,
    spotifyUi.backupOpen
      ? element("div", { class: "expander-body", id: "spotify-backup-body" }, backup.ready ? backupReady(backup) : backupSteps(backup))
      : null,
  ]);
}

/** @param {import("../api.js").Spotify["backup"]} backup */
function autoSwitchRow(backup) {
  return settingRow({
    key: "spotify-auto-switch",
    label: t("sources.autoSwitch"),
    detail: backup.ready ? t("sources.autoSwitchDetail") : t("sources.autoSwitchUnavailable"),
    control: toggle({
      key: "spotify-auto-switch",
      checked: backup.ready && backup.autoSwitch,
      labelledBy: "spotify-auto-switch-label",
      disabled: !backup.ready,
      onChange: (checked) => save(() => api.spotifySetAutoSwitch(checked)),
    }),
  });
}

/** @param {import("../api.js").Spotify["backup"]} backup */
function backupReady(backup) {
  const hidden = backup.clientId ? `Client ID ••••${backup.clientId.slice(-4)}` : "";
  return [
    element("p", { class: "station-detail" }, [t("sources.backupReadyIntro")]),
    element("div", { class: "setting-row" }, [
      element("span", { class: "station-text" }, [
        element("span", { class: "setting-label" }, [t("sources.backupApp")]),
        element("span", { class: "path" }, [hidden]),
      ]),
      button("spotify-remove-backup", t("sources.backupRemove"), () => save(api.spotifyRemoveBackup)),
    ]),
    autoSwitchRow(backup),
  ];
}

/** @param {import("../api.js").Spotify["backup"]} backup */
function backupSteps(backup) {
  if (!spotifyUi.clientId && backup.clientId) {
    spotifyUi.clientId = backup.clientId;
  }
  const step = (number, title, content) =>
    element("li", { class: "step" }, [
      element("span", { class: "step-number", "aria-hidden": "true" }, [String(number)]),
      element("span", { class: "step-content" }, [element("span", { class: "setting-label" }, [title]), ...content]),
    ]);
  const input = /** @type {HTMLInputElement} */ (
    element("input", {
      class: "field field-wide",
      id: "spotify-client-id",
      type: "text",
      spellcheck: "false",
      autocomplete: "off",
      "aria-label": t("sources.backupStep4"),
      placeholder: t("sources.backupClientIdPlaceholder"),
      "data-focus-key": "spotify-client-id",
    })
  );
  input.value = spotifyUi.clientId;
  input.disabled = spotifyUi.savingBackup;
  input.addEventListener("input", () => {
    spotifyUi.clientId = input.value;
  });
  const saveButton = element(
    "button",
    { class: "text-button text-button-strong", type: "submit", "data-focus-key": "spotify-save-backup" },
    [spotifyUi.savingBackup ? t("sources.spotifyWaiting") : t("sources.backupSave")],
  );
  saveButton.disabled = spotifyUi.savingBackup;
  const form = element("form", { class: "inline-form" }, [input, saveButton]);
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    setUpBackup();
  });
  return [
    element("p", { class: "station-detail" }, [t("sources.backupIntro")]),
    element("ol", { class: "steps" }, [
      step(1, t("sources.backupStep1"), [
        element("span", { class: "station-detail" }, [t("sources.backupStep1Detail")]),
        element("span", { class: "inline-form" }, [
          button("spotify-dashboard", t("sources.backupOpenDashboard"), () => api.spotifyOpenDashboard(), { iconName: "external" }),
          element("span", { class: "path" }, [SPOTIFY_DASHBOARD]),
        ]),
      ]),
      step(2, t("sources.backupStep2"), [
        element("span", { class: "inline-form" }, [
          element("code", { class: "code-box" }, [backup.redirectUri]),
          button("spotify-copy-redirect", t("sources.copy"), () => copyText(backup.redirectUri), { iconName: "copy" }),
        ]),
      ]),
      step(3, t("sources.backupStep3"), [element("span", { class: "station-detail" }, [t("sources.backupStep3Detail")])]),
      step(4, t("sources.backupStep4"), [form]),
    ]),
    element("p", { class: "station-detail" }, [t("sources.backupKeep")]),
    autoSwitchRow(backup),
  ];
}

// ---------- YouTube and local files ----------

/** Where the installation guide is. It is shown as text, to copy. */
const INSTALL_GUIDE = "github.com/geekabel/Unisson#youtube";

/** @param {import("../api.js").Settings} settings */
function youtubeDetails(settings) {
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
    element("div", { class: "inline-form" }, [
      button("check-yt-dlp", t("sources.checkAgain"), () => save(api.settingsCheckYtDlp)),
    ]),
  ]);
}

/** @param {import("../api.js").Settings} settings */
function localDetails(settings) {
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
      button("pick-folder", t("sources.changeFolder"), () => save(api.settingsPickMusicFolder)),
      settings.musicFolderIsDefault
        ? null
        : button("reset-folder", t("sources.resetFolder"), () => save(api.settingsResetMusicFolder)),
      button("rescan", t("sources.rescan"), async () => {
        await api.settingsRescanLibrary();
        showNotice({ owner: "settings", text: t("sources.rescanStarted"), duration: 3000 });
      }),
    ]),
  ]);
}

// ---------- Order of preference ----------

/**
 * Moves a source in the order, and says where it went.
 *
 * @param {string[]} ids
 * @param {number} index
 * @param {number} step
 * @param {boolean} [fromKeyboard] Whether Alt+Up or Alt+Down asked.
 */
async function move(ids, index, step, fromKeyboard = false) {
  const order = moved(ids, index, step);
  if (order === ids) {
    return;
  }
  const id = ids[index];
  await save(() => api.settingsSetPriority(order));
  announcer.textContent = t("sources.moved", { name: sourceName(id), position: String(index + step + 1) });
  if (fromKeyboard) {
    // The keyboard stays on the row that moved.
    const row = area.querySelector(`[data-source-row="${id}"]`);
    /** @type {?HTMLElement} */ (row?.querySelector("button:not(:disabled)"))?.focus();
  }
}

/** @param {import("../api.js").Settings} settings */
function priorityList(settings) {
  const ids = settings.sources.map((source) => source.id);
  const list = element("ol", { class: "priority" }, settings.sources.map((source, index) => {
    const name = sourceName(source.id);
    const arrow = (step, label, iconName) => {
      const node = iconButton({ icon: iconName, label, onClick: () => move(ids, index, step) });
      node.dataset.focusKey = `priority-${source.id}-${step}`;
      node.disabled = moved(ids, index, step) === ids;
      return node;
    };
    return element("li", { class: "priority-row", "data-enabled": String(source.enabled), "data-source-row": source.id }, [
      element("span", { class: "queue-number" }, [String(index + 1)]),
      element("span", { class: "priority-name" }, [
        name,
        source.enabled ? null : element("span", { class: "station-detail" }, [t("sources.sourceOff")]),
      ]),
      arrow(-1, t("sources.moveUp", { name }), "up"),
      arrow(1, t("sources.moveDown", { name }), "down"),
    ]);
  }));
  list.addEventListener("keydown", (event) => {
    const step = event.altKey ? { ArrowUp: -1, ArrowDown: 1 }[event.key] : undefined;
    const row = event.target instanceof Element ? event.target.closest("[data-source-row]") : null;
    if (!step || !(row instanceof HTMLElement)) {
      return;
    }
    event.preventDefault();
    move(ids, ids.indexOf(row.dataset.sourceRow ?? ""), step, true);
  });
  return list;
}

api.onSpotifySignInLink((address) => {
  spotifyUi.link = address;
  if (area.isConnected) {
    draw();
  }
});

onSettingsChange(() => {
  if (area.isConnected) {
    draw();
  }
});
