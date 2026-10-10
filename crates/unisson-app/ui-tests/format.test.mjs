// Run from the root of the repository with:
//   node --test crates/unisson-app/ui-tests/*.test.mjs
//
// The tests live outside the `ui` folder, which is embedded whole in the
// application.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  fillPercent,
  formatTime,
  looksLikeTarget,
  mediaKey,
  mediaSubtitle,
  openErrorMessage,
  playbackErrorMessage,
  sourceLabel,
} from "../ui/js/format.js";

const track = {
  source: "local",
  key: "/m/a.flac",
  kind: "track",
  title: "So What",
  artist: "Miles Davis",
  album: "Kind of Blue",
};
const station = { source: "radio", key: "fip", kind: "station", title: "FIP" };

test("formatTime writes minutes and seconds, and hours when needed", () => {
  assert.equal(formatTime(0), "0:00");
  assert.equal(formatTime(999), "0:00");
  assert.equal(formatTime(221_000), "3:41");
  assert.equal(formatTime(600_000), "10:00");
  assert.equal(formatTime(3_723_000), "1:02:03");
});

test("formatTime writes nothing for an unknown duration", () => {
  for (const unknown of [null, undefined, NaN, Infinity, -1]) {
    assert.equal(formatTime(unknown), "");
  }
});

test("sourceLabel names the known sources and keeps the others", () => {
  assert.equal(sourceLabel("youtube"), "YouTube");
  assert.equal(sourceLabel("local"), "Local");
  assert.equal(sourceLabel("cassette"), "cassette");
});

test("mediaSubtitle shows the artist and the album of a track", () => {
  assert.equal(mediaSubtitle(track), "Miles Davis, Kind of Blue");
  assert.equal(mediaSubtitle({ ...track, album: null }), "Miles Davis");
  assert.equal(mediaSubtitle({ ...track, artist: null, album: null }), "Artiste inconnu");
});

test("mediaSubtitle shows what a station announces, or that it is live", () => {
  assert.equal(mediaSubtitle(station), "En direct");
  assert.equal(mediaSubtitle(station, "Nina Simone - Sinnerman"), "Nina Simone - Sinnerman");
});

test("looksLikeTarget tells links and paths from words", () => {
  for (const target of ["https://a.fr/x.mp3", " http://a.fr ", "/home/me/a.mp3", "~/Music/a.mp3"]) {
    assert.equal(looksLikeTarget(target), true, target);
  }
  for (const words of ["miles davis", "", "/", "~", "ftp://a.fr", "a/b.mp3"]) {
    assert.equal(looksLikeTarget(words), false, words);
  }
});

test("mediaKey tells media apart across sources", () => {
  assert.equal(mediaKey(null), "");
  assert.notEqual(mediaKey(track), mediaKey({ ...track, source: "youtube" }));
  assert.equal(mediaKey(track), mediaKey({ ...track, title: "Other title" }));
});

test("fillPercent stays between 0% and 100%", () => {
  assert.equal(fillPercent(0, 100), "0.00%");
  assert.equal(fillPercent(39, 100), "39.00%");
  assert.equal(fillPercent(150, 100), "100.00%");
  assert.equal(fillPercent(-5, 100), "0.00%");
  assert.equal(fillPercent(5, 0), "0%");
});

test("every error has a sentence, even an unknown one", () => {
  for (const kind of ["notFound", "network", "unavailable", "playback", "other"]) {
    assert.ok(playbackErrorMessage({ kind }).length > 10);
  }
  for (const error of [{ kind: "invalid" }, { kind: "notFound" }, { kind: "unreadable" }, {}, null, "text"]) {
    assert.ok(openErrorMessage(error).length > 10);
  }
});
