// Checks the texts of the interface: every language says the same things.
//
// Run with: node --test crates/unisson-app/ui-tests/*.test.mjs

import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { test } from "node:test";

import { getLanguage, plural, setLanguage, t } from "../ui/js/i18n.js";
import { CATALOGS } from "../ui/js/locales/index.js";

const marks = (text) => [...text.matchAll(/\{(\w+)\}/g)].map((match) => match[1]).sort();

test("every language has the same keys", () => {
  const french = Object.keys(CATALOGS.fr).sort();
  const english = Object.keys(CATALOGS.en).sort();
  assert.deepEqual(
    english.filter((key) => !french.includes(key)),
    [],
    "keys only in English",
  );
  assert.deepEqual(
    french.filter((key) => !english.includes(key)),
    [],
    "keys only in French",
  );
  assert.ok(french.length > 100, "the catalogs are filled");
});

test("a text has the same marks in every language, and is never empty", () => {
  for (const [key, french] of Object.entries(CATALOGS.fr)) {
    const english = CATALOGS.en[key];
    assert.ok(french.trim() !== "" && english.trim() !== "", `${key} is empty`);
    assert.deepEqual(marks(english), marks(french), `${key} has different marks`);
  }
});

test("a key starts with the name of its file", () => {
  const folder = new URL("../ui/js/locales/", import.meta.url);
  for (const file of readdirSync(folder).filter((name) => name !== "index.js")) {
    const area = file.replace(/\.js$/, "");
    const source = readFileSync(new URL(file, folder), "utf8");
    for (const [, key] of source.matchAll(/^\s+"([^"]+)":/gm)) {
      assert.ok(key.startsWith(`${area}.`), `${key} in ${file}`);
    }
  }
});

test("a plural text has both of its forms", () => {
  for (const key of Object.keys(CATALOGS.fr)) {
    const [, stem, form] = key.match(/^(.*)\.(one|other)$/) ?? [];
    if (stem) {
      const twin = form === "one" ? "other" : "one";
      assert.ok(`${stem}.${twin}` in CATALOGS.fr, `${stem}.${twin} is missing`);
    }
  }
});

test("every key used by the pages and the scripts exists", () => {
  const root = new URL("../ui/", import.meta.url);
  const files = [
    ...readdirSync(root).filter((name) => name.endsWith(".html")),
    ...readdirSync(new URL("js/", root))
      // The translation module itself only names keys in its comments.
      .filter((name) => name.endsWith(".js") && name !== "i18n.js")
      .map((name) => `js/${name}`),
    ...readdirSync(new URL("js/views/", root)).map((name) => `js/views/${name}`),
  ];
  const missing = [];
  for (const file of files) {
    const source = readFileSync(new URL(file, root), "utf8");
    const used = [
      ...[...source.matchAll(/\bt\(\s*"([^"]+)"/g)].map((match) => match[1]),
      ...[...source.matchAll(/\bplural\([^,]+,\s*"([^"]+)"/g)].flatMap((match) => [`${match[1]}.one`, `${match[1]}.other`]),
      ...[...source.matchAll(/data-i18n="([^"]+)"/g)].map((match) => match[1]),
      ...[...source.matchAll(/data-i18n-attrs="([^"]+)"/g)].flatMap((match) =>
        match[1].split(",").map((pair) => pair.split(":")[1]?.trim()),
      ),
    ];
    for (const key of used) {
      if (!(key in CATALOGS.fr)) {
        missing.push(`${file}: ${key}`);
      }
    }
  }
  assert.deepEqual(missing, []);
});

test("t fills the marks, and falls back when a text is missing", () => {
  setLanguage("fr");
  assert.equal(getLanguage(), "fr");
  assert.equal(t("no.such.key"), "no.such.key");
  setLanguage("klingon");
  assert.equal(getLanguage(), "en", "an unknown language gets English");
  setLanguage("fr");
});

test("plural follows the rule of each language", () => {
  CATALOGS.fr["test.items.one"] = "{count} titre";
  CATALOGS.fr["test.items.other"] = "{count} titres";
  CATALOGS.en["test.items.one"] = "{count} track";
  CATALOGS.en["test.items.other"] = "{count} tracks";
  setLanguage("fr");
  assert.deepEqual([0, 1, 2].map((n) => plural(n, "test.items")), ["0 titre", "1 titre", "2 titres"]);
  setLanguage("en");
  assert.deepEqual([0, 1, 2].map((n) => plural(n, "test.items")), ["0 tracks", "1 track", "2 tracks"]);
  setLanguage("fr");
  for (const language of ["fr", "en"]) {
    for (const key of Object.keys(CATALOGS[language])) {
      if (key.startsWith("test.")) {
        delete CATALOGS[language][key];
      }
    }
  }
});
