// Translation exchange dialogs (task 3.4, ADR 0009).
//
// - Export: format (CSV / XLIFF 2.0 / XLIFF 1.2), target languages,
//   missing only, context, CSV delimiter + BOM, XLIFF markup mode, target
//   path (field + native save dialog). The backend writes the files.
// - Import: pick files → `strings_import_plan` (dry run) → review with
//   per-language counts, a diff per key (old → new), markup issues and
//   layout findings after the import (overflow, emergency breaks) →
//   `strings_import_apply` (one undo step). Options (create unknown keys,
//   empty cells) re-plan immediately.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { store } from '../store.js';
import { checkbox, errorLine, input, openModal, row, segmented, select } from './modal.js';

const FORMATS = [
  { value: 'csv', label: 'CSV' },
  { value: 'xliff20', label: 'XLIFF 2.0' },
  { value: 'xliff12', label: 'XLIFF 1.2' },
];

const SOFT_HYPHEN = '­';

const revealLabel = () => (/Mac/.test(navigator.platform) ? t('export.reveal.mac') : /Win/.test(navigator.platform) ? t('export.reveal.win') : t('export.reveal'));

function stem(name) {
  const s = String(name ?? '')
    .replace(/[^\p{L}\p{N}._-]+/gu, '-')
    .replace(/^[-.]+|[-.]+$/g, '');
  return s || 'strings';
}

/** Default file name for a format and language selection. */
function defaultName(project, format, langs) {
  const base = stem(project);
  if (format === 'csv') return `${base}-translations.csv`;
  return langs.length === 1 ? `${base}-${langs[0]}.xlf` : `${base}.xlf`;
}

function joinPath(dir, name) {
  if (!dir) return name;
  const sep = dir.includes('\\') && !dir.includes('/') ? '\\' : '/';
  return dir.endsWith(sep) ? dir + name : `${dir}${sep}${name}`;
}

