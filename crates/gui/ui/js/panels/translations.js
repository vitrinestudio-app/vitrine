// Translations panel (task 2.7): keys × languages, default language first.
//
// - Missing cells are marked and show the fallback text greyed out.
// - Filter: search (key or text) and "missing only".
// - Click a cell to edit it in place (Enter = save as one `set_string`
//   step, Shift+Enter = line break, Esc = cancel); clearing a cell of a
//   non-default language removes the translation (fallback again).
// - Keys: add, rename (`rename_string_key`), delete (all languages;
//   confirmation lists the text elements using it).
// - Languages: add (`add_language`), remove (`remove_language`), click a
//   header to switch the view language.
// - Markers per cell from validation of all targets (`text_overflow`,
//   `emergency_break`, `text_missing`), refreshed throttled after changes.
//
// The sidebar widens while this panel is shown (styles/translations.css).

import { h, icon } from '../dom.js';
import { checkbox, choiceDialog, confirmDialog, promptDialog } from '../dialogs/modal.js';

const SOFT_HYPHEN = '­';
const VALIDATE_DELAY = 600;
const MARKER_CODES = { text_overflow: 'overflow', emergency_break: 'emergency', text_missing: 'missing' };
const LANG_RE = /^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/;

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { store, editor, api, t }) {
  const state = {
    table: null,
    missing: new Map(), // `${lang}\u0000${key}` → { fallback, fallback_lang }
    markers: new Map(), // `${lang}\u0000${key}` → { codes:Set, msgs:[] }
    search: '',
    missingOnly: false,
    editing: null, // { key, lang }
  };
  const id = (lang, key) => `${lang}\u0000${key}`;

  // ----- skeleton -------------------------------------------------------------------
  const search = h('input', {
    class: 'tr-search',
    type: 'search',
    placeholder: t('translations.search'),
    'aria-label': t('translations.search'),
    oninput: () => {
      state.search = search.value.trim().toLowerCase();
      renderRows();
    },
  });
  const missingToggle = h('input', {
    type: 'checkbox',
    onchange: () => {
      state.missingOnly = missingToggle.checked;
      renderRows();
    },
  });
  const counts = h('span', { class: 'tr-counts muted small' });
  const toolbar = h(
    'div',
    { class: 'tr-toolbar' },
    search,
    h('label', { class: 'tr-toggle' }, missingToggle, t('translations.missing_only')),
    h('span', { class: 'tr-spacer' }),
    counts,
    h('button', { type: 'button', class: 'btn btn-small', onclick: addKey, title: t('translations.add_key') }, icon('plus'), t('translations.key')),
    h('button', { type: 'button', class: 'btn btn-small', onclick: addLanguage, title: t('translations.add_lang') }, icon('plus'), t('translations.lang')),
  );
  const thead = h('thead');
  const tbody = h('tbody');
  const table = h('table', { class: 'tr-table' }, thead, tbody);
  const scroller = h('div', { class: 'tr-scroll' }, table);
  const root = h('div', { class: 'tr-panel' }, h('div', { class: 'section-title' }, t('panel.translations')), toolbar, scroller);
  el.append(root);

  // ----- data -----------------------------------------------------------------------
  let loadSeq = 0;
  async function load() {
    const my = ++loadSeq;
    try {
      const [table, missing] = await Promise.all([api.stringsGet(), api.stringsMissing()]);
      if (my !== loadSeq) return;
      state.table = table;
      state.missing = new Map(missing.map((m) => [id(m.lang, m.key), m]));
      render();
      scheduleValidation();
    } catch (e) {
      editor.toast(e.message, true);
    }
  }

  let validateTimer = 0;
  let validateSeq = 0;
  function scheduleValidation() {
    clearTimeout(validateTimer);
    validateTimer = setTimeout(validate, VALIDATE_DELAY);
  }

  async function validate() {
    const my = ++validateSeq;
    const table = state.table;
    if (!table) return;
    const keyOf = new Map();
    for (const [key, ids] of Object.entries(table.used_by)) for (const e of ids) keyOf.set(e, key);
    try {
      const r = await api.validateMany();
      if (my !== validateSeq) return;
      const markers = new Map();
      for (const f of r.findings) {
        const kind = MARKER_CODES[f.code];
        if (!kind || !f.lang || !f.element) continue;
        const key = keyOf.get(f.element);
        if (!key) continue;
        const k = id(f.lang, key);
        if (!markers.has(k)) markers.set(k, { codes: new Set(), msgs: [] });
        const m = markers.get(k);
        m.codes.add(kind);
        if (kind !== 'missing') m.msgs.push(`${f.target ?? ''}: ${f.message}`);
      }
      state.markers = markers;
      renderRows();
    } catch {
      // Markers are a convenience; keep the old ones.
    }
  }

  // ----- rendering --------------------------------------------------------------------
  function languages() {
    const tb = state.table;
    return [tb.default_lang, ...tb.languages.filter((l) => l !== tb.default_lang)];
  }

  function render() {
    renderHead();
    renderRows();
  }

  function renderHead() {
    const tb = state.table;
    if (!tb) return;
    const current = store.get().lang;
    thead.replaceChildren(
      h(
        'tr',
        {},
        h('th', { class: 'tr-key-col' }, t('translations.key')),
        ...languages().map((lang) => {
          const isDefault = lang === tb.default_lang;
          const missing = [...state.missing.values()].filter((m) => m.lang === lang).length;
          return h(
            'th',
            { class: `tr-lang${lang === current ? ' current' : ''}`, title: isDefault ? t('translations.default_lang') : '' },
            h(
              'button',
              { type: 'button', class: 'tr-lang-btn', title: t('translations.switch_lang'), onclick: () => editor.setContext({ lang }) },
              lang,
              isDefault ? h('span', { class: 'tr-default' }, '★') : null,
              missing ? h('span', { class: 'tr-missing-count', title: t('translations.missing_count') }, String(missing)) : null,
            ),
            isDefault
              ? null
              : h('button', { type: 'button', class: 'tr-icon', title: t('translations.remove_lang'), onclick: () => removeLanguage(lang) }, icon('close')),
          );
        }),
      ),
    );
  }

  function matches(key) {
    const tb = state.table;
    if (state.missingOnly && !languages().some((l) => state.missing.has(id(l, key)) || state.markers.get(id(l, key))?.codes.size)) return false;
    if (!state.search) return true;
    if (key.toLowerCase().includes(state.search)) return true;
    return languages().some((l) => (tb.strings[l]?.[key] ?? '').toLowerCase().includes(state.search));
  }

  function renderRows() {
    const tb = state.table;
    if (!tb) return;
    const langs = languages();
    const keys = tb.keys.filter(matches);
    const nMissing = [...state.missing.keys()].length;
    counts.textContent = `${keys.length}/${tb.keys.length} · ${nMissing} ${t('translations.missing')}`;
    tbody.replaceChildren(
      ...keys.map((key) => {
        const users = tb.used_by[key] ?? [];
        return h(
          'tr',
          { dataset: { key } },
          h(
            'th',
            { class: 'tr-key', scope: 'row' },
            h('span', { class: 'tr-key-name', title: users.length ? `${t('translations.used_by')} ${users.join(', ')}` : t('translations.unused') }, key),
            users.length ? null : h('span', { class: 'tr-unused' }, t('translations.unused')),
            h(
              'span',
              { class: 'tr-row-actions' },
              h('button', { type: 'button', class: 'tr-icon', title: t('translations.rename'), onclick: () => renameKey(key) }, icon('text')),
              h('button', { type: 'button', class: 'tr-icon', title: t('translations.delete'), onclick: () => deleteKey(key) }, icon('close')),
            ),
          ),
          ...langs.map((lang) => cell(key, lang)),
        );
      }),
    );
    if (!keys.length) tbody.append(h('tr', {}, h('td', { class: 'placeholder', colSpan: langs.length + 1 }, t('translations.none'))));
  }

  function cell(key, lang) {
    const tb = state.table;
    const own = tb.strings[lang]?.[key];
    const miss = state.missing.get(id(lang, key));
    const marker = state.markers.get(id(lang, key));
    const cls = ['tr-cell'];
    if (own == null) cls.push('missing');
    if (lang === store.get().lang) cls.push('current');
    for (const c of marker?.codes ?? []) cls.push(`m-${c}`);
    const titles = [];
    if (own == null) titles.push(miss?.fallback != null ? `${t('translations.fallback_from')} ${miss.fallback_lang}` : t('translations.no_text'));
    if (marker?.codes.has('overflow')) titles.push(t('translations.overflow'));
    if (marker?.codes.has('emergency')) titles.push(t('translations.emergency'));
    titles.push(...(marker?.msgs ?? []));
    const text = own ?? miss?.fallback ?? '';
    const td = h(
      'td',
      { class: cls.join(' '), title: titles.join('\n'), tabindex: '0', onclick: () => editCell(td, key, lang), onkeydown: (e) => {
        if (e.key === 'Enter') {
          e.preventDefault();
          editCell(td, key, lang);
        }
      } },
      marker?.codes.has('overflow') || marker?.codes.has('emergency') ? h('span', { class: 'tr-marker' }, icon('warning')) : null,
      h('span', { class: 'tr-text' }, text.replaceAll(SOFT_HYPHEN, '\\-')),
    );
    return td;
  }

  // ----- inline editing ------------------------------------------------------------------
  function editCell(td, key, lang) {
    if (td.querySelector('textarea')) return;
    const tb = state.table;
    const own = tb.strings[lang]?.[key];
    const shown = (own ?? '').replaceAll(SOFT_HYPHEN, '\\-');
    const input = h('textarea', { class: 'tr-input', rows: '2', spellcheck: 'true', placeholder: state.missing.get(id(lang, key))?.fallback ?? '' });
    input.value = shown;
    td.replaceChildren(input);
    td.classList.add('editing');
    input.focus();
    input.setSelectionRange(input.value.length, input.value.length);
    let done = false;
    const finish = async (commit) => {
      if (done) return;
      done = true;
      const value = input.value;
      if (!commit || value === shown) {
        renderRows();
        return;
      }
      const next = value === '' && lang !== tb.default_lang ? null : value;
      if (next === null && own == null) {
        renderRows();
        return;
      }
      await editor.apply(api.setString(lang, key, next, `Edit text ${key} (${lang})`));
    };
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
        e.preventDefault();
        finish(true);
      } else if (e.key === 'Escape') {
        e.preventDefault();
        e.stopPropagation();
        finish(false);
      }
      e.stopPropagation();
    });
    input.addEventListener('click', (e) => e.stopPropagation());
    input.addEventListener('blur', () => finish(true));
  }

  // ----- keys + languages ------------------------------------------------------------------
  const keyError = (v, except) => {
    if (!v) return t('translations.key_empty');
    if (/\s/.test(v)) return t('translations.key_space');
    if (v !== except && state.table.keys.includes(v)) return t('translations.key_exists');
    return null;
  };

  async function addKey() {
    const res = await newKeyDialog({ t, keyError, langs: state.table.languages ?? [], defaultLang: state.table.default_lang });
    if (!res) return;
    const { key, text, all, copy } = res;
    const def = state.table.default_lang;
    const ops = [{ op: 'set_string', lang: def, key, value: text }];
    if (all) for (const l of state.table.languages ?? []) if (l !== def) ops.push({ op: 'set_string', lang: l, key, value: copy ? text : '' });
    const r = await editor.apply(api.applyOps(ops, `Add text key ${key}`));
    if (r?.applied) {
      state.search = '';
      search.value = '';
    }
  }

  async function renameKey(key) {
    const to = await promptDialog({ title: t('translations.rename'), label: t('translations.key'), value: key, validate: (v) => keyError(v, key) });
    if (!to || to === key) return;
    await editor.apply(api.applyOps([{ op: 'rename_string_key', from: key, to }], `Rename text key ${key} → ${to}`));
  }

  async function deleteKey(key) {
    const tb = state.table;
    const users = tb.used_by[key] ?? [];
    const ok = await confirmDialog({
      title: t('translations.delete'),
      message: users.length ? `${t('translations.delete_used')} ${key}` : `${t('translations.delete_confirm')} ${key}`,
      details: users,
      ok: t('translations.delete'),
      danger: true,
    });
    if (!ok) return;
    const ops = tb.languages.filter((l) => tb.strings[l]?.[key] != null).map((lang) => ({ op: 'set_string', lang, key, value: null }));
    if (!ops.length) return;
    await editor.apply(api.applyOps(ops, `Delete text key ${key}`));
  }

  async function addLanguage() {
    const tb = state.table;
    const lang = await promptDialog({
      title: t('translations.add_lang'),
      label: t('translations.lang_code'),
      validate: (v) => (!LANG_RE.test(v) ? t('translations.lang_invalid') : tb.languages.includes(v) ? t('translations.lang_exists') : null),
    });
    if (!lang) return;
    await editor.apply(api.applyOps([{ op: 'add_language', lang }], `Add language ${lang}`));
  }

  async function removeLanguage(lang) {
    const tb = state.table;
    const n = Object.keys(tb.strings[lang] ?? {}).length;
    const ok = await confirmDialog({
      title: t('translations.remove_lang'),
      message: `${t('translations.remove_lang_confirm')} ${lang} (${n} ${t('translations.texts')})`,
      ok: t('translations.remove_lang'),
      danger: true,
    });
    if (!ok) return;
    if (store.get().lang === lang) await editor.setContext({ lang: tb.default_lang });
    await editor.apply(api.applyOps([{ op: 'remove_language', lang }], `Remove language ${lang}`));
  }

  // ----- wiring ------------------------------------------------------------------------------
  let reloadTimer = 0;
  const reload = () => {
    clearTimeout(reloadTimer);
    reloadTimer = setTimeout(load, 60);
  };
  const unsubs = [
    editor.on('invalidate', reload),
    editor.on('reset', reload),
    store.select((s) => s.lang, () => render()),
  ];
  load();
  return {
    unmount() {
      unsubs.forEach((u) => u());
      clearTimeout(validateTimer);
      clearTimeout(reloadTimer);
      root.remove();
    },
  };
}

