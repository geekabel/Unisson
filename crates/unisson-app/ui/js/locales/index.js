/**
 * The texts of the interface, in every language.
 *
 * Each area of the interface has its own file, which holds its texts in
 * French and in English under the same keys. This module puts them
 * together: one flat table per language.
 */

import common from "./common.js";
import shell from "./shell.js";
import format from "./format.js";
import home from "./home.js";
import radios from "./radios.js";
import search from "./search.js";
import sources from "./sources.js";
import settings from "./settings.js";
import launcher from "./launcher.js";
import mini from "./mini.js";

const AREAS = [common, shell, format, home, radios, search, sources, settings, launcher, mini];

/** @type {Record<string, Record<string, string>>} */
export const CATALOGS = { fr: {}, en: {} };

for (const area of AREAS) {
  for (const language of Object.keys(CATALOGS)) {
    Object.assign(CATALOGS[language], area[language]);
  }
}
