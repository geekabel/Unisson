// Checks that the colors of both themes keep texts and controls readable.
//
// The rules come from WCAG 2.1, level AA: 4.5 to 1 for text, 3 to 1 for the
// edge of a control and for the focus ring. The colors are read from the
// style sheet itself, so a change that breaks a rule fails here.
//
// Run with: node --test crates/unisson-app/ui-tests/*.test.mjs

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

const css = readFileSync(new URL("../ui/styles/app.css", import.meta.url), "utf8");

/** Reads the color tokens of the block that starts with `selector`. */
function tokens(selector) {
  const start = css.indexOf(`${selector} {`);
  assert.notEqual(start, -1, `no block for ${selector}`);
  const block = css.slice(start, css.indexOf("\n}", start));
  const found = {};
  for (const [, name, value] of block.matchAll(/--color-([a-z-]+):\s*(#[0-9A-Fa-f]{6});/g)) {
    found[name] = value;
  }
  return found;
}

/** Relative luminance of a `#RRGGBB` color, as WCAG defines it. */
function luminance(color) {
  const [r, g, b] = [1, 3, 5].map((at) => {
    const channel = parseInt(color.slice(at, at + 2), 16) / 255;
    return channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  });
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}

function contrast(first, second) {
  const [light, dark] = [luminance(first), luminance(second)].sort((a, b) => b - a);
  return (light + 0.05) / (dark + 0.05);
}

const BACKGROUNDS = ["bg", "panel", "surface", "hover"];
/** Each rule: the colors drawn, what they are drawn on, the least ratio. */
const RULES = [
  { inks: ["text", "text-soft", "text-muted"], on: BACKGROUNDS, least: 4.5 },
  { inks: ["accent-ink", "live", "danger"], on: BACKGROUNDS, least: 4.5 },
  { inks: ["on-accent"], on: ["accent"], least: 4.5 },
  { inks: ["on-source"], on: ["source-radio", "source-local", "source-youtube", "source-spotify"], least: 4.5 },
  { inks: ["text", "danger"], on: ["danger-soft"], least: 4.5 },
  { inks: ["control"], on: ["bg", "panel", "surface"], least: 3 },
  { inks: ["knob"], on: ["control"], least: 3 },
];

const dark = tokens(":root");
const themes = { dark, light: { ...dark, ...tokens(':root[data-theme="light"]') } };

for (const [name, colors] of Object.entries(themes)) {
  test(`the ${name} theme keeps texts and controls readable`, () => {
    for (const { inks, on, least } of RULES) {
      for (const ink of inks) {
        for (const background of on) {
          assert.ok(colors[ink] && colors[background], `${ink} or ${background} is missing`);
          const ratio = contrast(colors[ink], colors[background]);
          assert.ok(
            ratio >= least,
            `${name}: ${ink} ${colors[ink]} on ${background} ${colors[background]} is ${ratio.toFixed(2)}, below ${least}`,
          );
        }
      }
    }
  });
}

test("the light theme defines every color of the dark one", () => {
  const light = tokens(':root[data-theme="light"]');
  assert.deepEqual(Object.keys(light).sort(), Object.keys(dark).sort());
});

test("the contrast formula matches the reference values", () => {
  assert.equal(contrast("#000000", "#FFFFFF").toFixed(1), "21.0");
  assert.equal(contrast("#777777", "#FFFFFF").toFixed(2), "4.48");
});