/**
 * New translation key: key, text in the default language and the option
 * to create it in every language (copying the text or empty).
 */
function newKeyDialog({ t, keyError, langs, defaultLang }) {
  const key = h('input', { class: 'field mono', type: 'text', spellcheck: 'false', 'aria-label': t('translations.key') });
  const text = h('input', { class: 'field', type: 'text', 'aria-label': t('inspector.new_key.text') });
  const error = h('div', { class: 'form-error', role: 'alert', hidden: true });
  let all = false;
  let copy = true;
  const copyBox = checkbox({ label: t('inspector.new_key.copy'), checked: copy, disabled: true, onchange: (v) => (copy = v) });
  const others = langs.filter((l) => l !== defaultLang);
  const allBox = checkbox({
    label: `${t('inspector.new_key.all')} (${others.join(', ') || '–'})`,
    checked: all,
    disabled: !others.length,
    onchange: (v) => {
      all = v;
      copyBox.querySelector('input').disabled = !v;
    },
  });
  const check = () => {
    const msg = keyError(key.value.trim());
    error.hidden = !msg;
    error.textContent = msg ?? '';
    return !msg;
  };
  key.addEventListener('input', () => {
    if (!error.hidden) check();
  });
  const body = h(
    'div',
    { class: 'stack' },
    h('label', { class: 'dlg-field' }, h('span', {}, t('translations.key')), key, error),
    h('label', { class: 'dlg-field' }, h('span', {}, `${t('inspector.new_key.text')} · ${defaultLang}`), text),
    allBox,
    copyBox,
  );
  return choiceDialog({
    title: t('translations.add_key'),
    body,
    buttons: [
      { label: t('project.dialog.cancel'), value: null },
      { label: t('project.dialog.ok'), primary: true, check, value: () => ({ key: key.value.trim(), text: text.value, all, copy }) },
    ],
  });
}
