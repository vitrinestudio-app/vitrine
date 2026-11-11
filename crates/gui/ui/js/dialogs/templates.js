// Templates (phase 6.3, ADR 0023): the template picker (cards with
// previews) used by "New project" and "Insert from template", the
// "Save as template" dialog, the insert form of the canvases dialog and
// importing a `.vitrine-template` file. All logic lives in Rust
// (`template_*` commands); this only collects choices.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { t } from '../i18n.js';
import { checkbox, errorLine, input, openModal, row, segmented } from './modal.js';

/** `t` with `{name}` parameters. */
const tt = (key, params = {}) => Object.entries(params).reduce((s, [k, v]) => s.replaceAll(`{${k}}`, String(v)), t(key));

/** Object URLs of preview images by `id|canvas` (kept for the session). */
const previews = new Map();

async function previewUrl(id, canvas) {
  const key = `${id}|${canvas ?? ''}`;
  if (!previews.has(key)) {
    previews.set(
      key,
      api.templatePreview(id, canvas).then((p) => URL.createObjectURL(new Blob([p.data], { type: 'image/png' }))),
    );
  }
  return previews.get(key);
}

/** Forgets cached previews of `id` (after replacing or removing it). */
function dropPreviews(id) {
  for (const k of [...previews.keys()]) if (k.startsWith(`${id}|`)) previews.delete(k);
}

/** A preview `<img>` that loads lazily. */
function previewImg(id, canvas) {
  const img = h('img', { class: 'tpl-preview', alt: '', draggable: 'false' });
  previewUrl(id, canvas)
    .then((url) => (img.src = url))
    .catch(() => img.classList.add('broken'));
  return img;
}

/** Imports a `.vitrine-template` file chosen by the user; resolves to the summary or null. */
export async function importTemplateFile() {
  const paths = await api.pickFiles({ title: tt('templates.import.pick'), multiple: false, extensions: ['vitrine-template'] });
  if (!paths?.length) return null;
  try {
    return await api.templateImport(paths[0]);
  } catch (e) {
    if (e.message?.includes('already installed')) {
      const r = await api.templateImport(paths[0], true);
      dropPreviews(r.id);
      return r;
    }
    throw e;
  }
}

/**
 * Template cards. `value`: selected id; `onchange(summary)`. Returns
 * `{ el, reload() }`. `allowImport` adds an "Import file…" card button.
 */
export function templatePicker({ value, onchange, allowImport = true, onerror }) {
  const el = h('div', { class: 'tpl-picker' });
  let list = [];
  let selected = value;

  function draw() {
    const cards = list.map((tpl) =>
      h(
        'button',
        {
          type: 'button',
          class: 'tpl-card',
          'aria-pressed': String(tpl.id === selected),
          title: tpl.description || tpl.name,
          onclick: () => {
            selected = tpl.id;
            draw();
            onchange?.(tpl);
          },
        },
        previewImg(tpl.id, tpl.canvases[0]?.id),
        h('span', { class: 'tpl-name' }, tpl.name),
        h(
          'span',
          { class: 'tpl-meta muted small' },
          `${tpl.canvases.length > 1 ? tt('templates.canvases', { n: tpl.canvases.length }) : tpl.canvases[0]?.id ?? ''}` +
            ` · ${t(`templates.source.${tpl.source}`)}`,
        ),
      ),
    );
    if (allowImport) {
      cards.push(
        h(
          'button',
          {
            type: 'button',
            class: 'tpl-card tpl-import',
            onclick: async () => {
              try {
                const r = await importTemplateFile();
                if (r) {
                  selected = r.id;
                  await reload();
                  onchange?.(list.find((x) => x.id === r.id));
                }
              } catch (e) {
                onerror?.(e);
              }
            },
          },
          icon('plus'),
          h('span', { class: 'tpl-name' }, tt('templates.import')),
          h('span', { class: 'tpl-meta muted small' }, '.vitrine-template'),
        ),
      );
    }
    el.replaceChildren(...cards);
  }

  async function reload() {
    try {
      list = await api.templateList();
    } catch (e) {
      onerror?.(e);
      list = [];
    }
    if (!list.some((x) => x.id === selected)) selected = list[0]?.id;
    draw();
    const cur = list.find((x) => x.id === selected);
    if (cur) onchange?.(cur);
    return list;
  }

  reload();
  return { el, reload, get selected() { return list.find((x) => x.id === selected); } };
}

