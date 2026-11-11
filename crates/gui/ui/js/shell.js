// App shell (KONZEPT §9.1): top bar (project, canvas, target, language,
// variant, validation badge, undo/redo), left sidebar tabs, centre view
// with the bottom view switch and zoom controls, right inspector.

import { actions } from './actions.js';
import { createAddMenu } from './add-menu.js';
import { h, icon, picker } from './dom.js';
import { editor } from './editor.js';
import { t } from './i18n.js';
import { panelContext, SIDEBAR_TABS } from './panels/index.js';
import * as inspector from './panels/inspector.js';
import { keyLabel } from './shortcuts.js';
import { currentCanvas, currentTarget, effectiveVariant, store } from './store.js';
import { mountCanvas } from './views/canvas.js';
import { mountGallery } from './views/gallery.js';
import { mountLanguages } from './views/languages.js';
import { mountPages } from './views/pages.js';
import { onViewportChange } from './views/viewport.js';
import { toggleValidation } from './dialogs/validation.js';

const BASE = '__base__';
/** Value of the "+ Add…" entry at the end of the top-bar pickers. */
const ADD = '__add__';

/**
 * `app`: app-level actions from main.js: `{ menu (button element), targets(), exportDialog(),
 * assistant (MCP indicator element), addCanvas(), addTarget(), addLanguage(), addVariant() }`
 * (the last four back the "+ Add…" entries of the top-bar pickers).
 */
