/**
 * A small menu that opens next to a button, such as "add to a list".
 *
 * Only one menu is open at a time. It closes on a choice, on Escape, and
 * on a click anywhere else. The keyboard moves through it with the arrows.
 */

import { element } from "./dom.js";

/** @type {?{node: HTMLElement, anchor: HTMLElement}} */
let open = null;

/** Closes the menu, and gives the focus back to its button. */
export function closeMenu({ restoreFocus = true } = {}) {
  if (!open) {
    return;
  }
  const { node, anchor } = open;
  open = null;
  node.remove();
  anchor.setAttribute("aria-expanded", "false");
  if (restoreFocus) {
    anchor.focus();
  }
}

/**
 * Opens a menu under `anchor`, or closes it if it is the one already open.
 *
 * @param {HTMLElement} anchor The button that opens the menu.
 * @param {string} label What the menu is for, for screen readers.
 * @param {(Node)[]} content The entries: see {@link menuItem}.
 */
export function toggleMenu(anchor, label, content) {
  const wasOpenHere = open?.anchor === anchor;
  closeMenu({ restoreFocus: false });
  if (wasOpenHere) {
    return;
  }
  const node = element("div", { class: "menu", role: "menu", "aria-label": label }, content);
  document.body.append(node);
  anchor.setAttribute("aria-expanded", "true");
  open = { node, anchor };
  place(node, anchor);
  firstFocusable(node)?.focus();
}

/**
 * Replaces the content of the open menu.
 *
 * @param {Node[]} content
 */
export function setMenuContent(content) {
  if (!open) {
    return;
  }
  open.node.replaceChildren(...content);
  place(open.node, open.anchor);
  firstFocusable(open.node)?.focus();
}

/**
 * Builds an entry of a menu.
 *
 * @param {string} label
 * @param {() => void} run What the entry does. The menu closes first,
 *   unless `keepOpen` is set.
 * @param {{keepOpen?: boolean, note?: string}} [options]
 */
export function menuItem(label, run, { keepOpen = false, note } = {}) {
  const item = element("button", { class: "menu-item", type: "button", role: "menuitem" }, [
    element("span", { class: "truncate" }, [label]),
    note ? element("span", { class: "section-note" }, [note]) : null,
  ]);
  item.addEventListener("click", () => {
    if (!keepOpen) {
      closeMenu();
    }
    run();
  });
  return item;
}

function firstFocusable(node) {
  return node.querySelector("button, input");
}

/** Puts the menu under its button, inside the window. */
function place(node, anchor) {
  const button = anchor.getBoundingClientRect();
  const menu = node.getBoundingClientRect();
  const margin = 8;
  const left = Math.min(
    Math.max(margin, button.right - menu.width),
    window.innerWidth - menu.width - margin,
  );
  const below = button.bottom + 4;
  // Open upwards when there is no room below.
  const top = below + menu.height > window.innerHeight - margin
    ? Math.max(margin, button.top - menu.height - 4)
    : below;
  node.style.left = `${left}px`;
  node.style.top = `${top}px`;
}

document.addEventListener("pointerdown", (event) => {
  if (open && !open.node.contains(event.target) && !open.anchor.contains(event.target)) {
    closeMenu({ restoreFocus: false });
  }
});

document.addEventListener("keydown", (event) => {
  if (!open) {
    return;
  }
  if (event.key === "Escape") {
    event.preventDefault();
    event.stopPropagation();
    closeMenu();
    return;
  }
  if (event.key !== "ArrowDown" && event.key !== "ArrowUp") {
    return;
  }
  const items = [...open.node.querySelectorAll("button")];
  if (items.length === 0) {
    return;
  }
  event.preventDefault();
  const current = items.indexOf(document.activeElement);
  const step = event.key === "ArrowDown" ? 1 : -1;
  items[(current + step + items.length) % items.length].focus();
}, true);

// The page under the menu moved: its place is no longer right.
window.addEventListener("resize", () => closeMenu({ restoreFocus: false }));
document.addEventListener("scroll", () => closeMenu({ restoreFocus: false }), true);
