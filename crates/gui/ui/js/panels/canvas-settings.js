// Canvas settings (task 2.6), shown in the inspector when nothing is
// selected: page aspect (target presets or custom), page count, page ids,
// canvas backgrounds (add/remove/reorder, fill editor) and variants
// (add/rename/remove, content scale). Every change is one undo step via
// `canvas_patch` / `canvas_variant`.

import { openTargets } from '../dialogs/targets.js';
import { openNewVariant, removeVariant } from '../dialogs/variants.js';
import { h, icon } from '../dom.js';
import { fillCss, fillEditor, numberInput, selectInput, textInput } from './fields.js';

const gcd = (a, b) => (b ? gcd(b, a % b) : a);

/** Aspect presets from the target catalog, unique by ratio. */
function aspectPresets(targets) {
  const seen = new Map();
  for (const tp of targets) {
    for (const s of [...(tp.portrait ?? []), ...(tp.landscape ?? [])]) {
      const [w, hgt] = Array.isArray(s) ? s : [s.w, s.h];
      const g = gcd(w, hgt);
      const ratio = `${w / g}:${hgt / g}`;
      if (!seen.has(ratio)) seen.set(ratio, { value: `${w}:${hgt}`, label: `${tp.label} · ${w}×${hgt}`, group: tp.store });
    }
  }
  return [...seen.values()];
}

const ratioOf = (aspect) => {
  const [w, hgt] = aspect.split(':').map(Number);
  const g = gcd(w, hgt) || 1;
  return `${w / g}:${hgt / g}`;
};

/**
 * Builds the canvas settings for the current canvas.
 * @param {import('./index.js').PanelContext} ctx
 * @param {{section: Function, row: Function, cache: object, palette: Function}} ui
 */
