// Plain 2D helpers for canvas lu geometry (overlay quads, rects, rotation).

export const pt = (x, y) => ({ x, y });
export const add = (a, b) => ({ x: a.x + b.x, y: a.y + b.y });
export const sub = (a, b) => ({ x: a.x - b.x, y: a.y - b.y });
export const scale = (a, k) => ({ x: a.x * k, y: a.y * k });
export const dot = (a, b) => a.x * b.x + a.y * b.y;

/** Rotates `p` around `c` by `deg` degrees (canvas: y down, clockwise). */
export function rotate(p, c, deg) {
  if (!deg) return { ...p };
  const r = (deg * Math.PI) / 180;
  const [s, k] = [Math.sin(r), Math.cos(r)];
  const dx = p.x - c.x;
  const dy = p.y - c.y;
  return { x: c.x + dx * k - dy * s, y: c.y + dx * s + dy * k };
}

/** Unit vectors of a box rotated by `deg`: u (local x), v (local y). */
export function axes(deg) {
  const r = (deg * Math.PI) / 180;
  return { u: { x: Math.cos(r), y: Math.sin(r) }, v: { x: -Math.sin(r), y: Math.cos(r) } };
}

/** Is `p` inside the convex quad `q` (4 points, either winding)? */
export function inQuad(p, q) {
  let sign = 0;
  for (let i = 0; i < 4; i++) {
    const a = q[i];
    const b = q[(i + 1) % 4];
    const cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
    if (Math.abs(cross) < 1e-9) continue;
    const sgn = Math.sign(cross);
    if (sign === 0) sign = sgn;
    else if (sgn !== sign) return false;
  }
  return true;
}

export function inRect(p, r) {
  return p.x >= r.x && p.x <= r.x + r.w && p.y >= r.y && p.y <= r.y + r.h;
}

/** Axis-aligned bounds of points. */
export function bounds(points) {
  let [x0, y0, x1, y1] = [Infinity, Infinity, -Infinity, -Infinity];
  for (const p of points) {
    x0 = Math.min(x0, p.x);
    y0 = Math.min(y0, p.y);
    x1 = Math.max(x1, p.x);
    y1 = Math.max(y1, p.y);
  }
  return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
}

export function union(a, b) {
  if (!a) return b;
  if (!b) return a;
  const x = Math.min(a.x, b.x);
  const y = Math.min(a.y, b.y);
  return { x, y, w: Math.max(a.x + a.w, b.x + b.w) - x, h: Math.max(a.y + a.h, b.y + b.h) - y };
}

export function intersects(a, b) {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

export function inflate(r, d) {
  return { x: r.x - d, y: r.y - d, w: r.w + 2 * d, h: r.h + 2 * d };
}

export function rectQuad(r) {
  return [pt(r.x, r.y), pt(r.x + r.w, r.y), pt(r.x + r.w, r.y + r.h), pt(r.x, r.y + r.h)];
}

/** Angle in (-180, 180]. */
export function normAngle(deg) {
  let a = ((deg % 360) + 360) % 360;
  if (a > 180) a -= 360;
  return a;
}

/** The page (column) whose x-range contains `x`, else the nearest one (as core does). */
export function pageAt(pages, x) {
  let best = null;
  let bestD = Infinity;
  for (const p of pages) {
    if (x >= p.rect.x && x < p.rect.x + p.rect.w) return p;
    const d = Math.min(Math.abs(x - p.rect.x), Math.abs(x - (p.rect.x + p.rect.w)));
    if (d < bestD) {
      bestD = d;
      best = p;
    }
  }
  return best;
}
