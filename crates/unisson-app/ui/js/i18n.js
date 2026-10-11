/**
 * The language of the interface.
 *
 * Every text shown to the user goes through `t()`, with a key such as
 * `home.resume`. The texts of each language live in `locales/`, one file
 * per area of the interface.
 *
 * The backend decides the language: the choice of the user, or the language
 * of the desktop. This module asks it once, before any other module of the
 * page runs, so that the first word written is already in the right
 * language. When the user changes the language, the page simply reloads:
 * playback lives in the backend and is not disturbed.
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

// In the application: ask the backend, then write the page. Outside of it
// (the tests of the pure helpers), the fallback language stays.
const tauri = globalThis.__TAURI__;
if (tauri) {
  try {
    setLanguage(await tauri.core.invoke("settings_language"));
  } catch (error) {
    console.error(error);
  }
  document.documentElement.lang = language;
  translatePage();
  tauri.event.listen("settings:changed", async () => {
    const wanted = await tauri.core.invoke("settings_language");
    if (wanted !== language) {
      window.location.reload();
    }
  });
}
