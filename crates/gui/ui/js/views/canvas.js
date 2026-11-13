// Canvas view: the canvas of the current target as Rust-rendered pixmaps
// (one per output image, plus the gaps rendered separately, shown
// semi-transparent under a hatch) positioned in canvas lu, a screen-space
// SVG overlay, zoom/pan (shared viewport, views/viewport.js) and direct manipulation (select, marquee, move,
// resize, rotate, nudge) with snapping and live previews.
//
// Pointer gestures: plain drag on empty space pans (like the other views;
// a click there clears the selection), Shift+drag on empty space draws a
// marquee that adds to the selection, Space/middle-drag pans anywhere,
// dragging an element moves it. Locked elements (`locked`) are skipped by
// clicks and marquees (the layers list still selects them). Right-click
// (or ⇧F10 / the menu key) opens the context menu (context-menu.js).
//
// Drag feedback: the overlay ghost follows the pointer immediately; in
// parallel the affected output images are re-rendered from a scratch copy
// of the project (`render_preview_edit`, interactive quality, self-paced at
// ≤ 15 fps). On drop one `edit_*` command commits the gesture; the preview
// pixmaps stay until the real ones arrive, so nothing flickers.

import { actions, idLocked, isLocked } from '../actions.js';
import { api, toImageBitmap } from '../api.js';
import { closeContextMenu, openContextMenu } from '../context-menu.js';
import { h, s } from '../dom.js';
import { editor } from '../editor.js';
import { add, axes, bounds, dot, inQuad, inRect, inflate, intersects, normAngle, pageAt, rotate, sub, union } from '../geometry.js';
import { t } from '../i18n.js';
import { currentTarget, effectiveVariant, store, viewArgs } from '../store.js';
import { drawOverlay, screen } from './overlay.js';
import { snapAngle, snapBox, snapLines } from './snap.js';
import { openTextEditor } from './textedit.js';
import { applyWorld, createViewport, densityFor, makeSlot as makePixmapSlot, paintSlot } from './viewport.js';

