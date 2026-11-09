// Snapping for canvas drags: page edges and centres (columns of grouped
// images included), image edges, page margins used by margin text boxes
// (applied to every page), and the edges and centres of other elements.
// Everything is in canvas lu; the threshold comes from the zoom.

import { bounds } from '../geometry.js';

/**
 * Snap lines of `overlay`, excluding the elements in `exclude` (ids of the
 * dragged elements; members of excluded groups are skipped too).
 * Each line: `{ v, lo, hi, kind }` – position on its axis and the extent
 * along the other axis (for drawing the guide).
 */
export function snapLines(overlay, exclude) {
  const xs = [];
  const ys = [];
  const c = overlay.canvas;
  for (const p of overlay.pages) {
    const r = p.rect;
    for (const v of [r.x, r.x + r.w / 2, r.x + r.w]) xs.push({ v, lo: r.y, hi: r.y + r.h, kind: 'page' });
    for (const v of [r.y, r.y + r.h / 2, r.y + r.h]) ys.push({ v, lo: r.x, hi: r.x + r.w, kind: 'page' });
  }
  for (const img of overlay.images) {
    const r = img.rect;
    for (const v of [r.x, r.x + r.w]) xs.push({ v, lo: r.y, hi: r.y + r.h, kind: 'page' });
  }
  // Margins: the left/right insets of margin text boxes, on every page.
  const insets = new Set();
  for (const e of overlay.elements) {
    if (e.kind !== 'text' || !e.text?.margins || e.rotation) continue;
    const page = overlay.pages.find((p) => p.index === e.page);
    if (!page) continue;
    const left = Math.round(e.rect.x - page.rect.x);
    const right = Math.round(page.rect.x + page.rect.w - (e.rect.x + e.rect.w));
    if (left > 0 && left < page.rect.w / 2) insets.add(left);
    if (right > 0 && right < page.rect.w / 2) insets.add(right);
  }
  for (const p of overlay.pages) {
    const r = p.rect;
    for (const m of insets) {
      xs.push({ v: r.x + m, lo: r.y, hi: r.y + r.h, kind: 'margin' });
      xs.push({ v: r.x + r.w - m, lo: r.y, hi: r.y + r.h, kind: 'margin' });
    }
  }
  const skip = new Set(exclude);
  for (const e of overlay.elements) {
    if (e.kind === 'background' || skip.has(e.id) || e.groups?.some((g) => skip.has(g))) continue;
    const b = bounds(e.quad);
    if (b.w <= 0 && b.h <= 0) continue;
    // Elements far outside the canvas (bleeding shapes) still snap.
    for (const v of [b.x, b.x + b.w / 2, b.x + b.w]) xs.push({ v, lo: b.y, hi: b.y + b.h, kind: 'element' });
    for (const v of [b.y, b.y + b.h / 2, b.y + b.h]) ys.push({ v, lo: b.x, hi: b.x + b.w, kind: 'element' });
  }
  ys.push({ v: c.y, lo: c.x, hi: c.x + c.w, kind: 'page' }, { v: c.y + c.h, lo: c.x, hi: c.x + c.w, kind: 'page' });
  return { xs, ys };
}

function best(candidates, lines, threshold) {
  let found = null;
  for (const c of candidates) {
    for (const l of lines) {
      const d = l.v - c;
      if (Math.abs(d) <= threshold && (!found || Math.abs(d) < Math.abs(found.d) - 1e-9)) found = { d, line: l };
    }
  }
  return found;
}

/** All lines at `v` (several sources can share a position). */
function matching(lines, v) {
  return lines.filter((l) => Math.abs(l.v - v) < 1e-6);
}

/**
 * Snaps a moving box: returns the correction `{ dx, dy }` and the guides
 * `{ axis: 'x'|'y', v, lo, hi }` to draw. `edges.x`/`edges.y` limit which
 * box edges take part (default: start, centre, end).
 */
export function snapBox(box, lines, threshold, edges = {}) {
  const ex = edges.x ?? [box.x, box.x + box.w / 2, box.x + box.w];
  const ey = edges.y ?? [box.y, box.y + box.h / 2, box.y + box.h];
  const fx = ex.length ? best(ex, lines.xs, threshold) : null;
  const fy = ey.length ? best(ey, lines.ys, threshold) : null;
  const dx = fx?.d ?? 0;
  const dy = fy?.d ?? 0;
  const moved = { x: box.x + dx, y: box.y + dy, w: box.w, h: box.h };
  const guides = [];
  if (fx) {
    const v = fx.line.v;
    let lo = moved.y;
    let hi = moved.y + moved.h;
    for (const l of matching(lines.xs, v)) {
      lo = Math.min(lo, l.lo);
      hi = Math.max(hi, l.hi);
    }
    guides.push({ axis: 'x', v, lo, hi });
  }
  if (fy) {
    const v = fy.line.v;
    let lo = moved.x;
    let hi = moved.x + moved.w;
    for (const l of matching(lines.ys, v)) {
      lo = Math.min(lo, l.lo);
      hi = Math.max(hi, l.hi);
    }
    guides.push({ axis: 'y', v, lo, hi });
  }
  return { dx, dy, guides };
}

/** Rotation snapping: 0°, ±5°, ±15°, ±45°, ±90°, 180° within 3°, else whole degrees. */
export function snapAngle(deg) {
  const stops = [0, 5, -5, 15, -15, 45, -45, 90, -90, 180, -180];
  for (const s of stops) if (Math.abs(deg - s) <= 3) return s === -180 ? 180 : s;
  return Math.round(deg);
}
