// Project settings → "Frames" tab (ADR 0027): "Store used device frames in
// the project". Copies the imported Apple bezels the project's targets draw
// into `frames/imported/` so that a build server (CI) without the app-wide
// frame store renders the same images. Shows what is stored (thumbnails,
// sizes, disk usage), what an update would change (new colour, re-imported
// bezel, unused copies) and the licence warning (Apple's bezels may be used
// for mock-ups but not redistributed: private repositories only).
// Backend: `frames_embed_status` (dry run), `frames_embed`, `frames_unembed`.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { checkbox, confirmDialog } from './modal.js';

/** Bytes as "1.2 MB" / "340 KB". */
export function formatBytes(n) {
  if (n >= 1e6) return `${(n / 1e6).toFixed(1)} MB`;
  return `${Math.max(1, Math.ceil(n / 1e3))} KB`;
}

const THUMB = 64;

/**
 * The tab's content. `rerender()` redraws the dialog pane after a change;
 * `state` keeps the "all sizes" choice across re-renders.
 */
export async function storedFramesPane(rerender, state) {
  let st;
  try {
    st = await api.framesEmbedStatus(state.allSizes);
  } catch (e) {
    return h('div', { class: 'form-error', role: 'alert' }, String(e?.message ?? e));
  }
  state.allSizes ??= st.all_sizes;
  const err = h('div', { class: 'form-error', role: 'alert', hidden: true });
  const fail = (e) => {
    err.hidden = false;
    err.textContent = String(e?.message ?? e);
  };

  const items = st.items ?? [];
  const kept = items.filter((i) => i.action !== 'remove');
  const count = (a) => items.filter((i) => i.action === a).length;
  const intro = h('p', { class: 'muted small' }, t('frames.stored.intro'));
  const license = h(
    'div',
    { class: 'fp-license hint-warn', role: 'note' },
    h('div', { class: 'inline' }, icon('warning'), h('strong', {}, t('frames.stored.license.title'))),
    h('p', {}, t('frames.stored.license')),
    h('p', {}, t('frames.stored.gitignore')),
  );

  if (!st.uses_imported && !st.stored) {
    return h('div', { class: 'stack' }, h('div', { class: 'section-title' }, t('frames.stored.title')), intro, h('div', { class: 'placeholder' }, t('frames.stored.none')));
  }

  // Status line.
  let status;
  if (st.stored) {
    const parts = [t('frames.stored.count').replace('{n}', String(count('keep') + count('update') + count('remove'))).replace('{size}', formatBytes(st.folder_bytes))];
    if (st.up_to_date) parts.push(t('frames.stored.up_to_date'));
    else parts.push(t('frames.stored.changes').replace('{add}', String(count('add'))).replace('{update}', String(count('update'))).replace('{remove}', String(count('remove'))));
    status = h('div', { class: st.up_to_date ? 'muted' : 'hint-warn' }, parts.join(' · '));
  } else {
    status = h('div', { class: 'muted' }, t('frames.stored.not_stored').replace('{n}', String(kept.length)).replace('{size}', formatBytes(st.bytes_after)));
  }

  const busy = (b, fn) => async () => {
    b.disabled = true;
    err.hidden = true;
    try {
      await fn();
    } catch (e) {
      fail(e);
    } finally {
      b.disabled = false;
    }
  };

  const embed = async () => {
    if (!st.stored) {
      const ok = await confirmDialog({
        title: t('frames.stored.confirm.title'),
        message: h(
          'div',
          {},
          h('p', { class: 'dlg-text' }, t('frames.stored.confirm').replace('{n}', String(kept.length)).replace('{size}', formatBytes(st.bytes_after))),
          h('div', { class: 'fp-license hint-warn' }, h('p', {}, t('frames.stored.license')), h('p', {}, t('frames.stored.gitignore'))),
        ),
        confirm: t('frames.stored.confirm.ok'),
      });
      if (!ok) return;
    }
    const r = await api.framesEmbed({ all_sizes: state.allSizes });
    editor.toast(t('frames.stored.done').replace('{n}', String(r.items.filter((i) => i.action !== 'remove').length)).replace('{size}', formatBytes(r.bytes_after)));
    await rerender();
  };
  const unembed = async () => {
    const ok = await confirmDialog({
      title: t('frames.stored.remove.title'),
      message: t('frames.stored.remove.confirm'),
      confirm: t('frames.stored.remove'),
      danger: true,
    });
    if (!ok) return;
    const bytes = await api.framesUnembed();
    editor.toast(t('frames.stored.removed').replace('{size}', formatBytes(bytes)));
    state.allSizes = undefined;
    await rerender();
  };

  const embedBtn = h('button', { class: 'btn btn-primary', type: 'button', disabled: st.stored && st.up_to_date ? true : undefined }, st.stored ? t('frames.stored.update') : t('frames.stored.store'));
  embedBtn.addEventListener('click', busy(embedBtn, embed));
  const removeBtn = st.stored ? h('button', { class: 'btn', type: 'button' }, icon('trash'), t('frames.stored.remove')) : null;
  removeBtn?.addEventListener('click', busy(removeBtn, unembed));
  const allSizes = checkbox({
    label: t('frames.stored.all_sizes'),
    title: t('frames.stored.all_sizes.hint'),
    checked: state.allSizes,
    onchange: async (v) => {
      state.allSizes = v;
      await rerender();
    },
  });

  // One card per frame.
  const badge = (a) => h('span', { class: `fp-badge fp-badge-${a === 'add' ? 'install' : a === 'keep' ? 'kept' : 'replace'}` }, t(`frames.stored.action.${a}`));
  const thumb = (i) => {
    const img = h('img', { class: 'fp-thumb', alt: '', draggable: 'false' });
    const source = i.origin === 'project' ? `project:${i.source}` : i.source;
    api
      .framesThumbnail(i.file, Math.round(THUMB * (window.devicePixelRatio || 1)), source)
      .then((p) => {
        const url = URL.createObjectURL(new Blob([p.data], { type: 'image/png' }));
        img.addEventListener('load', () => URL.revokeObjectURL(url), { once: true });
        img.src = url;
      })
      .catch(() => img.classList.add('broken'));
    return img;
  };
  const card = (i) =>
    h(
      'div',
      { class: 'fp-card' },
      thumb(i),
      h(
        'div',
        { class: 'fp-card-text' },
        h('div', { class: 'fp-card-model' }, i.bezel),
        h('div', { class: 'muted small' }, `${i.color_label} · ${t(`inspector.orientation.${i.orientation}`)} · ${formatBytes(i.bytes)}`),
        h('div', { class: 'muted small fp-card-src', title: i.targets.join(', ') }, i.targets.length ? i.targets.join(', ') : t('frames.stored.no_target')),
        badge(i.action),
      ),
    );

  const missing = (st.missing ?? []).length
    ? h(
        'div',
        { class: 'hint-warn small' },
        t('frames.stored.missing'),
        h('ul', { class: 'dlg-list' }, ...st.missing.map((m) => h('li', {}, `${m.model} · ${m.color ?? t('inspector.default')} · ${t(`inspector.orientation.${m.orientation}`)} (${m.targets.join(', ')})`))),
      )
    : null;
  const licMissing = (st.licenses_missing ?? []).length ? h('p', { class: 'muted small' }, t('frames.stored.license_missing').replace('{packages}', st.licenses_missing.join(', '))) : null;
  const folder = st.stored ? h('div', { class: 'mono small selectable muted' }, st.dir) : null;

  return h(
    'div',
    { class: 'stack' },
    h('div', { class: 'section-title' }, t('frames.stored.title')),
    intro,
    license,
    status,
    h('div', { class: 'inline wrap' }, embedBtn, removeBtn, allSizes),
    err,
    missing,
    items.length ? h('div', { class: 'fp-grid' }, ...items.map(card)) : null,
    licMissing,
    folder,
  );
}
