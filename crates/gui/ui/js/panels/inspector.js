// Inspector panel (right sidebar, task 2.6).
//
// - One element: common fields (id, home page + lock, position, size,
//   rotation, pivot, opacity, z, anchor, clip, visibility) and the
//   kind's own fields (device, text, image, shape, background). Values
//   come from `element_get` (base, variant override, effective); edits go
//   through `element_patch`, which writes the base or – in a variant
//   view – the variant's override (decided in core). Overridden fields
//   are marked and resettable.
// - Several elements: the common fields opacity, rotation, z, visibility.
// - Nothing selected: the canvas settings (panels/canvas-settings.js).
//
// Re-rendered after every edit (overlay refresh); focus and unsaved
// typing are kept by `data-key`.

import { h, icon } from '../dom.js';
import { currentCanvas, effectiveVariant } from '../store.js';
import {
  checkInput,
  colorInput,
  fillEditor,
  fmtNum,
  numberInput,
  segmentedInput,
  selectInput,
  textInput,
} from './fields.js';
import { renderCanvasSettings } from './canvas-settings.js';
import { openImagePreview } from '../dialogs/image-preview.js';

const KIND_ICON = { device: 'device', text: 'text', image: 'image', shape: 'shape', group: 'group', background: 'background' };
const BUILTIN_SHADOWS = ['soft', 'hard', 'none'];
const collapsed = new Set(JSON.parse(safeGet('vitrine.inspector.collapsed') ?? '[]'));

function safeGet(k) {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}
function safeSet(k, v) {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* private mode */
  }
}