export async function renderCanvasSettings(ctx, ui) {
  const { store, editor, api, t } = ctx;
  const { section, row, cache, palette } = ui;
  const canvasId = store.get().canvas;
  if (!canvasId) return [h('p', { class: 'placeholder' }, t('inspector.none'))];
  const [s, targets, assets] = await Promise.all([api.canvasGet(canvasId), cache.targets(), cache.assets()]);
  const images = (assets.assets ?? []).map((a) => a.path).filter((p) => /\.(png|jpe?g|svg|webp)$/i.test(p));
  const apply = (call) => editor.apply(call, {});
  const patch = (p) => apply(api.canvasPatch(canvasId, p));
  const variant = (change) => apply(api.canvasVariant(canvasId, change));

  const nodes = [
    h('div', { class: 'insp-head' }, h('span', { class: 'insp-kind' }, icon('canvases')), h('span', { class: 'insp-count mono' }, s.id), h('span', { class: 'muted small' }, `${s.targets.length} ${t('panel.canvases.targets')}`)),
    h('p', { class: 'insp-note muted small' }, t('canvas.hint')),
  ];

  // ----- pages -----
  const presets = aspectPresets(targets);
  const current = presets.find((p) => ratioOf(p.value) === ratioOf(s.aspect));
  const options = [...(current ? [] : [{ value: s.aspect, label: `${t('canvas.aspect.custom')} · ${s.aspect}` }]), ...presets];
  const pages = [
    row(t('canvas.aspect'), selectInput({ key: 'canvas.aspect.preset', options, value: current?.value ?? s.aspect, title: t('canvas.aspect'), onChange: (v) => patch({ aspect: v }) })),
    row(
      t('canvas.aspect.custom'),
      textInput({
        key: 'canvas.aspect',
        value: s.aspect,
        mono: true,
        placeholder: '1320:2868',
        title: t('canvas.aspect'),
        validate: (v) => (/^\s*\d+\s*:\s*\d+\s*$/.test(v) && !/^0|:\s*0+\s*$/.test(v) ? null : t('canvas.aspect.invalid')),
        onCommit: (v) => patch({ aspect: v.replace(/\s/g, '') }),
      }),
      { hint: `${t('canvas.height')} ${Math.round(s.height_lu)} lu` },
    ),
    row(t('canvas.count'), [
      h('button', { type: 'button', class: 'mini-btn', title: t('canvas.count.remove'), disabled: s.count <= 1, onclick: () => patch({ count: s.count - 1 }) }, icon('minus')),
      numberInput({ key: 'canvas.count', value: s.count, min: 1, max: 99, precision: 0, onCommit: (v) => patch({ count: Math.round(v) }) }).el,
      h('button', { type: 'button', class: 'mini-btn', title: t('canvas.count.add'), onclick: () => patch({ count: s.count + 1 }) }, icon('plus')),
    ]),
  ];
  const ids = s.page_ids.map((pid, i) => {
    const explicit = s.ids[i] ?? '';
    return h(
      'div',
      { class: 'page-id' },
      h('span', { class: 'muted small page-no' }, String(i + 1)),
      textInput({
        key: `canvas.ids.${i}`,
        value: explicit,
        mono: true,
        placeholder: `page-${String(i + 1).padStart(2, '0')}`,
        title: t('canvas.page_id'),
        validate: (v) => (v === '' || /^[a-z0-9][a-z0-9_-]*$/i.test(v) ? null : t('inspector.id.invalid')),
        onCommit: (v) => {
          const next = s.page_ids.map((_, j) => (j === i ? v : (s.ids[j] ?? '')));
          patch({ ids: next });
        },
      }),
    );
  });
  pages.push(row(t('canvas.page_ids'), h('div', { class: 'page-ids' }, ...ids), { wide: true }));
  nodes.push(section('canvas.pages', t('canvas.pages'), pages));

  // ----- backgrounds -----
  const bgs = s.backgrounds;
  const setBgs = (list) => patch({ backgrounds: list });
  const bgRows = bgs.map((fill, i) => {
    const move = (d) => {
      const l = [...bgs];
      [l[i], l[i + d]] = [l[i + d], l[i]];
      setBgs(l);
    };
    const swatch = h('span', { class: 'fill-preview' });
    swatch.style.background = fillCss(fill, palette());
    return h(
      'div',
      { class: 'bg-item' },
      h(
        'div',
        { class: 'bg-item-head' },
        h('span', { class: 'small muted' }, `${t('canvas.background')} ${i + 1}${i === 0 ? ` · ${t('canvas.background.bottom')}` : ''}`),
        h('span', { class: 'grow' }),
        h('button', { type: 'button', class: 'mini-btn', title: t('inspector.f.move_down'), disabled: i === 0, onclick: () => move(-1) }, icon('down')),
        h('button', { type: 'button', class: 'mini-btn', title: t('inspector.f.move_up'), disabled: i === bgs.length - 1, onclick: () => move(1) }, icon('up')),
        h('button', { type: 'button', class: 'mini-btn', title: t('canvas.background.remove'), onclick: () => setBgs(bgs.filter((_, j) => j !== i)) }, icon('trash')),
      ),
      fillEditor({ key: `canvas.bg${i}`, fill, palette: palette(), images, t, onError: (m) => editor.toast(m, true), onCommit: (f) => setBgs(bgs.map((b, j) => (j === i ? f : b))) }),
    );
  });
  nodes.push(
    section(
      'canvas.backgrounds',
      t('canvas.backgrounds'),
      bgRows.length ? bgRows : h('p', { class: 'insp-note muted small' }, t('canvas.backgrounds.none')),
      h('div', { class: 'insp-actions' }, h('button', { type: 'button', class: 'btn btn-small', onclick: () => setBgs([...bgs, '#FFFFFF']) }, icon('plus'), t('canvas.background.add'))),
    ),
  );

  // ----- variants -----
  const vRows = s.variants.map((v) =>
    h(
      'div',
      { class: 'variant-item' },
      h(
        'div',
        { class: 'variant-head' },
        textInput({
          key: `canvas.variant.${v.id}`,
          value: v.id,
          mono: true,
          title: t('canvas.variant.rename'),
          validate: (x) => (/^[a-z0-9][a-z0-9_-]*$/i.test(x) ? null : t('inspector.id.invalid')),
          onCommit: (x) => variant({ action: 'rename', id: v.id, to: x }),
        }),
        h('button', { type: 'button', class: 'mini-btn', title: t('canvases.duplicate'), onclick: () => openNewVariant({ canvas: canvasId, from: v.id }) }, icon('copy')),
        h('button', { type: 'button', class: 'mini-btn', title: t('canvas.variant.remove'), onclick: () => removeVariant(canvasId, v) }, icon('trash')),
      ),
      row(
        t('canvas.variant.scale'),
        numberInput({ key: `canvas.variant.${v.id}.scale`, label: '×', value: v.content_scale ?? null, optional: true, placeholder: '1', min: 0.05, max: 4, step: 0.01, big: 0.1, precision: 3, onCommit: (x) => variant({ action: 'scale', id: v.id, content_scale: x }) }).el,
      ),
      h('div', { class: 'insp-note muted small' }, `${v.overrides.length} ${t('canvas.variant.overrides')}${v.targets.length ? ` · ${v.targets.join(', ')}` : ''}`),
    ),
  );
  nodes.push(
    section(
      'canvas.variants',
      t('canvas.variants'),
      h('p', { class: 'insp-note muted small' }, t('variant.hint')),
      vRows.length ? vRows : h('p', { class: 'insp-note muted small' }, t('canvas.variants.none')),
      h(
        'div',
        { class: 'insp-actions' },
        h('button', { type: 'button', class: 'btn btn-small', onclick: () => openNewVariant({ canvas: canvasId, targets: [], switchTo: true }) }, icon('plus'), t('variant.new')),
        h('button', { type: 'button', class: 'btn btn-small', onclick: () => openTargets(null, { mode: 'canvas', canvas: canvasId }) }, t('canvases.manage')),
      ),
    ),
  );
  return nodes;
}