export function mountShell(app = {}) {
  const ctx = panelContext();
  /** `canvas`: the canvas view (null in other views); `view`: the mounted view (zoom API). */
  const shell = { canvas: null, view: null };

  // ----- top bar --------------------------------------------------------------
  const topbar = document.getElementById('topbar');
  const name = h('span', { class: 'project-name' });
  /** Picker change: "+ Add…" runs `add` (and restores the shown value), anything else `set`. */
  const choose = (add, set) => (v) => {
    if (v !== ADD) return set(v);
    renderTop();
    return add?.();
  };
  const canvas = picker({ label: t('top.canvas'), onchange: choose(app.addCanvas, (v) => editor.setContext({ canvas: v })) });
  const target = picker({ label: t('top.target'), onchange: choose(app.addTarget, (v) => editor.setContext({ target: v })) });
  const lang = picker({ label: t('top.lang'), onchange: choose(app.addLanguage, (v) => editor.setContext({ lang: v })) });
  const variant = picker({
    label: t('top.variant'),
    onchange: choose(app.addVariant, (v) => editor.setContext({ variant: v === '' ? undefined : v === BASE ? null : v })),
  });
  const addEntry = (key) => ({ value: ADD, label: `+ ${t(key)}` });
  const badge = h('button', { class: 'badge badge-btn', type: 'button', title: t('top.validation'), onclick: () => toggleValidation(badge, shell) });
  const targetsBtn = h('button', { class: 'icon-btn', type: 'button', title: t('project.menu.targets'), onclick: () => app.targets?.() }, icon('targets'));
  const exportBtn = h('button', { class: 'btn btn-small', type: 'button', title: `${t('project.menu.export')} (${keyLabel('mod+e')})`, onclick: () => app.exportDialog?.() }, icon('export'), t('export.button'));
  const addMenu = createAddMenu(() => shell.canvas);
  const undo = h('button', { class: 'icon-btn', type: 'button', onclick: () => actions.undo() }, icon('undo'));
  const redo = h('button', { class: 'icon-btn', type: 'button', onclick: () => actions.redo() }, icon('redo'));
  topbar.append(
    app.menu ?? '',
    h('img', { class: 'logo', src: 'img/logo.svg', alt: '' }),
    name,
    h('span', { class: 'sep' }),
    canvas.el,
    target.el,
    targetsBtn,
    lang.el,
    variant.el,
    h('span', { class: 'sep' }),
    addMenu.el,
    h('span', { class: 'spacer' }),
    app.assistant ?? '',
    exportBtn,
    badge,
    h('span', { class: 'sep' }),
    undo,
    redo,
  );

  function renderTop() {
    const st = store.get();
    const s = st.summary;
    if (!s) return;
    name.textContent = s.name;
    name.title = s.dir;
    canvas.set([...s.canvases.map((x) => ({ value: x.id, label: x.id })), addEntry('top.add.canvas')], st.canvas);
    target.set(
      [...s.targets.filter((x) => x.canvas === st.canvas).map((x) => ({ value: x.id, label: `${x.id}  (${x.size[0]}×${x.size[1]})`, short: x.id })), addEntry('top.add.target')],
      st.target ?? '',
    );
    lang.set(
      [
        ...s.languages.available.map((l) => ({
          value: l,
          label: l === s.languages.default ? `${l} · ${t('top.lang.default')}` : l,
          short: l,
        })),
        addEntry('top.add.lang'),
      ],
      st.lang,
    );
    const tg = currentTarget(st);
    const sp = currentCanvas(st);
    const own = tg?.variant ?? null;
    const options = [{ value: '', label: own ? `${own} · ${t('top.variant.target')}` : `${t('top.variant.base')} · ${t('top.variant.target')}`, short: own ?? t('top.variant.base') }];
    if (own) options.push({ value: BASE, label: t('top.variant.base'), short: t('top.variant.base') });
    for (const v of sp?.variants ?? []) if (v !== own) options.push({ value: v, label: v, short: v, group: t('top.variant.others') });
    const current = st.variant === undefined ? '' : st.variant === null ? (own ? BASE : '') : st.variant;
    options.push(addEntry('top.add.variant'));
    variant.set(options, current);
    variant.value.classList.toggle('variant-active', effectiveVariant(st) != null);
    variant.el.title = effectiveVariant(st) ? t('top.variant.hint') : '';

    const h0 = st.history;
    undo.disabled = !h0.can_undo;
    redo.disabled = !h0.can_redo;
    undo.title = `${t('top.undo')}${h0.undo.length ? ` ${h0.undo[h0.undo.length - 1]}` : ''} (${keyLabel('mod+z')})`;
    redo.title = `${t('top.redo')}${h0.redo.length ? ` ${h0.redo[h0.redo.length - 1]}` : ''} (${keyLabel('mod+shift+z')})`;

    const v = st.validation;
    badge.className = 'badge badge-btn';
    if (!v) {
      badge.replaceChildren(t('top.validation.pending'));
    } else if (v.errors) {
      badge.classList.add('error');
      badge.replaceChildren(icon('error'), String(v.errors), ...(v.warnings ? [h('span', { class: 'badge-sub' }, icon('warning'), String(v.warnings))] : []));
    } else if (v.warnings) {
      badge.classList.add('warn');
      badge.replaceChildren(icon('warning'), String(v.warnings));
    } else {
      badge.classList.add('ok');
      badge.replaceChildren(icon('check'), t('top.validation.ok'));
    }
    badge.title = v ? `${v.errors} ${t('top.validation.errors')}, ${v.warnings} ${t('top.validation.warnings')}` : t('top.validation');
  }
  store.select((st) => [st.summary, st.canvas, st.target, st.lang, st.variant, st.history, st.validation], renderTop);

  // ----- left sidebar -------------------------------------------------------------
  const sidebar = document.getElementById('sidebar');
  const tabs = h('div', { class: 'tabs', role: 'tablist' });
  const panelBody = h('div', { class: 'panel-body' });
  sidebar.append(tabs, panelBody);
  let mounted = null;

  function renderTabs() {
    const current = store.get().sidebarTab;
    tabs.replaceChildren(
      ...SIDEBAR_TABS.map((tab) =>
        h(
          'button',
          {
            type: 'button',
            role: 'tab',
            title: t(tab.title),
            'aria-selected': String(tab.id === current),
            onclick: () => store.set({ sidebarTab: tab.id }),
          },
          icon(tab.icon),
        ),
      ),
    );
    mounted?.unmount?.();
    panelBody.replaceChildren();
    const tab = SIDEBAR_TABS.find((x) => x.id === current) ?? SIDEBAR_TABS[0];
    mounted = tab.module.mount(panelBody, ctx);
  }
  store.select((st) => st.sidebarTab, renderTabs);
  renderTabs();

  // ----- inspector ------------------------------------------------------------------
  inspector.mount(document.getElementById('inspector'), ctx);

  // ----- centre view + bottom bar ---------------------------------------------------
  const viewEl = document.getElementById('view');
  const bottombar = document.getElementById('bottombar');
  const VIEWS = [
    { id: 'canvas', icon: 'canvas', mount: mountCanvas },
    { id: 'pages', icon: 'pages', mount: mountPages },
    { id: 'gallery', icon: 'gallery', mount: mountGallery },
    { id: 'languages', icon: 'languages', mount: mountLanguages },
  ];
  const segmented = h('div', { class: 'segmented', role: 'group', 'aria-label': t('view.switch') });
  const zoomValue = h('button', { class: 'zoom-value', type: 'button', title: `${t('zoom.actual')} (${keyLabel('mod+1')})`, onclick: () => shell.view?.zoomActual() });
  const fitBtn = h('button', { class: 'icon-btn', type: 'button', title: `${t('zoom.fit')} (${keyLabel('mod+0')})`, onclick: () => shell.view?.fit() }, icon('fit'));
  const zoom = h(
    'div',
    { class: 'zoom-controls' },
    h('button', { class: 'icon-btn', type: 'button', title: `${t('zoom.out')} (${keyLabel('mod+-')})`, onclick: () => shell.view?.zoomOut() }, icon('minus')),
    zoomValue,
    h('button', { class: 'icon-btn', type: 'button', title: `${t('zoom.in')} (${keyLabel('mod+=')})`, onclick: () => shell.view?.zoomIn() }, icon('plus')),
    fitBtn,
  );
  const status = h('span', { class: 'status-text' });
  bottombar.append(segmented, h('span', { class: 'spacer' }), status, zoom);

  let current = null;
  function renderView() {
    const id = store.get().view;
    segmented.replaceChildren(
      ...VIEWS.map((v) =>
        h('button', { type: 'button', 'aria-pressed': String(v.id === id), onclick: () => store.set({ view: v.id }) }, icon(v.icon), t(`view.${v.id}`)),
      ),
    );
    current?.unmount();
    viewEl.replaceChildren();
    const def = VIEWS.find((v) => v.id === id) ?? VIEWS[0];
    current = def.mount(viewEl);
    shell.canvas = def.id === 'canvas' ? current : null;
    shell.view = current;
    renderZoom();
  }

  function renderZoom() {
    if (shell.view?.percent) zoomValue.textContent = `${shell.view.percent()} %`;
    // Fit mode (re-fits on resize until the next manual zoom/pan).
    const fitted = Boolean(shell.view?.fitMode);
    fitBtn.setAttribute('aria-pressed', String(fitted));
    fitBtn.title = `${t(fitted ? 'zoom.fit.active' : 'zoom.fit')} (${keyLabel('mod+0')})`;
  }

  function renderStatus() {
    const st = store.get();
    const n = st.selection.length;
    const variantName = effectiveVariant(st);
    const parts = [];
    if (n) parts.push(n === 1 ? st.selection[0] : `${n} ${t('inspector.selected')}`);
    if (variantName) parts.push(`${t('status.variant')}: ${variantName}`);
    status.textContent = parts.join(' · ');
  }

  store.select((st) => st.view, renderView);
  store.select((st) => [st.zoom, st.target, st.overlay], renderZoom);
  onViewportChange(renderZoom);
  store.select((st) => [st.selection, st.variant, st.target], renderStatus);
  renderView();
  renderTop();
  renderStatus();
  return shell;
}
