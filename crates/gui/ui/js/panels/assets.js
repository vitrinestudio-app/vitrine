// Assets panel (task 2.8): the project's screenshots (by language →
// variant, with thumbnails, pixel size and the devices using each key),
// missing screenshots of used keys (placeholders with "import"), images in
// assets/ and fonts (project files + bundled families).
//
// Import: button (file dialog) or drag & drop onto the panel / a device on
// the canvas (see importer.js); remove with confirmation when the file is
// used. Refreshes on `project-changed` (reason `assets`, from the folder
// watcher) and after imports/removals.

import { listen } from '../api.js';
import { h, icon } from '../dom.js';
import { confirmDialog } from '../dialogs/modal.js';
import { onImported, pickAndImport } from './importer.js';
import { openScreenshotImport } from '../dialogs/screenshot-import.js';

const THUMB = 96;

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { store, editor, api, t }) {
  const body = h('div', { class: 'as-body' });
  const root = h(
    'div',
    { class: 'as-panel' },
    h(
      'div',
      { class: 'as-toolbar' },
      h('button', { type: 'button', class: 'btn btn-small', onclick: () => pickAndImport() }, icon('plus'), t('assets.import')),
      h('button', { type: 'button', class: 'btn btn-small', title: t('import.menu_hint'), onclick: () => openScreenshotImport() }, t('import.button')),
      h('span', { class: 'muted small' }, t('assets.drop_hint')),
    ),
    body,
  );
  el.append(root);

  const thumbs = new Map(); // path → object URL
  let listing = null;
  let fonts = null;
  let seq = 0;
  const open = new Set(['screenshots', 'images', 'fonts']);

  // ----- thumbnails (lazy) ------------------------------------------------------------
  const io = new IntersectionObserver((entries) => {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      io.unobserve(e.target);
      loadThumb(e.target);
    }
  });

  async function loadThumb(img) {
    const path = img.dataset.path;
    const stamp = img.dataset.stamp;
    const cacheKey = `${path}|${stamp}`;
    if (thumbs.has(cacheKey)) {
      img.src = thumbs.get(cacheKey);
      return;
    }
    try {
      const p = await api.assetThumbnail(path, Math.round(THUMB * (window.devicePixelRatio || 1)));
      const url = URL.createObjectURL(new Blob([p.data], { type: 'image/png' }));
      thumbs.set(cacheKey, url);
      img.src = url;
    } catch {
      img.classList.add('broken');
    }
  }

  function thumb(path, stamp = '') {
    const img = h('img', { class: 'as-thumb', alt: '', dataset: { path, stamp: String(stamp) }, draggable: 'false' });
    io.observe(img);
    return img;
  }

  // ----- data ------------------------------------------------------------------------------
  async function load() {
    const my = ++seq;
    try {
      const [l, f] = await Promise.all([api.assetsList(), fonts ? Promise.resolve(fonts) : api.catalogFonts().catch(() => null)]);
      if (my !== seq) return;
      listing = l;
      fonts = f;
      render();
    } catch (e) {
      editor.toast(e.message, true);
    }
  }

  // ----- rendering ---------------------------------------------------------------------------
  function section(id, title, count, ...children) {
    const d = h('details', { class: 'as-section', open: open.has(id) }, h('summary', { class: 'section-title' }, title, h('span', { class: 'as-count' }, String(count))), ...children);
    d.addEventListener('toggle', () => (d.open ? open.add(id) : open.delete(id)));
    return d;
  }

  function usedBy(ids) {
    if (!ids?.length) return h('span', { class: 'as-unused muted' }, t('assets.unused'));
    return h(
      'span',
      { class: 'as-users' },
      ...ids.map((id) =>
        h('button', { type: 'button', class: 'as-user', title: t('assets.select'), onclick: () => selectElement(id) }, id),
      ),
    );
  }

  function selectElement(id) {
    const o = store.get().overlay;
    if (!o?.elements.some((e) => e.id === id)) {
      editor.toast(`${id}: ${t('assets.not_in_view')}`);
      return;
    }
    store.set({ view: 'canvas', selection: [id] });
  }

  function item({ path, title, sub, users, removable = true, stamp }) {
    return h(
      'div',
      { class: 'as-item', title: path },
      path ? thumb(path, stamp) : h('div', { class: 'as-thumb placeholder-thumb' }, icon('image')),
      h('div', { class: 'as-meta' }, h('div', { class: 'as-name' }, title), sub ? h('div', { class: 'as-sub muted small' }, sub) : null, users),
      removable ? h('button', { type: 'button', class: 'as-remove', title: t('assets.remove'), onclick: () => remove(path, users?.dataset?.ids?.split(',') ?? []) }, icon('close')) : null,
    );
  }

  function size(f) {
    return f.width ? `${f.width} × ${f.height}` : '';
  }

  /** Where a language without own screenshots gets them from (core fallback chain). */
  function emptyHint(src) {
    const chain = src?.chain?.join(' → ') ?? '';
    let text;
    if (!src?.source) text = t('assets.lang_empty_none');
    else if (src.source_is_default) text = t('assets.lang_empty_default').replace('{lang}', src.source);
    else text = t('assets.lang_empty_fallback').replace('{lang}', src.source);
    return h('p', { class: 'placeholder', title: chain ? `${t('assets.lang_chain')}: ${chain}` : '' }, text);
  }

  /**
   * Screenshots `lang` shows from a fallback language (resolved per key in
   * Rust with the renderer's lookup), grouped by the source language.
   */
  function borrowedGroups(borrowed, def) {
    const bySource = new Map();
    for (const b of borrowed) {
      if (!bySource.has(b.lang)) bySource.set(b.lang, []);
      bySource.get(b.lang).push(b);
    }
    return [...bySource.entries()].map(([from, list]) =>
      h(
        'div',
        { class: 'as-variant as-borrowed' },
        h('div', { class: 'as-variant-title muted small' }, t(from === def ? 'assets.borrowed_default' : 'assets.borrowed').replace('{lang}', from)),
        ...list.map((b) => {
          const f = listing.screenshots.find((x) => x.path === b.path);
          const sub = [b.variant, f ? size(f) : ''].filter(Boolean).join(' · ');
          return item({ path: b.path, title: b.key, sub, removable: false, stamp: f ? `${f.width}x${f.height}` : '' });
        }),
      ),
    );
  }

  function render() {
    const l = listing;
    const s = store.get().summary;
    if (!l || !s) return;
    const def = s.languages.default;
    const langs = [def, ...s.languages.available.filter((x) => x !== def)];

    // Screenshots grouped by language → variant.
    const groups = langs.map((lang) => {
      const files = l.screenshots.filter((f) => f.lang === lang);
      const byVariant = new Map();
      for (const f of files) {
        const v = f.variant ?? '';
        if (!byVariant.has(v)) byVariant.set(v, []);
        byVariant.get(v).push(f);
      }
      const variants = [...byVariant.keys()].sort((a, b) => (a === '' ? 1 : b === '' ? -1 : a.localeCompare(b)));
      const src = l.screenshot_langs?.[lang];
      const borrowed = src?.borrowed ?? [];
      const notes = [];
      if (lang === def) notes.push(t('assets.default_lang'));
      if (src?.fallback_for?.length) notes.push(t('assets.fallback_for').replace('{langs}', src.fallback_for.join(', ')));
      return h(
        'details',
        { class: 'as-lang', open: lang === store.get().lang },
        h(
          'summary',
          { title: src?.chain ? `${t('assets.lang_chain')}: ${src.chain.join(' → ')}` : '' },
          h('strong', {}, lang),
          notes.length ? h('span', { class: 'muted small' }, ` · ${notes.join(' · ')}`) : null,
          h('span', { class: 'as-count' }, borrowed.length ? `${files.length} + ${borrowed.length}` : String(files.length)),
        ),
        files.length || borrowed.length ? null : emptyHint(src),
        ...variants.map((v) =>
          h(
            'div',
            { class: 'as-variant' },
            h('div', { class: 'as-variant-title muted small' }, v || t('assets.variant_none')),
            ...byVariant.get(v).map((f) => {
              const users = usedBy(l.screenshot_keys[f.key]);
              users.dataset.ids = (l.screenshot_keys[f.key] ?? []).join(',');
              return item({ path: f.path, title: f.key, sub: size(f), users, stamp: `${f.width}x${f.height}` });
            }),
          ),
        ),
        ...borrowedGroups(borrowed, def),
      );
    });

    // Keys used by devices without a file in the default language for a
    // target variant (no fallback at all → placeholder in the render).
    const missing = [];
    // Per key: the screenshot variants of the targets of the strips whose
    // devices use it.
    const stripOf = new Map();
    for (const sp of s.strips) for (const e of sp.elements) stripOf.set(e.id, sp.id);
    for (const [key, ids] of Object.entries(l.screenshot_keys)) {
      const strips = new Set(ids.map((i) => stripOf.get(i)));
      const variants = [...new Set(s.targets.filter((tg) => strips.has(tg.strip)).map((tg) => tg.screenshots).filter(Boolean))];
      for (const v of variants) {
        const ok = l.screenshots.some((f) => f.lang === def && f.key === key && (f.variant == null || f.variant === v));
        if (!ok) missing.push({ key, variant: v, ids });
      }
    }
    const missingList = missing.length
      ? h(
          'details',
          { class: 'as-missing' },
          h('summary', { class: 'as-variant-title as-missing-title small' }, icon('warning'), ` ${t('assets.missing')}`, h('span', { class: 'as-count' }, String(missing.length))),
          ...missing.map((m) =>
            h(
              'div',
              { class: 'as-item missing' },
              h('div', { class: 'as-thumb placeholder-thumb' }, m.key),
              h('div', { class: 'as-meta' }, h('div', { class: 'as-name' }, m.key), h('div', { class: 'as-sub muted small' }, `${def} · ${m.variant ?? t('assets.variant_none')}`), usedBy(m.ids)),
              h(
                'button',
                {
                  type: 'button',
                  class: 'btn btn-small',
                  onclick: () => pickAndImport({ kind: 'screenshot', key: m.key, lang: def, variant: m.variant ?? '', pickKind: 'screenshot', replace: true }),
                },
                t('assets.import'),
              ),
            ),
          ),
        )
      : null;

    const images = l.assets.map((f) => {
      const ids = l.asset_usage[f.path] ?? [];
      const users = usedBy(ids);
      users.dataset.ids = ids.join(',');
      return item({ path: f.path, title: f.path.replace(/^assets\//, ''), sub: size(f), users, stamp: size(f) });
    });

    const fontItems = [
      ...l.fonts.map((f) =>
        h('div', { class: 'as-font' }, icon('text'), h('span', { class: 'grow' }, f.path.replace(/^fonts\//, '')), h('button', { type: 'button', class: 'as-remove', title: t('assets.remove'), onclick: () => remove(f.path, []) }, icon('close'))),
      ),
      ...(fonts?.project ?? []).map((fam) => h('div', { class: 'as-font muted' }, icon('text'), h('span', { class: 'grow' }, fam), h('span', { class: 'small' }, t('assets.font_project')))),
      ...(fonts?.bundled ?? []).map((fam) => h('div', { class: 'as-font muted' }, icon('text'), h('span', { class: 'grow' }, fam), h('span', { class: 'small' }, t('assets.font_bundled')))),
    ];

    body.replaceChildren(
      section('screenshots', t('assets.screenshots'), l.screenshots.length, missingList, ...groups),
      section('images', t('assets.images'), l.assets.length, images.length ? images : h('p', { class: 'placeholder' }, t('assets.none')), h('button', { type: 'button', class: 'btn btn-small as-add', onclick: () => pickAndImport({ kind: 'asset', pickKind: 'asset' }) }, icon('plus'), t('assets.import_image'))),
      section('fonts', t('assets.fonts'), l.fonts.length, ...fontItems, h('button', { type: 'button', class: 'btn btn-small as-add', onclick: () => pickAndImport({ kind: 'font', pickKind: 'font' }) }, icon('plus'), t('assets.import_font'))),
    );
  }

  async function remove(path, users) {
    const ok = await confirmDialog({
      title: t('assets.remove'),
      message: users.length ? `${t('assets.remove_used')} ${path}` : `${t('assets.remove_confirm')} ${path}`,
      details: users,
      ok: t('assets.remove'),
      danger: true,
    });
    if (!ok) return;
    try {
      await api.assetsRemove([path]);
      editor.toast(`${t('assets.removed')}: ${path}`);
      load();
    } catch (e) {
      editor.toast(e.message, true);
    }
  }

  // ----- wiring --------------------------------------------------------------------------------
  let timer = 0;
  const later = () => {
    clearTimeout(timer);
    timer = setTimeout(load, 120);
  };
  let unlisten = null;
  let alive = true;
  listen('project-changed', (c) => {
    if (c.reason === 'assets') {
      if (c.changes?.fonts) fonts = null;
      later();
    } else later();
  }).then((u) => (alive ? (unlisten = u) : u()));
  const unsubs = [onImported(later), editor.on('reset', later), store.select((s) => [s.summary, s.lang], () => render())];
  load();
  return {
    unmount() {
      alive = false;
      clearTimeout(timer);
      unlisten?.();
      unsubs.forEach((u) => u());
      io.disconnect();
      for (const url of thumbs.values()) URL.revokeObjectURL(url);
      root.remove();
    },
  };
}
