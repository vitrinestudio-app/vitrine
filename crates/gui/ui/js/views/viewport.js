// Shared zoom/pan viewport of the centre views (canvas, pages, store
// gallery, all languages) and their pixmap slots.
//
// A viewport maps world units (lu for canvas/pages/languages, export px for
// the gallery) to screen px of its element: `screen = world × zoom + pan`.
// It owns the zoom steps, wheel/pinch zoom around the cursor, wheel and
// space/middle-drag panning (optionally plain left-drag), fit, 100 % and the
// **fit mode**: set by `fit()` (and initially), cleared by any manual zoom or
// pan; while it is set, a resize of the element (window, sidebars,
// inspector) re-fits. Zoom state and fit mode are remembered per view key
// for the session, so switching views keeps each view's zoom.
//
// Slots (`makeSlot`/`paintSlot`) are the rendered output images: a
// placeholder with a loading spinner until the first bitmap arrives;
// re-renders keep the old bitmap until the new one is drawn.

import { h } from '../dom.js';
import { intersects } from '../geometry.js';

export const ZOOM_STEPS = [0.02, 0.03, 0.05, 0.075, 0.1, 0.15, 0.2, 0.25, 0.33, 0.5, 0.67, 0.75, 1, 1.25, 1.5, 2, 3, 4, 6, 8];
/** Largest side of a requested pixmap (backend limit `MAX_SIDE`). */
export const MAX_SIDE = 8192;
const DRAG_THRESHOLD = 3;

const clamp = (v, a, b) => Math.min(b, Math.max(a, v));

/** Per-view memory: `key → { zoom, panX, panY, fitMode, contentKey }`. */
const memory = new Map();
const listeners = new Set();