/** Export dialog. */
export function openExchangeExport() {
  const s = store.get().summary;
  if (!s) return null;
  const targets = s.languages.available.filter((l) => l !== s.languages.default);
  const st = {
    format: 'xliff20',
    langs: new Set(targets),
    missing_only: false,
    context: true,
    delimiter: 'comma',
    bom: true,
    markup: 'inline',
    path: '',
    pathTouched: false,
  };
  const err = errorLine();
  const selected = () => targets.filter((l) => st.langs.has(l));
  const suggest = () => joinPath(s.dir, defaultName(s.name, st.format, selected()));
  const pathInput = input({
    value: suggest(),
    cls: 'grow',
    oninput: (v) => {
      st.path = v.trim();
      st.pathTouched = true;
    },
  });
  st.path = pathInput.value;
  const refreshPath = () => {
    if (st.pathTouched) return;
    st.path = suggest();
    pathInput.value = st.path;
  };

  const langBoxes = h(
    'div',
    { class: 'inline wrap' },
    targets.length
      ? targets.map((l) =>
          checkbox({
            label: l,
            checked: true,
            onchange: (on) => {
              if (on) st.langs.add(l);
              else st.langs.delete(l);
              refreshPath();
            },
          }),
        )
      : h('span', { class: 'muted' }, t('exchange.no_targets')),
  );
  const csvRow = row(
    t('exchange.csv'),
    h(
      'div',
      { class: 'inline wrap' },
      select({
        options: [
          { value: 'comma', label: t('exchange.delim.comma') },
          { value: 'semicolon', label: t('exchange.delim.semicolon') },
          { value: 'tab', label: t('exchange.delim.tab') },
        ],
        value: st.delimiter,
        onchange: (v) => (st.delimiter = v),
      }),
      checkbox({ label: t('exchange.bom'), checked: st.bom, onchange: (v) => (st.bom = v), title: t('exchange.bom.hint') }),
    ),
  );
  const xliffRow = row(
    t('exchange.markup'),
    select({
      options: [
        { value: 'inline', label: t('exchange.markup.inline') },
        { value: 'plain', label: t('exchange.markup.plain') },
      ],
      value: st.markup,
      onchange: (v) => (st.markup = v),
    }),
    t('exchange.markup.hint'),
  );
  const showFormatRows = () => {
    csvRow.hidden = st.format !== 'csv';
    xliffRow.hidden = st.format === 'csv';
  };
  showFormatRows();

  const result = h('div', { class: 'xc-result', hidden: true });
  const exportBtn = h('button', { class: 'btn btn-primary', type: 'button', onclick: () => run() }, t('exchange.export.start'));
  const closeBtn = h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.cancel'));
  const form = h(
    'div',
    { class: 'form' },
    row(
      t('exchange.format'),
      segmented({
        options: FORMATS,
        value: st.format,
        onchange: (v) => {
          st.format = v;
          showFormatRows();
          refreshPath();
        },
      }),
    ),
    row(t('exchange.langs'), langBoxes, t('exchange.langs.hint')),
    row(
      t('exchange.options'),
      h(
        'div',
        { class: 'stack' },
        checkbox({ label: t('exchange.missing_only'), checked: st.missing_only, onchange: (v) => (st.missing_only = v) }),
        checkbox({ label: t('exchange.context'), checked: st.context, onchange: (v) => (st.context = v), title: t('exchange.context.hint') }),
      ),
    ),
    csvRow,
    xliffRow,
    row(
      t('exchange.file'),
      h(
        'div',
        { class: 'inline' },
        pathInput,
        h(
          'button',
          {
            class: 'btn btn-small',
            type: 'button',
            onclick: async () => {
              const p = await chooseSavePath();
              if (p) {
                st.path = p;
                st.pathTouched = true;
                pathInput.value = p;
              }
            },
          },
          t('exchange.choose'),
        ),
      ),
      t('exchange.file.hint'),
    ),
  );

  async function chooseSavePath() {
    try {
      const xlf = st.format !== 'csv';
      return await api.saveFile(t('exchange.export.title'), defaultName(s.name, st.format, selected()), [
        xlf ? { name: 'XLIFF', extensions: ['xlf', 'xliff'] } : { name: 'CSV', extensions: ['csv'] },
      ]);
    } catch (e) {
      err.show(e);
      return null;
    }
  }

  async function run() {
    err.show(null);
    if (st.format !== 'csv' && !selected().length) {
      err.show(new Error(t('exchange.no_langs')));
      return;
    }
    let path = st.path;
    if (!path) path = await chooseSavePath();
    if (!path) return;
    exportBtn.disabled = true;
    try {
      const options = {
        missing_only: st.missing_only,
        context: st.context,
        delimiter: st.delimiter,
        bom: st.bom,
        markup: st.markup,
      };
      const langs = selected();
      // No language ticked (CSV): only the default language column.
      const r = await api.stringsExport(st.format, langs.length ? langs : [s.languages.default], path, options);
      showResult(r.files);
    } catch (e) {
      err.show(e);
    } finally {
      exportBtn.disabled = false;
    }
  }

  function showResult(files) {
    form.hidden = true;
    exportBtn.hidden = true;
    closeBtn.textContent = t('project.dialog.close');
    result.hidden = false;
    result.replaceChildren(
      h('p', { class: 'dlg-text' }, t('exchange.export.done')),
      h(
        'ul',
        { class: 'xc-files' },
        ...files.map((f) =>
          h(
            'li',
            {},
            h('code', { class: 'selectable' }, f.path),
            h('span', { class: 'muted small' }, ` · ${f.keys} ${t('exchange.keys')}${f.lang ? ` · ${f.lang}` : ''}`),
            f.skipped?.length ? h('div', { class: 'muted small' }, `${t('exchange.skipped_no_source')}: ${f.skipped.join(', ')}`) : null,
          ),
        ),
      ),
      files.length
        ? h('button', { class: 'btn btn-small', type: 'button', onclick: () => api.revealPath(files[0].path).catch((e) => err.show(e)) }, revealLabel())
        : null,
    );
  }

  const m = openModal({
    title: t('exchange.export.title'),
    size: 'medium',
    cls: 'xc-dialog',
    body: [err.el, form, result],
    footer: [h('span', { class: 'spacer' }), closeBtn, exportBtn],
  });
  return m;
}

// ----- import -------------------------------------------------------------------------------

/** Text with visible line breaks and soft hyphens for the diff. */
function shown(text) {
  if (text == null) return h('span', { class: 'xc-none' }, t('exchange.none'));
  const parts = [];
  for (const piece of String(text).replaceAll('\\-', SOFT_HYPHEN).split(/(\n|\\n|­)/)) {
    if (piece === '\n' || piece === '\\n') parts.push(h('span', { class: 'xc-mark', title: t('exchange.line_break') }, '⏎'));
    else if (piece === SOFT_HYPHEN) parts.push(h('span', { class: 'xc-mark', title: t('exchange.soft_hyphen') }, '·'));
    else if (piece) parts.push(piece);
  }
  return h('span', {}, ...parts);
}