/** "Save as template…": selected canvases (default: all) of the open project. */
export function openSaveTemplate({ canvases, current }) {
  const err = errorLine();
  const st = { name: '', id: '', description: '', author: '', tags: '', selected: new Set(canvases.map((c) => c.id)), file: false };
  if (current && canvases.length > 1) st.selected = new Set([current]);
  const saveBtn = h('button', { class: 'btn btn-primary', type: 'button', onclick: () => save() }, tt('templates.save.button'));
  const result = h('div', { class: 'tpl-result small' });

  async function save() {
    if (!st.name.trim()) return err.show(new Error(tt('templates.save.name_missing')));
    if (!st.selected.size) return err.show(new Error(tt('templates.save.none')));
    const args = {
      name: st.name.trim(),
      description: st.description,
      author: st.author,
      tags: st.tags.split(/[\s,]+/).filter(Boolean),
      canvases: canvases.map((c) => c.id).filter((id) => st.selected.has(id)),
    };
    if (st.id.trim()) args.id = st.id.trim();
    if (st.file) {
      const path = await api.saveFile(tt('templates.save.file_pick'), `${st.id || st.name || 'template'}.vitrine-template`, [
        { name: 'Vitrine Studio template', extensions: ['vitrine-template'] },
      ]);
      if (!path) return;
      args.file = path;
    }
    saveBtn.disabled = true;
    try {
      let r;
      try {
        r = await api.templateSave(args);
      } catch (e) {
        if (!e.message?.includes('already installed')) throw e;
        r = await api.templateSave({ ...args, replace: true });
      }
      dropPreviews(r.template.id);
      err.show(null);
      const notes = [tt('templates.save.done', { id: r.template.id })];
      if (r.report.placeholders) notes.push(tt('templates.save.placeholders', { n: r.report.placeholders }));
      for (const f of r.report.licenses_generated ?? []) notes.push(tt('templates.save.licence', { file: f }));
      for (const f of r.report.fonts_missing ?? []) notes.push(tt('templates.save.font_missing', { font: f }));
      if (r.file) notes.push(r.file);
      result.replaceChildren(...notes.map((n) => h('div', {}, n)));
      saveBtn.textContent = tt('templates.save.again');
    } catch (e) {
      err.show(e);
    } finally {
      saveBtn.disabled = false;
    }
  }

  const m = openModal({
    title: tt('templates.save.title'),
    size: 'medium',
    body: [
      err.el,
      h(
        'div',
        { class: 'form' },
        row(tt('templates.name'), input({ placeholder: tt('templates.name.placeholder'), oninput: (v) => (st.name = v) })),
        row(tt('templates.id'), input({ placeholder: tt('templates.id.placeholder'), cls: 'mono', oninput: (v) => (st.id = v) }), tt('templates.id.hint')),
        row(tt('templates.description'), input({ oninput: (v) => (st.description = v) })),
        row(tt('templates.author'), input({ oninput: (v) => (st.author = v) })),
        row(tt('templates.tags'), input({ placeholder: 'gradient, phone', oninput: (v) => (st.tags = v) })),
        row(
          tt('templates.save.canvases'),
          h(
            'div',
            { class: 'stack' },
            canvases.map((c) =>
              checkbox({
                label: c.id,
                checked: st.selected.has(c.id),
                onchange: (on) => (on ? st.selected.add(c.id) : st.selected.delete(c.id)),
              }),
            ),
          ),
        ),
        row('', checkbox({ label: tt('templates.save.file'), checked: false, onchange: (v) => (st.file = v) })),
        h('p', { class: 'form-hint' }, tt('templates.save.hint')),
        result,
      ),
    ],
    footer: [h('span', { class: 'spacer' }), h('button', { class: 'btn', type: 'button', onclick: () => m.close() }, t('project.dialog.close')), saveBtn],
  });
  m.el.querySelector('input')?.focus();
  return m;
}

