// Project settings → Fonts → "Store used system fonts in the project"
// (ADR 0028, the font counterpart of stored-frames.js). Copies the files of
// the installed system families the project uses (named by styles, or
// loaded as fallback, e.g. a CJK font) into `fonts/system/` so a build
// server (CI) or a colleague without them renders the same. Shows each
// file with the licence the font states (vendor, OFL or not) and a clear
// warning: most system and commercial fonts may not be redistributed.
// Backend: `fonts_embed_status` (dry run), `fonts_embed`, `fonts_unembed`.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { confirmDialog } from './modal.js';
import { formatBytes } from './stored-frames.js';

/** "used by styles" / "fallback for ja" / "fallback". */
function roleText(i) {
  if (i.role === 'named') return t('fonts.stored.role.named');
  return i.lang ? t('fonts.stored.role.fallback_lang').replace('{lang}', i.lang) : t('fonts.stored.role.fallback');
}

/** The licence line of an item: kind, vendor, stated licence (title). */
function licenseBadge(lic) {
  const cls = lic.kind === 'ofl' ? 'fp-badge fp-badge-kept' : 'fp-badge fp-badge-replace';
  const vendor = lic.vendor ? ` · ${lic.vendor}` : '';
  return h('span', { class: cls, title: [lic.license, lic.license_url, lic.copyright].filter(Boolean).join('\n') }, `${t(`fonts.stored.license.${lic.kind}`)}${vendor}`);
}

/** The section's content; `rerender()` redraws the pane after a change. */
export async function storedFontsPane(rerender) {
  let st;
  try {
    st = await api.fontsEmbedStatus();
  } catch (e) {
    return h('div', { class: 'form-error', role: 'alert' }, String(e?.message ?? e));
  }
  const err = h('div', { class: 'form-error', role: 'alert', hidden: true });
  const items = st.items ?? [];
  const kept = items.filter((i) => i.action !== 'remove');
  const files = new Set(kept.map((i) => i.file)).size;
  const count = (a) => items.filter((i) => i.action === a).length;
  const intro = h('p', { class: 'muted small' }, t('fonts.stored.intro'));
  const restricted = st.restricted ?? [];
  const license = h(
    'div',
    { class: 'fp-license hint-warn', role: 'note' },
    h('div', { class: 'inline' }, icon('warning'), h('strong', {}, t('fonts.stored.license.title'))),
    h('p', {}, t('fonts.stored.license.text')),
    restricted.length ? h('p', {}, t('fonts.stored.license.families').replace('{families}', restricted.join(', '))) : null,
    h('p', {}, t('fonts.stored.gitignore')),
  );
  const title = h('div', { class: 'section-title' }, t('fonts.stored.title'));

  if (!items.length) {
    const why = st.system_enabled ? t('fonts.stored.none') : t('fonts.stored.system_off');
    return h('div', { class: 'stack' }, title, intro, h('div', { class: 'placeholder' }, why));
  }

  let status;
  if (st.stored) {
    const parts = [t('fonts.stored.count').replace('{n}', String(files)).replace('{size}', formatBytes(st.folder_bytes))];
    if (st.up_to_date) parts.push(t('frames.stored.up_to_date'));
    else parts.push(t('fonts.stored.changes').replace('{add}', String(count('add'))).replace('{update}', String(count('update'))).replace('{remove}', String(count('remove'))));
    status = h('div', { class: st.up_to_date ? 'muted' : 'hint-warn' }, parts.join(' · '));
  } else {
    status = h('div', { class: 'muted' }, t('fonts.stored.not_stored').replace('{n}', String(files)).replace('{size}', formatBytes(st.bytes_after)));
  }

  const busy = (b, fn) => async () => {
    b.disabled = true;
    err.hidden = true;
    try {
      await fn();
    } catch (e) {
      err.hidden = false;
      err.textContent = String(e?.message ?? e);
    } finally {
      b.disabled = false;
    }
  };
  const store = async () => {
    const ok = await confirmDialog({
      title: t('fonts.stored.confirm.title'),
      message: h(
        'div',
        {},
        h('p', { class: 'dlg-text' }, t('fonts.stored.confirm').replace('{n}', String(files)).replace('{size}', formatBytes(st.bytes_after))),
        h('ul', { class: 'dlg-list' }, ...kept.map((i) => h('li', {}, `${i.family} – ${i.file} (${formatBytes(i.bytes)}) · `, licenseBadge(i.license)))),
        h('div', { class: 'fp-license hint-warn' }, h('p', {}, t('fonts.stored.license.text')), h('p', {}, t('fonts.stored.gitignore'))),
      ),
      confirm: t('fonts.stored.confirm.ok'),
    });
    if (!ok) return;
    const r = await api.fontsEmbed();
    editor.toast(t('fonts.stored.done').replace('{size}', formatBytes(r.bytes_after)));
    await rerender();
  };
  const remove = async () => {
    const ok = await confirmDialog({ title: t('fonts.stored.remove.title'), message: t('fonts.stored.remove.confirm'), confirm: t('frames.stored.remove'), danger: true });
    if (!ok) return;
    const bytes = await api.fontsUnembed();
    editor.toast(t('fonts.stored.removed').replace('{size}', formatBytes(bytes)));
    await rerender();
  };
  const storeBtn = h('button', { class: 'btn btn-primary', type: 'button', disabled: st.stored && st.up_to_date ? true : undefined }, st.stored ? t('fonts.stored.update') : t('fonts.stored.store'));
  storeBtn.addEventListener('click', busy(storeBtn, store));
  const removeBtn = st.stored ? h('button', { class: 'btn', type: 'button' }, icon('trash'), t('frames.stored.remove')) : null;
  removeBtn?.addEventListener('click', busy(removeBtn, remove));

  const rowEl = (i) =>
    h(
      'div',
      { class: 'ps-font-family' },
      h('div', { class: 'inline wrap' }, h('strong', {}, i.family), h('span', { class: 'muted small' }, ` · ${roleText(i)} · ${formatBytes(i.bytes)}`), h('span', { class: `fp-badge fp-badge-${i.action === 'add' ? 'install' : i.action === 'keep' ? 'kept' : 'replace'}` }, t(`fonts.stored.action.${i.action}`))),
      h('div', { class: 'inline wrap' }, h('span', { class: 'mono small muted selectable', title: i.source }, i.file), licenseBadge(i.license)),
    );

  return h(
    'div',
    { class: 'stack' },
    title,
    intro,
    license,
    status,
    h('div', { class: 'inline wrap' }, storeBtn, removeBtn),
    err,
    ...items.map(rowEl),
    st.stored ? h('div', { class: 'mono small selectable muted' }, st.dir) : null,
  );
}
