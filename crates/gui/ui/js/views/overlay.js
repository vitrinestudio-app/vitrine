// SVG overlay of the canvas view, drawn in screen space (constant stroke
// widths and handle sizes at every zoom): hatched gaps, page labels,
// selection, hover, handles, ghosts, snapping guides, markers, readouts.
// Pure function of its input; the canvas view calls `drawOverlay` on every
// change (a few dozen nodes, rebuilt as one string).

import { bounds, rotate } from '../geometry.js';
import { esc } from '../dom.js';

export const HANDLE = 8;
export const ROTATE_OFFSET = 24;

const fmt = (v) => (Math.round(v * 10) / 10).toString();

/** Screen mapping of a view. */
export function screen(view) {
  return (p) => ({ x: p.x * view.zoom + view.panX, y: p.y * view.zoom + view.panY });
}

function poly(points, cls, extra = '') {
  return `<polygon class="${cls}" points="${points.map((p) => `${fmt(p.x)},${fmt(p.y)}`).join(' ')}" ${extra}/>`;
}

function rect(r, cls, extra = '') {
  return `<rect class="${cls}" x="${fmt(r.x)}" y="${fmt(r.y)}" width="${fmt(Math.max(0, r.w))}" height="${fmt(Math.max(0, r.h))}" ${extra}/>`;
}

function line(a, b, cls) {
  return `<line class="${cls}" x1="${fmt(a.x)}" y1="${fmt(a.y)}" x2="${fmt(b.x)}" y2="${fmt(b.y)}"/>`;
}

/** A rounded label ("pill") centred at `at` (screen). */
export function pill(at, text, dark = false) {
  const w = text.length * 6.4 + 14;
  const x = at.x - w / 2;
  const y = at.y - 10;
  return `<g class="ov-pill${dark ? ' dark' : ''}"><rect x="${fmt(x)}" y="${fmt(y)}" width="${fmt(w)}" height="20" rx="10"/><text x="${fmt(at.x)}" y="${fmt(at.y + 4)}" text-anchor="middle">${esc(text)}</text></g>`;
}

const CURSORS = ['ew-resize', 'nwse-resize', 'ns-resize', 'nesw-resize'];

/** Resize cursor for a handle direction (screen vector from the centre). */
function cursorFor(dx, dy) {
  const a = ((Math.atan2(dy, dx) * 180) / Math.PI + 360 + 22.5) % 180;
  return CURSORS[Math.floor(a / 45) % 4];
}

/** Rotation handle position (screen) of an overlay element. */
export function rotateHandleAt(el, toScreen) {
  const n = el.handles.find((h) => h.handle === 'n');
  if (!n) return null;
  const top = toScreen(n.at);
  const up = rotate({ x: 0, y: -ROTATE_OFFSET }, { x: 0, y: 0 }, el.rotation);
  return { stem: top, at: { x: top.x + up.x, y: top.y + up.y } };
}

/**
 * @param {SVGSVGElement} svg
 * @param {object} m model: overlay, zoom, panX, panY, selection, hover,
 *   dropTarget (device id under a dragged screenshot),
 *   variantActive, drag ({ ghosts, guides, readout, homePage, marquee,
 *   hideSelection }), showHandles, highlight ({ ids, alpha }: an assistant's
 *   last edit)
 */
