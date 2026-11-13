// Project settings dialog (task 2.10): name, frame style, languages
// (add/remove/reorder, fallback map, default language with the §5.0
// impact warning), style tokens (colours, text styles, span styles, shadow
// presets), the fonts available to the project and the store mapping
// (Phase 5.1, `stores.js`), and the frames stored in the project
// (`stored-frames.js`, ADR 0027). Every change is one
// backend command building the ops in Rust, applied via `editor.apply`.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { store } from '../store.js';
import { checkbox, confirmDialog, errorLine, input, openModal, row, select } from './modal.js';
import { storesPane } from './stores.js';
import { storedFramesPane } from './stored-frames.js';
import { storedFontsPane } from './stored-fonts.js';
import { fontPicker } from './font-picker.js';

const TABS = ['general', 'languages', 'styles', 'fonts', 'frames', 'stores'];
/** Frame styles of the project default; `imported:*` are Apple's bezels (ADR 0012). */
const FRAMES = ['detailed', 'stylized', 'imported:apple'];
const WEIGHTS = [100, 200, 300, 400, 500, 600, 700, 800, 900];
const BUILTIN_SHADOWS = ['soft', 'hard', 'none'];

export function openProjectSettings(tab = 'general', { focusAdd = false } = {}) {
  const err = errorLine();
  const tabs = h('div', { class: 'dlg-tabs', role: 'tablist' });
  const pane = h('div', { class: 'ps-pane' });
  let current = tab;
  let styles = null;
  let fonts = null;
  /** Installed frame packages (for the "not installed" hint). */
  let frames = null;
  /** Expanded style entries (`kind:name`), kept across re-renders after edits. */
  const openStyles = new Set();
  /** "Frames" tab: the "all sizes" choice before it is applied. */
  const framesState = {};

  const m = openModal({
    title: t('project.settings'),
    size: 'large',
    cls: 'dlg-project',
    body: [tabs, err.el, pane],
    footer: [h('span', { class: 'spacer' }), h('button', { class: 'btn btn-primary', type: 'button', onclick: () => m.close() }, t('project.dialog.done'))],
  });

  async function commit(call) {
    let result;
    try {
      result = await call();
    } catch (e) {
      err.show(e);
      return null;
    }
    err.show(null);
    if (result.applied) {
      await editor.apply(Promise.resolve(result));
      if (result.changes?.styles) styles = null;
    }
    await render();
    return result;
  }

  function renderTabs() {
    tabs.replaceChildren(
      ...TABS.map((id) =>
        h(
          'button',
          {
            type: 'button',
            role: 'tab',
            'aria-selected': String(id === current),
            onclick: () => {
              current = id;
              err.show(null);
              renderTabs();
              render();
            },
          },
          t(`project.tab.${id}`),
        ),
      ),
    );
  }

  async function render() {
    const s = store.get().summary;
    if (!s) return;
    // Keep the scroll position of the dialog body across re-renders.
    const scroller = pane.closest('.dlg-body') ?? pane.parentElement;
    const top = scroller?.scrollTop ?? 0;
    await renderPane(s);
    if (scroller) scroller.scrollTop = top;
  }

  async function renderPane(s) {
    if (current === 'general') {
      frames ??= await api.framesList().catch(() => ({ sources: [] }));
      pane.replaceChildren(general(s));
    }
    else if (current === 'languages') pane.replaceChildren(languages(s));
    else if (current === 'stores') pane.replaceChildren(await storesPane(commit));
    else if (current === 'frames') pane.replaceChildren(await storedFramesPane(render, framesState));
    else if (current === 'styles') {
      styles ??= await api.stylesGet();
      fonts ??= await api.catalogFonts();
      pane.replaceChildren(stylesPane(s));
    } else {
      styles ??= await api.stylesGet();
      fonts ??= await api.catalogFonts();
      pane.replaceChildren(await fontsPane());
    }
  }

  /** A collapsible style entry that stays open across re-renders. */
  function styleDetails(kind, name, summary, form) {
    const id = `${kind}:${name}`;
    const d = h('details', { class: 'ps-style' }, summary, form);
    d.open = openStyles.has(id);
    d.addEventListener('toggle', () => (d.open ? openStyles.add(id) : openStyles.delete(id)));
    return d;
  }

  // ----- general --------------------------------------------------------------------
  function general(s) {
    return h(
      'div',
      { class: 'form' },
      row(t('project.name'), input({ value: s.name, onchange: (v) => commit(() => api.projectRename(v)) })),
      row(t('project.folder'), h('span', { class: 'mono small selectable' }, s.dir)),
      frameRow(s.styles.frame),
    );
  }

  /** Default frame: built-in styles or Apple's imported bezels; switching
   *  remaps device colours in Rust (`styles_patch`). */
  function frameRow(value) {
    // Own frame packs (ADR 0024), project packs first (they hide app packs).
    const packs = [];
    for (const p of frames?.packs ?? []) if (!packs.some((x) => x.id === p.id)) packs.push(p);
    const label = (f) => (f.startsWith('imported:') ? t(`frames.source.${f.slice(9)}`) : t(`settings.frame.${f}`));
    const options = [...FRAMES.map((f) => ({ value: f, label: label(f) })), ...packs.map((p) => ({ value: p.frame, label: t('frames.pack.label').replace('{name}', p.name) }))];
    if (value && !options.some((o) => o.value === value)) options.push({ value, label: value });
    const src = value?.startsWith('imported:') ? value.slice(9) : null;
    // Installed on this computer, or stored in the project (ADR 0027).
    const installed = src && (frames?.sources?.find((x) => x.source === src)?.frames?.length || frames?.stored?.[src]?.length);
    const hint = src && !installed ? h('span', {}, t('project.frame.hint'), h('br'), h('span', { class: 'hint-warn' }, t('project.frame.no_frames'))) : t('project.frame.hint');
    return row(t('project.frame'), select({ options, value, onchange: (v) => commit(() => api.stylesPatch({ frame: v })) }), hint);
  }

  // ----- languages ------------------------------------------------------------------
  function languages(s) {
    const L = s.languages;
    const list = h('div', { class: 'ps-langs' });
    L.available.forEach((l, i) => {
      const isDefault = l === L.default;
      const move = (d) => {
        const order = [...L.available];
        const j = i + d;
        [order[i], order[j]] = [order[j], order[i]];
        commit(() => api.languagesSet({ order }));
      };
      list.append(
        h(
          'div',
          { class: 'ps-lang' },
          h('span', { class: 'ps-lang-code mono' }, l),
          isDefault ? h('span', { class: 'tag tag-verified' }, t('project.lang.default')) : h('button', { class: 'btn btn-small', type: 'button', onclick: () => changeDefault(l) }, t('project.lang.make_default')),
          h('span', { class: 'spacer' }),
          isDefault
            ? h('span', { class: 'muted small' }, t('project.lang.fallback.root'))
            : h(
                'label',
                { class: 'mini' },
                h('span', { class: 'muted small' }, t('project.lang.fallback')),
                select({
                  options: [{ value: '', label: t('project.lang.fallback.auto') }, ...L.available.filter((x) => x !== l)],
                  value: L.fallback[l] ?? '',
                  onchange: (v) => {
                    const fb = { ...L.fallback };
                    if (v) fb[l] = v;
                    else delete fb[l];
                    commit(() => api.languagesSet({ fallback: fb }));
                  },
                }),
              ),
          h('button', { class: 'icon-btn', type: 'button', title: t('project.lang.up'), disabled: i === 0, onclick: () => move(-1) }, '↑'),
          h('button', { class: 'icon-btn', type: 'button', title: t('project.lang.down'), disabled: i === L.available.length - 1, onclick: () => move(1) }, '↓'),
          h(
            'button',
            {
              class: 'icon-btn',
              type: 'button',
              title: t('project.lang.remove'),
              disabled: isDefault,
              onclick: async () => {
                if (await confirmDialog({ title: t('project.lang.remove'), message: `${t('project.lang.remove.confirm')} (${l})`, confirm: t('project.lang.remove'), danger: true })) {
                  if (store.get().lang === l) store.set({ lang: L.default });
                  commit(() => api.languageRemove(l));
                }
              },
            },
            icon('close'),
          ),
        ),
      );
    });
    let newLang = '';
    const add = input({ placeholder: t('project.lang.add.placeholder'), cls: 'num-wide lang-add', oninput: (v) => (newLang = v.trim()) });
    const doAdd = () => newLang && commit(() => api.languageAdd(newLang));
    add.addEventListener('keydown', (e) => e.key === 'Enter' && doAdd());
    return h(
      'div',
      { class: 'stack' },
      h('p', { class: 'muted small' }, t('project.lang.hint')),
      list,
      h('div', { class: 'inline' }, add, h('button', { class: 'btn btn-small', type: 'button', onclick: doAdd }, icon('plus'), t('project.lang.add'))),
    );
  }

  async function changeDefault(lang) {
    let impact;
    try {
      impact = await api.defaultLanguageImpact(lang);
    } catch (e) {
      err.show(e);
      return;
    }
    const lost = impact.texts.length + impact.screenshots.length;
    const message = h(
      'div',
      { class: 'stack' },
      h('p', { class: 'dlg-text' }, `${t('project.lang.default.change')} ${impact.from} → ${impact.to}.`),
      lost
        ? h(
            'div',
            { class: 'impact' },
            h('p', { class: 'hint hint-warn' }, icon('warning'), `${lost} ${t('project.lang.impact.lost')}`),
            impact.texts.length
              ? h('div', {}, h('div', { class: 'section-title' }, t('project.lang.impact.texts')), h('ul', { class: 'impact-list' }, impact.texts.slice(0, 100).map((x) => h('li', {}, h('span', { class: 'mono' }, `${x.lang} · ${x.key}`), h('span', { class: 'muted' }, ` (${t('project.lang.impact.from')} ${x.from})`)))))
              : null,
            impact.screenshots.length
              ? h('div', {}, h('div', { class: 'section-title' }, t('project.lang.impact.screenshots')), h('ul', { class: 'impact-list' }, impact.screenshots.slice(0, 100).map((x) => h('li', {}, h('span', { class: 'mono' }, `${x.lang} · ${x.key}${x.variant ? ` (${x.variant})` : ''}`), h('span', { class: 'muted' }, ` ${x.path}`)))))
              : null,
          )
        : h('p', { class: 'hint hint-ok' }, icon('check'), t('project.lang.impact.none')),
    );
    if (await confirmDialog({ title: t('project.lang.default.title'), message, confirm: t('project.lang.make_default'), danger: lost > 0 })) {
      await commit(() => api.defaultLanguageSet(lang));
    }
  }

  // ----- styles ---------------------------------------------------------------------
  function colorRefInput(value, onchange) {
    return input({ value: value ?? '', placeholder: '#RRGGBB / @name', cls: 'color-ref', onchange: (v) => onchange(v.trim() || null), title: t('project.styles.color_ref') });
  }

  function stylesPane(s) {
    const listId = 'ps-color-names';
    const datalist = h('datalist', { id: listId }, Object.keys(styles.colors ?? {}).map((n) => h('option', { value: `@${n}` })));
    const shadowNames = [...new Set([...BUILTIN_SHADOWS, ...Object.keys(styles.shadows ?? {})])];
    const patch = (p) => commit(() => api.stylesPatch(p));
    const renameInput = (kind, name) =>
      input({
        value: name,
        cls: 'style-name mono',
        onchange: (v) => {
          if (!v || v === name) return;
          if (openStyles.delete(`${kind}:${name}`)) openStyles.add(`${kind}:${v}`);
          patch({ rename: { kind, from: name, to: v } });
        },
      });
    const removeBtn = (map, name) => h('button', { class: 'icon-btn', type: 'button', title: t('project.styles.remove'), onclick: () => patch({ [map]: { [name]: null } }) }, icon('close'));
    const addRow = (placeholder, onadd) => {
      let name = '';
      const el = input({ placeholder, cls: 'style-name mono', oninput: (v) => (name = v.trim()) });
      const go = () => name && onadd(name);
      el.addEventListener('keydown', (e) => e.key === 'Enter' && go());
      return h('div', { class: 'inline ps-add' }, el, h('button', { class: 'btn btn-small', type: 'button', onclick: go }, icon('plus'), t('project.styles.add')));
    };
    const withRefs = (input) => {
      input.setAttribute('list', listId);
      return input;
    };

    // colours
    const colors = Object.entries(styles.colors ?? {}).map(([name, hex]) => {
      const picker = h('input', { type: 'color', class: 'swatch', title: hex });
      picker.value = hex.slice(0, 7).toLowerCase();
      picker.addEventListener('change', () => patch({ colors: { [name]: `${picker.value}${hex.length === 9 ? hex.slice(7) : ''}`.toUpperCase() } }));
      return h('div', { class: 'ps-style-row' }, picker, renameInput('color', name), input({ value: hex, cls: 'mono hex', onchange: (v) => v && patch({ colors: { [name]: v } }) }), removeBtn('colors', name));
    });

    /** Outline and glow rows of a text or span style (`set(key, value)`). */
    const effectRows = (st, set) => {
      const stroke = st.stroke ?? null;
      const glow = st.glow ?? null;
      const setStroke = (k, v) => {
        const ns = { ...stroke, [k]: v };
        if (v == null || (k === 'position' && v === 'outside')) delete ns[k];
        set('stroke', ns);
      };
      const setGlow = (k, v) => set('glow', clean({ ...glow, [k]: v }));
      const mini = (label, value, onchange, step = 1, title) => h('label', { class: 'mini', title }, h('span', { class: 'muted small' }, label), input({ type: 'number', value, step, min: 0, cls: 'num', placeholder: '–', onchange }));
      return [
        row(
          t('inspector.outline'),
          h(
            'div',
            { class: 'inline wrap' },
            checkbox({ checked: !!stroke, title: t('inspector.outline'), onchange: (on) => set('stroke', on ? { color: '#FFFFFF', width: 4 } : null) }),
            stroke ? withRefs(colorRefInput(stroke.color, (v) => v && setStroke('color', v))) : null,
            stroke ? mini('W', stroke.width, (v) => setStroke('width', v ?? 0), 0.5, t('inspector.outline.width')) : null,
            stroke ? select({ options: ['outside', 'center'].map((x) => ({ value: x, label: t(`inspector.outline.${x}`) })), value: stroke.position ?? 'outside', title: t('inspector.outline.position'), onchange: (v) => setStroke('position', v) }) : null,
          ),
        ),
        row(
          t('inspector.glow'),
          h(
            'div',
            { class: 'inline wrap' },
            checkbox({ checked: !!glow, title: t('inspector.glow'), onchange: (on) => set('glow', on ? { color: '#FFFFFF', radius: 12 } : null) }),
            glow ? withRefs(colorRefInput(glow.color, (v) => v && setGlow('color', v))) : null,
            glow ? mini('R', glow.radius, (v) => setGlow('radius', v ?? 0), 1, t('inspector.glow.radius')) : null,
            glow ? mini('α', glow.opacity, (v) => setGlow('opacity', v == null ? null : Math.min(1, v)), 0.05, t('inspector.glow.opacity')) : null,
            glow ? mini('+', glow.spread, (v) => setGlow('spread', v), 1, t('inspector.glow.spread')) : null,
          ),
        ),
      ];
    };

    // text styles
    const text = Object.entries(styles.text ?? {}).map(([name, st]) => {
      const set = (k, v) => patch({ text: { [name]: clean({ ...st, [k]: v }) } });
      return styleDetails(
        'text',
        name,
        h('summary', {}, h('span', { class: 'mono' }, name), h('span', { class: 'muted small' }, ` ${st.font ?? 'Inter'} ${st.weight ?? 400} · ${st.size ?? '–'} lu`)),
        h(
          'div',
          { class: 'form' },
          row(t('project.styles.name'), h('div', { class: 'inline' }, renameInput('text', name), removeBtn('text', name))),
          row(t('project.styles.font'), fontPicker({ value: st.font, fonts, emptyLabel: t('project.styles.default'), onchange: (v) => set('font', v || null) })),
          row(t('project.styles.weight'), h('div', { class: 'inline' }, select({ options: [{ value: '', label: t('project.styles.default') }, ...WEIGHTS.map(String)], value: st.weight ? String(st.weight) : '', onchange: (v) => set('weight', v ? Number(v) : null) }), checkbox({ label: t('project.styles.italic'), checked: st.italic, onchange: (v) => set('italic', v || null) }))),
          row(t('project.styles.size'), num(st.size, (v) => set('size', v), 'lu')),
          row(t('project.styles.line_height'), num(st.line_height, (v) => set('line_height', v), '×', 0.05)),
          row(t('project.styles.tracking'), num(st.tracking, (v) => set('tracking', v), 'em', 0.01)),
          row(t('project.styles.color'), withRefs(colorRefInput(st.color, (v) => set('color', v)))),
          row(t('project.styles.transform'), select({ options: [{ value: '', label: t('project.styles.default') }, ...['none', 'uppercase', 'lowercase'].map((v) => ({ value: v, label: t(`project.styles.transform.${v}`) }))], value: st.transform ?? '', onchange: (v) => set('transform', v || null) })),
          row(t('project.styles.shadow'), select({ options: [{ value: '', label: t('project.styles.default') }, ...shadowNames], value: st.shadow ?? '', onchange: (v) => set('shadow', v || null) })),
          ...effectRows(st, set),
          row(t('hyphen.label'), checkbox({ label: t('hyphen.style_toggle'), checked: !!st.hyphenate, onchange: (v) => set('hyphenate', v || null) }), t('hyphen.hint')),
        ),
      );
    });

    // span styles
    const spans = Object.entries(styles.spans ?? {}).map(([name, st]) => {
      const set = (k, v) => patch({ spans: { [name]: clean({ ...st, [k]: v }) } });
      return styleDetails(
        'span',
        name,
        h('summary', {}, h('span', { class: 'mono' }, `[${name}]`), h('span', { class: 'muted small' }, ` ${[st.font, st.weight, st.color, st.background].filter(Boolean).join(' · ')}`)),
        h(
          'div',
          { class: 'form' },
          row(t('project.styles.name'), h('div', { class: 'inline' }, renameInput('span', name), removeBtn('spans', name))),
          row(t('project.styles.font'), fontPicker({ value: st.font, fonts, emptyLabel: t('project.styles.inherit'), onchange: (v) => set('font', v || null) })),
          row(t('project.styles.weight'), h('div', { class: 'inline' }, select({ options: [{ value: '', label: t('project.styles.inherit') }, ...WEIGHTS.map(String)], value: st.weight ? String(st.weight) : '', onchange: (v) => set('weight', v ? Number(v) : null) }), checkbox({ label: t('project.styles.italic'), checked: st.italic, onchange: (v) => set('italic', v || null) }))),
          row(t('project.styles.color'), withRefs(colorRefInput(st.color, (v) => set('color', v)))),
          row(t('project.styles.background'), withRefs(colorRefInput(st.background, (v) => set('background', v)))),
          st.background ? row(t('project.styles.padding'), spanBoxInputs(st, set), t('project.styles.padding.hint')) : null,
          ...effectRows(st, set),
          row(t('project.styles.shadow'), select({ options: [{ value: '', label: t('project.styles.inherit') }, ...shadowNames], value: st.shadow ?? '', onchange: (v) => set('shadow', v || null) })),
        ),
      );
    });

    // shadows
    const shadows = Object.entries(styles.shadows ?? {}).map(([name, preset]) => {
      const setLayers = (layers) => patch({ shadows: { [name]: { layers } } });
      const layers = preset.layers ?? [];
      return styleDetails(
        'shadow',
        name,
        h('summary', {}, h('span', { class: 'mono' }, name), h('span', { class: 'muted small' }, ` ${layers.length} ${t('project.styles.layers')}`)),
        h(
          'div',
          { class: 'form' },
          row(t('project.styles.name'), h('div', { class: 'inline' }, renameInput('shadow', name), removeBtn('shadows', name))),
          ...layers.map((l, i) =>
            row(
              `${t('project.styles.layer')} ${i + 1}`,
              h(
                'div',
                { class: 'inline wrap' },
                withRefs(colorRefInput(l.color, (v) => v && setLayers(layers.map((x, j) => (j === i ? { ...x, color: v } : x))))),
                ...['blur', 'x', 'y'].map((k) => h('label', { class: 'mini' }, h('span', { class: 'muted small' }, k), input({ type: 'number', value: l[k] ?? 0, cls: 'num', onchange: (v) => v != null && setLayers(layers.map((x, j) => (j === i ? { ...x, [k]: v } : x))) }))),
                h('button', { class: 'icon-btn', type: 'button', title: t('project.styles.remove'), disabled: layers.length <= 1, onclick: () => setLayers(layers.filter((_, j) => j !== i)) }, icon('close')),
              ),
            ),
          ),
          row('', h('button', { class: 'btn btn-small', type: 'button', onclick: () => setLayers([...layers, { color: '#00000033', blur: 20, y: 10 }]) }, icon('plus'), t('project.styles.layer.add'))),
        ),
      );
    });

    return h(
      'div',
      { class: 'ps-styles' },
      datalist,
      h('section', {}, h('div', { class: 'section-title' }, t('project.styles.colors')), colors.length ? colors : h('div', { class: 'placeholder' }, t('project.styles.none')), addRow(t('project.styles.new_name'), (n) => patch({ colors: { [n]: '#808080' } }))),
      h('section', {}, h('div', { class: 'section-title' }, t('project.styles.text')), text.length ? text : h('div', { class: 'placeholder' }, t('project.styles.none')), addRow(t('project.styles.new_name'), (n) => patch({ text: { [n]: { size: 60 } } }))),
      h('section', {}, h('div', { class: 'section-title' }, t('project.styles.spans')), h('p', { class: 'muted small' }, t('project.styles.spans.hint')), spans, addRow(t('project.styles.new_name'), (n) => patch({ spans: { [n]: { weight: 700 } } }))),
      h(
        'section',
        {},
        h('div', { class: 'section-title' }, t('project.styles.shadows')),
        h('p', { class: 'muted small' }, `${t('project.styles.shadows.builtin')}: ${BUILTIN_SHADOWS.join(', ')}`),
        shadows,
        addRow(t('project.styles.new_name'), (n) => patch({ shadows: { [n]: { layers: [{ color: '#00000040', blur: 30, y: 16 }] } } })),
      ),
    );
  }

  /** Padding (x, y) and corner radius of a span's highlight box (lu). */
  function spanBoxInputs(st, set) {
    const pad = st.padding == null ? {} : typeof st.padding === 'number' ? { x: st.padding, y: st.padding } : st.padding;
    const setPad = (k, v) => {
      const n = { x: pad.x ?? 0, y: pad.y ?? 0, [k]: v ?? 0 };
      set('padding', n.x === 0 && n.y === 0 ? null : n.x === n.y ? n.x : n);
    };
    const mini = (label, value, onchange, title) => h('label', { class: 'mini', title }, h('span', { class: 'muted small' }, label), input({ type: 'number', value, step: 1, min: 0, cls: 'num', placeholder: '0', onchange }));
    return h(
      'div',
      { class: 'inline wrap' },
      mini('X', pad.x, (v) => setPad('x', v), t('project.styles.padding.x')),
      mini('Y', pad.y, (v) => setPad('y', v), t('project.styles.padding.y')),
      mini('◜', st.radius, (v) => set('radius', v || null), t('project.styles.radius')),
    );
  }

  function num(value, onchange, unit, step = 1) {
    return h('div', { class: 'inline' }, input({ type: 'number', value, step, cls: 'num', placeholder: t('project.styles.default'), onchange: (v) => onchange(v) }), h('span', { class: 'muted small' }, unit));
  }

  /** Families the text and span styles name (the default is Inter). */
  function usedFamilies() {
    const used = new Set(['Inter']);
    for (const st of [...Object.values(styles.text ?? {}), ...Object.values(styles.spans ?? {})]) if (st.font) used.add(st.font);
    return used;
  }

  /** "100–900 · italic" for a bundled family. */
  function bundledSummary(f) {
    const w = f.weights[0] === f.weights[1] ? `${f.weights[0]}` : `${f.weights[0]}–${f.weights[1]}`;
    return [t(`fonts.category.${f.category}`), w, f.italic ? t('fonts.italic') : null].filter(Boolean).join(' · ');
  }

  const lower = (list) => new Set((list ?? []).map((f) => f.toLowerCase()));

  /** Where a family comes from: bundled, project, stored, this computer, missing. */
  function sourceOf(family) {
    const f = family.toLowerCase();
    if (lower(fonts.bundled).has(f)) return t('fonts.bundled');
    if (lower(fonts.stored).has(f)) return t('fonts.stored');
    if (lower(fonts.project).has(f)) return t('fonts.project');
    if (lower(fonts.system_named).has(f) || lower(fonts.system).has(f) || (fonts.installed ?? []).some((x) => x.family.toLowerCase() === f)) return t('fonts.system_font');
    return t('fonts.missing');
  }

  /** System font state: off, on and used (named / fallback), on but not needed. */
  function systemFontsText() {
    if (!fonts.system_enabled) return t('fonts.system.off');
    const parts = [];
    if (fonts.system_named?.length) parts.push(`${t('fonts.system.named')}: ${fonts.system_named.join(', ')}`);
    if (fonts.system.length) parts.push(`${t('fonts.system.loaded')}: ${fonts.system.join(', ')}`);
    if (!parts.length) parts.push(t('fonts.system.unneeded'));
    parts.push(t('fonts.system.installed').replace('{n}', String(fonts.installed?.length ?? 0)));
    return parts.join(' · ');
  }

  async function fontsPane() {
    const installed = (fonts.installed ?? []).map((f) => f.family);
    const all = [...fonts.bundled, ...fonts.project, ...installed];
    const known = lower(all);
    const settings = styles.fonts ?? {};
    // Rows for the families in use, project fonts and families with settings;
    // every bundled family is listed compactly below.
    const families = [...new Set([...usedFamilies(), ...fonts.project, ...Object.keys(settings)])];
    const listId = 'ps-font-names';
    const datalist = h('datalist', { id: listId }, all.map((f) => h('option', { value: f })));
    const setFallback = (family, list) => commit(() => api.stylesPatch({ fonts: { [family]: list.length ? { fallback: list } : null } }));
    const familyRow = (family) => {
      const list = settings[family]?.fallback ?? [];
      let adding = '';
      const add = input({ placeholder: t('fonts.add_fallback'), cls: 'mono', oninput: (v) => (adding = v.trim()) });
      add.setAttribute('list', listId);
      const go = () => adding && !list.includes(adding) && adding !== family && setFallback(family, [...list, adding]);
      add.addEventListener('keydown', (e) => e.key === 'Enter' && go());
      return h(
        'div',
        { class: 'ps-font-family' },
        h('div', { class: 'inline' }, h('strong', {}, family), h('span', { class: 'muted small' }, ` · ${sourceOf(family)}`)),
        h(
          'div',
          { class: 'inline wrap' },
          h('span', { class: 'muted small' }, t('fonts.fallback')),
          ...list.map((f, i) =>
            h(
              'span',
              { class: `font-chip${known.has(f.toLowerCase()) || fonts.system.includes(f) ? '' : ' missing'}`, title: known.has(f.toLowerCase()) ? `${f} · ${sourceOf(f)}` : t('fonts.missing') },
              f,
              i > 0 ? h('button', { class: 'icon-btn', type: 'button', title: t('fonts.up'), onclick: () => setFallback(family, [...list.slice(0, i - 1), f, list[i - 1], ...list.slice(i + 1)]) }, icon('up')) : null,
              h('button', { class: 'icon-btn', type: 'button', title: t('project.styles.remove'), onclick: () => setFallback(family, list.filter((x) => x !== f)) }, icon('close')),
            ),
          ),
          add,
          h('button', { class: 'btn btn-small', type: 'button', onclick: go }, icon('plus'), t('project.styles.add')),
        ),
      );
    };
    const rescan = h('button', { class: 'btn btn-small', type: 'button', disabled: fonts.system_enabled ? undefined : true, title: t('fonts.system.rescan.hint') }, icon('update'), t('fonts.system.rescan'));
    rescan.addEventListener('click', async () => {
      rescan.disabled = true;
      try {
        await api.fontsRefresh();
        fonts = null;
        await render();
      } catch (e) {
        err.show(e);
        rescan.disabled = false;
      }
    });
    const bundledList = (pred) => (fonts.bundled_info ?? []).filter(pred).map((f) => h('div', { class: 'inline' }, h('span', {}, f.family), h('span', { class: 'muted small' }, ` · ${bundledSummary(f)}`)));
    const stored = await storedFontsPane(async () => {
      fonts = null;
      await render();
    });
    return h(
      'div',
      { class: 'stack' },
      datalist,
      h('div', { class: 'section-title' }, t('fonts.families')),
      h('p', { class: 'muted small' }, t('fonts.fallback.hint')),
      ...families.map(familyRow),
      h('div', { class: 'section-title' }, t('fonts.system')),
      h('p', { class: 'muted small' }, systemFontsText()),
      h('p', { class: 'muted small' }, t('fonts.system.hint')),
      h('div', { class: 'inline' }, rescan),
      stored,
      h('div', { class: 'section-title' }, t('fonts.bundled_list')),
      h('p', { class: 'muted small' }, t('fonts.bundled_list.hint')),
      h('div', { class: 'stack' }, ...bundledList((f) => f.category !== 'script')),
      h('div', { class: 'section-title' }, t('fonts.scripts')),
      h('p', { class: 'muted small' }, t('fonts.scripts.hint')),
      h('div', { class: 'stack' }, ...bundledList((f) => f.category === 'script')),
      h('p', { class: 'muted small' }, t('project.fonts.hint')),
    );
  }

  renderTabs();
  render().then(() => focusAdd && pane.querySelector('.lang-add')?.focus());
  return m;
}

/** Drops null/undefined/'' fields (unset style properties). */
function clean(obj) {
  return Object.fromEntries(Object.entries(obj).filter(([, v]) => v != null && v !== ''));
}
