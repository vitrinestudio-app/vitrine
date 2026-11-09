// Canvas view: the strip of the current target as Rust-rendered pixmaps
// (one per output image, plus the gaps rendered separately, shown
// semi-transparent under a hatch) positioned in canvas lu, a screen-space
// SVG overlay, zoom/pan and direct manipulation (select, marquee, move,
// resize, rotate, nudge) with snapping and live previews.
//
// Drag feedback: the overlay ghost follows the pointer immediately; in
// parallel the affected output images are re-rendered from a scratch copy
// of the project (`render_preview_edit`, interactive quality, self-paced at
// ≤ 15 fps). On drop one `edit_*` command commits the gesture; the preview
// pixmaps stay until the real ones arrive, so nothing flickers.

import { api, toImageBitmap } from '../api.js';
import { h, s } from '../dom.js';
import { editor } from '../editor.js';
import { add, axes, bounds, dot, inQuad, inRect, inflate, intersects, normAngle, pageAt, rotate, sub, union } from '../geometry.js';
import { t } from '../i18n.js';
import { currentTarget, effectiveVariant, store, viewArgs } from '../store.js';
import { drawOverlay, screen } from './overlay.js';
import { snapAngle, snapBox, snapLines } from './snap.js';

const ZOOM_STEPS = [0.02, 0.03, 0.05, 0.075, 0.1, 0.15, 0.2, 0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4, 6, 8];
const MIN_ZOOM = 0.02;
const MAX_ZOOM = 8;
const MAX_SIDE = 8192;
const DRAG_THRESHOLD = 3;
const SNAP_PX = 6;
const PREVIEW_INTERVAL = 66;
const FIT_PADDING = 56;
const OPPOSITE = { nw: 'se', n: 's', ne: 'sw', e: 'w', se: 'nw', s: 'n', sw: 'ne', w: 'e' };
const UNIT = { nw: [-1, -1], n: [0, -1], ne: [1, -1], e: [1, 0], se: [1, 1], s: [0, 1], sw: [-1, 1], w: [-1, 0] };

const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

/**
 * Mounts the canvas view into `root`. Returns the view API used by the
 * shell and shortcuts: `fit()`, `zoomIn()`, `zoomOut()`, `zoomTo(z)`,
 * `zoomActual()`, `nudge(dx, dy)`, `cancel()`, `percent()`, `unmount()`.
 */
