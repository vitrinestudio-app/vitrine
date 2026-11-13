// "Template gallery" tab of the New-project dialog (ADR 0025): browses the
// online gallery (`gallery_list`, previews via the image command
// `gallery_preview`), filters locally by search text and tag facets, and
// installs the chosen template into the app-wide store (`gallery_download`)
// before the project is created from it like from a local template.
// Nothing is loaded until the tab is opened (`activate`).

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';

/** `t` with `{name}` parameters. */
const tt = (key, params = {}) => Object.entries(params).reduce((s, [k, v]) => s.replaceAll(`{${k}}`, String(v)), t(key));

/** UI language for labels from the tag taxonomy (`en` | `de`). */
const uiLang = () => (document.documentElement.lang === 'de' ? 'de' : 'en');

/** The gallery page of the website (allow-listed in `open_url`). */
export const galleryPageUrl = (id) =>
  `${uiLang() === 'de' ? 'https://www.vitrinestudio.app/de/galerie/' : 'https://www.vitrinestudio.app/gallery/'}${id ? `#${id}` : ''}`;

/** Facets shown as filters, in this order. */
const FACETS = ['style', 'color', 'use', 'devices'];
/** Most pages fetched (50 templates each). */
const MAX_PAGES = 10;
/** Previews loading at the same time. */
const PREVIEW_CONCURRENCY = 4;

/** Object URLs of gallery previews by `id|canvas|version` (kept for the session). */
const previews = new Map();
const queue = [];
let running = 0;

function pump() {
  while (running < PREVIEW_CONCURRENCY && queue.length) {
    const job = queue.shift();
    running++;
    job().finally(() => {
      running--;
      pump();
    });
  }
}

function previewUrl(tpl) {
  const canvas = tpl.canvases[0]?.id;
  const key = `${tpl.id}|${canvas ?? ''}|${tpl.version}`;
  if (!previews.has(key)) {
    previews.set(
      key,
      new Promise((resolve, reject) => {
        queue.push(() =>
          api
            .galleryPreview(tpl.id, canvas)
            .then((p) => resolve(URL.createObjectURL(new Blob([p.data], { type: 'image/png' }))))
            .catch((e) => {
              previews.delete(key);
              reject(e);
            }),
        );
        pump();
      }),
    );
  }
  return previews.get(key);
}

const megabytes = (bytes) => (bytes >= 10 * 1024 * 1024 ? Math.round(bytes / 1048576) : Math.max(0.1, Math.round(bytes / 104857.6) / 10));

/**
 * The gallery pane. `onchange(item|null)` fires when the selection
 * changes. Returns `{ el, activate(), selected, install() }`; `install()`
 * downloads the selected template (unless that version is installed) and
 * resolves to its id in the local store.
 */
