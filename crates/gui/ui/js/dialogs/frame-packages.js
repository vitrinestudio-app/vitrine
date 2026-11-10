// App settings → "Frame packages" (task 3.5, ADR 0009): import the Apple
// "Product Bezels" the user downloaded (DMG / ZIP / folder; picked or
// dropped), review the plan (matched files, unmatched files with manual
// assignment, sanity warnings), apply, and list/remove installed frames.
// Frames live in the app data folder, never in a project.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { setDropHook } from '../panels/importer.js';
import { choiceDialog, errorLine, openModal, select } from './modal.js';

const THUMB = 72;
const ORIENTATIONS = ['portrait', 'landscape'];

/** Opens the dialog; `path` starts a plan right away (e.g. a dropped DMG). */
export async function openFramePackages({ path } = {}) {
  const err = errorLine();
  let list = await api.framesList();
  let current = null; // { path, plan, assign: Map(name → {model, color, orientation} | null) }
  const thumbs = [];

  const planEl = h('div', { class: 'fp-plan' });
  const installedEl = h('div', { class: 'fp-installed' });
  const status = h('span', { class: 'muted small' });
  const applyBtn = h('button', { class: 'btn btn-primary', type: 'button', disabled: true, onclick: () => apply() }, t('frames.import.apply'));

  const source = () => list.sources[0];

  async function plan(p) {
    current = null;
    applyBtn.disabled = true;
    err.show(null);
    status.textContent = t('frames.import.analysing');
    planEl.replaceChildren(h('div', { class: 'muted' }, `${t('frames.import.analysing')} ${p}`));
    try {
      const result = await api.framesImportPlan(p);
      current = { path: p, plan: result, assign: new Map() };
      status.textContent = '';
      renderPlan();
    } catch (e) {
      status.textContent = '';
      planEl.replaceChildren();
      err.show(e);
    }
  }

  function installCount() {
    if (!current) return 0;
    const items = current.plan.items.filter((i) => i.action === 'install' || i.action === 'replace').length;
    const assigned = [...current.assign.values()].filter((a) => a && a.model && a.color && a.orientation).length;
    return items + assigned;
  }

  function updateApply() {
    const n = installCount();
    applyBtn.disabled = n === 0;
    applyBtn.textContent = n ? t('frames.import.apply_n').replace('{n}', String(n)) : t('frames.import.apply');
  }

  function renderPlan() {
    const p = current.plan;
    const label = (i) => `${i.model} · ${i.color_label} · ${t(`inspector.orientation.${i.orientation}`)}`;
    const actionText = (i) => t(`frames.action.${i.action}`);
    const notes = (i) => (i.action === 'superseded' ? `${t('frames.superseded_by')} ${i.by}` : (i.error ?? i.warnings?.join(' ') ?? ''));
    const rows = p.items.map((i) =>
      h(
        'tr',
        { class: `fp-${i.action}` },
        h('td', {}, h('span', { class: `fp-badge fp-badge-${i.action}` }, actionText(i))),
        h('td', { class: 'fp-file', title: i.file }, i.name),
        h('td', {}, label(i)),
        h('td', { class: 'mono small' }, i.screen ? `${i.screen.w}×${i.screen.h}` : ''),
        h('td', { class: 'small fp-notes' }, notes(i)),
      ),
    );
    const models = source()?.models ?? [];
    const unmatched = p.unmatched.map((u) => {
      const a = current.assign.get(u.name) ?? { model: '', color: '', orientation: 'portrait' };
      const set = (patch) => {
        Object.assign(a, patch);
        current.assign.set(u.name, a);
        updateApply();
      };
      const colorInput = h('input', { class: 'field fp-color', type: 'text', placeholder: t('frames.unmatched.color'), value: a.color, spellcheck: 'false' });
      colorInput.addEventListener('input', () => set({ color: colorInput.value.trim() }));
      return h(
        'tr',
        { class: 'fp-unmatched' },
        h('td', {}, h('span', { class: 'fp-badge fp-badge-unmatched' }, u.ignored ? t('frames.unmatched.ignored') : t('frames.unmatched.title'))),
        h('td', { class: 'fp-file', title: u.file }, u.name),
        h(
          'td',
          { colspan: '3' },
          h(
            'div',
            { class: 'fp-assign' },
            select({
              options: [{ value: '', label: t('frames.unmatched.skip') }, { value: '-', label: t('frames.unmatched.ignore') }, ...models.map((m) => ({ value: m, label: m }))],
              value: a.model,
              onchange: (v) => {
                if (v === '-') current.assign.set(u.name, null);
                else set({ model: v });
                if (v === '') current.assign.delete(u.name);
                updateApply();
              },
            }),
            colorInput,
            select({ options: ORIENTATIONS.map((o) => ({ value: o, label: t(`inspector.orientation.${o}`) })), value: a.orientation, onchange: (v) => set({ orientation: v }) }),
          ),
        ),
      );
    });
    const summary = t('frames.plan.summary')
      .replace('{package}', p.package)
      .replace('{n}', String(p.items.filter((i) => i.action === 'install' || i.action === 'replace').length))
      .replace('{u}', String(p.unmatched.length))
      .replace('{s}', String(p.skipped));
    planEl.replaceChildren(
      h('div', { class: 'fp-summary' }, summary),
      p.unverified_rules ? h('div', { class: 'form-hint' }, t('frames.unverified')) : null,
      h(
        'div',
        { class: 'ex-matrix-wrap fp-table-wrap' },
        h(
          'table',
          { class: 'fp-table' },
          h('thead', {}, h('tr', {}, ...['frames.col.action', 'frames.col.file', 'frames.col.frame', 'frames.col.screen', 'frames.col.notes'].map((k) => h('th', {}, t(k))))),
          h('tbody', {}, ...rows, ...unmatched),
        ),
      ),
    );
    updateApply();
  }

  async function apply() {
    if (!current) return;
    applyBtn.disabled = true;
    status.textContent = t('frames.import.importing');
    const assignments = [...current.assign.entries()]
      .filter(([, a]) => a === null || (a.model && a.color && a.orientation))
      .map(([name, a]) => (a === null ? { name } : { name, ...a }));
    try {
      const result = await api.framesImportApply(current.path, assignments);
      const n = result.items.filter((i) => i.action === 'install' || i.action === 'replace').length;
      status.textContent = t('frames.import.done').replace('{n}', String(n));
      current = null;
      updateApply();
      planEl.replaceChildren();
      list = await api.framesList();
      renderInstalled();
    } catch (e) {
      status.textContent = '';
      err.show(e);
      applyBtn.disabled = false;
    }
  }

  async function remove(args, ask = false) {
    if (ask) {
      const ok = await choiceDialog({
        title: t('frames.remove_all'),
        body: h('p', {}, t('frames.remove_all.confirm')),
        buttons: [
          { label: t('project.dialog.cancel'), value: false },
          { label: t('frames.remove_all'), danger: true, value: true },
        ],
      });
      if (!ok) return;
    }
    try {
      await api.framesRemove(args);
      list = await api.framesList();
      renderInstalled();
    } catch (e) {
      err.show(e);
    }
  }

  function thumb(file) {
    const img = h('img', { class: 'fp-thumb', alt: '', draggable: 'false' });
    api
      .framesThumbnail(file, Math.round(THUMB * (window.devicePixelRatio || 1)))
      .then((p) => {
        const url = URL.createObjectURL(new Blob([p.data], { type: 'image/png' }));
        thumbs.push(url);
        img.src = url;
      })
      .catch(() => img.classList.add('broken'));
    return img;
  }

  function renderInstalled() {
    const src = source();
    const frames = src?.frames ?? [];
    const cards = frames.map((f) =>
      h(
        'div',
        { class: 'fp-card' },
        thumb(f.file),
        h(
          'div',
          { class: 'fp-card-text' },
          h('div', { class: 'fp-card-model' }, f.model),
          h('div', {}, f.color_label),
          h('div', { class: 'muted small' }, `${t(`inspector.orientation.${f.orientation}`)} · ${f.size[0]}×${f.size[1]}`),
          h('div', { class: 'muted small fp-card-src', title: `${f.source_file} (${f.package}, ${f.imported})` }, f.source_file),
        ),
        h(
          'button',
          {
            class: 'icon-btn fp-remove',
            type: 'button',
            title: t('frames.remove'),
            onclick: () => remove({ model: f.model, color: f.color, orientation: f.orientation }),
          },
          icon('trash'),
        ),
      ),
    );
    installedEl.replaceChildren(
      h(
        'div',
        { class: 'fp-installed-head' },
        h('h3', {}, `${t('frames.installed')} (${frames.length})`),
        h('span', { class: 'spacer' }),
        frames.length ? h('button', { class: 'btn btn-danger', type: 'button', onclick: () => remove({}, true) }, t('frames.remove_all')) : null,
      ),
      frames.length ? h('div', { class: 'fp-grid' }, cards) : h('div', { class: 'muted' }, t('frames.none')),
      h('div', { class: 'muted small fp-root' }, `${t('frames.store')}: `, h('span', { class: 'mono selectable' }, list.root)),
    );
  }

  const pickArchive = async () => {
    const files = await api.pickFiles({ title: t('frames.pick.archive'), multiple: false });
    if (files?.length) plan(files[0]);
  };
  const pickFolder = async () => {
    const dir = await api.pickFolder(t('frames.pick.folder'));
    if (dir) plan(dir);
  };

  renderInstalled();
  const m = openModal({
    title: t('frames.title'),
    size: 'large',
    cls: 'fp-dialog',
    body: [
      h('p', { class: 'fp-intro' }, t('frames.intro')),
      h('p', { class: 'muted small' }, t('frames.licence')),
      err.el,
      h(
        'div',
        { class: 'fp-pick' },
        h('button', { class: 'btn', type: 'button', onclick: pickArchive }, t('frames.pick.archive')),
        h('button', { class: 'btn', type: 'button', onclick: pickFolder }, t('frames.pick.folder')),
        h('span', { class: 'muted small' }, t('frames.drop')),
      ),
      planEl,
      installedEl,
    ],
    footer: [status, h('span', { class: 'spacer' }), applyBtn, h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.done'))],
    onClose: () => {
      setDropHook(null);
      thumbs.forEach((u) => URL.revokeObjectURL(u));
    },
  });
  setDropHook((p) => plan(p));
  if (path) plan(path);
  return m;
}