export function drawOverlay(svg, m) {
  const o = m.overlay;
  if (!o) {
    svg.innerHTML = '';
    return;
  }
  const S = screen(m);
  const out = [];
  out.push(
    '<defs><pattern id="hatch" patternUnits="userSpaceOnUse" width="7" height="7" patternTransform="rotate(45)">' +
      '<line class="ov-hatch-line" x1="0" y1="0" x2="0" y2="7"/></pattern></defs>',
  );

  // Gaps: hatched over the semi-transparent content.
  for (const g of o.gaps) {
    const a = S({ x: g.x, y: g.y });
    const r = { x: a.x, y: a.y, w: g.w * m.zoom, h: g.h * m.zoom };
    out.push(rect(r, 'ov-gap'));
  }

  // Safe area of each output image (targets with `safe_area`): dashed.
  for (const img of o.images) {
    if (!img.safe) continue;
    const a = S({ x: img.safe.x, y: img.safe.y });
    out.push(rect({ x: a.x, y: a.y, w: img.safe.w * m.zoom, h: img.safe.h * m.zoom }, 'ov-safe'));
  }

  // Column dividers inside grouped images.
  for (const img of o.images) {
    if (img.pages.length < 2) continue;
    const pages = o.pages.filter((p) => img.pages.includes(p.index));
    for (const p of pages.slice(1)) {
      out.push(line(S({ x: p.rect.x, y: p.rect.y }), S({ x: p.rect.x, y: p.rect.y + p.rect.h }), 'ov-page-line'));
    }
  }

  // Page labels above the pages.
  const home = m.drag?.homePage;
  for (const p of o.pages) {
    const a = S({ x: p.rect.x, y: p.rect.y });
    const w = p.rect.w * m.zoom;
    if (w < 14) continue;
    const showId = w > 70;
    const cls = home === p.index ? 'ov-label home' : 'ov-label';
    out.push(
      `<text class="${cls}" x="${fmt(a.x + 1)}" y="${fmt(a.y - 8)}">${p.index + 1}${showId ? `<tspan class="ov-label-id" dx="6">${esc(p.id)}</tspan>` : ''}</text>`,
    );
    if (home === p.index) {
      out.push(rect({ x: a.x, y: a.y, w, h: p.rect.h * m.zoom }, 'ov-sel-group'));
    }
  }

  const byId = new Map(o.elements.map((e) => [e.id, e]));
  const groups = new Map(o.groups.map((g) => [g.id, g]));

  // Text markers: overflow (red) and emergency breaks (red, dashed).
  for (const e of o.elements) {
    if (!e.text) continue;
    if (e.text.overflow) out.push(poly(e.quad.map(S), 'ov-overflow'));
    else if (e.text.emergency_breaks?.length) out.push(poly(e.quad.map(S), 'ov-emergency'));
  }

  // Variant overrides: a small diamond at the top-right corner.
  if (m.variantActive) {
    const mark = (p) => {
      const c = S(p);
      return `<path class="ov-override" d="M${fmt(c.x)} ${fmt(c.y - 5)} L${fmt(c.x + 5)} ${fmt(c.y)} L${fmt(c.x)} ${fmt(c.y + 5)} L${fmt(c.x - 5)} ${fmt(c.y)} Z"/>`;
    };
    for (const e of o.elements) if (e.overridden?.length && e.kind !== 'background') out.push(mark(e.quad[1]));
    for (const g of o.groups) if (g.overridden?.length) out.push(mark({ x: g.bounds.x + g.bounds.w, y: g.bounds.y }));
  }

  const dragging = Boolean(m.drag?.ghosts);

  // Hover.
  if (m.hover && !m.selection.includes(m.hover) && !dragging) {
    const e = byId.get(m.hover);
    const g = groups.get(m.hover);
    if (e) out.push(poly(e.quad.map(S), 'ov-hover'));
    else if (g) {
      const a = S({ x: g.bounds.x, y: g.bounds.y });
      out.push(rect({ x: a.x, y: a.y, w: g.bounds.w * m.zoom, h: g.bounds.h * m.zoom }, 'ov-hover'));
    }
  }

  // Drop target (a screenshot dragged onto a device).
  if (m.dropTarget) {
    const e = byId.get(m.dropTarget);
    if (e) out.push(poly(e.quad.map(S), 'ov-drop'));
  }

  // Selection.
  if (!m.drag?.hideSelection) {
    for (const id of m.selection) {
      const e = byId.get(id);
      const g = groups.get(id);
      if (e) out.push(poly(e.quad.map(S), dragging ? 'ov-hover' : 'ov-sel'));
      else if (g) {
        for (const mid of g.members) {
          const me = byId.get(mid);
          if (me) out.push(poly(me.quad.map(S), 'ov-hover'));
        }
        const a = S({ x: g.bounds.x, y: g.bounds.y });
        out.push(rect({ x: a.x, y: a.y, w: g.bounds.w * m.zoom, h: g.bounds.h * m.zoom }, 'ov-sel-group'));
      }
    }
    if (m.selection.length > 1 && !dragging) {
      const pts = m.selection.flatMap((id) => byId.get(id)?.quad ?? (groups.get(id) ? rectPoints(groups.get(id).bounds) : []));
      if (pts.length) {
        const b = bounds(pts.map(S));
        out.push(rect({ x: b.x - 3, y: b.y - 3, w: b.w + 6, h: b.h + 6 }, 'ov-sel-group'));
      }
    }
  }

  // Elements an assistant just changed: a fading glow outline (ADR 0015).
  if (m.highlight) {
    const a = `opacity="${fmt(m.highlight.alpha)}"`;
    for (const id of m.highlight.ids) {
      const e = byId.get(id);
      const g = groups.get(id);
      const b = g?.bounds;
      const pts = e ? e.quad.map(S) : b ? [{ x: b.x, y: b.y }, { x: b.x + b.w, y: b.y }, { x: b.x + b.w, y: b.y + b.h }, { x: b.x, y: b.y + b.h }].map(S) : null;
      if (!pts) continue;
      out.push(poly(pts, 'ov-assistant-glow', a));
      out.push(poly(pts, 'ov-assistant', a));
    }
  }

  // Ghosts of the gesture in progress.
  for (const q of m.drag?.ghosts ?? []) out.push(poly(q.map(S), 'ov-ghost'));

  // Rotation pivot.
  if (m.drag?.pivot && m.drag.ghosts) {
    const c = S(m.drag.pivot);
    out.push(`<circle class="ov-pivot" cx="${fmt(c.x)}" cy="${fmt(c.y)}" r="4"/>`);
    out.push(line({ x: c.x - 7, y: c.y }, { x: c.x + 7, y: c.y }, 'ov-pivot'));
    out.push(line({ x: c.x, y: c.y - 7 }, { x: c.x, y: c.y + 7 }, 'ov-pivot'));
  }

  // Snapping guides.
  for (const g of m.drag?.guides ?? []) {
    const a = g.axis === 'x' ? S({ x: g.v, y: g.lo }) : S({ x: g.lo, y: g.v });
    const b = g.axis === 'x' ? S({ x: g.v, y: g.hi }) : S({ x: g.hi, y: g.v });
    out.push(line(a, b, 'ov-guide'));
    for (const p of [a, b]) {
      out.push(line({ x: p.x - 3, y: p.y - 3 }, { x: p.x + 3, y: p.y + 3 }, 'ov-guide-mark'));
      out.push(line({ x: p.x - 3, y: p.y + 3 }, { x: p.x + 3, y: p.y - 3 }, 'ov-guide-mark'));
    }
  }

  // Handles of a single selected element.
  if (m.showHandles && m.selection.length === 1 && !m.drag) {
    const e = byId.get(m.selection[0]);
    if (e && e.handles.length) {
      const c = S({ x: e.rect.x + e.rect.w / 2, y: e.rect.y + e.rect.h / 2 });
      const rot = rotateHandleAt(e, S);
      if (rot) {
        out.push(line(rot.stem, rot.at, 'rotate-stem'));
        out.push(`<circle class="rotate-handle" data-rotate="1" cx="${fmt(rot.at.x)}" cy="${fmt(rot.at.y)}" r="5"/>`);
      }
      const small = e.rect.w * m.zoom < 24 || e.rect.h * m.zoom < 24;
      for (const hd of e.handles) {
        if (small && hd.handle.length === 1) continue;
        const p = S(hd.at);
        const cur = cursorFor(p.x - c.x, p.y - c.y);
        const half = HANDLE / 2;
        const transform = e.rotation ? `transform="rotate(${fmt(e.rotation)} ${fmt(p.x)} ${fmt(p.y)})"` : '';
        out.push(
          `<rect class="handle" data-handle="${hd.handle}" x="${fmt(p.x - half)}" y="${fmt(p.y - half)}" width="${HANDLE}" height="${HANDLE}" rx="1.5" cursor="${cur}" ${transform}/>`,
        );
      }
    }
  }

  // Marquee.
  if (m.drag?.marquee) {
    const r = m.drag.marquee;
    const a = S({ x: r.x, y: r.y });
    out.push(rect({ x: a.x, y: a.y, w: r.w * m.zoom, h: r.h * m.zoom }, 'ov-marquee'));
  }

  // Readouts.
  for (const r of m.drag?.readouts ?? []) out.push(pill(r.at, r.text, r.dark));

  svg.innerHTML = out.join('');
}

function rectPoints(r) {
  return [
    { x: r.x, y: r.y },
    { x: r.x + r.w, y: r.y + r.h },
  ];
}