export function galleryPicker({ onchange }) {
  const el = h('div', { class: 'tg-pane' });
  const grid = h('div', { class: 'tg-grid', role: 'listbox', 'aria-label': t('tgallery.title') });
  const status = h('div', { class: 'tg-status' });
  const count = h('span', { class: 'small muted tg-count' });
  const filters = h('div', { class: 'tg-filters' });
  const st = { list: [], tax: null, query: '', facet: {}, selected: null, loaded: false, loading: false };

  const search = h('input', {
    type: 'search',
    class: 'field tg-search',
    placeholder: t('tgallery.search'),
    'aria-label': t('tgallery.search'),
    oninput: (e) => {
      st.query = e.target.value.trim().toLowerCase();
      draw();
    },
  });
  const site = h(
    'button',
    { class: 'btn btn-small', type: 'button', title: t('tgallery.website.hint'), onclick: () => openSite() },
    t('tgallery.website'),
    icon('external'),
  );
  el.append(h('div', { class: 'tg-toolbar' }, search, filters, h('span', { class: 'spacer' }), count, site), status, grid);

  function openSite(id) {
    api.openUrl(galleryPageUrl(id)).catch((e) => showError(e));
  }

  /** Label of a tag value from the taxonomy. */
  function label(facet, value) {
    const f = st.tax?.facets.find((x) => x.id === facet);
    return f?.values.find((v) => v.id === value)?.label[uiLang()] ?? value;
  }

  const tagsOf = (tpl, facet) => (facet === 'devices' ? tpl.devices ?? [] : tpl.tags?.[facet] ?? []);

  function matches(tpl) {
    for (const f of FACETS) if (st.facet[f] && !tagsOf(tpl, f).includes(st.facet[f])) return false;
    if (!st.query) return true;
    const hay = [tpl.name, tpl.description, tpl.author, ...FACETS.flatMap((f) => tagsOf(tpl, f).map((v) => label(f, v)))].join(' ').toLowerCase();
    return st.query.split(/\s+/).every((w) => hay.includes(w));
  }

  function drawFilters() {
    filters.replaceChildren(
      ...FACETS.map((f) => {
        const facet = st.tax?.facets.find((x) => x.id === f);
        // Only values some template has, so a filter never ends empty.
        const used = new Set(st.list.flatMap((tpl) => tagsOf(tpl, f)));
        const values = (facet?.values ?? []).filter((v) => used.has(v.id));
        const name = facet?.label[uiLang()] ?? f;
        return h(
          'select',
          {
            class: 'field tg-facet',
            title: name,
            'aria-label': name,
            onchange: (e) => {
              st.facet[f] = e.target.value || null;
              draw();
            },
          },
          h('option', { value: '' }, tt('tgallery.facet.any', { facet: name })),
          ...values.map((v) => h('option', { value: v.id, selected: st.facet[f] === v.id }, v.label[uiLang()] ?? v.id)),
        );
      }),
    );
  }

  // Previews load when their card scrolls into view.
  const observer = new IntersectionObserver(
    (entries) => {
      for (const e of entries) {
        if (!e.isIntersecting) continue;
        observer.unobserve(e.target);
        const tpl = st.list.find((x) => x.id === e.target.dataset.id);
        const img = e.target.querySelector('img');
        if (!tpl || !img) continue;
        previewUrl(tpl)
          .then((url) => {
            img.src = url;
            img.classList.remove('loading');
          })
          .catch(() => img.classList.replace('loading', 'broken'));
      }
    },
    { root: grid, rootMargin: '200px' },
  );

  function card(tpl) {
    const canvases = tpl.canvases ?? [];
    const pages = Math.max(0, ...canvases.map((c) => c.pages ?? 0));
    const meta = [
      canvases.length > 1 ? tt('templates.canvases', { n: canvases.length }) : canvases[0]?.id,
      pages ? tt('templates.pages', { n: pages }) : null,
      tpl.size ? tt('tgallery.size', { mb: megabytes(tpl.size) }) : null,
    ].filter(Boolean);
    const chips = ['devices', 'style', 'color', 'use'].flatMap((f) => tagsOf(tpl, f).map((v) => h('span', { class: `tg-tag tg-tag-${f}` }, label(f, v))));
    const badges = [];
    if (tpl.samples) badges.push(h('span', { class: 'tg-badge', title: t('tgallery.samples.hint') }, t('tgallery.samples')));
    if (tpl.installed_version) {
      const same = tpl.installed_version === tpl.version;
      badges.push(h('span', { class: 'tg-badge tg-installed', title: tt('tgallery.installed.hint', { v: tpl.installed_version }) }, same ? t('tgallery.installed') : t('tgallery.update')));
    }
    const on = tpl.id === st.selected;
    const el = h(
      'div',
      {
        class: 'tpl-card tg-card',
        role: 'option',
        tabindex: '0',
        'aria-selected': String(on),
        'aria-pressed': String(on),
        title: tpl.description || tpl.name,
        dataset: { id: tpl.id },
        onclick: () => select(tpl.id),
        onkeydown: (e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            select(tpl.id);
          }
        },
      },
      h('div', { class: 'tg-thumb' }, h('img', { class: 'tg-preview loading', alt: '', draggable: 'false' })),
      h(
        'div',
        { class: 'tg-head' },
        h('span', { class: 'tpl-name' }, tpl.name),
        h(
          'button',
          {
            class: 'icon-btn tg-open',
            type: 'button',
            title: t('tgallery.website.one'),
            'aria-label': t('tgallery.website.one'),
            onclick: (e) => {
              e.stopPropagation();
              openSite(tpl.id);
            },
          },
          icon('external'),
        ),
      ),
      h('span', { class: 'tpl-meta muted small' }, [tpl.author, ...meta].filter(Boolean).join(' · ')),
      h('div', { class: 'tg-tags' }, ...badges, ...chips),
    );
    // Reuse a loaded preview right away (redraws after filtering).
    const cached = previews.get(`${tpl.id}|${canvases[0]?.id ?? ''}|${tpl.version}`);
    if (cached) {
      cached
        .then((url) => {
          const img = el.querySelector('img');
          img.src = url;
          img.classList.remove('loading');
        })
        .catch(() => observer.observe(el));
    } else {
      observer.observe(el);
    }
    return el;
  }

  function select(id) {
    st.selected = id;
    for (const c of grid.children) {
      const on = c.dataset.id === id;
      c.setAttribute('aria-selected', String(on));
      c.setAttribute('aria-pressed', String(on));
    }
    onchange?.(selectedItem());
  }

  const selectedItem = () => st.list.find((x) => x.id === st.selected) ?? null;

  function draw() {
    observer.disconnect();
    const shown = st.list.filter(matches);
    count.textContent = st.loaded ? tt('tgallery.count', { n: shown.length, total: st.list.length }) : '';
    if (st.loaded && !shown.length) {
      grid.replaceChildren(h('div', { class: 'tg-empty muted' }, st.list.length ? t('tgallery.no_match') : t('tgallery.empty')));
    } else {
      grid.replaceChildren(...shown.map(card));
    }
    // A filtered-out template stays chosen (shown in the footer), so typing
    // a search does not silently change what "Create" uses.
  }

  function showStatus(kind, text, retry) {
    status.className = `tg-status ${kind}`;
    status.replaceChildren(
      ...(kind === 'loading' ? [h('span', { class: 'tg-spinner', 'aria-hidden': 'true' })] : []),
      h('span', {}, text),
      retry ? h('button', { class: 'btn btn-small', type: 'button', onclick: () => load() }, t('tgallery.retry')) : null,
    );
    status.hidden = !text;
  }

  function showError(e) {
    if (e?.code === 'unreachable') showStatus('error', t('tgallery.unreachable'), true);
    else showStatus('error', tt('tgallery.error', { message: e?.message ?? String(e) }), true);
  }

  async function load() {
    if (st.loading) return;
    st.loading = true;
    showStatus('loading', t('tgallery.loading'));
    try {
      if (!st.tax) st.tax = await api.templateTags().catch(() => null);
      const list = [];
      for (let page = 0, offset = 0; page < MAX_PAGES; page++) {
        const r = await api.galleryList({ offset });
        list.push(...r.templates);
        if (!r.has_more) break;
        offset = r.offset + r.limit;
      }
      st.list = list;
      st.loaded = true;
      showStatus('', '');
      drawFilters();
      draw();
    } catch (e) {
      showError(e);
    } finally {
      st.loading = false;
    }
  }

  async function install() {
    const tpl = selectedItem();
    if (!tpl) throw new Error(t('tgallery.none'));
    if (tpl.installed_version === tpl.version) return tpl.id;
    const r = await api.galleryDownload(tpl.id, Boolean(tpl.installed_version));
    tpl.installed_version = r.version ?? tpl.version;
    return r.id;
  }

  return {
    el,
    /** Loads the gallery on first use. */
    activate() {
      if (!st.loaded) load();
      search.focus();
    },
    get selected() {
      return selectedItem();
    },
    install,
  };
}
