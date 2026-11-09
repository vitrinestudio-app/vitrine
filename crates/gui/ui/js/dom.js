// Small DOM helpers and the inline SVG icon set (own drawings, 16×16,
// stroked with currentColor; no icon fonts, no external files).

const SVG_NS = 'http://www.w3.org/2000/svg';

/**
 * Creates an element: `h('button', { class: 'btn', onclick: fn }, 'Text', child)`.
 * Attributes starting with `on` become listeners; `dataset`/`aria` objects are
 * spread; `null`/`false` attributes and children are skipped. Never sets inline
 * `style` attributes (the Tauri CSP forbids them; use classes or `el.style.*`).
 */
export function h(tag, attrs = {}, ...children) {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs ?? {})) {
    if (v == null || v === false) continue;
    if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
    else if (k === 'dataset') Object.assign(el.dataset, v);
    else if (k === 'class') el.className = v;
    else if (k in el && typeof v !== 'string') el[k] = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  append(el, children);
  return el;
}

function append(el, children) {
  for (const c of children.flat()) {
    if (c == null || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
}

/** Creates an SVG element with attributes. */
export function s(tag, attrs = {}, ...children) {
  const el = document.createElementNS(SVG_NS, tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (v == null || v === false) continue;
    el.setAttribute(k, v);
  }
  append(el, children);
  return el;
}

/** Escapes text for SVG/HTML markup strings. */
export function esc(text) {
  return String(text).replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
}

// Path data of the icons (16×16 view box).
const ICONS = {
  logo: 'M3 12.5 6.5 4l3 6 1.5-3 2 5.5',
  undo: 'M6 4 3 7l3 3M3 7h6.5a3.5 3.5 0 0 1 0 7H8',
  redo: 'M10 4l3 3-3 3M13 7H6.5a3.5 3.5 0 0 0 0 7H8',
  chevron: 'M4.5 6.5 8 10l3.5-3.5',
  plus: 'M8 3.5v9M3.5 8h9',
  minus: 'M3.5 8h9',
  fit: 'M2.5 6V3.5a1 1 0 0 1 1-1H6M10 2.5h2.5a1 1 0 0 1 1 1V6M13.5 10v2.5a1 1 0 0 1-1 1H10M6 13.5H3.5a1 1 0 0 1-1-1V10',
  warning: 'M8 2.5 14 13H2L8 2.5ZM8 6.5v3M8 11.2v.1',
  check: 'M3.5 8.5 6.5 11.5 12.5 4.5',
  error: 'M8 2a6 6 0 1 1 0 12A6 6 0 0 1 8 2ZM8 5v3.5M8 10.8v.1',
  strips: 'M1.5 4.5h4v7h-4zM6 4.5h4v7H6zM10.5 4.5h4v7h-4z',
  layers: 'M8 2 14 5.2 8 8.4 2 5.2 8 2ZM2 8l6 3.2L14 8M2 10.8 8 14l6-3.2',
  assets: 'M2.5 3.5h11v9h-11zM2.5 10.5 6 7l3 3 1.5-1.5 3 3M10.5 6.2v.1',
  translations: 'M2 4h7M5.5 2.5V4M3.5 4c.6 2.6 2.4 4.4 4.5 5M7.5 4c-.5 2.7-2.2 4.6-5 5.5M8.5 14l2.8-7 2.8 7M9.3 12h4',
  canvas: 'M1.5 4h3.5v8H1.5zM6.25 4h3.5v8h-3.5zM11 4h3.5v8H11z',
  pages: 'M2 3.5h5v9H2zM9 3.5h5v9H9z',
  gallery: 'M1.5 3h13v10h-13zM4 5.5h3v5H4zM9 5.5h3v5H9z',
  languages: 'M8 2a6 6 0 1 1 0 12A6 6 0 0 1 8 2ZM2 8h12M8 2c1.8 1.7 2.6 3.7 2.6 6S9.8 12.3 8 14M8 2C6.2 3.7 5.4 5.7 5.4 8s.8 4.3 2.6 6',
  reset: 'M3 8a5 5 0 1 0 1.5-3.6M3 2.5V5h2.5',
  close: 'M4.5 4.5l7 7M11.5 4.5l-7 7',
  inspector: 'M2.5 2.5h11v11h-11zM10 2.5v11',
  device: 'M5 1.5h6a1 1 0 0 1 1 1v11a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1ZM7 12.5h2',
  text: 'M3 3.5h10M8 3.5v9.5M6 13h4',
  image: 'M2.5 3.5h11v9h-11zM2.5 10.5 6 7l3 3 1.5-1.5 3 3',
  shape: 'M3 3h10v10H3z',
  group: 'M2 2h5v5H2zM9 9h5v5H9zM7 4.5h2.5a1 1 0 0 1 1 1V9',
  background: 'M2 2.5h12v11H2zM2 9.5l12-5',
};

/** An inline SVG icon by name. */
export function icon(name, cls = '') {
  return s('svg', { class: `icon ${cls}`.trim(), viewBox: '0 0 16 16', 'aria-hidden': 'true' }, s('path', { d: ICONS[name] ?? '' }));
}

/** Compact native `<select>` that shows `label value ▾`. */
export function picker({ label, title, onchange }) {
  const value = h('span', { class: 'picker-value' });
  const select = h('select', { title, 'aria-label': title ?? label, onchange: () => onchange(select.value) });
  const el = h('label', { class: 'picker' }, label ? h('span', { class: 'picker-label' }, label) : null, value, icon('chevron'), select);
  return {
    el,
    select,
    /** `options`: [{ value, label, group? }]; selects `current`. */
    set(options, current, display) {
      const groups = new Map();
      const nodes = [];
      for (const o of options) {
        const opt = h('option', { value: o.value }, o.label);
        if (o.group) {
          if (!groups.has(o.group)) {
            const g = h('optgroup', { label: o.group });
            groups.set(o.group, g);
            nodes.push(g);
          }
          groups.get(o.group).append(opt);
        } else nodes.push(opt);
      }
      select.replaceChildren(...nodes);
      select.value = current;
      const chosen = options.find((o) => o.value === current);
      value.textContent = display ?? chosen?.short ?? chosen?.label ?? '—';
      select.disabled = options.length === 0;
    },
    value,
  };
}
