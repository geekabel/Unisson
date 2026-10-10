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
