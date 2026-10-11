/**
 * Small helpers to build page elements without writing HTML in strings.
 * Text always goes through `textContent`, so a title can never inject markup.
 */

/**
 * Creates an element.
 *
 * @param {string} tag
 * @param {Record<string, string | boolean | null | undefined>} [attributes]
 *   `class` and any attribute. `false`, `null` and `undefined` are skipped.
 * @param {(Node | string | null | undefined)[]} [children]
 * @returns {HTMLElement}
 */
export function element(tag, attributes = {}, children = []) {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) {
    if (value === false || value == null) {
      continue;
    }
    node.setAttribute(name, value === true ? "" : value);
  }
  for (const child of children) {
    if (child != null) {
      node.append(child);
    }
  }
  return node;
}

/** What can take the keyboard. */
const CONTROLS = "button, input, select, textarea, a[href], [tabindex]";

/** Describes a control well enough to find its twin after a redraw. */
function signature(node) {
  return `${node.tagName}|${node.className}|${node.getAttribute("aria-label") ?? node.textContent.trim()}`;
}

/**
 * Replaces the children of `container`, and keeps the keyboard where it
 * was.
 *
 * The interface redraws whole areas when something changes: a track
 * starts, a source answers. If the control that had the keyboard is among
 * the replaced ones, the browser drops the keyboard to the top of the
 * page, and someone who uses Tab has to walk all the way back. This puts it
 * on the control that took the place of the old one: the one with the
 * same `data-focus-key`, else the same look and name, else the same rank.
 *
 * @param {Element} container
 * @param {...(Node | string)} children
 */
export function replaceKeepingFocus(container, ...children) {
  const active = document.activeElement;
  const mark =
    active instanceof HTMLElement && active !== container && container.contains(active)
      ? {
          key: active.dataset.focusKey,
          signature: signature(active),
          rank: [...container.querySelectorAll(CONTROLS)].indexOf(active),
        }
      : null;
  container.replaceChildren(...children);
  if (!mark) {
    return;
  }
  const controls = [...container.querySelectorAll(CONTROLS)].filter((control) => !control.disabled);
  const twin =
    (mark.key && controls.find((control) => control.dataset.focusKey === mark.key)) ||
    controls.find((control) => signature(control) === mark.signature) ||
    controls[Math.min(mark.rank, controls.length - 1)];
  // The scroll position is the business of the caller.
  twin?.focus({ preventScroll: true });
}

const SVG = "http://www.w3.org/2000/svg";

/**
 * Creates an icon that reuses a `<symbol>` of the page.
 *
 * @param {string} name The symbol id, without its `icon-` prefix.
 * @param {{filled?: boolean}} [options]
 * @returns {SVGElement}
 */
export function icon(name, { filled = false } = {}) {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("class", filled ? "icon icon-filled" : "icon");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(SVG, "use");
  use.setAttribute("href", `#icon-${name}`);
  svg.append(use);
  return svg;
}

/**
 * Returns an element of the page by its id.
 *
 * @param {string} id
 * @returns {HTMLElement}
 */
export function byId(id) {
  const node = document.getElementById(id);
  if (!node) {
    throw new Error(`missing element: #${id}`);
  }
  return node;
}
