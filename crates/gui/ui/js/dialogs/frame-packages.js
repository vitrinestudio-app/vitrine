// App settings → "Frame packages" (task 3.5, ADR 0012): import the Apple
// "Product Bezels" the user downloaded (DMG / ZIP / folder; picked or
// dropped), review the plan (matched files, unmatched files with manual
// assignment, sanity warnings), apply, and list/remove installed frames.
// Frames live in the app data folder, never in a project.
// Own frame packs (6.4, ADR 0023): a folder or .zip with `frame.yaml` is
// recognised on pick/drop, checked (compiled), and imported for this
// computer or into the open project (`frames/<id>/`, travels with it).

import { api } from '../api.js';
import { store } from '../store.js';
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

  const hasProject = () => Boolean(store.get().summary);
  let packTarget = 'app';

  async function plan(p) {
    current = null;
    applyBtn.disabled = true;
    err.show(null);
    status.textContent = t('frames.import.analysing');
    planEl.replaceChildren(h('div', { class: 'muted' }, `${t('frames.import.analysing')} ${p}`));
    // An own frame pack (frame.yaml)? Otherwise a vendor package.
    try {
      const result = await api.framesPackImport(p, { dry_run: true, target: packTarget === 'project' && hasProject() ? 'project' : 'app' });
      current = { kind: 'pack', path: p, result };
      status.textContent = '';
      renderPackPlan();
      return;
    } catch (e) {
      if (e?.details?.kind !== 'not_a_pack') {
        status.textContent = '';
        planEl.replaceChildren();
        err.show(e);
        return;
      }
    }
    try {
      const result = await api.framesImportPlan(p);
      current = { path: p, plan: result, assign: new Map() };
      status.textContent = '';
      renderPlan();
    } catch (e) {
      status.textContent = '';
      planEl.replaceChildren();
      if (e?.details?.kind === 'dmg_license') showLicenseGuidance(e.details);
      else err.show(e);
    }
  }

  // A DMG with a licence agreement (SLA): Vitrine Studio never accepts it for the
  // user. Explain how to mount it in Finder and offer the folder picker.
  function showLicenseGuidance(details) {
    planEl.replaceChildren(
      h(
        'div',
        { class: 'fp-guidance', role: 'status' },
        h('p', {}, t('frames.dmg_license').replace('{volume}', details.volume ?? '/Volumes/…')),
        h('button', { class: 'btn btn-primary', type: 'button', onclick: pickFolder }, t('frames.dmg_license.pick')),
      ),
    );
  }

  function renderPackPlan() {
    const r = current.result;
    const pk = r.pack;
    const files = [];
    for (const f of pk.frames) if (!files.some((x) => x.file === f.file)) files.push(f);
    const rows = files.map((f) =>
      h(
        'tr',
        {},
        h('td', { class: 'fp-file', title: f.source_file }, f.source_file),
        h('td', {}, `${f.apple_model} · ${f.color_label} · ${t(`inspector.orientation.${f.orientation}`)}`),
        h('td', { class: 'mono small' }, `${f.screen.w}×${f.screen.h}`),
      ),
    );
    const models = pk.models.length ? pk.models.join(', ') : t('frames.pack.any_model');
    const targetSel = select({
      options: [
        { value: 'app', label: t('frames.pack.target.app') },
        ...(hasProject() ? [{ value: 'project', label: t('frames.pack.target.project') }] : []),
      ],
      value: packTarget === 'project' && hasProject() ? 'project' : 'app',
      onchange: (v) => {
        packTarget = v;
      },
    });
    planEl.replaceChildren(
      h('div', { class: 'fp-summary' }, t('frames.pack.summary').replace('{name}', pk.name).replace('{id}', r.id).replace('{n}', String(files.length)).replace('{models}', models)),
      pk.license || pk.author || pk.attribution
        ? h('div', { class: 'fp-license' }, [pk.license && `${t('frames.pack.license')}: ${pk.license}`, pk.author && `${t('frames.pack.author')}: ${pk.author}`, pk.attribution].filter(Boolean).join(' · '))
        : null,
      r.replaces ? h('div', { class: 'form-hint' }, t('frames.pack.replaces').replace('{id}', r.id)) : null,
      ...pk.errors.map((e) => h('div', { class: 'hint-warn small' }, e)),
      ...pk.warnings.map((w) => h('div', { class: 'muted small' }, w)),
      h('div', { class: 'fp-assign' }, h('span', {}, t('frames.pack.target')), targetSel),
      h(
        'div',
        { class: 'ex-matrix-wrap fp-table-wrap' },
        h(
          'table',
          { class: 'fp-table' },
          h('thead', {}, h('tr', {}, ...['frames.col.file', 'frames.col.frame', 'frames.col.screen'].map((k) => h('th', {}, t(k))))),
          h('tbody', {}, ...rows),
        ),
      ),
    );
    applyBtn.disabled = files.length === 0;
    applyBtn.textContent = t('frames.pack.import');
  }

  function installCount() {
    if (!current) return 0;
    if (current.kind === 'pack') return current.result.pack.frames.length;
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
    const label = (i) => [h('span', {}, `${i.apple_model} · ${i.color_label} · ${t(`inspector.orientation.${i.orientation}`)}`), h('span', { class: 'muted small' }, ` (${i.model})`)];
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
    // Files a `skip` rule of the source excludes on purpose (e.g. marketing
    // composites): not a failure, shown with the reason and no assignment.
    const onPurpose = p.unmatched.filter((u) => u.reason);
    const skippedRows = onPurpose.map((u) =>
      h(
        'tr',
        { class: 'fp-skipped' },
        h('td', {}, h('span', { class: 'fp-badge' }, t('frames.unmatched.on_purpose'))),
        h('td', { class: 'fp-file', title: u.file }, u.name),
        h('td', { colspan: '3', class: 'small fp-notes' }, `${t('frames.unmatched.on_purpose_note')}: ${u.reason}`),
      ),
    );
    const unmatched = p.unmatched.filter((u) => !u.reason).map((u) => {
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
      .replace('{u}', String(p.unmatched.length - onPurpose.length))
      .replace('{s}', String(p.skipped));
    const psd = p.psd ? ` ${t('frames.plan.psd').replace('{n}', String(p.psd))}` : '';
    const purpose = onPurpose.length ? ` ${t('frames.plan.on_purpose').replace('{n}', String(onPurpose.length))}` : '';
    const licenseFile = p.licenses?.[0]?.split(/[\\/]/).pop();
    planEl.replaceChildren(
      h('div', { class: 'fp-summary' }, summary + psd + purpose),
      licenseFile
        ? h(
            'div',
            { class: 'fp-license' },
            h('div', {}, t('frames.license_notice').replace('{file}', licenseFile)),
            ...p.licenses.map((l) => h('div', { class: 'mono small selectable muted' }, l)),
          )
        : null,
      p.unverified_rules ? h('div', { class: 'form-hint' }, t('frames.unverified')) : null,
      h(
        'div',
        { class: 'ex-matrix-wrap fp-table-wrap' },
        h(
          'table',
          { class: 'fp-table' },
          h('thead', {}, h('tr', {}, ...['frames.col.action', 'frames.col.file', 'frames.col.frame', 'frames.col.screen', 'frames.col.notes'].map((k) => h('th', {}, t(k))))),
          h('tbody', {}, ...rows, ...unmatched, ...skippedRows),
        ),
      ),
    );
    updateApply();
  }

  async function apply() {
    if (!current) return;
    applyBtn.disabled = true;
    status.textContent = t('frames.import.importing');
    if (current.kind === 'pack') {
      try {
        const target = packTarget === 'project' && hasProject() ? 'project' : 'app';
        const r = await api.framesPackImport(current.path, { target });
        status.textContent = t('frames.pack.done').replace('{frame}', r.pack.frame);
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
      return;
    }
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

  function thumb(file, source = 'apple') {
    const img = h('img', { class: 'fp-thumb', alt: '', draggable: 'false' });
    api
      .framesThumbnail(file, Math.round(THUMB * (window.devicePixelRatio || 1)), source)
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
          h('div', { class: 'fp-card-model' }, f.apple_model || f.model),
          h('div', {}, f.color_label),
          h('div', { class: 'muted small' }, `${f.model} · ${f.screen.w}×${f.screen.h}`),
          h('div', { class: 'muted small' }, `${t(`inspector.orientation.${f.orientation}`)} · ${f.size[0]}×${f.size[1]}`),
          h('div', { class: 'muted small fp-card-src', title: `${f.source_file} (${f.package}, ${f.imported})` }, f.source_file),
        ),
        h(
          'button',
          {
            class: 'icon-btn fp-remove',
            type: 'button',
            title: t('frames.remove'),
            onclick: () => remove({ model: f.model, color: f.color, orientation: f.orientation, bezel: f.apple_model || null }),
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
      ...packsSection(),
    );
  }

  async function removePack(p) {
    const ok = await choiceDialog({
      title: t('frames.pack.remove'),
      body: h('p', {}, t(`frames.pack.remove.confirm.${p.origin}`).replace('{name}', p.name)),
      buttons: [
        { label: t('project.dialog.cancel'), value: false },
        { label: t('frames.pack.remove'), danger: true, value: true },
      ],
    });
    if (!ok) return;
    try {
      await api.framesPackRemove(p.id, p.origin);
      list = await api.framesList();
      renderInstalled();
    } catch (e) {
      err.show(e);
    }
  }

  /** Own frame packs: one block per pack with a thumbnail per image. */
  function packsSection() {
    const packs = list.packs ?? [];
    const blocks = packs.map((p) => {
      const files = [];
      for (const f of p.frames) if (!files.some((x) => x.file === f.file)) files.push(f);
      const cards = files.map((f) =>
        h(
          'div',
          { class: 'fp-card' },
          thumb(f.file, p.frame),
          h(
            'div',
            { class: 'fp-card-text' },
            h('div', { class: 'fp-card-model' }, f.apple_model),
            h('div', {}, f.color_label),
            h('div', { class: 'muted small' }, `${t(`inspector.orientation.${f.orientation}`)} · ${f.screen.w}×${f.screen.h}`),
            h('div', { class: 'muted small fp-card-src', title: f.source_file }, f.source_file),
          ),
        ),
      );
      const meta = [t(`frames.pack.origin.${p.origin}`), p.models.length ? p.models.join(', ') : t('frames.pack.any_model'), p.license && `${t('frames.pack.license')}: ${p.license}`].filter(Boolean).join(' · ');
      return h(
        'div',
        { class: 'fp-pack' },
        h(
          'div',
          { class: 'fp-installed-head' },
          h('h4', {}, p.name, ' ', h('span', { class: 'mono small muted selectable' }, p.frame)),
          h('span', { class: 'spacer' }),
          h('button', { class: 'icon-btn', type: 'button', title: t('frames.pack.remove'), onclick: () => removePack(p) }, icon('trash')),
        ),
        h('div', { class: 'muted small' }, meta),
        p.shadows_app_pack ? h('div', { class: 'form-hint' }, t('frames.pack.shadows')) : null,
        ...p.errors.map((e) => h('div', { class: 'hint-warn small' }, e)),
        cards.length ? h('div', { class: 'fp-grid' }, cards) : null,
      );
    });
    return [
      h('div', { class: 'fp-installed-head' }, h('h3', {}, `${t('frames.pack.title')} (${packs.length})`)),
      h('p', { class: 'muted small' }, t('frames.pack.intro')),
      ...(blocks.length ? blocks : [h('div', { class: 'muted' }, t('frames.pack.none'))]),
    ];
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