function kindBadge(kind) {
  return h('span', { class: `xc-kind xc-kind-${kind}` }, t(`exchange.kind.${kind}`));
}

/**
 * Import: pick files (or `paths`), plan, review, apply. `editor` applies
 * the result as one undo step.
 */
export async function openExchangeImport(editor, paths = null) {
  let files = paths;
  if (!files) {
    try {
      files = await api.pickFiles({ title: t('exchange.import.pick'), multiple: true });
    } catch (e) {
      editor.toast(e.message, true);
      return null;
    }
  }
  if (!files?.length) return null;

  const options = { create_keys: false, empty: 'keep' };
  let plan = null;
  const err = errorLine();
  const content = h('div', { class: 'xc-review' }, h('p', { class: 'muted' }, t('exchange.import.checking')));
  const applyBtn = h('button', { class: 'btn btn-primary', type: 'button', disabled: true, onclick: () => apply() }, t('exchange.import.apply'));
  const cancelBtn = h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.cancel'));
  const opts = h(
    'div',
    { class: 'inline wrap xc-opts' },
    checkbox({
      label: t('exchange.create_keys'),
      checked: false,
      onchange: (v) => {
        options.create_keys = v;
        replan();
      },
    }),
    h('span', { class: 'muted small' }, t('exchange.empty')),
    select({
      options: [
        { value: 'keep', label: t('exchange.empty.keep') },
        { value: 'remove', label: t('exchange.empty.remove') },
      ],
      value: 'keep',
      onchange: (v) => {
        options.empty = v;
        replan();
      },
    }),
  );
  const m = openModal({
    title: t('exchange.import.title'),
    size: 'large',
    cls: 'xc-dialog',
    body: [err.el, opts, content],
    footer: [h('span', { class: 'spacer' }), cancelBtn, applyBtn],
  });

  let seq = 0;
  async function replan() {
    const my = ++seq;
    applyBtn.disabled = true;
    err.show(null);
    try {
      const p = await api.stringsImportPlan(files, options);
      if (my !== seq) return;
      plan = p;
      render();
    } catch (e) {
      if (my === seq) err.show(e);
    }
  }

  async function apply() {
    if (!plan?.changes.length) return;
    applyBtn.disabled = true;
    const r = await editor.apply(api.stringsImportApply(plan));
    if (r?.applied) {
      editor.toast(`${t('exchange.import.done')} (${plan.changes.length})`);
      m.close(r);
    } else {
      applyBtn.disabled = false;
    }
  }

  function render() {
    const n = plan.changes.length;
    applyBtn.disabled = n === 0;
    applyBtn.textContent = n ? `${t('exchange.import.apply')} (${n})` : t('exchange.import.apply');
    content.replaceChildren(
      section(t('exchange.files'), filesList()),
      section(t('exchange.summary'), summary()),
      n ? section(`${t('exchange.changes')} (${n})`, changesTable()) : h('p', { class: 'xc-ok' }, icon('check'), t('exchange.no_changes')),
      plan.layout.length ? section(t('exchange.layout'), layoutList()) : null,
      plan.issues.length ? section(t('exchange.issues'), issuesList()) : null,
      plan.unknown_keys.length ? section(t('exchange.unknown_keys'), unknownList()) : null,
    );
  }

  function section(title, body) {
    return h('section', { class: 'xc-section' }, h('h3', {}, title), body);
  }

  function filesList() {
    return h(
      'ul',
      { class: 'xc-files' },
      ...plan.files.map((f) =>
        h(
          'li',
          { class: f.error ? 'xc-bad' : '' },
          h('strong', {}, f.name),
          f.error
            ? h('div', { class: 'xc-error' }, icon('error'), f.error)
            : h('span', { class: 'muted small' }, ` · ${(f.format ?? '').toUpperCase()} · ${f.langs.join(', ')} · ${f.entries} ${t('exchange.values')}`),
          ...(f.unknown_columns ?? []).map((c) => h('div', { class: 'xc-warn' }, icon('warning'), `${t('exchange.unknown_column')} ${c}`)),
          ...(f.warnings ?? []).map((w) => h('div', { class: 'xc-warn' }, icon('warning'), w)),
        ),
      ),
    );
  }

  function summary() {
    if (!plan.langs.length) return h('p', { class: 'muted' }, t('exchange.nothing_read'));
    const cell = (v, cls = '') => h('td', { class: cls }, String(v));
    return h(
      'table',
      { class: 'xc-table xc-sum' },
      h('thead', {}, h('tr', {}, ...['lang', 'kind.new', 'kind.changed', 'kind.removed', 'unchanged', 'skipped'].map((k) => h('th', {}, t(`exchange.${k}`))))),
      h(
        'tbody',
        {},
        ...plan.langs.map((l) =>
          h(
            'tr',
            {},
            h('td', {}, h('strong', {}, l.lang)),
            cell(l.new.length, l.new.length ? 'xc-n-new' : 'muted'),
            cell(l.changed.length, l.changed.length ? 'xc-n-changed' : 'muted'),
            cell(l.removed.length, l.removed.length ? 'xc-n-removed' : 'muted'),
            cell(l.unchanged, 'muted'),
            h('td', { class: 'muted', title: l.skipped.join(', ') }, String(l.skipped.length)),
          ),
        ),
      ),
    );
  }

  function changesTable() {
    const issuesBy = new Map();
    for (const i of plan.issues) {
      const k = `${i.lang}\u0000${i.key}`;
      if (!issuesBy.has(k)) issuesBy.set(k, []);
      issuesBy.get(k).push(i);
    }
    const layoutBy = new Map();
    for (const f of plan.layout) {
      if (!f.key || !f.new) continue;
      const k = `${f.lang}\u0000${f.key}`;
      if (!layoutBy.has(k)) layoutBy.set(k, new Set());
      layoutBy.get(k).add(f.code);
    }
    return h(
      'div',
      { class: 'xc-scroll' },
      h(
        'table',
        { class: 'xc-table xc-diff' },
        h('thead', {}, h('tr', {}, h('th', {}, t('translations.key')), h('th', {}, t('exchange.lang')), h('th', {}), h('th', {}, t('exchange.old')), h('th', {}, t('exchange.new')))),
        h(
          'tbody',
          {},
          ...plan.changes.map((c) => {
            const k = `${c.lang}\u0000${c.key}`;
            const marks = [];
            for (const code of layoutBy.get(k) ?? []) marks.push(h('span', { class: 'xc-flag xc-flag-error', title: t(`exchange.code.${code}`) }, icon('warning'), t(`exchange.code.${code}`)));
            for (const i of issuesBy.get(k) ?? []) marks.push(h('span', { class: `xc-flag ${i.kind === 'differs_from_source' ? 'xc-flag-info' : 'xc-flag-warn'}`, title: i.message }, t(`exchange.issue.${i.kind}`)));
            return h(
              'tr',
              {},
              h('td', { class: 'xc-key' }, c.key, c.created_key ? h('span', { class: 'xc-flag xc-flag-info' }, t('exchange.created_key')) : null),
              h('td', {}, c.lang),
              h('td', {}, kindBadge(c.kind)),
              h('td', { class: 'xc-old' }, shown(c.old)),
              h('td', { class: 'xc-new' }, shown(c.new), marks.length ? h('div', { class: 'xc-marks' }, ...marks) : null),
            );
          }),
        ),
      ),
    );
  }

  function layoutList() {
    const items = [...plan.layout].sort((a, b) => Number(b.new) - Number(a.new));
    return h(
      'ul',
      { class: 'xc-list' },
      ...items.map((f) =>
        h(
          'li',
          { class: f.level === 'error' ? 'xc-error' : 'xc-warn' },
          icon(f.level === 'error' ? 'error' : 'warning'),
          h('span', {}, `${f.target ?? ''} (${f.lang}): ${f.message}`),
          f.new ? null : h('span', { class: 'muted small' }, ` · ${t('exchange.layout.before')}`),
        ),
      ),
    );
  }

  function issuesList() {
    return h(
      'ul',
      { class: 'xc-list' },
      ...plan.issues.map((i) =>
        h('li', { class: i.kind === 'differs_from_source' ? 'muted' : 'xc-warn' }, icon(i.kind === 'differs_from_source' ? 'check' : 'warning'), h('span', {}, `${i.key} (${i.lang}): ${i.message}`)),
      ),
    );
  }

  function unknownList() {
    return h(
      'div',
      {},
      h('p', { class: 'muted small' }, t('exchange.unknown_keys.hint')),
      h('div', { class: 'inline wrap' }, ...plan.unknown_keys.map((k) => h('code', { class: 'xc-code', title: k.file }, k.key))),
    );
  }

  replan();
  return m.closed;
}