/** Text lines of an insert report (`template_insert_plan`). */
function reportLines(r) {
  const out = [];
  for (const c of r.canvases) out.push(c.from === c.to ? tt('templates.insert.canvas', { id: c.to }) : tt('templates.insert.canvas_renamed', { from: c.from, to: c.to }));
  for (const x of r.targets.filter((x) => x.from !== x.to)) out.push(tt('templates.insert.target_renamed', { from: x.from, to: x.to }));
  if (r.elements_renamed.length) out.push(tt('templates.insert.elements_renamed', { n: r.elements_renamed.length }));
  for (const s of r.styles_renamed) out.push(tt('templates.insert.style_renamed', { name: s.name, to: s.to }));
  for (const s of r.styles_from_project) out.push(tt('templates.insert.style_project', { name: s.name }));
  if (r.styles_added.length) out.push(tt('templates.insert.styles_added', { n: r.styles_added.length }));
  for (const k of r.keys_renamed) out.push(tt('templates.insert.key_renamed', { from: k.from, to: k.to }));
  for (const [lang, from] of r.copied_texts) out.push(tt('templates.insert.texts_copied', { lang, from }));
  if (r.files.length) out.push(tt('templates.insert.files', { n: r.files.length }));
  return out;
}

/**
 * Body of the canvases dialog's "From template" option. `commit(call)`
 * runs the insert like other canvas changes (editor.apply); `onInserted(r)`
 * gets the result.
 */
export function insertFromTemplate({ commit, onInserted, err }) {
  const st = { template: null, canvases: new Set(), styles: 'rename' };
  const canvasBox = h('div', { class: 'stack' });
  const report = h('div', { class: 'tpl-result small' });
  let seq = 0;

  async function plan() {
    const my = ++seq;
    if (!st.template) return;
    try {
      const r = await api.templateInsertPlan(args());
      if (my === seq) report.replaceChildren(...reportLines(r).map((l) => h('div', {}, l)));
    } catch (e) {
      if (my === seq) report.replaceChildren(h('div', { class: 'form-error' }, e.message));
    }
  }

  function args() {
    return { template: st.template.id, canvases: [...st.canvases], styles: st.styles };
  }

  function drawCanvases() {
    const tpl = st.template;
    if (!tpl) return canvasBox.replaceChildren();
    canvasBox.replaceChildren(
      ...tpl.canvases.map((c) =>
        h(
          'div',
          { class: 'tpl-canvas' },
          checkbox({
            label: `${c.id} · ${c.aspect} · ${tt('templates.pages', { n: c.pages })}`,
            checked: st.canvases.has(c.id),
            onchange: (on) => {
              if (on) st.canvases.add(c.id);
              else st.canvases.delete(c.id);
              plan();
            },
          }),
          tpl.canvases.length > 1 ? previewImg(tpl.id, c.id) : null,
        ),
      ),
    );
  }

  const picker = templatePicker({
    onerror: (e) => err.show(e),
    onchange: (tpl) => {
      if (!tpl || tpl.id === st.template?.id) return;
      st.template = tpl;
      st.canvases = new Set(tpl.canvases.map((c) => c.id));
      drawCanvases();
      plan();
    },
  });

  const el = h(
    'div',
    { class: 'form' },
    picker.el,
    row(tt('templates.insert.canvases'), canvasBox),
    row(
      tt('templates.insert.styles'),
      segmented({
        options: ['rename', 'use_project'].map((v) => ({ value: v, label: t(`templates.insert.styles.${v}`) })),
        value: st.styles,
        onchange: (v) => {
          st.styles = v;
          plan();
        },
      }),
      tt('templates.insert.styles.hint'),
    ),
    report,
  );

  async function insert() {
    if (!st.template || !st.canvases.size) return err.show(new Error(tt('templates.insert.none')));
    const r = await commit(() => api.templateInsert(args()));
    if (r) onInserted?.(r);
  }

  return { el, insert };
}
