// Image preview ("lightbox") for screenshots and image assets: the image
// large (Rust thumbnail up to 2048 px, `asset_thumbnail`), its file name,
// pixel size, file size, format, colour space (+ the note when the profile
// is ignored, `asset_info`) and the elements using it. ←/→ step through
// the list it was opened from; Esc, the close button or a click outside
// close it. Opened from the assets panel (screenshots, images) and the
// inspector of image elements.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { openModal } from './modal.js';

/** Largest preview edge the backend renders. */
const MAX_EDGE = 2048;

/**
 * `items`: `[{ path, title?, users?: [element ids], lang?, variant?, key?, borrowedFrom? }]`;
 * `index`: the one shown first; `onSelectElement(id)`: "used by" chip
 * clicked (the preview closes first).
 */
export function openImagePreview({ items, index = 0, onSelectElement }) {
  if (!items?.length) return null;
  let i = Math.max(0, Math.min(items.length - 1, index));
  let seq = 0;
  let url = null;

  const img = h('img', { class: 'ip-img', alt: '', draggable: 'false' });
  const spinner = h('div', { class: 'ip-spinner' });
  const prev = h('button', { type: 'button', class: 'ip-nav ip-prev', title: `${t('preview.prev')} (←)`, onclick: () => step(-1) }, icon('chevron-left'));
  const next = h('button', { type: 'button', class: 'ip-nav ip-next', title: `${t('preview.next')} (→)`, onclick: () => step(1) }, icon('chevron-right'));
  const stage = h('div', { class: 'ip-stage' }, img, spinner, prev, next);
  const meta = h('dl', { class: 'ip-meta' });
  const counter = h('span', { class: 'ip-counter muted small' });
  const m = openModal({
    title: '',
    size: 'large',
    cls: 'ip-dialog',
    body: h('div', { class: 'ip-body' }, stage, h('div', { class: 'ip-side' }, meta, counter)),
    onClose: () => {
      document.removeEventListener('keydown', onKey, true);
      if (url) URL.revokeObjectURL(url);
    },
  });
  const heading = m.el.querySelector('.dlg-head h2');

  function onKey(e) {
    if (e.key !== 'ArrowLeft' && e.key !== 'ArrowRight') return;
    if (e.target?.closest?.('input, textarea, select')) return;
    e.preventDefault();
    e.stopPropagation();
    step(e.key === 'ArrowLeft' ? -1 : 1);
  }
  document.addEventListener('keydown', onKey, true);

  function step(d) {
    if (items.length < 2) return;
    i = (i + d + items.length) % items.length;
    show();
  }

  function fmtBytes(n) {
    if (n == null) return '';
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / 1024 / 1024).toFixed(1)} MB`;
  }

  function row(label, ...value) {
    return [h('dt', {}, label), h('dd', {}, ...value)];
  }

  function renderMeta(it, info) {
    const rows = [];
    rows.push(...row(t('preview.file'), h('span', { class: 'mono ip-path' }, it.path)));
    if (it.key) rows.push(...row(t('preview.key'), h('span', { class: 'mono' }, it.key)));
    if (it.lang) rows.push(...row(t('preview.lang'), it.borrowedFrom ? `${it.lang} (${t('preview.borrowed').replace('{lang}', it.borrowedFrom)})` : it.lang));
    if (it.key) rows.push(...row(t('preview.variant'), it.variant || t('assets.variant_none')));
    if (info) {
      rows.push(...row(t('preview.pixels'), info.width ? `${info.width} × ${info.height} px` : t('preview.vector')));
      rows.push(...row(t('preview.size'), fmtBytes(info.bytes)));
      if (info.format) rows.push(...row(t('preview.format'), info.format.toUpperCase()));
      if (info.color_space) {
        rows.push(
          ...row(
            t('preview.color'),
            h('span', {}, info.color_space),
            info.color_note ? h('div', { class: 'ip-note small' }, icon('warning'), h('span', {}, info.color_note)) : null,
          ),
        );
      }
    }
    const users = it.users ?? [];
    rows.push(
      ...row(
        t('preview.used_by'),
        users.length
          ? h(
              'span',
              { class: 'as-users' },
              ...users.map((id) =>
                h(
                  'button',
                  {
                    type: 'button',
                    class: 'as-user',
                    title: t('assets.select'),
                    onclick: () => {
                      m.close();
                      onSelectElement?.(id);
                    },
                  },
                  id,
                ),
              ),
            )
          : h('span', { class: 'muted' }, t('assets.unused')),
      ),
    );
    meta.replaceChildren(...rows);
  }

  async function show() {
    const my = ++seq;
    const it = items[i];
    heading.textContent = it.title ?? it.path.split('/').pop();
    counter.textContent = items.length > 1 ? `${i + 1} / ${items.length}` : '';
    prev.hidden = next.hidden = items.length < 2;
    stage.classList.add('loading');
    stage.classList.remove('broken');
    renderMeta(it, null);
    const r = stage.getBoundingClientRect();
    const edge = Math.min(MAX_EDGE, Math.round(Math.max(r.width, r.height, 400) * (window.devicePixelRatio || 1)));
    const [pic, info] = await Promise.allSettled([api.assetThumbnail(it.path, edge), api.assetInfo(it.path)]);
    if (my !== seq) return;
    stage.classList.remove('loading');
    if (pic.status === 'fulfilled') {
      if (url) URL.revokeObjectURL(url);
      url = URL.createObjectURL(new Blob([pic.value.data], { type: 'image/png' }));
      img.src = url;
      img.alt = it.path;
    } else {
      img.removeAttribute('src');
      stage.classList.add('broken');
    }
    renderMeta(it, info.status === 'fulfilled' ? info.value : null);
  }

  show();
  return m;
}
