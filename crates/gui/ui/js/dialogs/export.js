// Export dialog (task 2.9): targets × languages matrix, output folder,
// force re-render, gallery previews; progress from `export-progress`
// events, cancel, summary with written/skipped counts and issues, "Show in
// Finder/Explorer". Cells with the same language set are exported in one
// `export` call (targets × langs is a cross product in the backend).

import { api, listen } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { store } from '../store.js';
import { checkbox, errorLine, input, openModal, row } from './modal.js';

const revealLabel = () => (/Mac/.test(navigator.platform) ? t('export.reveal.mac') : /Win/.test(navigator.platform) ? t('export.reveal.win') : t('export.reveal'));

function issueText(i) {
  const src = i.resolve ?? i.render ?? i;
  return src.message ?? JSON.stringify(src);
}

export function openExport() {
  const s = store.get().summary;
  if (!s) return null;
  const langs = s.languages.available;
  /** Selected cells: Set of `target|lang`. */
  const cells = new Set();
  for (const tg of s.targets) for (const l of tg.langs) cells.add(`${tg.id}|${l}`);
  let outDir = null;
  let force = false;
  let gallery = true;
  let running = false;
  let cancelled = false;

  const err = errorLine();
  const matrix = h('div', { class: 'ex-matrix-wrap' });
  const outInput = input({ value: '', placeholder: `${s.dir}/export`, cls: 'grow', onchange: (v) => (outDir = v.trim() || null) });
  const progress = h('div', { class: 'ex-progress', hidden: true });
  const summary = h('div', { class: 'ex-summary', hidden: true });
  const startBtn = h('button', { class: 'btn btn-primary', type: 'button', onclick: () => start() }, t('export.start'));
  const cancelBtn = h('button', { class: 'btn', type: 'button', hidden: true, onclick: () => cancel() }, t('export.cancel'));
  const closeBtn = h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.close'));
  const config = h(
    'div',
    { class: 'ex-config' },
    matrix,
    h(
      'div',
      { class: 'form' },
      row(
        t('export.folder'),
        h(
          'div',
          { class: 'inline' },
          outInput,
          h(
            'button',
            {
              class: 'btn btn-small',
              type: 'button',
              onclick: async () => {
                try {
                  const dir = await api.pickFolder(t('export.folder.pick'));
                  if (dir) {
                    outDir = dir;
                    outInput.value = dir;
                  }
                } catch (e) {
                  err.show(e);
                }
              },
            },
            t('export.folder.choose'),
          ),
        ),
        t('export.folder.hint'),
      ),
      row(
        t('export.options'),
        h(
          'div',
          { class: 'inline wrap' },
          checkbox({ label: t('export.force'), checked: force, onchange: (v) => (force = v) }),
          checkbox({ label: t('export.gallery'), checked: gallery, onchange: (v) => (gallery = v) }),
        ),
      ),
    ),
  );

  const m = openModal({
    title: t('export.title'),
    size: 'large',
    cls: 'dlg-export',
    dismissable: true,
    body: [err.el, config, progress, summary],
    footer: [h('span', { class: 'count muted small' }), h('span', { class: 'spacer' }), closeBtn, cancelBtn, startBtn],
    onClose: () => {
      if (running) api.exportCancel().catch(() => {});
    },
  });
  const countEl = m.el.querySelector('.count');

  function renderMatrix() {
    const all = s.targets.flatMap((tg) => tg.langs.map((l) => `${tg.id}|${l}`));
    const head = h(
      'tr',
      {},
      h('th', { class: 'ex-corner' }, checkbox({ label: t('export.all'), checked: all.every((k) => cells.has(k)), onchange: (v) => toggle(all, v) })),
      langs.map((l) => {
        const col = s.targets.filter((tg) => tg.langs.includes(l)).map((tg) => `${tg.id}|${l}`);
        return h('th', {}, checkbox({ label: l, checked: col.length && col.every((k) => cells.has(k)), onchange: (v) => toggle(col, v) }));
      }),
    );
    const rows = [];
    for (const strip of s.strips) {
      const targets = s.targets.filter((tg) => tg.strip === strip.id);
      if (!targets.length) continue;
      rows.push(h('tr', { class: 'ex-strip' }, h('td', { colspan: String(langs.length + 1) }, `${t('targets.strip')}: ${strip.id}`)));
      for (const tg of targets) {
        const rowKeys = tg.langs.map((l) => `${tg.id}|${l}`);
        rows.push(
          h(
            'tr',
            {},
            h(
              'th',
              { class: 'ex-target' },
              checkbox({ label: tg.id, checked: rowKeys.every((k) => cells.has(k)), onchange: (v) => toggle(rowKeys, v) }),
              h('span', { class: 'muted small' }, tg.kind === 'banner' ? ` ${t('targets.kind.banner')}` : ` ${tg.size[0]}×${tg.size[1]}`),
            ),
            langs.map((l) => {
              const key = `${tg.id}|${l}`;
              if (!tg.langs.includes(l)) return h('td', { class: 'ex-na', title: t('export.na') }, '–');
              return h('td', {}, checkbox({ checked: cells.has(key), onchange: (v) => toggle([key], v) }));
            }),
          ),
        );
      }
    }
    matrix.replaceChildren(h('table', { class: 'ex-matrix' }, h('thead', {}, head), h('tbody', {}, rows)));
    countEl.textContent = `${cells.size} ${t('export.selected')}`;
    startBtn.disabled = running || cells.size === 0;
  }

  function toggle(keys, on) {
    for (const k of keys) {
      if (on) cells.add(k);
      else cells.delete(k);
    }
    renderMatrix();
  }

  /** Groups the selected cells into export calls `{targets, langs}`. */
  function jobs() {
    const byLangs = new Map();
    for (const tg of s.targets) {
      const ls = langs.filter((l) => cells.has(`${tg.id}|${l}`));
      if (!ls.length) continue;
      const key = ls.join(',');
      if (!byLangs.has(key)) byLangs.set(key, { targets: [], langs: ls });
      byLangs.get(key).targets.push(tg.id);
    }
    return [...byLangs.values()];
  }

  async function start() {
    const list = jobs();
    if (!list.length) return;
    running = true;
    cancelled = false;
    err.show(null);
    config.hidden = true;
    summary.hidden = true;
    progress.hidden = false;
    startBtn.hidden = true;
    cancelBtn.hidden = false;
    closeBtn.disabled = true;
    const bar = h('progress', { class: 'ex-bar', max: '1', value: '0' });
    const label = h('div', { class: 'small muted ex-file' });
    const step = h('div', { class: 'small' });
    progress.replaceChildren(step, bar, label);
    const reports = [];
    let unlisten = null;
    let job = 0;
    try {
      unlisten = await listen('export-progress', (p) => {
        bar.max = p.total || 1;
        bar.value = p.done;
        step.textContent = `${t('export.running')} ${p.done} / ${p.total}${list.length > 1 ? ` · ${t('export.batch')} ${job + 1}/${list.length}` : ''}`;
        label.textContent = `${p.file.skipped ? t('export.unchanged') : t('export.written')}: ${p.file.path}`;
      });
      for (job = 0; job < list.length && !cancelled; job++) {
        step.textContent = t('export.preparing');
        reports.push(await api.export({ ...list[job], force, gallery, ...(outDir ? { out_dir: outDir } : {}) }));
        if (reports.at(-1).cancelled) cancelled = true;
      }
    } catch (e) {
      err.show(e);
    } finally {
      unlisten?.();
      running = false;
      cancelBtn.hidden = true;
      closeBtn.disabled = false;
      startBtn.hidden = false;
      startBtn.textContent = t('export.again');
      progress.hidden = true;
      showSummary(reports);
    }
  }

  function showSummary(reports) {
    const files = reports.flatMap((r) => r.files);
    const written = files.filter((f) => !f.skipped).length;
    const skipped = files.length - written;
    const errors = reports.flatMap((r) => r.errors);
    const issues = reports.flatMap((r) => r.issues ?? []);
    const dir = reports[0]?.out_dir;
    summary.hidden = false;
    const parts = [
      h(
        'div',
        { class: `ex-result ${errors.length ? 'bad' : cancelled ? 'warn' : 'good'}` },
        icon(errors.length ? 'error' : cancelled ? 'warning' : 'check'),
        h('strong', {}, cancelled ? t('export.cancelled') : errors.length ? t('export.failed') : t('export.done')),
      ),
      h(
        'dl',
        { class: 'props ex-props' },
        h('dt', {}, t('export.written')),
        h('dd', {}, String(written)),
        h('dt', {}, t('export.unchanged')),
        h('dd', {}, String(skipped)),
        h('dt', {}, t('export.issues')),
        h('dd', {}, String(issues.length)),
        h('dt', {}, t('export.folder')),
        h('dd', { class: 'mono small', title: dir ?? '' }, dir ?? '–'),
      ),
      errors.length ? h('ul', { class: 'ex-errors' }, errors.map((e) => h('li', {}, e))) : null,
      issues.length
        ? h(
            'details',
            { class: 'ex-issues' },
            h('summary', {}, `${issues.length} ${t('export.issues')}`),
            h('ul', {}, issues.slice(0, 200).map((i) => h('li', { class: 'small' }, `${i.target} ${i.lang}: ${issueText(i)}`))),
          )
        : null,
      h(
        'div',
        { class: 'inline' },
        dir
          ? h(
              'button',
              {
                class: 'btn',
                type: 'button',
                onclick: () => api.revealPath(dir).catch((e) => err.show(e)),
              },
              revealLabel(),
            )
          : null,
        h(
          'button',
          {
            class: 'btn',
            type: 'button',
            onclick: () => {
              summary.hidden = true;
              config.hidden = false;
              startBtn.textContent = t('export.start');
              renderMatrix();
            },
          },
          t('export.back'),
        ),
      ),
    ];
    summary.replaceChildren(...parts.filter(Boolean));
  }

  async function cancel() {
    cancelled = true;
    cancelBtn.disabled = true;
    try {
      await api.exportCancel();
    } finally {
      cancelBtn.disabled = false;
    }
  }

  renderMatrix();
  return m;
}