export function mountCanvas(root) {
  const world = h('div', { class: 'canvas-world' });
  const svg = s('svg', { class: 'canvas-overlay' });
  const empty = h('div', { class: 'canvas-empty', hidden: true }, t('canvas.empty'));
  const el = h('div', { class: 'canvas-view', tabindex: '0' }, world, svg, empty);
  root.append(el);

  /** Rendered pixmaps: output images and gaps. */
  let slots = [];
  let slotSignature = '';
  /** Gesture in progress (see the start* functions). */
  let drag = null;
  let dragSeq = 0;
  let spaceDown = false;
  let fitPending = true;
  let resTimer = 0;
  const pump = { inflight: false, sentKey: null, last: 0, timer: 0 };
  const unsubs = [];

  // ----- coordinates ------------------------------------------------------

  const view = () => {
    const st = store.get();
    return { zoom: st.zoom, panX: st.panX, panY: st.panY };
  };

  function toLu(e) {
    const r = el.getBoundingClientRect();
    const { zoom, panX, panY } = view();
    return { x: (e.clientX - r.left - panX) / zoom, y: (e.clientY - r.top - panY) / zoom };
  }

  function localPoint(e) {
    const r = el.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  /** Export px per lu of the current target (100 % zoom). */
  function exportScale() {
    const o = store.get().overlay;
    const target = currentTarget();
    if (!o?.images.length || !target) return 1;
    return target.size[0] / o.images[0].rect.w;
  }

  // ----- slots (pixmaps) ----------------------------------------------------

  function makeSlot(kind, index, rect) {
    const real = h('canvas', { class: 'real' });
    const preview = h('canvas', { class: 'preview' });
    const node = h('div', { class: `canvas-slot ${kind}` }, real, preview);
    node.style.left = `${rect.x}px`;
    node.style.top = `${rect.y}px`;
    node.style.width = `${rect.w}px`;
    node.style.height = `${rect.h}px`;
    world.append(node);
    return { kind, index, rect, el: node, real, preview, level: 0, gen: 0, dirty: true };
  }

  function rebuildSlots() {
    const o = store.get().overlay;
    world.replaceChildren();
    slots = [];
    empty.hidden = Boolean(o);
    if (!o) return;
    for (const img of o.images) slots.push(makeSlot('image', img.index, img.rect));
    o.gaps.forEach((g, i) => slots.push(makeSlot('gap', i, g)));
    slotSignature = signature(o);
  }

  function signature(o) {
    return JSON.stringify([o.canvas, o.images.map((i) => i.rect), o.gaps]);
  }

  /** Pixel density wanted for the current zoom (quantised to √2 steps). */
  function levelFor(slot) {
    const want = view().zoom * (window.devicePixelRatio || 1);
    let level = 2 ** (Math.ceil(Math.log2(want) * 2) / 2);
    level = Math.min(level, exportScale());
    level = Math.min(level, MAX_SIDE / Math.max(slot.rect.w, slot.rect.h));
    return level;
  }

  function isVisible(slot) {
    const { zoom, panX, panY } = view();
    const r = { x: slot.rect.x * zoom + panX, y: slot.rect.y * zoom + panY, w: slot.rect.w * zoom, h: slot.rect.h * zoom };
    return intersects(r, { x: -200, y: -200, w: el.clientWidth + 400, h: el.clientHeight + 400 });
  }

  async function renderSlot(slot) {
    const level = levelFor(slot);
    const gen = ++slot.gen;
    slot.dirty = false;
    const v = viewArgs();
    try {
      const width = Math.max(1, Math.round(slot.rect.w * level));
      const img =
        slot.kind === 'image' ? await api.renderImage(v, slot.index, width) : await api.renderRegion(v, slot.rect, level);
      const bitmap = await toImageBitmap(img);
      if (gen !== slot.gen) return;
      slot.real.width = bitmap.width;
      slot.real.height = bitmap.height;
      slot.real.getContext('2d').drawImage(bitmap, 0, 0);
      bitmap.close?.();
      slot.level = level;
      if (!drag || drag.committing) slot.el.classList.remove('previewing');
    } catch (e) {
      if (gen === slot.gen) {
        slot.dirty = true;
        console.warn('render failed', e);
      }
    }
  }

  /** Renders visible slots that are dirty or at the wrong density. */
  function updateResolution() {
    for (const slot of slots) {
      if (!isVisible(slot)) continue;
      if (slot.dirty || Math.abs(slot.level - levelFor(slot)) > 1e-9) renderSlot(slot);
    }
  }

  function scheduleResolution(delay = 160) {
    clearTimeout(resTimer);
    resTimer = setTimeout(updateResolution, delay);
  }

  function invalidate({ all, rects }) {
    const area = rects ?? [];
    for (const slot of slots) {
      const hit = all || area.some((r) => intersects(r, slot.rect)) || slot.el.classList.contains('previewing');
      if (!hit) continue;
      slot.dirty = true;
      if (isVisible(slot)) renderSlot(slot);
      else slot.el.classList.remove('previewing');
    }
  }

  // ----- zoom / pan ------------------------------------------------------------

  function applyTransform() {
    const { zoom, panX, panY } = view();
    world.style.transform = `translate(${panX}px, ${panY}px) scale(${zoom})`;
  }

  function setView(zoom, panX, panY) {
    store.set({ zoom: clamp(zoom, MIN_ZOOM, MAX_ZOOM), panX, panY });
  }

  function zoomAround(z, p) {
    const { zoom, panX, panY } = view();
    const nz = clamp(z, MIN_ZOOM, MAX_ZOOM);
    setView(nz, p.x - ((p.x - panX) * nz) / zoom, p.y - ((p.y - panY) * nz) / zoom);
  }

  const centre = () => ({ x: el.clientWidth / 2, y: el.clientHeight / 2 });

  function fit() {
    const o = store.get().overlay;
    if (!o || !el.clientWidth) return;
    const c = o.canvas;
    const w = el.clientWidth - FIT_PADDING * 2;
    const hh = el.clientHeight - FIT_PADDING * 2 - 16;
    const z = clamp(Math.min(w / c.w, hh / c.h), MIN_ZOOM, MAX_ZOOM);
    setView(z, (el.clientWidth - c.w * z) / 2 - c.x * z, (el.clientHeight - c.h * z) / 2 - c.y * z + 8);
    fitPending = false;
  }

  function step(dir) {
    const z = view().zoom;
    const next = dir > 0 ? ZOOM_STEPS.find((v) => v > z * 1.001) : [...ZOOM_STEPS].reverse().find((v) => v < z / 1.001);
    zoomAround(next ?? z, centre());
  }

  el.addEventListener(
    'wheel',
    (e) => {
      e.preventDefault();
      const unit = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? el.clientHeight : 1;
      if (e.ctrlKey || e.metaKey) {
        const d = clamp(e.deltaY * unit, -50, 50);
        zoomAround(view().zoom * Math.exp(-d * 0.005), localPoint(e));
      } else {
        const { zoom, panX, panY } = view();
        const dx = (e.shiftKey && !e.deltaX ? e.deltaY : e.deltaX) * unit;
        const dy = (e.shiftKey && !e.deltaX ? 0 : e.deltaY) * unit;
        setView(zoom, panX - dx, panY - dy);
      }
    },
    { passive: false },
  );

  // ----- hit testing ---------------------------------------------------------------

  function groupsOf(o, id) {
    return o.elements.find((e) => e.id === id)?.groups ?? o.groups.find((g) => g.id === id)?.groups ?? [];
  }

  /** Element (or enclosing group) at canvas point `p`; `deep` skips groups. */
  function hitTest(p, deep = false) {
    const o = store.get().overlay;
    if (!o) return null;
    const sel = store.get().selection;
    for (let i = o.elements.length - 1; i >= 0; i--) {
      const e = o.elements[i];
      if (e.kind === 'background') continue;
      if (e.rect.w <= 0 && e.rect.h <= 0) continue;
      if (!inQuad(p, e.quad)) continue;
      if (e.clip && !inRect(p, e.clip)) continue;
      if (deep || !e.groups?.length) return e.id;
      // Outermost group, unless the selection is already inside a group of
      // the path (then the next level down).
      const path = [...e.groups, e.id];
      let level = 0;
      for (let k = 1; k < path.length; k++) {
        const parent = path[k - 1];
        if (sel.some((id) => id !== parent && groupsOf(o, id).includes(parent))) level = k;
      }
      return path[level];
    }
    return null;
  }

  /** Top-level ids (outermost groups) of elements intersecting `r`. */
  function inMarquee(r) {
    const o = store.get().overlay;
    const ids = new Set();
    for (const e of o.elements) {
      if (e.kind === 'background') continue;
      const b = bounds(e.quad);
      if (!intersects(b, r)) continue;
      ids.add(e.groups?.[0] ?? e.id);
    }
    return [...ids];
  }

  // ----- gestures ------------------------------------------------------------------

  function itemsOf(ids) {
    const o = store.get().overlay;
    const quads = [];
    for (const id of ids) {
      const e = o.elements.find((x) => x.id === id);
      if (e) {
        quads.push(e.quad);
        continue;
      }
      const g = o.groups.find((x) => x.id === id);
      if (g) for (const m of g.members) quads.push(o.elements.find((x) => x.id === m).quad);
    }
    return quads;
  }

  function movePointOf(id) {
    const o = store.get().overlay;
    return o.elements.find((x) => x.id === id)?.move_point ?? o.groups.find((x) => x.id === id)?.move_point;
  }

  function startMove(ids, p, screenPt) {
    const quads = itemsOf(ids);
    if (!quads.length) return;
    drag = {
      type: 'move',
      session: ++dragSeq,
      ids,
      start: p,
      startScreen: screenPt,
      moved: false,
      quads,
      box0: bounds(quads.flat()),
      lines: snapLines(store.get().overlay, ids),
      delta: { x: 0, y: 0 },
    };
  }

  /** Rounds free (unsnapped) deltas so the YAML stays tidy. */
  function tidy(v) {
    const z = view().zoom;
    const q = z >= 2 ? 0.1 : 1;
    return Math.round(v / q) * q;
  }

  function updateMove(e) {
    const p = toLu(e);
    const sp = localPoint(e);
    if (!drag.moved && Math.hypot(sp.x - drag.startScreen.x, sp.y - drag.startScreen.y) < DRAG_THRESHOLD) return;
    drag.moved = true;
    let raw = sub(p, drag.start);
    if (e.shiftKey) raw = Math.abs(raw.x) > Math.abs(raw.y) ? { x: raw.x, y: 0 } : { x: 0, y: raw.y };
    raw = { x: tidy(raw.x), y: tidy(raw.y) };
    let guides = [];
    if (!e.altKey) {
      const box = { ...drag.box0, x: drag.box0.x + raw.x, y: drag.box0.y + raw.y };
      const snap = snapBox(box, drag.lines, SNAP_PX / view().zoom, {
        x: e.shiftKey && raw.x === 0 ? [] : undefined,
        y: e.shiftKey && raw.y === 0 ? [] : undefined,
      });
      raw = { x: raw.x + snap.dx, y: raw.y + snap.dy };
      guides = snap.guides;
    }
    drag.delta = raw;
    drag.guides = guides;
    drag.ghosts = drag.quads.map((q) => q.map((pt) => add(pt, raw)));
    drag.edit = { kind: 'move_by', ids: drag.ids, dx: raw.x, dy: raw.y };
    // Home page follows the reference point (single element/group).
    const o = store.get().overlay;
    const mp = movePointOf(drag.ids[0]);
    const page = mp && drag.ids.length === 1 ? pageAt(o.pages, mp.x + raw.x) : null;
    drag.homePage = page?.index;
    const S = screen(view());
    const gb = bounds(drag.ghosts.flat().map(S));
    drag.readouts = page ? [{ at: { x: gb.x + gb.w / 2, y: gb.y - 16 }, text: `${t('canvas.page')} ${page.index + 1} · ${page.id}` }] : [];
    schedulePreview();
  }

  function startResize(id, handle, p) {
    const o = store.get().overlay;
    const e = o.elements.find((x) => x.id === id);
    if (!e) return;
    const fixed = OPPOSITE[handle];
    const F = e.handles.find((x) => x.handle === fixed).at;
    const { u, v } = axes(e.rotation);
    drag = {
      type: 'resize',
      session: ++dragSeq,
      id,
      el: e,
      handle,
      fixed,
      F,
      u,
      v,
      w0: e.rect.w,
      h0: e.rect.h,
      start: p,
      moved: true,
      box0: bounds(e.quad),
      lines: snapLines(o, [id]),
    };
  }

  function updateResize(ev) {
    const p = toLu(ev);
    const d = drag;
    const [hx, hy] = UNIT[d.handle];
    const rel = sub(p, d.F);
    let w = hx ? Math.max(1, dot(rel, d.u) * hx) : d.w0;
    let hh = hy ? Math.max(1, dot(rel, d.v) * hy) : d.h0;
    let guides = [];
    // Snap the moving edges of unrotated elements.
    if (!ev.altKey && !d.el.rotation) {
      const lines = d.lines;
      const thr = SNAP_PX / view().zoom;
      const snap = snapBox(
        { x: hx > 0 ? d.F.x + w : d.F.x - w, y: hy > 0 ? d.F.y + hh : d.F.y - hh, w: 0, h: 0 },
        lines,
        thr,
        { x: hx ? undefined : [], y: hy ? undefined : [] },
      );
      if (hx) w = Math.max(1, w + snap.dx * hx);
      if (hy) hh = Math.max(1, hh + snap.dy * hy);
      guides = snap.guides;
    }
    const lock = d.el.aspect_locked || (ev.shiftKey && d.el.kind !== 'text');
    if (lock && d.w0 > 0 && d.h0 > 0) {
      const k = hx && hy ? Math.max(w / d.w0, hh / d.h0) : hx ? w / d.w0 : hh / d.h0;
      w = d.w0 * k;
      hh = d.h0 * k;
      guides = guides.filter((g) => (hx && hy ? true : hx ? g.axis === 'x' : g.axis === 'y'));
    }
    if (!guides.length) {
      w = Math.max(1, tidy(w));
      hh = Math.max(1, tidy(hh));
    }
    const C = add(d.F, add({ x: d.u.x * ((hx * w) / 2), y: d.u.y * ((hx * w) / 2) }, { x: d.v.x * ((hy * hh) / 2), y: d.v.y * ((hy * hh) / 2) }));
    const corner = (sx, sy) => ({
      x: C.x + d.u.x * ((sx * w) / 2) + d.v.x * ((sy * hh) / 2),
      y: C.y + d.u.y * ((sx * w) / 2) + d.v.y * ((sy * hh) / 2),
    });
    d.size = { w, h: hh };
    d.ghosts = [[corner(-1, -1), corner(1, -1), corner(1, 1), corner(-1, 1)]];
    d.guides = guides;
    const size = d.el.aspect_locked ? { width: w } : { width: hx ? w : undefined, height: hy ? hh : undefined };
    d.edit = { kind: 'resize', id: d.id, fixed: d.fixed, ...size };
    const sp = localPoint(ev);
    d.readouts = [{ at: { x: sp.x + 16, y: sp.y + 26 }, text: `${Math.round(w)} × ${Math.round(hh)}`, dark: true }];
    schedulePreview();
  }

  function startRotate(id, p) {
    const e = store.get().overlay.elements.find((x) => x.id === id);
    if (!e) return;
    drag = {
      type: 'rotate',
      session: ++dragSeq,
      id,
      el: e,
      pivot: e.pivot,
      a0: Math.atan2(p.y - e.pivot.y, p.x - e.pivot.x),
      r0: e.rotation,
      angle: e.rotation,
      moved: true,
      box0: bounds(e.quad),
    };
  }

  function updateRotate(ev) {
    const p = toLu(ev);
    const d = drag;
    const a = Math.atan2(p.y - d.pivot.y, p.x - d.pivot.x);
    const raw = normAngle(d.r0 + ((a - d.a0) * 180) / Math.PI);
    d.angle = ev.shiftKey ? Math.round(raw * 10) / 10 : snapAngle(raw);
    const delta = d.angle - d.r0;
    d.ghosts = [d.el.quad.map((q) => rotate(q, d.pivot, delta))];
    d.edit = { kind: 'rotate', id: d.id, degrees: d.angle };
    const sp = localPoint(ev);
    d.readouts = [{ at: { x: sp.x + 18, y: sp.y + 26 }, text: `${fmtAngle(d.angle)}°`, dark: true }];
    schedulePreview();
  }

  function fmtAngle(a) {
    return Number.isInteger(a) ? String(a) : a.toFixed(1);
  }

  function startMarquee(p, additive) {
    drag = { type: 'marquee', start: p, additive, base: additive ? store.get().selection : [] };
  }

  function updateMarquee(e) {
    const p = toLu(e);
    const a = drag.start;
    const r = { x: Math.min(a.x, p.x), y: Math.min(a.y, p.y), w: Math.abs(p.x - a.x), h: Math.abs(p.y - a.y) };
    drag.marquee = r;
    const hits = r.w * view().zoom > 2 || r.h * view().zoom > 2 ? inMarquee(r) : [];
    store.set({ selection: [...new Set([...drag.base, ...hits])] });
  }

  async function commit() {
    const d = drag;
    if (!d) return;
    d.committing = true;
    d.guides = [];
    d.readouts = [];
    redraw();
    const v = viewArgs();
    let call = null;
    if (d.type === 'move' || d.type === 'nudge') {
      if (d.delta.x || d.delta.y) call = api.moveBy(v, d.ids, d.delta.x, d.delta.y);
    } else if (d.type === 'resize' && d.edit) {
      call = api.resize(v, d.id, { width: d.edit.width, height: d.edit.height }, d.fixed);
    } else if (d.type === 'rotate' && Math.abs(d.angle - d.r0) > 1e-9) {
      call = api.rotate(v, d.id, d.angle);
    }
    if (call) await editor.apply(call, { touched: d.ids ?? [d.id] });
    if (drag === d) drag = null;
    if (!call) clearPreviews();
    redraw();
  }

  function cancel() {
    if (!drag) return false;
    if (drag.type === 'nudge') clearTimeout(drag.timer);
    drag = null;
    clearPreviews();
    redraw();
    return true;
  }

  function clearPreviews() {
    for (const slot of slots) slot.el.classList.remove('previewing');
  }

  // ----- live previews -------------------------------------------------------------

  function schedulePreview() {
    redraw();
    if (pump.inflight) return;
    clearTimeout(pump.timer);
    const wait = Math.max(0, PREVIEW_INTERVAL - (performance.now() - pump.last));
    pump.timer = setTimeout(sendPreview, wait);
  }

  async function sendPreview() {
    const d = drag;
    if (!d?.edit || d.committing) return;
    const key = JSON.stringify(d.edit);
    if (key === pump.sentKey) return;
    pump.inflight = true;
    pump.sentKey = key;
    pump.last = performance.now();
    d.previewed ??= new Set();
    const area = inflate(union(d.box0, bounds(d.ghosts.flat())), 60);
    const targets = slots.filter((x) => x.kind === 'image' && (intersects(area, x.rect) || d.previewed.has(x)));
    const v = viewArgs();
    await Promise.all(
      targets.map(async (slot) => {
        try {
          const width = Math.max(1, slot.real.width || Math.round(slot.rect.w * levelFor(slot)));
          // Small images are cheap: only large ones drop to half size.
          const img = await api.renderPreviewEdit(v, d.edit, slot.index, width, width > 600);
          const bitmap = await toImageBitmap(img);
          if (drag !== d || d.committing) return;
          slot.preview.width = bitmap.width;
          slot.preview.height = bitmap.height;
          slot.preview.getContext('2d').drawImage(bitmap, 0, 0);
          bitmap.close?.();
          slot.el.classList.add('previewing');
          d.previewed.add(slot);
        } catch (e) {
          // An unsolvable intermediate position: keep the last preview.
          if (e.code !== 'op_rejected') console.warn('preview failed', e);
        }
      }),
    );
    pump.inflight = false;
    if (drag === d && !d.committing && JSON.stringify(d.edit) !== pump.sentKey) schedulePreview();
  }

  // ----- pointer ---------------------------------------------------------------------

  el.addEventListener('pointerdown', (e) => {
    el.focus({ preventScroll: true });
    if (drag?.type === 'nudge') {
      clearTimeout(drag.timer);
      commit();
    }
    if (drag) return;
    const sp = localPoint(e);
    if (e.button === 1 || (e.button === 0 && spaceDown)) {
      e.preventDefault();
      const { panX, panY } = view();
      drag = { type: 'pan', startScreen: sp, panX, panY };
      el.classList.add('panning');
      capture(e);
      return;
    }
    if (e.button !== 0) return;
    const p = toLu(e);
    const target = e.target;
    const sel = store.get().selection;
    if (target?.dataset?.rotate && sel.length === 1) {
      startRotate(sel[0], p);
    } else if (target?.dataset?.handle && sel.length === 1) {
      startResize(sel[0], target.dataset.handle, p);
    } else {
      const hit = hitTest(p, e.metaKey || e.ctrlKey);
      if (hit) {
        if (e.shiftKey) {
          store.set({ selection: sel.includes(hit) ? sel.filter((x) => x !== hit) : [...sel, hit] });
          return;
        }
        const ids = sel.includes(hit) ? sel : [hit];
        if (!sel.includes(hit)) store.set({ selection: ids });
        startMove(ids, p, sp);
      } else {
        if (!e.shiftKey) store.set({ selection: [] });
        startMarquee(p, e.shiftKey);
      }
    }
    if (drag) capture(e);
  });

  function capture(e) {
    try {
      el.setPointerCapture(e.pointerId);
    } catch {
      // Synthetic or already released pointer.
    }
  }

  el.addEventListener('pointermove', (e) => {
    if (!drag) {
      const hit = spaceDown ? null : hitTest(toLu(e), e.metaKey || e.ctrlKey);
      if (hit !== store.get().hover) store.set({ hover: hit });
      return;
    }
    if (drag.committing) return;
    switch (drag.type) {
      case 'pan': {
        const sp = localPoint(e);
        setView(view().zoom, drag.panX + sp.x - drag.startScreen.x, drag.panY + sp.y - drag.startScreen.y);
        break;
      }
      case 'move':
        updateMove(e);
        break;
      case 'resize':
        updateResize(e);
        break;
      case 'rotate':
        updateRotate(e);
        break;
      case 'marquee':
        updateMarquee(e);
        redraw();
        break;
      default:
    }
  });

  const finish = (e) => {
    if (!drag || drag.committing) return;
    if (el.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
    if (drag.type === 'pan') {
      drag = null;
      el.classList.remove('panning');
      return;
    }
    if (drag.type === 'marquee') {
      drag = null;
      redraw();
      return;
    }
    if (drag.type === 'move' && !drag.moved) {
      drag = null;
      redraw();
      return;
    }
    commit();
  };
  el.addEventListener('pointerup', finish);
  el.addEventListener('pointercancel', () => cancel());
  el.addEventListener('pointerleave', () => {
    if (!drag && store.get().hover) store.set({ hover: null });
  });

  el.addEventListener('dblclick', (e) => {
    const hit = hitTest(toLu(e), true);
    if (hit) store.set({ selection: [hit] });
  });

  // Space-drag panning.
  const isTyping = (e) => /^(INPUT|TEXTAREA|SELECT)$/.test(e.target?.tagName) || e.target?.isContentEditable;
  const onKeyDown = (e) => {
    if (e.code === 'Space' && !isTyping(e)) {
      if (!e.repeat) {
        spaceDown = true;
        el.classList.add('space');
      }
      e.preventDefault();
    }
  };
  const onKeyUp = (e) => {
    if (e.code === 'Space') {
      spaceDown = false;
      el.classList.remove('space');
    }
  };
  window.addEventListener('keydown', onKeyDown);
  window.addEventListener('keyup', onKeyUp);

  // ----- nudge -----------------------------------------------------------------------

  function nudge(dx, dy) {
    const ids = store.get().selection;
    if (!ids.length || (drag && drag.type !== 'nudge') || drag?.committing) return;
    if (!drag) {
      const quads = itemsOf(ids);
      if (!quads.length) return;
      drag = { type: 'nudge', session: ++dragSeq, ids, quads, box0: bounds(quads.flat()), delta: { x: 0, y: 0 } };
    }
    const d = drag;
    d.delta = { x: d.delta.x + dx, y: d.delta.y + dy };
    d.ghosts = d.quads.map((q) => q.map((p) => add(p, d.delta)));
    d.hideSelection = true;
    d.edit = { kind: 'move_by', ids: d.ids, dx: d.delta.x, dy: d.delta.y };
    clearTimeout(d.timer);
    d.timer = setTimeout(() => {
      if (drag === d) commit();
    }, 450);
    schedulePreview();
  }

  // ----- drawing ---------------------------------------------------------------------

  let frame = 0;
  function redraw() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      const st = store.get();
      drawOverlay(svg, {
        overlay: st.overlay,
        zoom: st.zoom,
        panX: st.panX,
        panY: st.panY,
        selection: st.selection,
        hover: st.hover,
        variantActive: effectiveVariant(st) != null,
        drag,
        showHandles: true,
      });
    });
  }

  // ----- wiring ----------------------------------------------------------------------

  unsubs.push(
    store.select(
      (st) => [st.zoom, st.panX, st.panY],
      () => {
        applyTransform();
        redraw();
        scheduleResolution();
      },
    ),
    store.select((st) => st.overlay, (o, prev) => {
      if (!o || signature(o) !== slotSignature) {
        const resized = !prev || !o || prev.canvas.w !== o.canvas.w || prev.canvas.h !== o.canvas.h;
        rebuildSlots();
        if (fitPending || resized) fit();
        updateResolution();
      }
      redraw();
    }),
    store.select((st) => [st.selection, st.hover], redraw),
    editor.on('invalidate', (p) => invalidate(p)),
    // View context changed (target, variant, language, reload): every
    // pixmap is stale; the overlay subscription already rebuilt the slots
    // if the geometry changed.
    editor.on('reset', () => {
      cancel();
      for (const slot of slots) slot.dirty = true;
      if (fitPending) fit();
      updateResolution();
      redraw();
    }),
  );

  const resize = new ResizeObserver(() => {
    if (fitPending) fit();
    redraw();
    scheduleResolution();
  });
  resize.observe(el);

  applyTransform();
  rebuildSlots();
  if (store.get().overlay) {
    fit();
    updateResolution();
  }
  redraw();

  return {
    el,
    fit,
    zoomIn: () => step(1),
    zoomOut: () => step(-1),
    zoomTo: (z) => zoomAround(z, centre()),
    /** 100 % = one export pixel per screen pixel. */
    zoomActual: () => zoomAround(exportScale(), centre()),
    percent: () => Math.round((view().zoom / exportScale()) * 100),
    nudge,
    cancel,
    isBusy: () => Boolean(drag),
    unmount() {
      cancel();
      for (const u of unsubs) u();
      resize.disconnect();
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
      el.remove();
    },
  };
}