/** Calls `fn()` after any viewport's zoom or pan changed (zoom readout). */
export function onViewportChange(fn) {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

function notify() {
  for (const fn of [...listeners]) fn();
}

/**
 * Creates the viewport of element `el`.
 *
 * `opts`:
 * - `key`: memory key (view id)
 * - `content()`: world rect to fit (`{x, y, w, h}`) or null
 * - `unit()`: zoom that counts as 100 % (one export pixel per screen pixel)
 * - `state`: optional `{ get() → {zoom, panX, panY}, set(zoom, panX, panY) }`
 *   (the canvas keeps its zoom in the store); default: the per-key memory
 * - `onChange()`: after zoom/pan changed (not needed with an external state
 *   that notifies on its own)
 * - `onResize()`: after the element changed size (and was re-fitted)
 * - `padding` (px around the fitted content), `inset` `{top, right,
 *   bottom, left}` (px reserved for fixed chrome), `minZoom`, `maxZoom`
 * - `fitFloor()`: optional smallest zoom `fit()` uses (as long as the width
 *   fits); taller content is then top-aligned and panned
 * - `pointer`: `'drag'` = left-drag pans too (clicks after a drag are
 *   swallowed), `'pan'` = only space/middle-drag (default), `false` = the
 *   view handles pointers itself (use `panStart`/`panMove`/`panEnd`)
 */
export function createViewport(el, opts) {
  const padding = opts.padding ?? 56;
  const inset = { top: 0, right: 0, bottom: 0, left: 0, ...opts.inset };
  const minZoom = opts.minZoom ?? 0.005;
  const maxZoom = opts.maxZoom ?? 8;
  const mem = memory.get(opts.key) ?? { zoom: 1, panX: 0, panY: 0, fitMode: true, contentKey: undefined };
  memory.set(opts.key, mem);

  const state = opts.state ?? {
    get: () => ({ zoom: mem.zoom, panX: mem.panX, panY: mem.panY }),
    set(zoom, panX, panY) {
      if (zoom === mem.zoom && panX === mem.panX && panY === mem.panY) return;
      mem.zoom = zoom;
      mem.panX = panX;
      mem.panY = panY;
      opts.onChange?.();
    },
  };

  let spaceDown = false;
  let pan = null;
  let suppressClick = false;
  let fitFrame = 0;

  const view = () => state.get();
  const bars = createScrollbars(el, {
    view,
    content: () => opts.content(),
    inset,
    padding,
    pan: (panX, panY) => setView(view().zoom, panX, panY),
  });

  function apply(zoom, panX, panY, fit) {
    mem.fitMode = fit;
    state.set(clamp(zoom, minZoom, maxZoom), panX, panY);
    bars.update();
    notify();
  }

  /** Manual view change (clears the fit mode). */
  function setView(zoom, panX, panY) {
    apply(zoom, panX, panY, false);
  }

  function zoomAround(z, p) {
    const { zoom, panX, panY } = view();
    const nz = clamp(z, minZoom, maxZoom);
    setView(nz, p.x - ((p.x - panX) * nz) / zoom, p.y - ((p.y - panY) * nz) / zoom);
  }

  const centre = () => ({
    x: inset.left + (el.clientWidth - inset.left - inset.right) / 2,
    y: inset.top + (el.clientHeight - inset.top - inset.bottom) / 2,
  });

  /** Zoom/pan that fits world rect `r` into the free area (null if none). */
  function fitting(r) {
    const w = el.clientWidth - inset.left - inset.right;
    const hh = el.clientHeight - inset.top - inset.bottom;
    if (!r || !(r.w > 0) || !(r.h > 0) || w <= 0 || hh <= 0) return null;
    const zw = (w - padding * 2) / r.w;
    let z = Math.min(zw, (hh - padding * 2) / r.h);
    // A floor (e.g. a readable row height) wins over fitting the height.
    if (opts.fitFloor) z = Math.max(z, Math.min(zw, opts.fitFloor()));
    z = clamp(z, minZoom, maxZoom);
    // Centre what fits; align what does not to the top/left edge.
    const x = r.w * z <= w - padding * 2 ? inset.left + (w - r.w * z) / 2 : inset.left + padding;
    const y = r.h * z <= hh - padding * 2 + 0.5 ? inset.top + (hh - r.h * z) / 2 : inset.top + padding;
    return { z, x: x - r.x * z, y: y - r.y * z };
  }

  /** Fits the content and enters fit mode (re-fits on resize). */
  function fit() {
    mem.fitMode = true;
    const f = fitting(opts.content());
    if (f) apply(f.z, f.x, f.y, true);
  }

  /** Fits world rect `r` (e.g. one page) without entering fit mode. */
  function zoomToRect(r) {
    const f = fitting(r);
    if (f) setView(f.z, f.x, f.y);
  }

  function step(dir) {
    const z = view().zoom;
    const next = dir > 0 ? ZOOM_STEPS.find((v) => v > z * 1.001) : [...ZOOM_STEPS].reverse().find((v) => v < z / 1.001);
    zoomAround(next ?? z, centre());
  }

  function localPoint(e) {
    const r = el.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  // ----- wheel / pinch ------------------------------------------------------

  const onWheel = (e) => {
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
  };
  el.addEventListener('wheel', onWheel, { passive: false });

  // ----- panning --------------------------------------------------------------

  /** True for a pan gesture start (middle button, or space + left button). */
  const isPanTrigger = (e) => e.button === 1 || (e.button === 0 && spaceDown);

  function capture(id) {
    try {
      el.setPointerCapture(id);
    } catch {
      // Synthetic or already released pointer.
    }
  }

  /**
   * Starts a pan at pointer event `e`. `armed` false: a left-button press
   * that becomes a pan only after a few px (until then clicks still reach
   * their target, so no pointer capture yet).
   */
  function panStart(e, armed = true) {
    const { panX, panY } = view();
    pan = { start: localPoint(e), panX, panY, active: armed, id: e.pointerId };
    if (armed) {
      el.classList.add('panning');
      capture(e.pointerId);
    }
  }

  /** Continues a pan; returns false when no pan is in progress. */
  function panMove(e) {
    if (!pan) return false;
    const sp = localPoint(e);
    const dx = sp.x - pan.start.x;
    const dy = sp.y - pan.start.y;
    if (!pan.active) {
      if (Math.hypot(dx, dy) < DRAG_THRESHOLD) return true;
      pan.active = true;
      el.classList.add('panning');
      capture(pan.id);
    }
    setView(view().zoom, pan.panX + dx, pan.panY + dy);
    return true;
  }

  function panEnd(e) {
    if (!pan) return false;
    if (pan.active) suppressClick = true;
    if (e && el.hasPointerCapture?.(e.pointerId)) el.releasePointerCapture(e.pointerId);
    pan = null;
    el.classList.remove('panning');
    return true;
  }

  const onPointerDown = (e) => {
    el.focus?.({ preventScroll: true });
    suppressClick = false;
    if (isPanTrigger(e)) {
      e.preventDefault();
      panStart(e);
    } else if (e.button === 0 && opts.pointer === 'drag') {
      panStart(e, false);
    }
  };
  const onPointerUp = (e) => panEnd(e);
  const onClickCapture = (e) => {
    if (!suppressClick) return;
    suppressClick = false;
    e.stopPropagation();
    e.preventDefault();
  };
  if (opts.pointer !== false) {
    el.addEventListener('pointerdown', onPointerDown);
    el.addEventListener('pointermove', panMove);
    el.addEventListener('pointerup', onPointerUp);
    el.addEventListener('pointercancel', onPointerUp);
    el.addEventListener('click', onClickCapture, true);
  }

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

  // ----- resize ---------------------------------------------------------------------

  const ro = new ResizeObserver(() => {
    if (!el.clientWidth || !el.clientHeight) return;
    if (mem.fitMode && !fitFrame) {
      // Coalesce bursts (window drag, sidebar animation) into one fit per frame.
      fitFrame = requestAnimationFrame(() => {
        fitFrame = 0;
        if (mem.fitMode) fit();
      });
    }
    bars.update();
    opts.onResize?.();
  });
  ro.observe(el);

  return {
    view,
    setView,
    zoomAround,
    fit,
    zoomToRect,
    zoomIn: () => step(1),
    zoomOut: () => step(-1),
    zoomTo: (z) => zoomAround(z, centre()),
    /** 100 % = one export pixel per screen pixel. */
    zoomActual: () => zoomAround(opts.unit(), centre()),
    percent: () => Math.round((view().zoom / (opts.unit() || 1)) * 100),
    get fitMode() {
      return mem.fitMode;
    },
    /**
     * Call after the content may have changed: re-fits in fit mode, or when
     * `contentKey` (e.g. the content size) differs from the last one seen.
     */
    refresh(contentKey) {
      const changed = contentKey !== mem.contentKey;
      mem.contentKey = contentKey;
      if (mem.fitMode || changed) fit();
      bars.update();
    },
    /** Re-measures the scrollbars (after the content changed without a refresh). */
    updateScrollbars: () => bars.update(),
    localPoint,
    isPanTrigger,
    panStart,
    panMove,
    panEnd,
    isPanning: () => Boolean(pan?.active),
    spaceDown: () => spaceDown,
    /** True when world rect `r` is (nearly) on screen. */
    isVisible(r, margin = 200) {
      const { zoom, panX, panY } = view();
      const s = { x: r.x * zoom + panX, y: r.y * zoom + panY, w: r.w * zoom, h: r.h * zoom };
      return intersects(s, { x: -margin, y: -margin, w: el.clientWidth + 2 * margin, h: el.clientHeight + 2 * margin });
    },
    destroy() {
      cancelAnimationFrame(fitFrame);
      ro.disconnect();
      bars.destroy();
      el.removeEventListener('wheel', onWheel);
      el.removeEventListener('pointerdown', onPointerDown);
      el.removeEventListener('pointermove', panMove);
      el.removeEventListener('pointerup', onPointerUp);
      el.removeEventListener('pointercancel', onPointerUp);
      el.removeEventListener('click', onClickCapture, true);
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
    },
  };
}

/** Smallest scrollbar thumb (px). */
const MIN_THUMB = 28;

/**
 * Overlay scrollbars of a viewport element: shown on an axis when the
 * content (plus `padding`) does not fit the free area; thumb drag and
 * track clicks pan the view, and every zoom/pan moves the thumbs
 * (`update()`). The scroll range is the content box united with the
 * visible area, so panning past the content still scrolls back.
 */
function createScrollbars(el, { view, content, inset, padding, pan }) {
  const make = (axis) => {
    const thumb = h('div', { class: 'vp-thumb' });
    const track = h('div', { class: `vp-scroll vp-scroll-${axis}`, hidden: true, 'aria-hidden': 'true' }, thumb);
    return { axis, track, thumb, ratio: 1, shown: false };
  };
  const x = make('x');
  const y = make('y');
  el.append(x.track, y.track);
  let frame = 0;

  /** Visible span, scroll range and track length of `axis` (px), or null. */
  function measure(axis) {
    const c = content();
    const { zoom, panX, panY } = view();
    if (!c || !(c.w > 0) || !(c.h > 0)) return null;
    const horizontal = axis === 'x';
    const v0 = horizontal ? inset.left : inset.top;
    const v1 = horizontal ? el.clientWidth - inset.right : el.clientHeight - inset.bottom;
    const c0 = (horizontal ? c.x * zoom + panX : c.y * zoom + panY) - padding / 2;
    const c1 = c0 + (horizontal ? c.w : c.h) * zoom + padding;
    if (v1 - v0 <= 0) return null;
    // Everything (with a little air) on screen: no bar.
    if (c0 >= v0 - 0.5 && c1 <= v1 + 0.5) return null;
    const r0 = Math.min(c0, v0);
    const r1 = Math.max(c1, v1);
    return { v0, v1, r0, r1 };
  }

  function place(bar) {
    const m = measure(bar.axis);
    bar.shown = Boolean(m);
    bar.track.hidden = !m;
    if (!m) return;
    const horizontal = bar.axis === 'x';
    const other = horizontal ? y : x;
    // Leave the corner free when both bars show.
    const end = other.shown || measure(other.axis) ? 12 : 2;
    const start = horizontal ? inset.left + 2 : inset.top + 2;
    const length = (horizontal ? el.clientWidth - inset.right : el.clientHeight - inset.bottom) - end - start;
    if (horizontal) {
      bar.track.style.left = `${start}px`;
      bar.track.style.width = `${Math.max(0, length)}px`;
    } else {
      bar.track.style.top = `${start}px`;
      bar.track.style.height = `${Math.max(0, length)}px`;
    }
    const range = m.r1 - m.r0;
    const size = Math.max(MIN_THUMB, (length * (m.v1 - m.v0)) / range);
    const free = Math.max(1, length - size);
    const pos = ((m.v0 - m.r0) / Math.max(1, range - (m.v1 - m.v0))) * free;
    // Screen px of panning per px of thumb movement.
    bar.ratio = Math.max(1e-6, (range - (m.v1 - m.v0)) / free);
    bar.thumb.style[horizontal ? 'width' : 'height'] = `${size}px`;
    bar.thumb.style[horizontal ? 'height' : 'width'] = '';
    bar.thumb.style.transform = horizontal ? `translateX(${pos}px)` : `translateY(${pos}px)`;
    bar.length = length;
    bar.size = size;
    bar.pos = pos;
  }

  function update() {
    if (frame) return;
    frame = requestAnimationFrame(() => {
      frame = 0;
      place(x);
      place(y);
      place(x);
    });
  }

  /** Pans by `d` screen px of content along `axis`. */
  function panBy(axis, d) {
    const { panX, panY } = view();
    if (axis === 'x') pan(panX - d, panY);
    else pan(panX, panY - d);
  }

  function bind(bar) {
    let drag = null;
    bar.track.addEventListener('pointerdown', (e) => {
      // Keep the view's own gestures (pan, marquee, selection) out of it.
      e.stopPropagation();
      if (e.button !== 0) return;
      e.preventDefault();
      const horizontal = bar.axis === 'x';
      const at = horizontal ? e.clientX : e.clientY;
      if (e.target === bar.thumb) {
        drag = { start: at, ratio: bar.ratio, moved: 0 };
        bar.track.classList.add('dragging');
        try {
          bar.track.setPointerCapture(e.pointerId);
        } catch {
          // Synthetic pointer.
        }
        return;
      }
      // Track click: one visible span towards the click.
      const r = bar.thumb.getBoundingClientRect();
      const before = at < (horizontal ? r.left : r.top);
      const span = horizontal ? el.clientWidth : el.clientHeight;
      panBy(bar.axis, (before ? -1 : 1) * span * 0.9);
    });
    bar.track.addEventListener('pointermove', (e) => {
      if (!drag) return;
      const at = bar.axis === 'x' ? e.clientX : e.clientY;
      const d = (at - drag.start) * drag.ratio;
      panBy(bar.axis, d - drag.moved);
      drag.moved = d;
    });
    const end = (e) => {
      if (!drag) return;
      drag = null;
      bar.track.classList.remove('dragging');
      if (bar.track.hasPointerCapture?.(e.pointerId)) bar.track.releasePointerCapture(e.pointerId);
    };
    bar.track.addEventListener('pointerup', end);
    bar.track.addEventListener('pointercancel', end);
    // No click-through (e.g. "all languages" opens a cell on click).
    bar.track.addEventListener('click', (e) => e.stopPropagation());
    bar.track.addEventListener('dblclick', (e) => e.stopPropagation());
    bar.track.addEventListener('contextmenu', (e) => e.stopPropagation());
  }
  bind(x);
  bind(y);

  return {
    update,
    destroy() {
      cancelAnimationFrame(frame);
      x.track.remove();
      y.track.remove();
    },
  };
}

/**
 * Applies the viewport transform to a world element and sets `--px` (one
 * screen pixel in world units) for hairlines, outlines and markers in CSS.
 */
export function applyWorld(world, { zoom, panX, panY }) {
  world.style.transform = `translate(${panX}px, ${panY}px) scale(${zoom})`;
  world.style.setProperty('--px', `${1 / zoom}px`);
}

/**
 * Pixel density (px per world unit) wanted for `zoom`, quantised to √2
 * steps so small zoom changes reuse the pixmap; at most `cap` and at most
 * `MAX_SIDE` px on the longer side of `rect`.
 */
export function densityFor(zoom, rect, cap = Infinity) {
  const want = zoom * (window.devicePixelRatio || 1);
  let level = 2 ** (Math.ceil(Math.log2(want) * 2) / 2);
  level = Math.min(level, cap);
  level = Math.min(level, MAX_SIDE / Math.max(rect.w, rect.h, 1));
  return level;
}

/**
 * Creates a pixmap slot in `parent` at world `rect` (positioned in world
 * units: the parent carries the zoom transform). It shows a loading
 * placeholder until `paintSlot` draws its first bitmap. `extra`: more
 * children (e.g. a preview canvas or markers).
 */
export function makeSlot(parent, rect, cls = '', ...extra) {
  const real = h('canvas', { class: 'real' });
  const node = h('div', { class: `vp-slot slot-loading ${cls}` }, real, ...extra, h('div', { class: 'slot-spinner', 'aria-hidden': 'true' }));
  placeSlot({ el: node }, rect);
  parent.append(node);
  return { rect, el: node, real, level: 0, gen: 0, dirty: true, loaded: false };
}

/** Moves/resizes slot `slot` to world `rect`. */
export function placeSlot(slot, rect) {
  slot.rect = rect;
  slot.el.style.left = `${rect.x}px`;
  slot.el.style.top = `${rect.y}px`;
  slot.el.style.width = `${rect.w}px`;
  slot.el.style.height = `${rect.h}px`;
}

/** Draws `bitmap` into the slot (replacing the old one) and ends loading. */
export function paintSlot(slot, bitmap, level) {
  slot.real.width = bitmap.width;
  slot.real.height = bitmap.height;
  slot.real.getContext('2d').drawImage(bitmap, 0, 0);
  bitmap.close?.();
  if (level != null) slot.level = level;
  slot.loaded = true;
  slot.el.classList.remove('slot-loading');
}
