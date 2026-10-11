// Run from the root of the repository with:
//   node --test crates/unisson-app/ui-tests/*.test.mjs
//
// The tests live outside the `ui` folder, which is embedded whole in the
// application.

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  capitalize,
  countLabel,
  countryName,
  displayPath,
  fillPercent,
  formatTime,
  looksLikeTarget,
  matchesText,
  mediaKey,
  mediaSubtitle,
  moved,
  openErrorMessage,
  playbackErrorMessage,
  searchSummary,
  sourceLabel,
  sourceName,
  stationDescription,
  stationMedia,
  stationQuality,
  tileText,
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

const fip = {
  uuid: "fip",
  name: "FIP",
  streamUrl: "https://example.org/fip",
  favicon: "https://example.org/fip.png",
  tags: ["eclectic", " jazz ", ""],
  countryCode: "fr",
};

test("stationMedia turns a station into a live media", () => {
  assert.deepEqual(stationMedia(fip), {
    source: "radio",
    key: "fip",
    kind: "station",
    title: "FIP",
    artist: null,
    album: null,
    durationMs: null,
    artwork: "https://example.org/fip.png",
  });
});

test("countryName gives the French name, or nothing", () => {
  assert.equal(countryName("FR"), "France");
  assert.equal(countryName("us"), "États-Unis");
  for (const unknown of [null, undefined, "", "F", "FRA", "12"]) {
    assert.equal(countryName(unknown), "");
  }
});

test("stationDescription shows the first tags and the country", () => {
  assert.equal(stationDescription(fip), "Eclectic, France");
  assert.equal(stationDescription(fip, 2), "Eclectic, Jazz, France");
  assert.equal(stationDescription({ ...fip, tags: [], countryCode: null }), "");
});

test("capitalize and countLabel", () => {
  assert.equal(capitalize("électro"), "Électro");
  assert.equal(capitalize(""), "");
  assert.equal(countLabel(0, "titre"), "0 titre");
  assert.equal(countLabel(1, "titre"), "1 titre");
  assert.equal(countLabel(12, "titre"), "12 titres");
  assert.equal(countLabel(2, "journal", "journaux"), "2 journaux");
});

test("tileText describes each kind of tile", () => {
  assert.deepEqual(tileText({ kind: "station", source: "radio" }), { badge: "Radio", subtitle: "En direct" });
  assert.deepEqual(tileText({ kind: "album", source: "local", artist: "Miles Davis" }), {
    badge: "Album",
    subtitle: "Album, Miles Davis",
  });
  assert.deepEqual(tileText({ kind: "album", source: "local", artist: null }), { badge: "Album", subtitle: "Album" });
  assert.deepEqual(tileText({ kind: "track", source: "youtube", artist: null }), { badge: "YouTube", subtitle: "Titre" });
});

test("stationQuality shows the format and the bitrate that are known", () => {
  assert.equal(stationQuality({ codec: "MP3", bitrate: 128 }), "MP3 128 kb/s");
  assert.equal(stationQuality({ codec: "AAC", bitrate: 0 }), "AAC");
  assert.equal(stationQuality({ codec: null, bitrate: 96 }), "96 kb/s");
  assert.equal(stationQuality({ codec: null, bitrate: null }), "");
});

test("searchSummary follows the search from the first letter to the last answer", () => {
  const hit = (source) => ({ media: { source }, sources: [source] });
  assert.equal(searchSummary("m", null, 0, 2), "Tapez au moins 2 lettres.");
  assert.equal(searchSummary("miles", null, 0, 2), "Recherche en cours…");
  const partial = { hits: [hit("local")], pending: ["radio"], errors: [], done: false };
  assert.equal(searchSummary("miles", partial, 1, 2), "1 résultat, 1 source. Recherche en cours…");
  const full = { hits: [hit("local"), hit("local"), hit("radio")], pending: [], errors: [], done: true };
  assert.equal(searchSummary("miles", full, 3, 2), "3 résultats, 2 sources.");
  const none = { hits: [], pending: [], errors: [], done: true };
  assert.equal(searchSummary("zzz", none, 0, 2), "Aucun résultat pour « zzz ».");
  assert.equal(searchSummary("zzz", { ...none, done: false }, 0, 2), "Recherche en cours…");
});

test("displayPath shortens the home folder", () => {
  assert.equal(displayPath("/home/me/Music"), "~/Music");
  assert.equal(displayPath("/home/me"), "~");
  assert.equal(displayPath("/data/home/me/Music"), "/data/home/me/Music");
  assert.equal(displayPath("/srv/media/Music", "/srv/media"), "~/Music");
  assert.equal(displayPath("/srv/mediatheque", "/srv/media"), "/srv/mediatheque");
  assert.equal(displayPath(null), "");
});

test("moved swaps an item with its neighbor, or changes nothing", () => {
  const list = ["a", "b", "c"];
  assert.deepEqual(moved(list, 0, 1), ["b", "a", "c"]);
  assert.deepEqual(moved(list, 2, -1), ["a", "c", "b"]);
  assert.equal(moved(list, 0, -1), list);
  assert.equal(moved(list, 2, 1), list);
  assert.equal(moved(list, 5, -1), list);
  assert.deepEqual(list, ["a", "b", "c"], "the original list is not touched");
});

test("sourceName gives the full name of a source", () => {
  assert.equal(sourceName("local"), "Fichiers locaux");
  assert.equal(sourceName("cassette"), "cassette");
});

test("matchesText ignores case and accents, and wants every word", () => {
  assert.equal(matchesText("Pause café", "cafe"), true);
  assert.equal(matchesText("Pause café", "CAFÉ pau"), true);
  assert.equal(matchesText("Pause café", "the"), false);
  assert.equal(matchesText("Pause café", "pause thé"), false);
  assert.equal(matchesText("Pause café", "   "), false);
});
