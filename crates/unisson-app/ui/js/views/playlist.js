/**
 * The view of one playlist: play it, rename it, reorder it, empty it.
 */

import * as api from "../api.js";
import { confirmButton, iconButton, mediaRow, queueButton } from "../components.js";
import { element } from "../dom.js";
import { countLabel, moved } from "../format.js";
import { showNotice } from "../notice.js";
import { allPlaylists } from "../playlists.js";

/** The action button to focus again after the list is redrawn. */
let focusAfterRender = null;
/**
 * The playlist being renamed and what was typed so far, or `null`.
 *
 * It lives outside the view because the view is redrawn when the playlists
 * change, and that must not throw away a name half typed.
 *
 * @type {?{id: number, draft: string}}
 */
let renaming = null;

/**
 * Builds the view.
 *
 * @param {object} context
 * @param {(name: string) => void} context.showView
 * @param {number} context.param The identifier of the playlist.
 * @param {{renaming?: boolean}} context.options
 * @returns {Promise<Node[]>}
 */
export async function render({ showView, param: id, options }) {
  const playlist = allPlaylists().find((other) => other.id === id);
  if (!playlist) {
    // It was deleted, here or from another place.
    queueMicrotask(() => showView("home"));
    return [];
  }
  const items = await api.playlistItems(id);

  const save = async (changed, focus) => {
    focusAfterRender = focus;
    try {
      await api.playlistSetItems(id, changed);
    } catch {
      showNotice({ owner: "playlists", tone: "error", text: "La liste n'a pas pu être modifiée.", duration: 8000 });
    }
  };

  const play = element("button", { class: "text-button text-button-strong", type: "button" }, ["Lire"]);
  play.disabled = items.length === 0;
  play.addEventListener("click", () => api.playList(items, 0));

  const head = element("div", { class: "playlist-head" });
  const showName = () => {
    renaming = null;
    const rename = element("button", { class: "text-button", type: "button" }, ["Renommer"]);
    rename.addEventListener("click", () => {
      renaming = { id, draft: playlist.name };
      showRename();
    });
    head.replaceChildren(
      element("div", { class: "station-text" }, [
        element("h1", { class: "view-title truncate" }, [playlist.name]),
        element("span", { class: "section-note" }, [`Liste, ${countLabel(items.length, "titre")}`]),
      ]),
      play,
      rename,
      confirmButton({
        label: "Supprimer",
        confirmLabel: "Cliquer encore pour supprimer",
        onConfirm: () => api.playlistDelete(id),
      }),
    );
  };
  const showRename = () => {
    const input = /** @type {HTMLInputElement} */ (
      element("input", { class: "field field-wide", type: "text", "aria-label": "Nom de la liste" })
    );
    input.value = renaming.draft;
    input.addEventListener("input", () => {
      renaming.draft = input.value;
    });
    const cancel = element("button", { class: "text-button", type: "button" }, ["Annuler"]);
    cancel.addEventListener("click", showName);
    const form = element("form", { class: "inline-form" }, [
      input,
      element("button", { class: "text-button text-button-strong", type: "submit" }, ["Enregistrer"]),
      cancel,
    ]);
    form.addEventListener("submit", async (event) => {
      event.preventDefault();
      try {
        await api.playlistRename(id, input.value);
        // The view is redrawn with the new name when the backend confirms.
        renaming = null;
      } catch (error) {
        showNotice({
          owner: "playlists",
          tone: "error",
          text: error?.kind === "invalid" ? "Donnez un nom à la liste." : "La liste n'a pas pu être renommée.",
          duration: 8000,
        });
      }
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "Escape") {
        event.stopPropagation();
        showName();
      }
    });
    head.replaceChildren(form);
    // Wait for the view to be on screen before taking the focus.
    const untouched = renaming.draft === playlist.name;
    setTimeout(() => {
      input.focus();
      if (untouched) {
        input.select();
      }
    }, 0);
  };
  if (options.renaming && renaming?.id !== id) {
    renaming = { id, draft: playlist.name };
  }
  if (renaming?.id === id) {
    showRename();
  } else {
    showName();
  }

  const body = items.length === 0
    ? element("p", { class: "hint" }, [
        "Cette liste est vide. Dans la recherche, l'accueil ou l'historique, " +
          "le bouton « Ajouter à une liste » de chaque ligne la remplit.",
      ])
    : element("div", { class: "rows" }, items.map((media, index) => {
        const action = (name, label, iconName, changed) => {
          const button = iconButton({
            icon: iconName,
            label: `${label} ${media.title}`,
            onClick: () => save(changed.items, { index: changed.focus, name }),
          });
          button.dataset.action = name;
          button.disabled = changed.items === items;
          return button;
        };
        const row = mediaRow(media, {
          onPlay: () => api.playList(items, index),
          actions: [
            queueButton(media),
            action("up", "Monter", "up", { items: moved(items, index, -1), focus: index - 1 }),
            action("down", "Descendre", "down", { items: moved(items, index, 1), focus: index + 1 }),
            action("remove", "Retirer", "close", {
              items: items.filter((_, other) => other !== index),
              focus: index,
            }),
          ],
        });
        row.dataset.index = String(index);
        return row;
      }));

  if (focusAfterRender) {
    const { index, name } = focusAfterRender;
    focusAfterRender = null;
    setTimeout(() => {
      const rows = body.querySelectorAll?.(".row") ?? [];
      const row = rows[Math.min(index, rows.length - 1)];
      (row?.querySelector(`[data-action="${name}"]:not(:disabled)`) ?? row?.querySelector("button"))?.focus();
    }, 0);
  }
  return [head, body];
}