/** Cached lookups shared by renders (catalog, assets, string keys). */
function createCache(api) {
  let devices = null;
  let targets = null;
  let assets = null;
  let assetsAt = 0;
  const keys = new Map();
  return {
    devices: () => (devices ??= api.catalogDevices().catch(() => [])),
    targets: () => (targets ??= api.catalogTargets().catch(() => [])),
    assets() {
      if (!assets || performance.now() - assetsAt > 5000) {
        assetsAt = performance.now();
        assets = api.assetsList().catch(() => ({ screenshots: [], assets: [], fonts: [] }));
      }
      return assets;
    },
    keys(lang, fresh) {
      if (fresh || !keys.has(lang)) keys.set(lang, api.stringsGet(lang).then((r) => r.keys).catch(() => []));
      return keys.get(lang);
    },
    invalidate() {
      assets = null;
      keys.clear();
    },
  };
}

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, ctx) {
  const { store, editor, api, t, viewArgs } = ctx;
  const title = h('span', {}, t('panel.inspector'));
  const sub = h('span', { class: 'insp-sub muted small' });
  const header = h('div', { class: 'panel-header' }, title, sub);
  const body = h('div', { class: 'panel-body insp' });
  el.append(header, body);
  const cache = createCache(api);
  let seq = 0;

  // ----- render plumbing ---------------------------------------------------------------

  /** Replace the body, keeping focus and unsaved typing (by data-key). */
  function swap(nodes) {
    const a = document.activeElement;
    const keep = a && body.contains(a) && a.dataset?.key ? { key: a.dataset.key, value: a.value, dirty: a.dataset.dirty, s: a.selectionStart, e: a.selectionEnd } : null;
    const scroll = body.scrollTop;
    body.replaceChildren(...nodes);
    body.scrollTop = scroll;
    if (keep) {
      const n = body.querySelector(`[data-key="${CSS.escape(keep.key)}"]`);
      if (n) {
        if (keep.dirty && 'value' in n) {
          n.value = keep.value;
          n.dataset.dirty = '1';
        }
        n.focus({ preventScroll: true });
        try {
          if (keep.s != null) n.setSelectionRange(keep.s, keep.e);
        } catch {
          /* not a text input */
        }
      }
    }
  }

  function section(id, label, ...content) {
    const open = !collapsed.has(id);
    const sec = h('section', { class: `insp-section${open ? '' : ' closed'}` });
    const toggle = () => {
      if (collapsed.has(id)) collapsed.delete(id);
      else collapsed.add(id);
      safeSet('vitrine.inspector.collapsed', JSON.stringify([...collapsed]));
      sec.classList.toggle('closed');
    };
    sec.append(
      h('button', { type: 'button', class: 'insp-section-title', 'aria-expanded': String(open), onclick: toggle }, icon('chevron'), h('span', {}, label)),
      h('div', { class: 'insp-section-body' }, ...content.flat()),
    );
    return sec;
  }

  /**
   * A labelled row. `field`: YAML field name(s) for the override marker
   * and reset; `hint`: small text under the controls.
   */
  function row(label, controls, { field, info, hint, wide } = {}) {
    const fields = [field].flat().filter(Boolean);
    const langOver = info ? fields.filter((f) => (info.lang_overridden ?? []).includes(f)) : [];
    const over = info && !langOver.length ? fields.filter((f) => info.overridden.includes(f)) : [];
    const cls = langOver.length ? ' is-lang-overridden' : over.length ? ' is-overridden' : '';
    const langs = (info?.lang_override_langs ?? []).join(', ');
    const r = h('div', { class: `insp-row${cls}${wide ? ' wide' : ''}` });
    r.append(h('span', { class: 'insp-label', title: langOver.length ? `${t('langoverride.overridden')} ${langs}` : over.length ? t('inspector.overridden') : label }, label));
    r.append(h('div', { class: 'insp-controls' }, ...[controls].flat().filter(Boolean)));
    if (langOver.length) {
      r.append(
        h(
          'button',
          {
            type: 'button',
            class: 'mini-btn reset',
            title: `${t('langoverride.reset_field')} (${langs}): ${langOver.join(', ')}`,
            onclick: async () => {
              for (const f of langOver) await editor.apply(api.resetOverride(viewArgs(), info.id, f, true), { touched: [info.id] });
            },
          },
          icon('reset'),
        ),
      );
    } else if (over.length) {
      r.append(
        h(
          'button',
          {
            type: 'button',
            class: 'mini-btn reset',
            title: `${t('inspector.reset_field')}: ${over.join(', ')}`,
            onclick: async () => {
              for (const f of over) await editor.apply(api.resetOverride(viewArgs(), info.id, f), { touched: [info.id] });
            },
          },
          icon('reset'),
        ),
      );
    } else r.append(h('span'));
    if (hint) r.append(h('div', { class: 'insp-hint muted small' }, hint));
    return r;
  }

  const palette = () => store.get().summary?.styles.colors ?? {};

  // ----- refresh -------------------------------------------------------------------------

  async function refresh() {
    const st = store.get();
    const my = ++seq;
    const sel = st.selection;
    const variant = effectiveVariant(st);
    sub.textContent = variant ? `${t('status.variant')}: ${variant}` : '';
    sub.classList.toggle('variant', !!variant);
    if (!st.summary || !st.overlay) {
      swap([h('p', { class: 'placeholder' }, t('inspector.none'))]);
      return;
    }
    try {
      if (!sel.length) {
        title.textContent = t('canvas.title');
        const nodes = await renderCanvasSettings(ctx, { section, row, cache, palette });
        if (my === seq) swap(nodes);
        return;
      }
      title.textContent = t('panel.inspector');
      if (sel.length > 1) {
        const infos = await Promise.all(sel.slice(0, 40).map((id) => api.elementGet(viewArgs(), id).catch(() => null)));
        if (my === seq) swap(renderMulti(sel, infos.filter(Boolean)));
        return;
      }
      const info = await api.elementGet(viewArgs(), sel[0]);
      const extra = await loadExtras(info, st);
      if (my === seq) swap(renderElement(info, extra));
    } catch (e) {
      if (my === seq) swap([h('p', { class: 'placeholder' }, `${t('error.edit')}: ${e.message}`)]);
    }
  }

  async function loadExtras(info, st) {
    const out = {};
    if (info.kind === 'device') {
      [out.devices, out.assets, out.frames, out.resolved] = await Promise.all([
        cache.devices(),
        cache.assets(),
        api.framesList().catch(() => null),
        // The resolved device (imported bezel choice for this view).
        api.elementInfo(viewArgs(), info.id).catch(() => null),
      ]);
    } else if (info.kind === 'text') {
      out.keys = await cache.keys(st.lang);
    } else if (info.kind === 'image' || info.kind === 'shape' || info.kind === 'background') {
      out.assets = await cache.assets();
    }
    return out;
  }

  // ----- edits ---------------------------------------------------------------------------

  const patch = (ids, fields, strings) =>
    editor.apply(api.elementPatch(viewArgs(), ids, fields, strings), { touched: [ids].flat() });

  // ----- one element ---------------------------------------------------------------------

  function renderElement(info, extra) {
    const st = store.get();
    const e = info.effective;
    const id = info.id;
    const o = st.overlay;
    const oe = o.elements.find((x) => x.id === id);
    const og = o.groups.find((x) => x.id === id);
    const set = (fields, strings) => patch(id, fields, strings);
    const kind = info.kind;
    const isMarginText = kind === 'text' && e.box != null;
    const nodes = [];

    // Head: kind icon, id (rename), kind name.
    const idInput = textInput({
      key: 'id',
      value: id,
      mono: true,
      title: t('inspector.id'),
      validate: (v) => (/^[a-z0-9][a-z0-9_-]*$/i.test(v) ? null : t('inspector.id.invalid')),
      onCommit: (v) => editor.apply(api.rename(id, v), { touched: [id], select: [v] }),
    });
    nodes.push(
      h('div', { class: 'insp-head' }, h('span', { class: 'insp-kind', title: t(`kind.${kind}`) }, icon(KIND_ICON[kind] ?? 'shape')), idInput, h('span', { class: 'muted small' }, t(`kind.${kind}`))),
    );

    const variant = info.variant;
    if (variant) {
      nodes.push(
        h(
          'div',
          { class: 'insp-variant' },
          h('span', { class: 'grow' }, `${t('inspector.variant_edit')} ${variant}`),
          info.overridden.length
            ? h('button', { type: 'button', class: 'btn btn-small', onclick: () => editor.apply(api.resetOverride(viewArgs(), id, undefined), { touched: [id] }) }, icon('reset'), t('inspector.reset_all'))
            : null,
        ),
      );
    }

    // ----- layout -----
    const layout = [];
    if (kind !== 'group') {
      const canvas = currentCanvas(st);
      const locked = !!e.lock_page;
      const pages = (canvas?.page.ids ?? []).map((pid, i) => ({ value: String(i), label: `${i + 1} · ${pid}` }));
      const lock = h(
        'button',
        {
          type: 'button',
          class: `mini-btn${locked ? ' on' : ''}`,
          title: locked ? t('inspector.page.unlock') : t('inspector.page.lock'),
          'aria-pressed': String(locked),
          onclick: () => set({ lock_page: locked ? null : true }),
        },
        icon(locked ? 'lock' : 'unlock'),
      );
      layout.push(
        row(
          t('inspector.page'),
          [selectInput({ key: 'page', options: pages, value: String(e.page ?? 0), title: t('inspector.page'), onChange: (v) => editor.apply(api.setPage(viewArgs(), id, Number(v), true), { touched: [id] }) }), lock],
          { field: ['page', 'lock_page'], info, hint: locked ? t('inspector.page.locked') : null },
        ),
      );
    }
    if (kind !== 'background' && !isMarginText) {
      const at = e.at ?? { x: 0, y: 0 };
      const mp = oe?.move_point ?? og?.move_point;
      layout.push(
        row(
          kind === 'group' ? t('inspector.offset') : t('inspector.position'),
          [
            numberInput({ key: 'at.x', label: 'X', value: at.x, precision: 1, onCommit: (v) => set({ at: { x: v, y: at.y } }) }).el,
            numberInput({ key: 'at.y', label: 'Y', value: at.y, precision: 1, onCommit: (v) => set({ at: { x: at.x, y: v } }) }).el,
          ],
          { field: 'at', info, hint: mp ? `${t('inspector.canvas')} ${fmtNum(mp.x, 0)}, ${fmtNum(mp.y, 0)}` : null },
        ),
      );
    }
    if (kind === 'device') {
      layout.push(
        row(t('inspector.size'), [
          numberInput({ key: 'width', label: 'W', value: e.width ?? null, optional: true, placeholder: '800', min: 1, precision: 1, onCommit: (v) => set({ width: v }) }).el,
          h('span', { class: 'muted small insp-derived' }, oe ? `H ${fmtNum(oe.rect.h, 0)}` : ''),
        ], { field: 'width', info }),
      );
    } else if (kind === 'image' || kind === 'shape' || (kind === 'text' && !isMarginText)) {
      layout.push(
        row(t('inspector.size'), [
          numberInput({ key: 'width', label: 'W', value: e.width ?? null, optional: true, placeholder: kind === 'text' ? '840' : '', min: 0, precision: 1, onCommit: (v) => set({ width: v }) }).el,
          numberInput({ key: 'height', label: 'H', value: e.height ?? null, optional: true, placeholder: t('inspector.auto'), min: 0, precision: 1, onCommit: (v) => set({ height: v }) }).el,
        ], { field: ['width', 'height'], info }),
      );
    }
    if (kind !== 'background') {
      const pv = e.pivot ?? null;
      layout.push(
        row(t('inspector.rotation'), numberInput({ key: 'rotation', label: '∠', value: e.rotation ?? 0, unit: '°', precision: 2, step: 1, big: 15, onCommit: (v) => set({ rotation: v === 0 ? null : v }) }).el, { field: 'rotation', info }),
      );
      if (kind !== 'group' || pv) {
        layout.push(
          row(
            t('inspector.pivot'),
            [
              numberInput({ key: 'pivot.x', label: 'X', value: pv?.x ?? null, optional: true, placeholder: '0', precision: 1, onCommit: (v) => set({ pivot: v == null && !pv?.y ? null : { x: v ?? 0, y: pv?.y ?? 0 } }) }).el,
              numberInput({ key: 'pivot.y', label: 'Y', value: pv?.y ?? null, optional: true, placeholder: '0', precision: 1, onCommit: (v) => set({ pivot: v == null && !pv?.x ? null : { x: pv?.x ?? 0, y: v ?? 0 } }) }).el,
            ],
            { field: 'pivot', info, hint: kind === 'text' && !e.height && !e.box?.bottom ? t('inspector.pivot.text') : t('inspector.pivot.hint') },
          ),
        );
      }
    }
    if (layout.length) nodes.push(section('layout', t('inspector.layout'), layout));

    // ----- kind specific -----
    if (kind === 'device') nodes.push(deviceSection(info, e, extra, set));
    if (kind === 'text') nodes.push(textSection(info, e, extra, set, oe));
    if (kind === 'image') nodes.push(imageSection(info, e, extra, set));
    if (kind === 'shape') nodes.push(shapeSection(info, e, extra, set));
    if (kind === 'background') {
      nodes.push(section('fill', t('inspector.fill'), row(t('inspector.fill'), fillEditor({ key: 'fill', fill: e.fill, palette: palette(), images: imagePaths(extra), t, onError: (m) => editor.toast(m, true), onCommit: (f) => set({ fill: f }) }), { field: 'fill', info, wide: true })));
    }
    if (kind === 'group') {
      nodes.push(section('group', t('kind.group'), row(t('inspector.members'), h('span', { class: 'insp-value' }, String(info.children?.length ?? 0)))));
    }

    // ----- appearance & visibility -----
    nodes.push(section('appearance', t('inspector.appearance'), commonRows(info, e, set)));
    return nodes;
  }

  function commonRows(info, e, set) {
    const st = store.get();
    const rows = [
      row(t('inspector.opacity'), numberInput({ key: 'opacity', label: 'α', value: e.opacity ?? 1, scale: 100, unit: '%', precision: 0, min: 0, max: 1, step: 0.01, big: 0.1, onCommit: (v) => set({ opacity: v === 1 ? null : v }) }).el, { field: 'opacity', info }),
      row(t('inspector.z'), numberInput({ key: 'z', label: 'Z', value: e.z ?? 0, precision: 0, step: 1, big: 10, onCommit: (v) => set({ z: v === 0 ? null : Math.round(v) }) }).el, { field: 'z', info }),
    ];
    if (info.kind !== 'group' && info.kind !== 'background') {
      const anchors = [{ value: '', label: t('inspector.default') }, ...['top', 'bottom', 'center', 'stretch'].map((a) => ({ value: a, label: t(`inspector.anchor.${a}`) }))];
      rows.push(row(t('inspector.anchor'), selectInput({ key: 'anchor_y', options: anchors, value: e.anchor_y ?? '', title: t('inspector.anchor'), onChange: (v) => set({ anchor_y: v || null }) }), { field: 'anchor_y', info }));
    }
    if (info.kind !== 'group') {
      const def = info.kind === 'text' ? t('inspector.clip.page') : t('inspector.clip.none');
      const clips = [{ value: '', label: `${t('inspector.default')} (${def})` }, { value: 'none', label: t('inspector.clip.none') }, { value: 'page', label: t('inspector.clip.page') }];
      rows.push(row(t('inspector.clip'), selectInput({ key: 'clip', options: clips, value: e.clip ?? '', title: t('inspector.clip'), onChange: (v) => set({ clip: v || null }) }), { field: 'clip', info }));
    }
    rows.push(row(t('inspector.visible'), checkInput({ key: 'visible', checked: e.visible !== false, label: t('inspector.visible.on'), onChange: (v) => set({ visible: v ? null : false }) }), { field: 'visible', info }));

    // visible_in: chips per target of the canvas and per language (none = all).
    const vi = e.visible_in ?? {};
    const toggleIn = (dim, value, all) => {
      const cur = vi[dim]?.length ? [...vi[dim]] : [];
      const next = cur.includes(value) ? cur.filter((x) => x !== value) : [...cur, value];
      const norm = next.length === all.length ? [] : next;
      const out = { ...vi, [dim]: norm };
      if (!out.targets?.length) delete out.targets;
      if (!out.langs?.length) delete out.langs;
      set({ visible_in: Object.keys(out).length ? out : null });
    };
    const chips = (dim, all) =>
      h(
        'div',
        { class: 'insp-chips' },
        ...all.map((v) => {
          const on = !vi[dim]?.length || vi[dim].includes(v);
          return h('button', { type: 'button', class: `toggle-chip${on ? ' on' : ''}`, 'aria-pressed': String(on), title: v, onclick: () => toggleIn(dim, v, all) }, v);
        }),
      );
    const targets = st.summary.targets.filter((x) => x.canvas === st.canvas).map((x) => x.id);
    rows.push(row(t('inspector.visible_in.targets'), chips('targets', targets), { field: 'visible_in', info, wide: true }));
    rows.push(row(t('inspector.visible_in.langs'), chips('langs', st.summary.languages.available), { field: 'visible_in', info, wide: true }));
    return rows;
  }

  function shadowSelect(info, e, set, emptyLabel = t('inspector.shadow.off')) {
    const project = store.get().summary.styles.shadows ?? [];
    const names = [...new Set([...BUILTIN_SHADOWS, ...project])];
    const options = [{ value: '', label: emptyLabel }, ...names.map((n) => ({ value: n, label: n }))];
    return row(t('inspector.shadow'), selectInput({ key: 'shadow', options, value: e.shadow ?? '', title: t('inspector.shadow'), onChange: (v) => set({ shadow: v || null }) }), { field: 'shadow', info });
  }

  function deviceSection(info, e, extra, set) {
    const devices = extra.devices ?? [];
    const dev = devices.find((d) => d.id === e.model);
    const rows = [];
    const models = devices.map((d) => ({ value: d.id, label: d.label, group: t(`inspector.class.${d.class}`) }));
    if (!dev) models.unshift({ value: e.model, label: e.model });
    rows.push(row(t('inspector.model'), selectInput({ key: 'model', options: models, value: e.model, title: t('inspector.model'), onChange: (v) => set({ model: v }) }), { field: 'model', info }));
    const frameDefault = store.get().summary.styles.frame;
    // Own frame packs (ADR 0024): the project's first, then app-wide; a
    // project pack hides an app pack with the same id.
    const packs = [];
    for (const p of extra.frames?.packs ?? []) if (!packs.some((x) => x.id === p.id)) packs.push(p);
    const packOf = (f) => (f?.startsWith('pack:') ? packs.find((p) => p.frame === f) : null);
    const frameLabel = (f) => {
      if (f?.startsWith('imported:')) return t(`frames.source.${f.slice(9)}`);
      if (f?.startsWith('pack:')) return t('frames.pack.label').replace('{name}', packOf(f)?.name ?? f.slice(5));
      return t(`inspector.frame.${f}`);
    };
    // Short form for the 288 px panel ("Project (Apple)").
    const frameShort = (f) => {
      if (f?.startsWith('imported:')) return t(`frames.source_short.${f.slice(9)}`);
      if (f?.startsWith('pack:')) return packOf(f)?.name ?? f.slice(5);
      return t(`inspector.frame.${f}`);
    };
    // Imported frames (ADR 0012) and frame packs: the installed colours of
    // this model, with the housing colour measured at import.
    const effFrame = e.frame ?? frameDefault;
    const isPack = effFrame?.startsWith('pack:');
    const isImported = effFrame?.startsWith('imported:') || isPack;
    const pack = isPack ? packOf(effFrame) : null;
    const importedSrc = effFrame?.startsWith('imported:') ? extra.frames?.sources?.find((x) => x.source === effFrame.slice(9)) : null;
    const packColors = () => {
      const out = [];
      for (const f of pack?.frames ?? []) {
        if ((f.model === e.model || f.model === '*') && !out.some((c) => c.id === f.color)) out.push({ id: f.color, label: f.color_label, swatch: f.swatch });
      }
      return out;
    };
    const importedColors = isImported ? (extra.resolved?.element?.props?.imported?.colors ?? (isPack ? packColors() : importedSrc?.colors?.[e.model]) ?? []) : [];
    let frameHint = null;
    if (isPack) {
      if (!pack) frameHint = t('frames.inspector.pack_missing');
      else if (!importedColors.length) frameHint = t('frames.inspector.pack_no_model');
      else if (e.color && !e.color.startsWith('#') && !importedColors.some((c) => c.id === e.color)) frameHint = t('frames.inspector.color_missing');
    } else if (isImported) {
      const drawn = extra.resolved?.element?.props?.imported;
      if (!importedSrc?.models?.includes(e.model)) frameHint = t('frames.inspector.no_model');
      else if (!importedColors.length) frameHint = t('frames.inspector.missing');
      else if (e.color && !e.color.startsWith('#') && !importedColors.some((c) => c.id === e.color)) frameHint = t('frames.inspector.color_missing');
      // Where the drawn bezel comes from: the project's stored copy or this
      // computer's frame store (ADR 0027).
      else if (drawn) frameHint = t(`frames.inspector.origin.${drawn.origin === 'project' ? 'project' : 'app'}`);
    }
    const packOptions = packs.map((p) => ({ value: p.frame, label: t('frames.pack.label').replace('{name}', p.name), title: `${p.frame} (${t(`frames.pack.origin.${p.origin}`)})` }));
    if (e.frame?.startsWith('pack:') && !packOf(e.frame)) packOptions.push({ value: e.frame, label: t('frames.pack.missing_option').replace('{id}', e.frame.slice(5)) });
    rows.push(
      row(
        t('inspector.frame'),
        selectInput({
          key: 'frame',
          options: [
            { value: '', label: t('inspector.frame.project').replace('{frame}', frameShort(frameDefault)), title: `${t('inspector.project_default')} (${frameLabel(frameDefault)})` },
            { value: 'stylized', label: t('inspector.frame.stylized') },
            { value: 'detailed', label: t('inspector.frame.detailed') },
            { value: 'imported:apple', label: t('frames.source.apple') },
            ...packOptions,
          ],
          value: e.frame ?? '',
          title: e.frame ? t('inspector.frame') : `${t('inspector.project_default')} (${frameLabel(frameDefault)})`,
          // The colour follows in the same undo step when the new source
          // does not have it (mapped in Rust, `Editor::patch`).
          onChange: (v) => set({ frame: v || null }),
        }),
        { field: 'frame', info, hint: frameHint },
      ),
    );
    // Imported frames: which Apple model's bezel (several sizes per device,
    // e.g. iPhone 18 Pro and Pro Max). Automatic = per target. Short labels
    // ("Auto · 18 Pro Max"), the full text is the tooltip.
    const imp = extra.resolved?.element?.props?.imported;
    if (isImported && (imp?.bezels?.length > 1 || e.bezel)) {
      const bezels = imp?.bezels ?? [];
      const short = (b) => b.replace(/^(iPhone|iPad|Apple)\s+/i, '');
      const known = !e.bezel || bezels.some((b) => b.toLowerCase() === e.bezel.toLowerCase());
      const autoFull = imp?.automatic ? t('frames.inspector.bezel.auto').replace('{bezel}', imp.automatic) : t('inspector.default');
      const options = [{ value: '', label: imp?.automatic ? t('frames.inspector.bezel.auto_short').replace('{bezel}', short(imp.automatic)) : t('inspector.default'), title: autoFull }, ...bezels.map((b) => ({ value: b, label: b }))];
      if (!known) options.push({ value: e.bezel, label: t('frames.inspector.bezel.not_installed').replace('{bezel}', e.bezel) });
      const current = e.bezel ? (bezels.find((b) => b.toLowerCase() === e.bezel.toLowerCase()) ?? e.bezel) : '';
      const sel = selectInput({ key: 'bezel', options, value: current, title: current ? current : autoFull, onChange: (v) => set({ bezel: v || null }) });
      rows.push(row(t('frames.inspector.bezel'), sel, { field: 'bezel', info, hint: known ? null : t('frames.inspector.bezel.unknown') }));
    }
    // Colour: one set of swatches. Imported frame with installed colours →
    // those colours (measured swatch, vendor name); otherwise the catalog
    // palette + a custom hex (parametric frames, also the fallback).
    const current = e.color ?? null;
    const swatchButton = (name, label, hex) => {
      const b = h('button', { type: 'button', class: `dev-swatch${current === name ? ' current' : ''}`, title: label, 'aria-label': label, 'aria-pressed': String(current === name), onclick: () => set({ color: name }) });
      if (hex) b.style.setProperty('--swatch', hex);
      return b;
    };
    const defaultButton = (label) => h('button', { type: 'button', class: `dev-swatch default${current == null ? ' current' : ''}`, title: label, 'aria-label': label, onclick: () => set({ color: null }) }, 'A');
    let colorControls;
    if (importedColors.length) {
      const def = importedColors.find((c) => c.id === dev?.default_color) ?? importedColors[0];
      colorControls = [
        h('div', { class: 'insp-swatches' }, defaultButton(`${t('inspector.default')} (${def.label})`), ...importedColors.map((c) => swatchButton(c.id, c.label, c.swatch ?? '#8E8E93'))),
      ];
    } else {
      const swatches = dev?.swatches ?? [];
      const defLabel = swatches.find((s) => s.name === dev?.default_color)?.label ?? dev?.default_color;
      const custom = current && current.startsWith('#') ? current : '';
      colorControls = [
        h('div', { class: 'insp-swatches' }, defaultButton(`${t('inspector.default')}${defLabel ? ` (${defLabel})` : ''}`), ...swatches.map((s) => swatchButton(s.name, s.label, s.hex))),
        colorInput({ key: 'color', value: custom, palette: {}, swatches: [], optional: true, placeholder: t('inspector.color.custom'), t, onCommit: (v) => set({ color: v }) }),
      ];
    }
    rows.push(row(t('inspector.color'), colorControls, { field: 'color', info, wide: true }));
    const orients = [{ value: '', label: `${t('inspector.default')}${dev ? ` (${t(`inspector.orientation.${dev.natural}`)})` : ''}` }, ...(dev?.orientations ?? ['portrait', 'landscape']).map((x) => ({ value: x, label: t(`inspector.orientation.${x}`) }))];
    rows.push(row(t('inspector.orientation'), selectInput({ key: 'orientation', options: orients, value: e.orientation ?? '', title: t('inspector.orientation'), onChange: (v) => set({ orientation: v || null }) }), { field: 'orientation', info }));
    if (dev?.fold) {
      // Foldables: open (inner display) or folded (outer display).
      const folds = [{ value: '', label: `${t('inspector.default')} (${t(`inspector.fold.${dev.fold.state}`)})` }, ...['open', 'folded'].map((x) => ({ value: x, label: t(`inspector.fold.${x}`) }))];
      rows.push(row(t('inspector.fold'), selectInput({ key: 'fold', options: folds, value: e.fold ?? '', title: t('inspector.fold'), onChange: (v) => set({ fold: v || null }) }), { field: 'fold', info }));
    }
    const keys = [...new Set((extra.assets?.screenshots ?? []).map((s) => s.key))].sort();
    rows.push(
      row(t('inspector.screenshot'), textInput({ key: 'screenshot', value: e.screenshot ?? '', list: keys, mono: true, placeholder: t('inspector.screenshot.none'), title: t('inspector.screenshot'), onCommit: (v) => set({ screenshot: v || null }) }), {
        field: 'screenshot',
        info,
        hint: e.screenshot && !keys.includes(e.screenshot) ? t('inspector.screenshot.unknown') : null,
      }),
    );
    rows.push(row(t('inspector.fit'), fitSelect(e.fit, 'cover', (v) => set({ fit: v || null })), { field: 'fit', info }));
    rows.push(shadowSelect(info, e, set));
    const computer = dev && (dev.class === 'laptop' || dev.class === 'desktop');
    if (computer) {
      rows.push(
        row(t('inspector.chrome'), selectInput({ key: 'chrome', options: [{ value: '', label: t('inspector.default') }, { value: 'none', label: t('inspector.chrome.none') }, { value: 'browser', label: t('inspector.chrome.browser') }], value: e.chrome ?? '', title: t('inspector.chrome'), onChange: (v) => set({ chrome: v || null }) }), { field: 'chrome', info }),
      );
    }
    if (computer || dev?.browser) {
      rows.push(row(t('inspector.url'), textInput({ key: 'url', value: e.url ?? '', placeholder: 'example.com', title: t('inspector.url'), onCommit: (v) => set({ url: v || null }) }), { field: 'url', info }));
      rows.push(
        row(t('inspector.appearance_mode'), selectInput({ key: 'appearance', options: [{ value: '', label: t('inspector.default') }, { value: 'light', label: t('inspector.appearance.light') }, { value: 'dark', label: t('inspector.appearance.dark') }], value: e.appearance ?? '', title: t('inspector.appearance_mode'), onChange: (v) => set({ appearance: v || null }) }), { field: 'appearance', info }),
      );
    }
    return section('device', t('kind.device'), rows);
  }

  function fitSelect(value, def, onChange) {
    const options = [{ value: '', label: `${t('inspector.default')} (${t(`inspector.f.fit.${def}`)})` }, ...['cover', 'contain', 'stretch'].map((x) => ({ value: x, label: t(`inspector.f.fit.${x}`) }))];
    return selectInput({ key: 'fit', options, value: typeof value === 'string' ? value : '', title: t('inspector.fit'), onChange });
  }

  function textSection(info, e, extra, set, oe) {
    const st = store.get();
    const rows = [];
    const tx = info.text;
    const keys = extra.keys ?? [];
    // "Only for <lang>": edits of this text go into its language override.
    const scoped = !!st.langScope;
    const langs = info.lang_override_langs ?? [];
    rows.push(
      h(
        'div',
        { class: `insp-lang-scope${scoped ? ' on' : ''}` },
        checkInput({ key: 'lang_scope', checked: scoped, label: `${t('langoverride.scope')} ${st.lang}`, title: t('langoverride.scope.hint'), onChange: (v) => {
          store.set({ langScope: v });
          refresh();
        } }),
        h('span', { class: 'grow muted small' }, langs.length ? `${t('langoverride.list')}: ${langs.join(', ')}` : ''),
        langs.includes(st.lang)
          ? h('button', { type: 'button', class: 'btn btn-small', title: t('langoverride.reset_all'), onclick: () => editor.apply(api.resetOverride(viewArgs(), info.id, undefined, true), { touched: [info.id] }) }, icon('reset'), t('inspector.reset_all'))
          : null,
      ),
    );
    // Key: pick an existing key or type a new one (created with the current text).
    rows.push(
      row(
        t('inspector.key'),
        textInput({
          key: 'text.key',
          value: e.text,
          list: keys,
          mono: true,
          title: t('inspector.key'),
          validate: (v) => (/^[a-z0-9][a-z0-9._-]*$/i.test(v) ? null : t('inspector.key.invalid')),
          onCommit: (v) => {
            const strings = keys.includes(v) ? [] : [{ lang: st.lang, key: v, value: tx?.value ?? tx?.fallback ?? '' }];
            cache.keys(st.lang, true);
            set({ text: v }, strings);
          },
        }),
        { field: 'text', info, hint: tx && !keys.includes(e.text) ? t('inspector.key.new') : null },
      ),
    );
    // Live text of the current language.
    const area = h('textarea', { class: 'insp-textarea', rows: 3, spellcheck: 'true', dataset: { key: 'text.value' }, placeholder: tx?.fallback ? `${tx.fallback}` : t('inspector.text.empty') });
    area.value = tx?.value ?? '';
    const commitText = () => {
      delete area.dataset.dirty;
      const v = area.value;
      if (v === (tx?.value ?? '')) return;
      set({}, [{ lang: st.lang, key: e.text, value: v === '' ? null : v }]);
    };
    area.addEventListener('input', () => {
      area.dataset.dirty = '1';
    });
    area.addEventListener('keydown', (ev) => {
      if (ev.key === 'Enter' && (ev.metaKey || ev.ctrlKey)) {
        ev.preventDefault();
        commitText();
      } else if (ev.key === 'Escape') {
        area.value = tx?.value ?? '';
        delete area.dataset.dirty;
        area.blur();
      }
    });
    area.addEventListener('blur', () => area.dataset.dirty && commitText());
    const hint = tx?.value == null ? (tx?.fallback_lang ? `${t('inspector.text.fallback')} ${tx.fallback_lang}` : t('inspector.text.missing')) : t('inspector.text.hint');
    rows.push(row(`${t('inspector.text')} · ${st.lang}`, area, { hint, wide: true }));

    const styles = st.summary.styles.text;
    rows.push(row(t('inspector.style'), selectInput({ key: 'style', options: [{ value: '', label: t('inspector.style.none') }, ...styles.map((s) => ({ value: s, label: s }))], value: e.style ?? '', title: t('inspector.style'), onChange: (v) => set({ style: v || null }) }), { field: 'style', info }));
    const ot = oe?.text;
    rows.push(
      row(t('inspector.font_size'), numberInput({ key: 'font_size', label: 'A', unit: 'lu', value: e.font_size ?? null, optional: true, placeholder: ot ? fmtNum(ot.font_size, 1) : t('inspector.auto'), min: 1, precision: 1, onCommit: (v) => set({ font_size: v }) }).el, { field: 'font_size', info }),
    );
    rows.push(row(t('inspector.color'), colorInput({ key: 'text.color', value: e.color ?? '', palette: palette(), optional: true, placeholder: t('inspector.style.color'), t, onCommit: (v) => set({ color: v }) }), { field: 'color', info }));
    rows.push(...textEffectRows(info, e, set));

    // Box: page margins or a free box.
    const margins = e.box != null;
    const toFree = () => {
      const b = e.box ?? {};
      const w = 1000 - (b.left ?? 0) - (b.right ?? 0);
      const hgt = ot ? ot.content.h : 200;
      set({ box: null, at: { x: (b.left ?? 0) + w / 2, y: (b.top ?? 0) + hgt / 2 }, width: Math.max(1, w), valign: e.valign ?? 'top' });
    };
    const toMargins = () => {
      const w = e.width ?? 840;
      const at = e.at ?? { x: 500, y: 0 };
      const hgt = e.height ?? (oe ? oe.rect.h : 0);
      set({ box: { top: Math.round(at.y - hgt / 2), left: Math.round(at.x - w / 2), right: Math.round(1000 - at.x - w / 2) }, at: null, width: null, height: null });
    };
    rows.push(
      row(t('inspector.box'), segmentedInput({ options: [{ value: 'margins', label: t('inspector.box.margins') }, { value: 'free', label: t('inspector.box.free') }], value: margins ? 'margins' : 'free', onChange: (v) => (v === 'free' ? toFree() : toMargins()), title: t('inspector.box') }), {
        field: 'box',
        info,
      }),
    );
    if (margins) {
      const b = e.box;
      const m = (side, label) =>
        numberInput({
          key: `box.${side}`,
          label,
          title: t(`inspector.box.${side}`),
          value: b[side] ?? null,
          optional: true,
          placeholder: side === 'bottom' ? t('inspector.auto') : '0',
          precision: 1,
          onCommit: (v) => {
            const nb = { ...b, [side]: v };
            if (v == null) delete nb[side];
            set({ box: nb });
          },
        }).el;
      rows.push(row(t('inspector.box.margins'), h('div', { class: 'insp-grid2' }, m('top', 'T'), m('bottom', 'B'), m('left', 'L'), m('right', 'R')), { field: 'box', info }));
    }
    rows.push(
      row(t('inspector.align'), segmentedInput({ options: ['left', 'center', 'right'].map((a) => ({ value: a, icon: `align-${a}`, title: t(`inspector.align.${a}`) })), value: e.align ?? 'left', onChange: (v) => set({ align: v === 'left' ? null : v }), title: t('inspector.align') }), {
        field: 'align',
        info,
      }),
    );
    rows.push(
      row(t('inspector.valign'), segmentedInput({ options: ['top', 'center', 'bottom'].map((a) => ({ value: a, icon: `valign-${a}`, title: t(`inspector.valign.${a}`) })), value: e.valign ?? (margins ? 'top' : 'center'), onChange: (v) => set({ valign: v }), title: t('inspector.valign') }), {
        field: 'valign',
        info,
      }),
    );
    const fit = typeof e.fit === 'object' && e.fit ? e.fit : {};
    const setFit = (k, v) => {
      const nf = { ...fit, [k]: v };
      if (v == null) delete nf[k];
      set({ fit: Object.keys(nf).length ? nf : null });
    };
    rows.push(
      row(t('inspector.autofit'), [
        numberInput({ key: 'fit.shrink_to', label: '↓', title: t('inspector.shrink_to'), value: fit.shrink_to ?? null, optional: true, placeholder: t('inspector.shrink_to.off'), min: 1, precision: 1, onCommit: (v) => setFit('shrink_to', v) }).el,
        numberInput({ key: 'fit.max_lines', label: '≡', title: t('inspector.max_lines'), value: fit.max_lines ?? null, optional: true, placeholder: '∞', min: 1, precision: 0, onCommit: (v) => setFit('max_lines', v == null ? null : Math.round(v)) }).el,
      ], { field: 'fit', info }),
    );
    const hy = e.hyphenate;
    rows.push(
      row(
        t('hyphen.label'),
        selectInput({
          key: 'hyphenate',
          options: [
            { value: '', label: t('hyphen.style') },
            { value: 'on', label: t('hyphen.on') },
            { value: 'off', label: t('hyphen.off') },
          ],
          value: hy == null ? '' : hy ? 'on' : 'off',
          title: t('hyphen.hint'),
          onChange: (v) => set({ hyphenate: v === '' ? null : v === 'on' }),
        }),
        { field: 'hyphenate', info, hint: t('hyphen.hint') },
      ),
    );
    if (ot) {
      const status = ot.overflow
        ? h('span', { class: 'insp-status error' }, icon('error'), t('inspector.overflow'))
        : ot.emergency_breaks?.length
          ? h('span', { class: 'insp-status warn' }, icon('warning'), `${t('inspector.emergency')}: ${ot.emergency_breaks.join(', ')}`)
          : ot.hyphenated?.length
            ? h('span', { class: 'insp-status ok' }, icon('check'), `${t('hyphen.used')}: ${ot.hyphenated.join(', ')}`)
            : h('span', { class: 'insp-status ok' }, icon('check'), t('inspector.fits'));
      rows.push(row(t('inspector.computed'), h('div', { class: 'insp-computed' }, h('span', {}, `${fmtNum(ot.font_size, 1)} lu · ${ot.lines} ${t('inspector.lines_unit')}`), status), { wide: true }));
    }
    return section('text', t('kind.text'), rows);
  }

  /**
   * Outline, glow and shadow of a text element. Unchecked = the text
   * style's value; a width / opacity of 0 or shadow `none` switches the
   * style's effect off.
   */
  function textEffectRows(info, e, set) {
    const stroke = e.stroke ?? null;
    const glow = e.glow ?? null;
    const setStroke = (patch) => set({ stroke: { ...stroke, ...patch } });
    const setGlow = (patch) => {
      const g = { ...glow, ...patch };
      for (const k of ['opacity', 'spread']) if (g[k] == null) delete g[k];
      set({ glow: g });
    };
    return [
      row(
        t('inspector.outline'),
        [
          checkInput({ key: 'stroke.on', checked: !!stroke, onChange: (on) => set({ stroke: on ? { color: '#FFFFFF', width: 4 } : null }), title: t('inspector.fx.override') }),
          stroke ? colorInput({ key: 'stroke.color', value: stroke.color, palette: palette(), t, onCommit: (v) => setStroke({ color: v ?? '#000000' }) }) : null,
          stroke
            ? h(
                'div',
                { class: 'insp-grid2' },
                numberInput({ key: 'stroke.width', label: 'W', title: t('inspector.outline.width'), unit: 'lu', value: stroke.width, min: 0, precision: 1, onCommit: (v) => setStroke({ width: v ?? 0 }) }).el,
                selectInput({
                  key: 'stroke.position',
                  options: ['outside', 'center'].map((x) => ({ value: x, label: t(`inspector.outline.${x}`) })),
                  value: stroke.position ?? 'outside',
                  title: t('inspector.outline.position'),
                  onChange: (v) => {
                    const ns = { ...stroke, position: v };
                    if (v === 'outside') delete ns.position;
                    set({ stroke: ns });
                  },
                }),
              )
            : null,
        ],
        { field: 'stroke', info, wide: !!stroke, hint: stroke ? null : t('inspector.fx.from_style') },
      ),
      row(
        t('inspector.glow'),
        [
          checkInput({ key: 'glow.on', checked: !!glow, onChange: (on) => set({ glow: on ? { color: '#FFFFFF', radius: 12 } : null }), title: t('inspector.fx.override') }),
          glow ? colorInput({ key: 'glow.color', value: glow.color, palette: palette(), t, onCommit: (v) => setGlow({ color: v ?? '#FFFFFF' }) }) : null,
          glow
            ? h(
                'div',
                { class: 'insp-grid2' },
                numberInput({ key: 'glow.radius', label: 'R', title: t('inspector.glow.radius'), value: glow.radius, min: 0, precision: 1, onCommit: (v) => setGlow({ radius: v ?? 0 }) }).el,
                numberInput({ key: 'glow.opacity', label: '%', title: t('inspector.glow.opacity'), value: glow.opacity ?? null, optional: true, placeholder: '100', scale: 100, min: 0, max: 1, precision: 0, onCommit: (v) => setGlow({ opacity: v }) }).el,
                numberInput({ key: 'glow.spread', label: '+', title: t('inspector.glow.spread'), value: glow.spread ?? null, optional: true, placeholder: '0', min: 0, precision: 1, onCommit: (v) => setGlow({ spread: v }) }).el,
              )
            : null,
        ],
        { field: 'glow', info, wide: !!glow, hint: glow ? null : t('inspector.fx.from_style') },
      ),
      shadowSelect(info, e, set, t('inspector.shadow.style')),
    ];
  }

  function imagePaths(extra) {
    return (extra.assets?.assets ?? []).map((a) => a.path).filter((p) => /\.(png|jpe?g|svg|webp)$/i.test(p));
  }

  function imageSection(info, e, extra, set) {
    const list = imagePaths(extra);
    // Preview of the image (all project images, ←/→ step through them).
    const previewBtn = list.includes(e.asset)
      ? h(
          'button',
          {
            type: 'button',
            class: 'mini-btn',
            title: t('preview.open'),
            onclick: () => {
              const usage = extra.assets?.asset_usage ?? {};
              openImagePreview({
                items: list.map((path) => ({ path, title: path.replace(/^assets\//, ''), users: usage[path] ?? [] })),
                index: list.indexOf(e.asset),
                onSelectElement: (sel) => {
                  if (store.get().overlay?.elements.some((x) => x.id === sel)) store.set({ selection: [sel] });
                  else editor.toast(`${sel}: ${t('assets.not_in_view')}`);
                },
              });
            },
          },
          icon('eye'),
        )
      : null;
    return section('image', t('kind.image'), [
      row(t('inspector.asset'), [textInput({ key: 'asset', value: e.asset ?? '', list, mono: true, title: t('inspector.asset'), onCommit: (v) => v && set({ asset: v }) }), previewBtn], { field: 'asset', info, hint: e.asset && !list.includes(e.asset) ? t('inspector.asset.unknown') : null }),
      row(t('inspector.fit'), fitSelect(e.fit, 'contain', (v) => set({ fit: v || null })), { field: 'fit', info }),
      clipToRow(info, e, set),
      shadowSelect(info, e, set),
    ]);
  }

  /** `clip_to`: a rect/ellipse shape of this canvas as the image's mask (ADR 0031). */
  function clipToRow(info, e, set) {
    const shapes = (currentCanvas(store.get())?.elements ?? []).filter((x) => x.kind === 'shape' && x.label !== 'line');
    const known = !e.clip_to || shapes.some((x) => x.id === e.clip_to);
    const options = [
      { value: '', label: t('inspector.clip_to.none') },
      ...shapes.map((x) => ({ value: x.id, label: `${x.id} · ${t(`inspector.shape.${x.label}`)}${x.visible ? '' : ` (${t('inspector.clip_to.hidden')})`}` })),
      ...(known ? [] : [{ value: e.clip_to, label: e.clip_to }]),
    ];
    return row(t('inspector.clip_to'), selectInput({ key: 'clip_to', options, value: e.clip_to ?? '', title: t('inspector.clip_to.title'), onChange: (v) => set({ clip_to: v || null }) }), { field: 'clip_to', info, hint: known ? null : t('inspector.clip_to.unknown') });
  }

  function shapeSection(info, e, extra, set) {
    const rows = [
      row(t('inspector.shape'), selectInput({ key: 'shape', options: ['rect', 'ellipse', 'line'].map((x) => ({ value: x, label: t(`inspector.shape.${x}`) })), value: e.shape, title: t('inspector.shape'), onChange: (v) => set({ shape: v }) }), { field: 'shape', info }),
    ];
    if (e.shape === 'rect') {
      rows.push(row(t('inspector.radius'), numberInput({ key: 'radius', label: '◜', value: e.radius ?? null, optional: true, placeholder: '0', min: 0, precision: 1, onCommit: (v) => set({ radius: v }) }).el, { field: 'radius', info }));
    }
    rows.push(row(t('inspector.fill'), fillEditor({ key: 'fill', fill: e.fill ?? null, optional: true, palette: palette(), images: imagePaths(extra), t, onError: (m) => editor.toast(m, true), onCommit: (f) => set({ fill: f }) }), { field: 'fill', info, wide: true }));
    const stroke = e.stroke ?? null;
    rows.push(
      row(
        t('inspector.stroke'),
        [
          checkInput({ key: 'stroke.on', checked: !!stroke, onChange: (on) => set({ stroke: on ? { color: '#000000', width: 4 } : null }), title: t('inspector.stroke') }),
          stroke ? colorInput({ key: 'stroke.color', value: stroke.color, palette: palette(), t, onCommit: (v) => set({ stroke: { ...stroke, color: v ?? '#000000' } }) }) : null,
          stroke ? numberInput({ key: 'stroke.width', label: 'W', value: stroke.width, min: 0, precision: 1, onCommit: (v) => set({ stroke: { ...stroke, width: v } }) }).el : null,
        ],
        { field: 'stroke', info, wide: !!stroke },
      ),
    );
    rows.push(shadowSelect(info, e, set));
    return section('shape', t('kind.shape'), rows);
  }

  // ----- several elements -----------------------------------------------------------

  function renderMulti(ids, infos) {
    const same = (get) => {
      const vals = infos.map((i) => get(i.effective));
      return vals.every((v) => v === vals[0]) ? vals[0] : undefined;
    };
    const set = (fields) => patch(ids, fields);
    const mixed = t('inspector.mixed');
    const op = same((e) => e.opacity ?? 1);
    const rot = same((e) => e.rotation ?? 0);
    const z = same((e) => e.z ?? 0);
    const vis = same((e) => e.visible !== false);
    const rows = [
      row(t('inspector.opacity'), numberInput({ key: 'm.opacity', label: 'α', value: op ?? null, placeholder: mixed, scale: 100, unit: '%', precision: 0, min: 0, max: 1, step: 0.01, big: 0.1, onCommit: (v) => set({ opacity: v === 1 ? null : v }) }).el),
      row(t('inspector.rotation'), numberInput({ key: 'm.rotation', label: '∠', value: rot ?? null, placeholder: mixed, unit: '°', precision: 2, big: 15, onCommit: (v) => set({ rotation: v === 0 ? null : v }) }).el),
      row(t('inspector.z'), numberInput({ key: 'm.z', label: 'Z', value: z ?? null, placeholder: mixed, precision: 0, onCommit: (v) => set({ z: v === 0 ? null : Math.round(v) }) }).el),
      row(t('inspector.visible'), checkInput({ key: 'm.visible', checked: vis !== false, label: vis === undefined ? mixed : t('inspector.visible.on'), onChange: (v) => set({ visible: v ? null : false }) })),
    ];
    return [
      h('div', { class: 'insp-head' }, h('span', { class: 'insp-kind' }, icon('layers')), h('span', { class: 'insp-count' }, `${ids.length} ${t('inspector.elements')}`)),
      h('ul', { class: 'insp-idlist' }, ...ids.map((id) => h('li', { class: 'mono small' }, id))),
      section('appearance', t('inspector.appearance'), rows),
    ];
  }

  // ----- subscriptions --------------------------------------------------------------------

  const unsubs = [
    store.select((s) => [s.selection, s.overlay, s.variant, s.lang, s.target, s.canvas, s.summary], () => refresh()),
    editor.on('reset', () => cache.invalidate()),
    // Full invalidations (asset imports, external edits): asset lists and
    // translation keys may have changed.
    editor.on('invalidate', (p) => {
      if (!p?.all) return;
      cache.invalidate();
      refresh();
    }),
  ];
  refresh();
  return {
    unmount() {
      for (const u of unsubs) u();
    },
  };
}
