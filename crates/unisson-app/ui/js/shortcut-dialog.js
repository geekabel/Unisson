/**
 * The help window that lists the keyboard shortcuts. F1 opens it from any
 * screen, and the Settings screen has a button for it.
 *
 * It is a native `<dialog>`: the browser keeps the keyboard inside it,
 * closes it with Escape, and gives the focus back where it was.
 */

import { byId, element } from "./dom.js";
import { SHORTCUT_GROUPS } from "./shortcut-list.js";

const dialog = /** @type {HTMLDialogElement} */ (byId("shortcuts-dialog"));
const body = byId("shortcuts-body");

/** Fills the window once: the list never changes while the page lives. */
function fill() {
  if (body.childElementCount > 0) {
    return;
  }
  body.append(
    ...SHORTCUT_GROUPS.map((group) =>
      element("section", { class: "shortcut-group" }, [
        element("h3", { class: "group-title" }, [group.title]),
        element("dl", { class: "shortcut-list" }, group.items.flatMap((item) => [
          element("dt", {}, item.keys.flatMap((keys, index) => [
            index > 0 ? " " : null,
            element("kbd", {}, [keys.join(" ")]),
          ])),
          element("dd", {}, [item.text]),
        ])),
      ]),
    ),
  );
}

/** Opens the window. */
export function openShortcuts() {
  if (dialog.open) {
    return;
  }
  fill();
  dialog.showModal();
}

byId("shortcuts-close").addEventListener("click", () => dialog.close());
// A click on the dimmed backdrop closes the window too.
dialog.addEventListener("click", (event) => {
  if (event.target === dialog) {
    dialog.close();
  }
});
