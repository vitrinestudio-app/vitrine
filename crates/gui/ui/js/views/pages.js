// Pages view: the output images of the current target side by side, spaced
// like the store gallery (the target's gap width), each rendered exactly as
// exported (`render_image`). Zoom/pan like the canvas (views/viewport.js):
// world units are lu, the pixmaps are requested at zoom × devicePixelRatio.

import { api, toImageBitmap } from '../api.js';
import { h } from '../dom.js';
import { editor } from '../editor.js';
import { intersects } from '../geometry.js';
import { t } from '../i18n.js';
import { currentTarget, store, viewArgs } from '../store.js';
import { applyWorld, createViewport, densityFor, makeSlot, paintSlot } from './viewport.js';

export function mountPages(root) {
  const world = h('div', { class: 'canvas-world' });
  const labels = h('div', { class: 'vp-labels' });
  const empty = h('div', { class: 'canvas-empty', hidden: true }, t('canvas.empty'));
  const el = h('div', { class: 'canvas-view zoom-view', tabindex: '0' }, world, labels, empty);
  root.append(el);

  let slots = [];
  let sig = null;
  let content = null;
  let resTimer = 0;

  /** Export px per lu of the current target (100 % zoom). */
  function exportScale() {
    const o = store.get().overlay;
    const target = currentTarget();
    if (!o?.images.length || !target) return 1;
    return target.size[0] / o.images[0].rect.w;
  }

  const vp = createViewport(el, {
    key: 'pages',
    content: () => content,
    unit: exportScale,
    pointer: 'drag',
    padding: 40,
    inset: { bottom: 22 },
    onChange() {
      applyTransform();
      scheduleResolution();
    },
    onResize: () => scheduleResolution(),
  });

  function applyTransform() {
    const { zoom, panX, panY } = vp.view();
    applyWorld(world, { zoom, panX, panY });
    for (const slot of slots) {
      slot.label.style.left = `${(slot.rect.x + slot.rect.w / 2) * zoom + panX}px`;
      slot.label.style.top = `${(slot.rect.y + slot.rect.h) * zoom + panY + 6}px`;
    }
  }

  const signature = (o) => (o ? JSON.stringify([o.images.map((i) => [i.rect, i.pages]), o.gaps[0]?.w ?? 0]) : '');

  function rebuild(o) {
    world.replaceChildren();
    labels.replaceChildren();
    slots = [];
    sig = signature(o);
    empty.hidden = Boolean(o?.images.length);
    content = null;
    if (!o?.images.length) return;
    const gapLu = o.gaps[0]?.w ?? 0;
    let x = 0;
    let maxH = 0;
    for (const img of o.images) {
      const rect = { x, y: 0, w: img.rect.w, h: img.rect.h };
      const slot = makeSlot(world, rect, 'page-slot');
      const ids = img.pages.map((i) => o.pages.find((p) => p.index === i)?.id ?? i + 1).join(' + ');
      slot.label = h('div', { class: 'vp-label' }, `${t('canvas.image')} ${img.index + 1} · ${ids}`);
      slot.index = img.index;
      slot.source = img.rect;
      labels.append(slot.label);
      slots.push(slot);
      x += img.rect.w + Math.max(gapLu, img.rect.h * 0.02);
      maxH = Math.max(maxH, img.rect.h);
    }
    content = { x: 0, y: 0, w: slots[slots.length - 1].rect.x + slots[slots.length - 1].rect.w, h: maxH };
  }

  async function renderSlot(slot) {
    const level = densityFor(vp.view().zoom, slot.rect, exportScale());
    const gen = ++slot.gen;
    slot.dirty = false;
    try {
      const bitmap = await toImageBitmap(await api.renderImage(viewArgs(), slot.index, Math.max(1, Math.round(slot.rect.w * level))));
      if (gen !== slot.gen) {
        bitmap.close?.();
        return;
      }
      paintSlot(slot, bitmap, level);
    } catch (e) {
      if (gen === slot.gen) {
        slot.dirty = true;
        editor.toast(e.message, true);
      }
    }
  }

  function updateResolution() {
    const zoom = vp.view().zoom;
    for (const slot of slots) {
      if (!vp.isVisible(slot.rect)) continue;
      if (slot.dirty || Math.abs(slot.level - densityFor(zoom, slot.rect, exportScale())) > 1e-9) renderSlot(slot);
    }
  }

  function scheduleResolution(delay = 160) {
    clearTimeout(resTimer);
    resTimer = setTimeout(updateResolution, delay);
  }

  /** Rebuilds the slots when the image geometry changed (and re-fits). */
  function sync() {
    const o = store.get().overlay;
    if (signature(o) !== sig) {
      rebuild(o);
      if (content) vp.refresh(`${store.get().summary?.dir}|${content.w}x${content.h}`);
    }
    applyTransform();
  }

  function invalidate({ all, rects }) {
    sync();
    for (const slot of slots) {
      if (all || (rects ?? []).some((r) => intersects(r, slot.source))) slot.dirty = true;
    }
    updateResolution();
  }

  const unsubs = [
    store.select((st) => st.overlay, sync),
    editor.on('invalidate', invalidate),
    editor.on('reset', () => invalidate({ all: true })),
  ];
  sync();
  updateResolution();
  return {
    el,
    fit: vp.fit,
    zoomIn: vp.zoomIn,
    zoomOut: vp.zoomOut,
    zoomTo: vp.zoomTo,
    zoomActual: vp.zoomActual,
    percent: vp.percent,
    /** Fit mode: the view re-fits on resize until a manual zoom/pan. */
    get fitMode() {
      return vp.fitMode;
    },
    unmount() {
      clearTimeout(resTimer);
      unsubs.forEach((u) => u());
      vp.destroy();
      el.remove();
    },
  };
}
