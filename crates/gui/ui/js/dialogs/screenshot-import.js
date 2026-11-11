// Screenshot import dialog (task 3.3): pick a folder (fastlane snapshot
// output, a Vitrine Studio `screenshots/` tree or a flat folder), preview the
// plan from `import_plan` grouped by language · variant, correct rows
// (language, variant, key, skip) or the device/locale mapping, then
// `import_apply` (re-plans with the same arguments, copies, optionally
// saves the mapping to project.yaml as one undo step). The folder watcher
// refreshes assets and previews; listeners of `onImported` are notified.
// All detection happens in Rust (`core::import`); this module only edits
// `{format, mapping, overrides}` and re-plans.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { notifyImported } from '../panels/importer.js';
import { store } from '../store.js';
import { checkbox, errorLine, openModal, select } from './modal.js';

const FORMATS = ['auto', 'fastlane', 'vitrine', 'flat'];
const ACTIONS = ['new', 'overwrite', 'unchanged', 'duplicate', 'unresolved'];
const ACTION_CLASS = { new: 'ok', overwrite: 'warn', duplicate: 'warn', unresolved: 'error', unchanged: '' };
const KEY_RE = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

const baseName = (p) => p.split('/').pop();

/** `el.replaceChildren` without the `null`s of conditional children. */
const fill = (el, ...children) => el.replaceChildren(...children.filter((c) => c != null && c !== false));

/** Variants named by the project's targets (unique, in order). */
function targetVariants() {
  const out = [];
  for (const tg of store.get().summary?.targets ?? []) if (tg.screenshots && !out.includes(tg.screenshots)) out.push(tg.screenshots);
  return out;
}

/**
 * Opens the import dialog. Without `dir` the folder picker is shown first
 * (the dev bridge answers it with `--pick-folder`).
 */
