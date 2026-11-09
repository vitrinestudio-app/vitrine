// Store gallery view: the gallery preview of the current target
// (`render_gallery`: output images with the store's gaps, as the store
// shows them).

import { api, toImageBitmap } from '../api.js';
import { h } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { currentTarget, store, viewArgs } from '../store.js';

export function mountGallery(root) {
  const canvas = h('canvas');
  const caption = h('div', { class: 'muted small' });
  const el = h('div', { class: 'strip-view' }, h('div', { class: 'gallery-frame' }, canvas, caption));
  root.append(el);
  let seq = 0;

  async function render() {
    const my = ++seq;
    const target = currentTarget();
    if (!target) return;
    if (target.kind === 'banner') {
      canvas.hidden = true;
      caption.textContent = t('gallery.banner');
      return;
    }
    canvas.hidden = false;
    const dpr = window.devicePixelRatio || 1;
    const cssHeight = Math.max(120, el.clientHeight - 120);
    try {
      const img = await api.renderGallery(viewArgs(), Math.round(cssHeight * dpr));
      const bitmap = await toImageBitmap(img);
      if (my !== seq) return;
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      canvas.style.width = `${bitmap.width / dpr}px`;
      canvas.style.height = `${bitmap.height / dpr}px`;
      canvas.getContext('2d').drawImage(bitmap, 0, 0);
      caption.textContent = `${target.id}${target.gallery ? ` · ${target.gallery}` : ''}`;
    } catch (e) {
      editor.toast(e.message, true);
    }
  }

  const unsubs = [editor.on('invalidate', render), editor.on('reset', render), store.select((s) => s.target, render)];
  const ro = new ResizeObserver(() => render());
  ro.observe(el);
  render();
  return {
    unmount() {
      unsubs.forEach((u) => u());
      ro.disconnect();
      el.remove();
    },
  };
}
