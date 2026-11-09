// Pages view: the output images of the current target side by side, spaced
// like the store gallery (the target's gap width), each rendered exactly as
// exported (`render_image`).

import { api, toImageBitmap } from '../api.js';
import { h } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { store, viewArgs } from '../store.js';

export function mountPages(root) {
  const row = h('div', { class: 'strip-row' });
  const el = h('div', { class: 'strip-view' }, row);
  root.append(el);
  let seq = 0;

  async function render() {
    const my = ++seq;
    const o = store.get().overlay;
    if (!o) {
      row.replaceChildren();
      return;
    }
    const height = Math.max(120, el.clientHeight - 110);
    const dpr = window.devicePixelRatio || 1;
    const gapLu = o.gaps[0]?.w ?? 0;
    const figures = o.images.map((img) => {
      const k = height / img.rect.h;
      const canvas = h('canvas');
      canvas.style.width = `${img.rect.w * k}px`;
      canvas.style.height = `${height}px`;
      const ids = img.pages.map((i) => o.pages.find((p) => p.index === i)?.id ?? i + 1).join(' + ');
      const fig = h('figure', {}, canvas, h('figcaption', {}, `${t('canvas.image')} ${img.index + 1} · ${ids}`));
      fig.style.marginRight = `${Math.max(8, gapLu * k)}px`;
      return { img, canvas, width: Math.round(img.rect.w * k * dpr), fig };
    });
    row.replaceChildren(...figures.map((f) => f.fig));
    const v = viewArgs();
    await Promise.all(
      figures.map(async (f) => {
        const bitmap = await toImageBitmap(await api.renderImage(v, f.img.index, f.width));
        if (my !== seq) return;
        f.canvas.width = bitmap.width;
        f.canvas.height = bitmap.height;
        f.canvas.getContext('2d').drawImage(bitmap, 0, 0);
      }),
    ).catch((e) => editor.toast(e.message, true));
  }

  const unsubs = [
    editor.on('invalidate', render),
    editor.on('reset', render),
  ];
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
