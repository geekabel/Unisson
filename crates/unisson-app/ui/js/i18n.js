/**
 * The language of the interface.
 *
 * Every text shown to the user goes through `t()`, with a key such as
 * `home.resume`. The texts of each language live in `locales/`, one file
 * per area of the interface.
 *
 * The backend decides the language: the choice of the user, or the language
 * of the desktop. A page cannot wait for its answer before writing its first
 * word, so it starts in the language of its last load, then asks. When the
 * answer differs, as it does when the user changes the language, the page
 * simply reloads: playback lives in the backend and is not disturbed.
 */

import { CATALOGS } from "./locales/index.js";

/** The language used when none is known, and where a missing text is looked up. */
const FALLBACK = "fr";

let language = FALLBACK;
let plurals = new Intl.PluralRules(language);

/**
 * Chooses the language. An unknown one gets English.
 *
 * @param {string} code `fr` or `en`.
 */
export function setLanguage(code) {
  language = Object.hasOwn(CATALOGS, code) ? code : "en";
  plurals = new Intl.PluralRules(language);
}

/** @returns {string} The language in use: `fr` or `en`. */
export function getLanguage() {
  return language;
}

/**
 * Returns a text in the language in use.
 *
 * @param {string} key Such as `home.resume`.
 * @param {Record<string, string | number>} [values] What replaces the
 *   `{name}` marks of the text.
 * @returns {string} The text. A key that no language knows is returned as
 *   it is, which makes the mistake easy to see.
 */
export function t(key, values) {
  const text = CATALOGS[language][key] ?? CATALOGS[FALLBACK][key] ?? key;
  if (!values) {
    return text;
  }
  return text.replace(/\{(\w+)\}/g, (mark, name) => (name in values ? String(values[name]) : mark));
}

/**
 * Returns a text that depends on a number, such as "1 track" and
 * "3 tracks".
 *
 * The catalog holds one text per form: `key.one` and `key.other`. Each
 * language has its own rule: French says "0 titre", English "0 tracks".
 *
 * @param {number} count
 * @param {string} key The key without its `.one` or `.other` ending.
 * @param {Record<string, string | number>} [values] `count` is always
 *   available as `{count}`.
 */
export function plural(count, key, values) {
  const form = plurals.select(count) === "one" ? "one" : "other";
  return t(`${key}.${form}`, { count, ...values });
}

/**
 * Writes the texts of the elements that carry a key in the page itself.
 *
 * - `data-i18n="key"` sets the text of the element;
 * - `data-i18n-attrs="title:key, aria-label:other.key"` sets attributes.
 *
 * @param {ParentNode} [root]
 */
export function translatePage(root = document) {
  for (const node of root.querySelectorAll("[data-i18n]")) {
    node.textContent = t(node.dataset.i18n);
  }
  for (const node of root.querySelectorAll("[data-i18n-attrs]")) {
    for (const pair of node.dataset.i18nAttrs.split(",")) {
      const [attribute, key] = pair.split(":").map((part) => part.trim());
      if (attribute && key) {
        node.setAttribute(attribute, t(key));
      }
    }
  }
}

/** Where the language is remembered between two loads of a page. */
const STORAGE_KEY = "unisson-language";

function remembered() {
  try {
    return globalThis.localStorage?.getItem(STORAGE_KEY) ?? null;
  } catch {
    return null;
  }
}

/** Remembers the language. Returns whether it could be written. */
function remember(code) {
  try {
    globalThis.localStorage.setItem(STORAGE_KEY, code);
    return globalThis.localStorage.getItem(STORAGE_KEY) === code;
  } catch {
    return false;
  }
}

/**
 * Asks the backend which language to use. If it is not the one on screen,
 * the page reloads in the right one.
 */
async function follow(tauri) {
  const wanted = await tauri.core.invoke("settings_language");
  if (wanted === language) {
    remember(wanted);
    return;
  }
  // Without a place to remember it, a reload would start in the wrong
  // language again, for ever: stay as we are.
  if (remember(wanted)) {
    window.location.reload();
  }
}

// In the application, the language must be known at once: the modules of
// the page write their texts as soon as they load, and cannot wait for an
// answer of the backend. So the language of the last load is used, or, the
// very first time, the language of the system as the browser engine sees
// it. The backend is then asked, and the page reloads if it disagrees:
// that happens when the user changes the language, and almost never
// otherwise. Outside of the application (the tests of the pure helpers),
// the fallback language stays.
const tauri = globalThis.__TAURI__;
if (tauri) {
  setLanguage(remembered() ?? (globalThis.navigator?.language ?? "en").slice(0, 2).toLowerCase());
  document.documentElement.lang = language;
  translatePage();
  follow(tauri).catch((error) => console.error(error));
  tauri.event.listen("settings:changed", () => follow(tauri).catch((error) => console.error(error)));
}
