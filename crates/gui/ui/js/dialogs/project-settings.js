// Project settings dialog (task 2.10): name, frame style, languages
// (add/remove/reorder, fallback map, default language with the §5.0
// impact warning), style tokens (colours, text styles, span styles, shadow
// presets) and the fonts available to the project. Every change is one
// backend command building the ops in Rust, applied via `editor.apply`.

import { api } from '../api.js';
import { h, icon } from '../dom.js';
import { editor } from '../editor.js';
import { t } from '../i18n.js';
import { store } from '../store.js';
import { checkbox, confirmDialog, errorLine, input, openModal, row, select } from './modal.js';

const TABS = ['general', 'languages', 'styles', 'fonts'];
const FRAMES = ['detailed', 'stylized'];
const WEIGHTS = [100, 200, 300, 400, 500, 600, 700, 800, 900];
const BUILTIN_SHADOWS = ['soft', 'hard', 'none'];

export function openProjectSettings(tab = 'general') {
  const err = errorLine();
  const tabs = h('div', { class: 'dlg-tabs', role: 'tablist' });
  const pane = h('div', { class: 'ps-pane' });
  let current = tab;
  let styles = null;
  let fonts = null;

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
    if (current === 'general') pane.replaceChildren(general(s));
    else if (current === 'languages') pane.replaceChildren(languages(s));
    else if (current === 'styles') {
      styles ??= await api.stylesGet();
      fonts ??= await api.catalogFonts();
      pane.replaceChildren(stylesPane(s));
    } else {
      fonts ??= await api.catalogFonts();
      pane.replaceChildren(fontsPane());
    }
  }

  // ----- general --------------------------------------------------------------------
  function general(s) {
    return h(
      'div',
      { class: 'form' },
      row(t('project.name'), input({ value: s.name, onchange: (v) => commit(() => api.projectRename(v)) })),
      row(t('project.folder'), h('span', { class: 'mono small selectable' }, s.dir)),
      row(
        t('project.frame'),
        select({ options: FRAMES.map((f) => ({ value: f, label: t(`settings.frame.${f}`) })).concat(FRAMES.includes(s.styles.frame) ? [] : [{ value: s.styles.frame, label: s.styles.frame }]), value: s.styles.frame, onchange: (v) => commit(() => api.stylesPatch({ frame: v })) }),
        t('project.frame.hint'),
      ),
    );
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
    const add = input({ placeholder: t('project.lang.add.placeholder'), cls: 'num-wide', oninput: (v) => (newLang = v.trim()) });
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
    const fontNames = [...fonts.bundled, ...fonts.project];
    const shadowNames = [...new Set([...BUILTIN_SHADOWS, ...Object.keys(styles.shadows ?? {})])];
    const patch = (p) => commit(() => api.stylesPatch(p));
    const renameInput = (kind, name) => input({ value: name, cls: 'style-name mono', onchange: (v) => v && v !== name && patch({ rename: { kind, from: name, to: v } }) });
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

    // text styles
    const text = Object.entries(styles.text ?? {}).map(([name, st]) => {
      const set = (k, v) => patch({ text: { [name]: clean({ ...st, [k]: v }) } });
      return h(
        'details',
        { class: 'ps-style' },
        h('summary', {}, h('span', { class: 'mono' }, name), h('span', { class: 'muted small' }, ` ${st.font ?? 'Inter'} ${st.weight ?? 400} · ${st.size ?? '–'} lu`)),
        h(
          'div',
          { class: 'form' },
          row(t('project.styles.name'), h('div', { class: 'inline' }, renameInput('text', name), removeBtn('text', name))),
          row(t('project.styles.font'), select({ options: [{ value: '', label: t('project.styles.default') }, ...fontNames], value: st.font ?? '', onchange: (v) => set('font', v || null) })),
          row(t('project.styles.weight'), h('div', { class: 'inline' }, select({ options: [{ value: '', label: t('project.styles.default') }, ...WEIGHTS.map(String)], value: st.weight ? String(st.weight) : '', onchange: (v) => set('weight', v ? Number(v) : null) }), checkbox({ label: t('project.styles.italic'), checked: st.italic, onchange: (v) => set('italic', v || null) }))),
          row(t('project.styles.size'), num(st.size, (v) => set('size', v), 'lu')),
          row(t('project.styles.line_height'), num(st.line_height, (v) => set('line_height', v), '×', 0.05)),
          row(t('project.styles.tracking'), num(st.tracking, (v) => set('tracking', v), 'em', 0.01)),
          row(t('project.styles.color'), withRefs(colorRefInput(st.color, (v) => set('color', v)))),
          row(t('project.styles.transform'), select({ options: [{ value: '', label: t('project.styles.default') }, ...['none', 'uppercase', 'lowercase'].map((v) => ({ value: v, label: t(`project.styles.transform.${v}`) }))], value: st.transform ?? '', onchange: (v) => set('transform', v || null) })),
          row(t('project.styles.shadow'), select({ options: [{ value: '', label: t('project.styles.default') }, ...shadowNames], value: st.shadow ?? '', onchange: (v) => set('shadow', v || null) })),
        ),
      );
    });

    // span styles
    const spans = Object.entries(styles.spans ?? {}).map(([name, st]) => {
      const set = (k, v) => patch({ spans: { [name]: clean({ ...st, [k]: v }) } });
      return h(
        'details',
        { class: 'ps-style' },
        h('summary', {}, h('span', { class: 'mono' }, `[${name}]`), h('span', { class: 'muted small' }, ` ${[st.font, st.weight, st.color, st.background].filter(Boolean).join(' · ')}`)),
        h(
          'div',
          { class: 'form' },
          row(t('project.styles.name'), h('div', { class: 'inline' }, renameInput('span', name), removeBtn('spans', name))),
          row(t('project.styles.font'), select({ options: [{ value: '', label: t('project.styles.inherit') }, ...fontNames], value: st.font ?? '', onchange: (v) => set('font', v || null) })),
          row(t('project.styles.weight'), h('div', { class: 'inline' }, select({ options: [{ value: '', label: t('project.styles.inherit') }, ...WEIGHTS.map(String)], value: st.weight ? String(st.weight) : '', onchange: (v) => set('weight', v ? Number(v) : null) }), checkbox({ label: t('project.styles.italic'), checked: st.italic, onchange: (v) => set('italic', v || null) }))),
          row(t('project.styles.color'), withRefs(colorRefInput(st.color, (v) => set('color', v)))),
          row(t('project.styles.background'), withRefs(colorRefInput(st.background, (v) => set('background', v)))),
        ),
      );
    });

    // shadows
    const shadows = Object.entries(styles.shadows ?? {}).map(([name, preset]) => {
      const setLayers = (layers) => patch({ shadows: { [name]: { layers } } });
      const layers = preset.layers ?? [];
      return h(
        'details',
        { class: 'ps-style' },
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

  function num(value, onchange, unit, step = 1) {
    return h('div', { class: 'inline' }, input({ type: 'number', value, step, cls: 'num', placeholder: t('project.styles.default'), onchange: (v) => onchange(v) }), h('span', { class: 'muted small' }, unit));
  }

  function fontsPane() {
    return h(
      'div',
      { class: 'stack' },
      h('div', { class: 'section-title' }, t('project.fonts.bundled')),
      h('ul', { class: 'ps-fonts' }, fonts.bundled.map((f) => h('li', {}, f))),
      h('div', { class: 'section-title' }, t('project.fonts.project')),
      fonts.project.length ? h('ul', { class: 'ps-fonts' }, fonts.project.map((f) => h('li', {}, f))) : h('div', { class: 'placeholder' }, t('project.fonts.none')),
      h('p', { class: 'muted small' }, t('project.fonts.hint')),
    );
  }

  renderTabs();
  render();
  return m;
}

/** Drops null/undefined/'' fields (unset style properties). */
function clean(obj) {
  return Object.fromEntries(Object.entries(obj).filter(([, v]) => v != null && v !== ''));
}
