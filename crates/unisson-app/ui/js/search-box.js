/**
 * The search box at the top of the window.
 *
 * It does two things with what is typed:
 *
 * - words are searched in every source, in the search view;
 * - a link or the full path of a file is played with Enter.
 *
 * The focus stays in the box while the keyboard drives the results, so
 * the user can keep typing at any time.
 */

import * as api from "./api.js";
import { byId } from "./dom.js";
import { looksLikeTarget, openErrorMessage } from "./format.js";
import { t } from "./i18n.js";
import { hideNotice, showNotice } from "./notice.js";
import { activeView, onViewChange, showView } from "./views.js";
import { activateSelection, clearQuery, moveSelection, setQuery } from "./views/search.js";

/** Pause in the typing after which the sources are asked. */
const TYPING_PAUSE = 250;

const form = byId("search-form");
const input = /** @type {HTMLInputElement} */ (byId("search-input"));

/** The view to come back to when the search is left. */
let viewBefore = "home";
let typingTimer = null;

/** Starts the search for what is in the box, now. */
function searchNow() {
  clearTimeout(typingTimer);
  const text = input.value.trim();
  if (!text || looksLikeTarget(text)) {
    return;
  }
  if (activeView() !== "search") {
    viewBefore = activeView();
    showView("search");
  }
  setQuery(text);
}

/** Empties the box and comes back to the view shown before the search. */
function leaveSearch() {
  clearTimeout(typingTimer);
  input.value = "";
  clearQuery();
  hideNotice("search");
  if (activeView() === "search") {
    showView(viewBefore);
  }
}

async function openTarget(text) {
  // A YouTube link takes a few seconds: say that something is happening.
  showNotice({ owner: "search", text: t("shell.openingLink") });
  try {
    await api.open(text);
    hideNotice("search");
    input.value = "";
    input.blur();
  } catch (error) {
    showNotice({ owner: "search", tone: "error", text: openErrorMessage(error), duration: 8000 });
  }
}

input.addEventListener("input", () => {
  clearTimeout(typingTimer);
  const text = input.value.trim();
  if (!text) {
    leaveSearch();
  } else if (!looksLikeTarget(text)) {
    typingTimer = setTimeout(searchNow, TYPING_PAUSE);
  }
});

input.addEventListener("keydown", (event) => {
  const text = input.value.trim();
  switch (event.key) {
    case "ArrowDown":
    case "ArrowUp":
      if (activeView() === "search") {
        event.preventDefault();
        moveSelection(event.key === "ArrowDown" ? 1 : -1);
      }
      break;
    case "Enter":
      event.preventDefault();
      if (!text) {
        break;
      }
      if (looksLikeTarget(text)) {
        openTarget(text);
        break;
      }
      // Enter may come before the pause that starts the search.
      searchNow();
      activateSelection(event.shiftKey ? "enqueue" : event.ctrlKey || event.metaKey ? "next" : "now");
      break;
    case "Escape":
      event.preventDefault();
      if (text) {
        leaveSearch();
      } else {
        input.blur();
      }
      break;
    default:
  }
});

// Enter is handled above: the form must never reload the page.
form.addEventListener("submit", (event) => event.preventDefault());

// Leaving the search view by the sidebar empties the box.
onViewChange((name) => {
  if (name !== "search" && input.value && !looksLikeTarget(input.value)) {
    clearTimeout(typingTimer);
    input.value = "";
    clearQuery();
  }
});

// Another part of the interface asks for a search, such as "find this
// title" on the radio view.
document.addEventListener("unisson:search", (event) => {
  input.value = /** @type {CustomEvent<string>} */ (event).detail;
  input.focus();
  searchNow();
});

// Another window asked for a search, such as the mini-player.
api.onFocusSearch((text) => {
  input.focus();
  if (text) {
    // The launcher hands over what was typed in it.
    input.value = text;
    searchNow();
  } else {
    input.select();
  }
});

document.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
    event.preventDefault();
    input.focus();
    input.select();
  }
});