const DRAG_THRESHOLD = 3;
const SNAP_PX = 6;
const PREVIEW_INTERVAL = 66;
const OPPOSITE = { nw: 'se', n: 's', ne: 'sw', e: 'w', se: 'nw', s: 'n', sw: 'ne', w: 'e' };
const UNIT = { nw: [-1, -1], n: [0, -1], ne: [1, -1], e: [1, 0], se: [1, 1], s: [0, 1], sw: [-1, 1], w: [-1, 0] };

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
    const preview = h('canvas', { class: 'preview' });
    const slot = makePixmapSlot(world, rect, `canvas-slot ${kind}`, preview);
    // Gaps are shown dimmed under a hatch: no loading spinner there.
    if (kind === 'gap') slot.el.classList.remove('slot-loading');
    return { ...slot, kind, index, preview };
  }

  function rebuildSlots() {
    const o = store.get().overlay;
    world.replaceChildren();
    slots = [];
    empty.hidden = Boolean(o);
    // Forget the geometry with the slots: an overlay with the same geometry
    // (e.g. after "Close project" and opening another project) must build
    // them again.
    slotSignature = '';
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
    return densityFor(view().zoom, slot.rect, exportScale());
  }

  function isVisible(slot) {
    return vp.isVisible(slot.rect);
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
      if (gen !== slot.gen) {
        bitmap.close?.();
        return;
      }
      paintSlot(slot, bitmap, level);
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

  // ----- zoom / pan (views/viewport.js) ----------------------------------------

  function applyTransform() {
    const { zoom, panX, panY } = view();
    applyWorld(world, { zoom, panX, panY });
  }

  const vp = createViewport(el, {
    key: 'canvas',
    // The canvas zoom lives in the store (overlay, text editor, shell read it).
    state: { get: view, set: (zoom, panX, panY) => store.set({ zoom, panX, panY }) },
    content: () => store.get().overlay?.canvas ?? null,
    unit: exportScale,
    pointer: false,
    minZoom: 0.02,
    inset: { top: 16 },
    onResize() {
      redraw();
      scheduleResolution();
    },
  });

  /** Content key of the overlay: a different canvas size re-fits. */
  const contentKey = (o) => (o ? `${store.get().summary?.dir}|${o.canvas.w}x${o.canvas.h}` : '');

  // ----- hit testing ---------------------------------------------------------------

  function groupsOf(o, id) {
    return o.elements.find((e) => e.id === id)?.groups ?? o.groups.find((g) => g.id === id)?.groups ?? [];
  }

  /**
   * Element (or enclosing group) at canvas point `p`; `deep` skips groups.
   * Locked elements are transparent unless `withLocked` (context menu).
   */
  function hitTest(p, deep = false, withLocked = false) {
    const o = store.get().overlay;
    if (!o) return null;
    const sel = store.get().selection;
    for (let i = o.elements.length - 1; i >= 0; i--) {
      const e = o.elements[i];
      if (e.kind === 'background') continue;
      if (!withLocked && isLocked(o, e)) continue;
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

  /**
   * Background elements (kind `background`, they fill their home page) at
   * canvas point `p`, topmost first. Not hit by clicks or the marquee;
   * double-click on empty page space and the canvas context menu select them.
   */
  function backgroundsAt(p) {
    const o = store.get().overlay;
    if (!o || !p) return [];
    const out = [];
    for (let i = o.elements.length - 1; i >= 0; i--) {
      const e = o.elements[i];
      if (e.kind !== 'background' || isLocked(o, e)) continue;
      if (inQuad(p, e.quad)) out.push(e.id);
    }
    return out;
  }

  /** Top-level ids (outermost groups) of elements intersecting `r`. */
  function inMarquee(r) {
    const o = store.get().overlay;
    const ids = new Set();
    for (const e of o.elements) {
      if (e.kind === 'background' || isLocked(o, e)) continue;
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
    drag.edit = { kind: 'move_by', ids: drag.ids, dx: raw.x, dy: raw.y, keep_page: keepPage(drag.ids) };
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

  /** Home page locked (`lock_page`) for any of `ids` (core also honours it). */
  function keepPage(ids) {
    const els = store.get().overlay?.elements ?? [];
    return ids.some((id) => els.find((e) => e.id === id)?.lock_page);
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
      if (d.delta.x || d.delta.y) call = api.moveBy(v, d.ids, d.delta.x, d.delta.y, keepPage(d.ids));
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
    if (drag.type === 'pan') vp.panEnd();
    el.classList.remove('marquee');
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
    if (vp.isPanTrigger(e)) {
      e.preventDefault();
      drag = { type: 'pan' };
      vp.panStart(e);
      return;
    }
    if (e.button !== 0) return;
    const p = toLu(e);
    const target = e.target;
    const sel = store.get().selection;
    const locked = sel.some((id) => idLocked(store.get().overlay, id));
    if (target?.dataset?.rotate && sel.length === 1 && !locked) {
      startRotate(sel[0], p);
    } else if (target?.dataset?.handle && sel.length === 1 && !locked) {
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
        // A selection with locked elements (picked in the layers list) stays put.
        if (!ids.some((id) => idLocked(store.get().overlay, id))) startMove(ids, p, sp);
      } else if (e.shiftKey) {
        // Shift+drag on empty space: marquee adding to the selection.
        startMarquee(p, true);
        el.classList.add('marquee');
      } else {
        // Plain drag on empty space pans; a click (no drag) clears the selection.
        drag = { type: 'pan', clear: true };
        vp.panStart(e, false);
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
      const hit = vp.spaceDown() ? null : hitTest(toLu(e), e.metaKey || e.ctrlKey);
      if (hit !== store.get().hover) store.set({ hover: hit });
      const onHandle = Boolean(e.target?.dataset?.handle || e.target?.dataset?.rotate);
      el.classList.toggle('over-empty', !hit && !onHandle);
      el.classList.toggle('shift', e.shiftKey);
      return;
    }
    if (drag.committing) return;
    switch (drag.type) {
      case 'pan':
        vp.panMove(e);
        break;
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
      const click = drag.clear && !vp.isPanning();
      drag = null;
      vp.panEnd(e);
      if (click) store.set({ selection: [] });
      return;
    }
    if (drag.type === 'marquee') {
      drag = null;
      el.classList.remove('marquee');
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
    el.classList.remove('over-empty');
  });

  // Shift turns the empty-space cursor into a crosshair (marquee).
  const onShift = (e) => {
    if (e.key === 'Shift') el.classList.toggle('shift', e.type === 'keydown');
  };
  window.addEventListener('keydown', onShift);
  window.addEventListener('keyup', onShift);

  // ----- context menu (context-menu.js) ---------------------------------------------

  el.addEventListener('contextmenu', (e) => {
    // ⌥/Alt + right-click keeps the web-view menu (main.js, debugging).
    if (e.altKey) return;
    e.preventDefault();
    if (drag) return;
    const p = toLu(e);
    openMenu({ x: e.clientX, y: e.clientY, p, hit: hitTest(p, e.metaKey || e.ctrlKey, true) });
  });

  /**
   * Opens the context menu: of the element `hit` (selected first) or the
   * selection, else of the empty canvas at canvas point `p`. Without
   * arguments (⇧F10 / menu key): the selection at its centre, else the
   * empty canvas at the view centre.
   */
  function openMenu({ x, y, p, hit } = {}) {
    const o = store.get().overlay;
    if (!o) return;
    let sel = store.get().selection;
    if (hit && !sel.includes(hit)) {
      sel = [hit];
      store.set({ selection: sel });
    }
    const forSelection = Boolean(hit) || (!p && sel.length > 0);
    if (x == null) {
      const r = el.getBoundingClientRect();
      const quads = forSelection ? itemsOf(sel) : [];
      const S = screen(view());
      const b = quads.length ? bounds(quads.flat().map(S)) : null;
      x = r.left + (b ? b.x + b.w / 2 : el.clientWidth / 2);
      y = r.top + (b ? b.y + b.h / 2 : el.clientHeight / 2);
      p = toLu({ clientX: x, clientY: y });
    }
    const items = forSelection ? selectionItems(o, sel) : canvasItems(o, p);
    openContextMenu({ x, y, items, label: t(forSelection ? 'ctx.menu' : 'ctx.menu.canvas') });
  }

  function selectionItems(o, sel) {
    const drawn = new Set([...o.elements.map((e) => e.id), ...o.groups.map((g) => g.id)]);
    const shown = sel.filter((id) => drawn.has(id));
    const hidden = sel.filter((id) => !drawn.has(id));
    const allLocked = shown.length > 0 && shown.every((id) => idLocked(o, id));
    const groups = sel.some((id) => o.groups.some((g) => g.id === id));
    const clip = store.get().clipboard;
    return [
      { label: t('ctx.cut'), key: 'mod+x', action: () => actions.cut() },
      { label: t('ctx.copy'), key: 'mod+c', action: () => actions.copy() },
      { label: t('ctx.paste'), key: 'mod+v', disabled: !clip, action: () => actions.paste() },
      { label: t('ctx.duplicate'), key: 'mod+d', action: () => actions.duplicate() },
      { label: t('ctx.delete'), key: 'delete', action: () => actions.remove() },
      '-',
      { label: t('ctx.front'), key: 'shift+]', action: () => actions.reorder('front') },
      { label: t('ctx.forward'), key: ']', action: () => actions.reorder('forward') },
      { label: t('ctx.backward'), key: '[', action: () => actions.reorder('backward') },
      { label: t('ctx.back'), key: 'shift+[', action: () => actions.reorder('back') },
      '-',
      sel.length > 1 ? { label: t('ctx.group'), key: 'mod+g', action: () => actions.group() } : null,
      groups ? { label: t('ctx.ungroup'), key: 'mod+shift+g', action: () => actions.ungroup() } : null,
      '-',
      allLocked
        ? { label: t('ctx.unlock'), icon: 'unlock', action: () => actions.lock(false) }
        : { label: t('ctx.lock'), icon: 'lock', action: () => actions.lock(true) },
      shown.length ? { label: t('ctx.hide'), icon: 'eye-off', action: () => actions.hide(true, shown) } : null,
      hidden.length ? { label: t('ctx.show'), icon: 'eye', action: () => actions.hide(false, hidden) } : null,
    ];
  }

  function canvasItems(o, p) {
    const page = p ? pageAt(o.pages, p.x) : null;
    const n = page ? `${page.index + 1} · ${page.id}` : '';
    const clip = store.get().clipboard;
    return [
      {
        label: page ? t('ctx.paste_here').replace('{page}', n) : t('ctx.paste'),
        key: 'mod+v',
        disabled: !clip,
        action: () => actions.paste(page?.index),
      },
      '-',
      page ? { label: t('ctx.select_page').replace('{page}', n), action: () => actions.selectAllOnPage(page.index) } : null,
      ...backgroundsAt(p).map((id) => ({ label: t('ctx.select_background').replace('{id}', id), icon: 'background', action: () => store.set({ selection: [id] }) })),
      { label: t('ctx.select_all'), key: 'mod+a', action: () => actions.selectAll() },
      '-',
      { label: t('ctx.fit'), key: 'mod+0', action: () => vp.fit() },
    ];
  }

  el.addEventListener('dblclick', (e) => {
    const p = toLu(e);
    const hit = hitTest(p, true);
    if (hit) store.set({ selection: [hit] });
    else {
      // Empty page space: the page's background element (topmost), if any.
      const bg = backgroundsAt(p)[0];
      if (bg) store.set({ selection: [bg] });
    }
    if (store.get().overlay?.elements.find((x) => x.id === hit)?.kind === 'text') editText(hit);
  });

  // ----- inline text editing (views/textedit.js) ---------------------------------------

  let textEdit = null;
  const textHooks = {
    el,
    view,
    slots: () => slots,
    drawPreview(slot, bitmap) {
      slot.preview.width = bitmap.width;
      slot.preview.height = bitmap.height;
      slot.preview.getContext('2d').drawImage(bitmap, 0, 0);
      bitmap.close?.();
      slot.el.classList.add('previewing');
    },
    clearPreviews,
  };

  async function editText(id) {
    if (drag) return;
    textEdit?.close(true);
    textEdit = await openTextEditor(textHooks, id);
  }

  /** Overlay element (not background) under client point (`x`, `y`), or null. */
  function elementAt(x, y) {
    const id = hitTest(toLu({ clientX: x, clientY: y }), true);
    return id ? store.get().overlay.elements.find((e) => e.id === id) ?? null : null;
  }

  /**
   * Topmost device element under client point (`x`, `y`), also when a text
   * or image lies above it (drop targets: screenshots onto a device), or
   * null. Locked devices count: a drop changes the screenshot, not the
   * position.
   */
  function deviceAt(x, y) {
    if (!el.contains(document.elementFromPoint(x, y))) return null;
    const o = store.get().overlay;
    if (!o) return null;
    const p = toLu({ clientX: x, clientY: y });
    for (let i = o.elements.length - 1; i >= 0; i--) {
      const e = o.elements[i];
      if (e.kind !== 'device' || !inQuad(p, e.quad)) continue;
      if (e.clip && !inRect(p, e.clip)) continue;
      return e;
    }
    return null;
  }

  // ----- nudge -----------------------------------------------------------------------

  function nudge(dx, dy) {
    const ids = store.get().selection;
    if (!ids.length || (drag && drag.type !== 'nudge') || drag?.committing) return;
    if (ids.some((id) => idLocked(store.get().overlay, id))) return;
    if (!drag) {
      const quads = itemsOf(ids);
      if (!quads.length) return;
      drag = { type: 'nudge', session: ++dragSeq, ids, quads, box0: bounds(quads.flat()), delta: { x: 0, y: 0 } };
    }
    const d = drag;
    d.delta = { x: d.delta.x + dx, y: d.delta.y + dy };
    d.ghosts = d.quads.map((q) => q.map((p) => add(p, d.delta)));
    d.hideSelection = true;
    d.edit = { kind: 'move_by', ids: d.ids, dx: d.delta.x, dy: d.delta.y, keep_page: keepPage(d.ids) };
    clearTimeout(d.timer);
    d.timer = setTimeout(() => {
      if (drag === d) commit();
    }, 450);
    schedulePreview();
  }

  // ----- drawing ---------------------------------------------------------------------

  // Outline of the elements Claude just changed: full for 60 %, then fades.
  const GLOW_MS = 1500;
  let glow = null;
  function glowModel() {
    if (!glow) return null;
    const t = performance.now() - glow.start;
    if (t > GLOW_MS) {
      glow = null;
      return null;
    }
    const fade = GLOW_MS * 0.4;
    return { ids: glow.ids, alpha: t < GLOW_MS - fade ? 1 : 1 - (t - (GLOW_MS - fade)) / fade };
  }
  function startGlow(ids) {
    const running = Boolean(glow);
    glow = { ids, start: performance.now() };
    if (running) return;
    const tick = () => {
      redraw();
      if (glow) requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
  }

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
        dropTarget: st.dropTarget,
        variantActive: effectiveVariant(st) != null,
        drag,
        showHandles: true,
        highlight: glowModel(),
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
    store.select((st) => st.overlay, (o) => {
      if (!o || signature(o) !== slotSignature || !slots.length) {
        rebuildSlots();
        if (o) vp.refresh(contentKey(o));
        updateResolution();
      }
      redraw();
    }),
    store.select((st) => [st.selection, st.hover, st.dropTarget], redraw),
    editor.on('invalidate', (p) => invalidate(p)),
    editor.on('highlight', ({ ids }) => startGlow(ids)),
    // View context changed (target, variant, language, reload): every
    // pixmap is stale; the overlay subscription already rebuilt the slots
    // if the geometry changed.
    editor.on('reset', () => {
      cancel();
      for (const slot of slots) slot.dirty = true;
      const o = store.get().overlay;
      if (o) vp.refresh(contentKey(o));
      updateResolution();
      redraw();
    }),
  );

  applyTransform();
  rebuildSlots();
  if (store.get().overlay) {
    vp.refresh(contentKey(store.get().overlay));
    updateResolution();
  }
  redraw();

  return {
    el,
    fit: vp.fit,
    zoomIn: vp.zoomIn,
    zoomOut: vp.zoomOut,
    zoomTo: vp.zoomTo,
    /** Index of the page at (or nearest to) the centre of the visible area. */
    visiblePage() {
      const o = store.get().overlay;
      if (!o?.pages?.length) return null;
      const { zoom, panX } = view();
      return pageAt(o.pages, (el.clientWidth / 2 - panX) / zoom)?.index ?? null;
    },
    /** Fits the canvas rect `r` (lu) into the view (validation "go to page"). */
    zoomToRect: vp.zoomToRect,
    /** 100 % = one export pixel per screen pixel. */
    zoomActual: vp.zoomActual,
    percent: vp.percent,
    /** Fit mode: the view re-fits on resize until a manual zoom/pan. */
    get fitMode() {
      return vp.fitMode;
    },
    nudge,
    cancel,
    editText,
    elementAt,
    deviceAt,
    isBusy: () => Boolean(drag),
    openMenu: () => openMenu(),
    unmount() {
      cancel();
      closeContextMenu();
      window.removeEventListener('keydown', onShift);
      window.removeEventListener('keyup', onShift);
      textEdit?.close(false);
      for (const u of unsubs) u();
      vp.destroy();
      el.remove();
    },
  };
}