export async function openScreenshotImport({ dir } = {}) {
  const s = store.get().summary;
  if (!s) return;
  if (!dir) {
    try {
      dir = await api.pickFolder(t('import.pick_title'));
    } catch (e) {
      editor.toast(e.message, true);
      return;
    }
    if (!dir) return;
  }

  const state = {
    dir,
    format: 'auto',
    mapping: { devices: {}, locales: {}, keys: {} },
    overrides: {},
    saveMapping: true,
  };
  let plan = null;
  let seq = 0;
  let busy = false;

  const err = errorLine();
  const sourceLine = h('div', { class: 'imp-source' });
  const formatSel = select({
    options: FORMATS.map((f) => ({ value: f, label: t(`import.format.${f}`) })),
    value: state.format,
    title: t('import.format'),
    onchange: (v) => {
      state.format = v;
      replan();
    },
  });
  const detected = h('span', { class: 'muted small' });
  const counts = h('div', { class: 'imp-counts' });
  const mappingBox = h('details', { class: 'imp-mapping' });
  const table = h('div', { class: 'imp-table' });
  const skippedBox = h('details', { class: 'imp-skipped' });
  const missingBox = h('div', { class: 'imp-missing' });
  const saveBox = checkbox({ label: t('import.save_mapping'), checked: state.saveMapping, onchange: (v) => (state.saveMapping = v), title: t('import.save_mapping_hint') });

  const body = h(
    'div',
    { class: 'imp' },
    h(
      'div',
      { class: 'imp-head' },
      sourceLine,
      h('button', { type: 'button', class: 'btn btn-small', onclick: () => changeFolder() }, t('import.change_folder')),
      h('label', { class: 'imp-format' }, h('span', { class: 'muted small' }, t('import.format')), formatSel),
      detected,
    ),
    counts,
    mappingBox,
    table,
    skippedBox,
    missingBox,
    err.el,
  );
  const applyBtn = h('button', { class: 'btn btn-primary', type: 'button', onclick: () => run() }, t('import.apply'));
  const cancelBtn = h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.cancel'));
  const m = openModal({
    title: t('import.title'),
    size: 'large',
    cls: 'imp-dialog',
    body,
    footer: [saveBox, h('span', { class: 'spacer' }), cancelBtn, applyBtn],
  });

  function args() {
    return { dir: state.dir, format: state.format, mapping: state.mapping, overrides: state.overrides };
  }

  let timer = 0;
  function replan(delay = 0) {
    clearTimeout(timer);
    timer = setTimeout(load, delay);
  }

  async function load() {
    const my = ++seq;
    body.classList.add('loading');
    try {
      const p = await api.importPlan(args());
      if (my !== seq) return;
      plan = p;
      err.show(null);
      render();
    } catch (e) {
      if (my !== seq) return;
      plan = null;
      err.show(e);
      render();
    } finally {
      if (my === seq) body.classList.remove('loading');
    }
  }

  async function changeFolder() {
    try {
      const d = await api.pickFolder(t('import.pick_title'));
      if (!d) return;
      state.dir = d;
      state.overrides = {};
      replan();
    } catch (e) {
      err.show(e);
    }
  }

  // ----- row edits -------------------------------------------------------------------
  function override(source, patch) {
    const o = { ...(state.overrides[source] ?? {}), ...patch };
    for (const k of Object.keys(o)) if (o[k] == null || o[k] === false) delete o[k];
    if (Object.keys(o).length) state.overrides[source] = o;
    else delete state.overrides[source];
    replan(150);
  }

  function rowFor(it) {
    const langs = s.languages.available;
    const langSel = select({
      options: [...(it.lang && !langs.includes(it.lang) ? [it.lang] : []), ...langs],
      value: it.lang ?? '',
      title: t('import.col.lang'),
      onchange: (v) => override(it.source, { lang: v }),
    });
    if (!it.lang) langSel.prepend(h('option', { value: '' }, '–'));
    langSel.value = it.lang ?? '';
    const variants = [...new Set([...(it.variant ? [it.variant] : []), ...(it.candidates ?? []), ...targetVariants()])];
    const varSel = select({
      options: [
        ...variants.map((v) => ({ value: v, label: `${v}${(it.candidates ?? []).includes(v) ? ' ✓' : ''}` })),
        { value: '', label: t('import.variant_all') },
      ],
      value: it.variant ?? '',
      title: t('import.col.variant'),
      onchange: (v) => v !== '__none' && override(it.source, { variant: v }),
    });
    if (!it.variant && it.action === 'unresolved' && !(it.notes ?? []).some((n) => n.includes('all variants'))) {
      varSel.prepend(h('option', { value: '__none' }, '–'));
      varSel.value = '__none';
    }
    const keyIn = h('input', { class: 'field imp-key', type: 'text', value: it.key ?? '', spellcheck: 'false', 'aria-label': t('import.col.key') });
    keyIn.addEventListener('change', () => {
      const v = keyIn.value.trim();
      if (!KEY_RE.test(v)) {
        keyIn.classList.add('invalid');
        editor.toast(t('assets.key_invalid'), true);
        return;
      }
      keyIn.classList.remove('invalid');
      override(it.source, { key: v });
    });
    const notes = (it.notes ?? []).join(' · ');
    const conf = h('span', { class: `imp-conf conf-${it.confidence}`, title: `${t('import.confidence')}: ${t(`import.conf.${it.confidence}`)}` });
    const actions = h(
      'div',
      { class: 'imp-row-actions' },
      it.overridden
        ? h('button', { type: 'button', class: 'icon-btn', title: t('import.reset_row'), onclick: () => override(it.source, { lang: null, variant: null, key: null }) }, icon('reset'))
        : null,
      h('button', { type: 'button', class: 'icon-btn', title: t('import.skip_row'), onclick: () => override(it.source, { skip: true }) }, icon('close')),
    );
    return h(
      'div',
      { class: `imp-row act-${it.action}${it.overridden ? ' overridden' : ''}` },
      h('span', { class: `badge ${ACTION_CLASS[it.action] ?? ''}` }, t(`import.action.${it.action}`)),
      h(
        'div',
        { class: 'imp-src', title: it.source },
        h('div', { class: 'imp-name' }, conf, baseName(it.source), it.width ? h('span', { class: 'muted small' }, ` · ${it.width}×${it.height}`) : null),
        notes ? h('div', { class: 'imp-notes muted small', title: notes }, notes) : null,
      ),
      langSel,
      varSel,
      keyIn,
      actions,
    );
  }

  // ----- rendering -----------------------------------------------------------------
  function render() {
    sourceLine.replaceChildren(icon('assets'), h('span', { class: 'imp-dir', title: state.dir }, state.dir));
    if (!plan) {
      counts.replaceChildren();
      table.replaceChildren();
      mappingBox.hidden = skippedBox.hidden = true;
      missingBox.replaceChildren();
      applyBtn.disabled = true;
      return;
    }
    detected.textContent = state.format === 'auto' ? `${t('import.detected')}: ${t(`import.format.${plan.format}`)}` : '';

    const n = Object.fromEntries(ACTIONS.map((a) => [a, plan.items.filter((i) => i.action === a).length]));
    const toCopy = n.new + n.overwrite;
    fill(counts, 
      ...ACTIONS.filter((a) => n[a]).map((a) => h('span', { class: `badge ${ACTION_CLASS[a]}` }, `${n[a]} ${t(`import.action.${a}`)}`)),
      plan.skipped.length ? h('span', { class: 'badge' }, `${plan.skipped.length} ${t('import.skipped')}`) : null,
      plan.items.length || plan.skipped.length ? null : h('span', { class: 'muted' }, t('import.empty')),
    );
    applyBtn.disabled = busy || toCopy === 0;
    applyBtn.textContent = toCopy ? `${t('import.apply')} (${toCopy})` : t('import.apply');

    renderMapping();

    // Rows: unresolved first, then by language · variant.
    const groups = new Map();
    const unresolved = plan.items.filter((i) => !i.dest);
    for (const it of plan.items.filter((i) => i.dest)) {
      const g = `${it.lang} · ${it.variant ?? t('import.variant_all')}`;
      if (!groups.has(g)) groups.set(g, []);
      groups.get(g).push(it);
    }
    const head = h('div', { class: 'imp-row imp-cols muted small' }, h('span', {}, ''), h('span', {}, t('import.col.source')), h('span', {}, t('import.col.lang')), h('span', {}, t('import.col.variant')), h('span', {}, t('import.col.key')), h('span', {}, ''));
    fill(table, 
      plan.items.length ? head : null,
      unresolved.length ? h('div', { class: 'imp-group error' }, icon('warning'), ` ${t('import.unresolved')}`) : null,
      ...unresolved.map(rowFor),
      ...[...groups.entries()].flatMap(([g, items]) => [h('div', { class: 'imp-group' }, g, h('span', { class: 'as-count' }, String(items.length))), ...items.map(rowFor)]),
    );

    // Skipped files (rows skipped by the user can be brought back).
    skippedBox.hidden = !plan.skipped.length;
    fill(skippedBox, 
      h('summary', { class: 'muted small' }, `${t('import.skipped')} (${plan.skipped.length})`),
      ...plan.skipped.map((f) =>
        h(
          'div',
          { class: 'imp-skip small' },
          h('span', { class: 'grow', title: f.source }, f.source),
          h('span', { class: 'muted' }, f.reason),
          state.overrides[f.source]?.skip ? h('button', { type: 'button', class: 'btn btn-small', onclick: () => override(f.source, { skip: null }) }, t('import.include')) : null,
        ),
      ),
    );

    // Still missing after the import.
    const missing = plan.missing.filter((x) => x.status === 'missing');
    const fallback = plan.missing.filter((x) => x.status === 'fallback');
    const byGroup = new Map();
    for (const x of missing) {
      const g = `${x.lang} · ${x.variant}`;
      if (!byGroup.has(g)) byGroup.set(g, []);
      byGroup.get(g).push(x.key);
    }
    const fbLangs = new Map();
    for (const x of fallback) fbLangs.set(x.lang, (fbLangs.get(x.lang) ?? 0) + 1);
    fill(missingBox, 
      missing.length || fallback.length ? h('div', { class: 'section-title' }, t('import.after')) : null,
      ...[...byGroup.entries()].map(([g, keys]) => h('div', { class: 'imp-miss small' }, h('span', { class: 'badge warn' }, t('import.missing')), ` ${g}: ${keys.join(', ')}`)),
      ...[...fbLangs.entries()].map(([l, c]) => h('div', { class: 'imp-miss small muted' }, h('span', { class: 'badge' }, t('import.fallback')), ` ${l}: ${c} ${t('import.fallback_count')}`)),
    );
  }

  /** Device → variant and locale → language of the plan, editable. */
  function renderMapping() {
    const devices = [...new Set(plan.items.map((i) => i.device).filter(Boolean))];
    const locales = [...new Set(plan.items.map((i) => i.locale).filter((l) => l && !s.languages.available.includes(l)))];
    mappingBox.hidden = !devices.length && !locales.length;
    const open = mappingBox.open;
    const variants = targetVariants();
    const devRows = devices.map((d) => {
      const rows = plan.items.filter((i) => i.device === d);
      const current = state.mapping.devices[d] ?? plan.mapping.devices[d] ?? '';
      const cands = [...new Set(rows.flatMap((r) => r.candidates ?? []))];
      const opts = [...new Set([...(current ? [current] : []), ...cands, ...variants])];
      const sel = select({
        options: [{ value: '', label: '–' }, ...opts.map((v) => ({ value: v, label: `${v}${cands.includes(v) ? ' ✓' : ''}` }))],
        value: current,
        onchange: (v) => {
          if (v) state.mapping.devices[d] = v;
          else delete state.mapping.devices[d];
          replan();
        },
      });
      return h('div', { class: 'imp-map-row' }, h('span', { class: 'imp-map-name', title: d }, d), icon('chevron', 'imp-arrow'), sel, h('span', { class: 'muted small' }, `${rows.length}×`));
    });
    const locRows = locales.map((l) => {
      const current = state.mapping.locales[l] ?? plan.mapping.locales[l] ?? '';
      const sel = select({
        options: [{ value: '', label: '–' }, ...s.languages.available],
        value: current,
        onchange: (v) => {
          if (v) state.mapping.locales[l] = v;
          else delete state.mapping.locales[l];
          replan();
        },
      });
      return h('div', { class: 'imp-map-row' }, h('span', { class: 'imp-map-name' }, l), icon('chevron', 'imp-arrow'), sel);
    });
    fill(mappingBox, 
      h('summary', { class: 'section-title' }, t('import.mapping'), h('span', { class: 'muted small' }, ` · ${t('import.mapping_hint')}`)),
      devRows.length ? h('div', { class: 'imp-map-title muted small' }, t('import.mapping.devices')) : null,
      ...devRows,
      locRows.length ? h('div', { class: 'imp-map-title muted small' }, t('import.mapping.locales')) : null,
      ...locRows,
    );
    mappingBox.open = open || plan.items.some((i) => i.action === 'unresolved' && i.device);
  }

  // ----- apply ---------------------------------------------------------------------------
  async function run() {
    if (busy) return;
    busy = true;
    applyBtn.disabled = true;
    applyBtn.textContent = t('import.running');
    try {
      const r = await api.importApply({ ...args(), save_mapping: state.saveMapping });
      if (r.mapping) await editor.apply(Promise.resolve(r.mapping));
      const failed = r.report.failed ?? [];
      editor.toast(
        failed.length
          ? `${t('import.failed')}: ${failed.map(([d, e]) => `${d}: ${e}`).join('; ')}`
          : `${t('import.done')}: ${r.report.copied.length} ${t('import.files')}${r.mapping ? ` · ${t('import.mapping_saved')}` : ''}`,
        failed.length > 0,
      );
      notifyImported(r.report.copied.map((path) => ({ path })));
      m.close();
    } catch (e) {
      err.show(e);
      busy = false;
      render();
    }
  }

  render();
  load();
}
