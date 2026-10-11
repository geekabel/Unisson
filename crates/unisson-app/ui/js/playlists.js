/**
 * The playlists, as the interface knows them, and their place in the
 * sidebar.
 *
 * The backend owns the playlists. This module keeps a copy of their names
 * and sizes, so that menus and the sidebar need no round trip.
 */

import * as api from "./api.js";
import { byId, element, icon, replaceKeepingFocus } from "./dom.js";
import { t } from "./i18n.js";
import { closeMenu, menuItem, setMenuContent, toggleMenu } from "./menu.js";
import { showNotice } from "./notice.js";
import { activeParam, activeView, showView } from "./views.js";

const group = byId("playlists-nav");
const list = byId("playlists-list");
const newButton = byId("playlist-new");

/** @type {import("./api.js").Playlist[]} */
let playlists = [];
const listeners = new Set();

/** Fetches the playlists again from the backend, and tells the listeners. */
export async function loadPlaylists() {
  playlists = await api.playlistsList();
  drawPlaylistsSidebar();
  for (const listener of listeners) {
    listener();
  }
}

/** Returns the playlists, sorted by name. */
export function allPlaylists() {
  return playlists;
}

/**
 * Calls `listener` every time the playlists change.
 *
 * @param {() => void} listener
 */
export function onPlaylistsChange(listener) {
  listeners.add(listener);
}

/** Draws the playlists of the sidebar, and highlights the one on screen. */
export function drawPlaylistsSidebar() {
  group.hidden = false;
  const viewed = activeView() === "playlist" ? activeParam() : null;
  replaceKeepingFocus(list,
    ...playlists.map((playlist) => {
      const item = element(
        "button",
        {
          class: "nav-item",
          type: "button",
          title: playlist.name,
          "data-playlist-id": String(playlist.id),
          "aria-current": playlist.id === viewed ? "page" : null,
        },
        [icon("list"), element("span", { class: "nav-label truncate" }, [playlist.name])],
      );
      item.addEventListener("click", () => showView("playlist", { param: playlist.id }));
      return item;
    }),
  );
}

/**
 * Creates a playlist, and reports a failure to the user.
 *
 * @param {string} name
 * @returns {Promise<?import("./api.js").Playlist>} `null` if it failed.
 */
export async function createPlaylist(name) {
  try {
    const playlist = await api.playlistCreate(name);
    await loadPlaylists();
    return playlist;
  } catch (error) {
    showNotice({
      owner: "playlists",
      tone: "error",
      text: error?.kind === "invalid" ? t("shell.playlistNeedsName") : t("shell.playlistNotCreated"),
      duration: 8000,
    });
    return null;
  }
}

/**
 * Opens the "add to a list" menu under a button.
 *
 * @param {HTMLElement} anchor
 * @param {import("./api.js").Media} media
 */
export function openAddToListMenu(anchor, media) {
  const add = async (playlist) => {
    try {
      await api.playlistAdd(playlist.id, media);
      showNotice({
        owner: "playlists",
        text: t("shell.addedToPlaylist", { playlist: playlist.name, title: media.title }),
        duration: 2500,
      });
    } catch {
      showNotice({ owner: "playlists", tone: "error", text: t("shell.trackNotAdded"), duration: 8000 });
    }
  };

  const nameForm = () => {
    const input = element("input", {
      class: "field",
      type: "text",
      placeholder: t("shell.playlistName"),
      "aria-label": t("shell.newPlaylistName"),
    });
    const form = element("form", { class: "menu-form" }, [
      input,
      element("button", { class: "text-button text-button-strong", type: "submit" }, [t("common.create")]),
    ]);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      const playlist = await createPlaylist(input.value);
      if (playlist) {
        closeMenu();
        add(playlist);
      }
    });
    return form;
  };

  toggleMenu(anchor, t("shell.addToPlaylist", { title: media.title }), [
    ...playlists.map((playlist) => menuItem(playlist.name, () => add(playlist))),
    menuItem(t("shell.newPlaylistEntry"), () => setMenuContent([nameForm()]), { keepOpen: true }),
  ]);
}

newButton.addEventListener("click", async () => {
  const playlist = await createPlaylist(t("shell.newPlaylist"));
  if (playlist) {
    // Open it with its name ready to be changed.
    showView("playlist", { param: playlist.id, renaming: true });
  }
});
