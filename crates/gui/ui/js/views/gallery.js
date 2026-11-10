// Store gallery view: the gallery preview of the current target
// (`render_gallery`: output images with the store's gaps, as the store
// shows them). Zoom/pan like the canvas (views/viewport.js): world units
// are export px of the target (100 % = full-size gallery); the preview is
// requested at zoom × devicePixelRatio (never above full size).

import { api, toImageBitmap } from '../api.js';
import { h } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { currentTarget, store, viewArgs } from '../store.js';
import { applyWorld, createViewport, densityFor, makeSlot, paintSlot, placeSlot } from './viewport.js';

export function mountGallery(root) {
  const world = h('div', { class: 'canvas-world' });
  const caption = h('div', { class: 'vp-caption muted small' });
  const message = h('div', { class: 'canvas-empty', hidden: true });
  const el = h('div', { class: 'canvas-view zoom-view', tabindex: '0' }, world, caption, message);
  root.append(el);

  let slot = null;
  let sig = null;
  /** Re-fit once the first bitmap gives the exact gallery size. */
  let fitOnPaint = false;
  let resTimer = 0;

  const vp = createViewport(el, {
    key: 'gallery',
    content: () => slot?.rect ?? null,
    unit: () => 1,
    pointer: 'drag',
    padding: 32,
    inset: { bottom: 24 },
    onChange() {
      applyTransform();
      scheduleResolution();
    },
    onResize: () => scheduleResolution(),
  });

  /** Gallery px per export px for the zoom (≤ 1; 5 % headroom for the padding). */
  const levelOf = (r) => densityFor(vp.view().zoom, { w: r.w * 1.05, h: r.h }, 1);

  function applyTransform() {
    const { zoom, panX, panY } = vp.view();
    applyWorld(world, { zoom, panX, panY });
  }

  /** Estimated gallery size in export px (exact after the first render). */
  function estimate(o, target) {
    const first = o.images[0];
    const last = o.images[o.images.length - 1];
    const ppl = target.size[1] / first.rect.h;
    const pad = Math.max((o.gaps[0]?.w ?? 0) * ppl, target.size[1] * 0.05);
    return { x: 0, y: 0, w: (last.rect.x + last.rect.w - first.rect.x) * ppl + 2 * pad, h: target.size[1] + 2 * pad };
  }

  function sync() {
    const o = store.get().overlay;
    const target = currentTarget();
    const banner = target?.kind === 'banner';
    const next = !o?.images.length || !target ? '' : banner ? 'banner' : JSON.stringify([target.id, target.size, o.images.map((i) => i.rect), o.gaps[0]?.w ?? 0]);
    caption.textContent = target && !banner ? `${target.id}${target.gallery ? ` · ${target.gallery}` : ''}` : '';
    if (next === sig) return false;
    sig = next;
    world.replaceChildren();
    slot = null;
    message.hidden = next !== 'banner' && next !== '';
    message.textContent = next === 'banner' ? t('gallery.banner') : t('canvas.empty');
    if (!next || next === 'banner') return true;
    slot = makeSlot(world, estimate(o, target), 'gallery-slot');
    slot.height = target.size[1];
    fitOnPaint = true;
    vp.refresh(`${store.get().summary?.dir}|${sig}`);
    applyTransform();
    return true;
  }

  async function render() {
    const s = slot;
    if (!s) return;
    const level = levelOf(s.rect);
    const gen = ++s.gen;
    s.dirty = false;
    try {
      const height = Math.max(1, Math.round(s.height * level));
      const bitmap = await toImageBitmap(await api.renderGallery(viewArgs(), height));
      if (gen !== s.gen || s !== slot) {
        bitmap.close?.();
        return;
      }
      const k = height / s.height;
      placeSlot(s, { x: 0, y: 0, w: bitmap.width / k, h: bitmap.height / k });
      paintSlot(s, bitmap, level);
      if (fitOnPaint) {
        fitOnPaint = false;
        if (vp.fitMode) vp.fit();
      }
    } catch (e) {
      if (gen === s.gen) {
        s.dirty = true;
        editor.toast(e.message, true);
      }
    }
  }

  function updateResolution() {
    if (!slot) return;
    if (slot.dirty || Math.abs(slot.level - levelOf(slot.rect)) > 1e-9) render();
  }

  function scheduleResolution(delay = 160) {
    clearTimeout(resTimer);
    resTimer = setTimeout(updateResolution, delay);
  }

  function invalidate() {
    sync();
    if (slot) slot.dirty = true;
    updateResolution();
  }

  const unsubs = [
    store.select((st) => [st.overlay, st.target], () => {
      if (sync()) updateResolution();
    }),
    editor.on('invalidate', invalidate),
    editor.on('reset', invalidate),
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
