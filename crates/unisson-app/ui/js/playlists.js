/**
 * The playlists, as the interface knows them, and their place in the
 * sidebar.
 *
 * The backend owns the playlists. This module keeps a copy of their names
 * and sizes, so that menus and the sidebar need no round trip.
 */

import * as api from "./api.js";
import { byId, element, icon } from "./dom.js";
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
  list.replaceChildren(
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
      text: error?.kind === "invalid" ? "Donnez un nom à la liste." : "La liste n'a pas pu être créée.",
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
      showNotice({ owner: "playlists", text: `Ajouté à « ${playlist.name} » : ${media.title}`, duration: 2500 });
    } catch {
      showNotice({ owner: "playlists", tone: "error", text: "Le titre n'a pas pu être ajouté.", duration: 8000 });
    }
  };

  const nameForm = () => {
    const input = element("input", {
      class: "field",
      type: "text",
      placeholder: "Nom de la liste",
      "aria-label": "Nom de la nouvelle liste",
    });
    const form = element("form", { class: "menu-form" }, [
      input,
      element("button", { class: "text-button text-button-strong", type: "submit" }, ["Créer"]),
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

  toggleMenu(anchor, `Ajouter ${media.title} à une liste`, [
    ...playlists.map((playlist) => menuItem(playlist.name, () => add(playlist))),
    menuItem("Nouvelle liste…", () => setMenuContent([nameForm()]), { keepOpen: true }),
  ]);
}

newButton.addEventListener("click", async () => {
  const playlist = await createPlaylist("Nouvelle liste");
  if (playlist) {
    // Open it with its name ready to be changed.
    showView("playlist", { param: playlist.id, renaming: true });
  }
});
