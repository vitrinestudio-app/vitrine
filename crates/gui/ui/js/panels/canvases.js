// Canvases panel: the project's canvases; choosing one shows it with its first
// target. "New canvas…" and "Manage…" open the canvases & targets dialog
// (create, rename, duplicate, remove, variants); the canvas's own settings
// (pages, backgrounds, variants) are in the inspector with nothing selected.

import { openTargets } from '../dialogs/targets.js';
import { h, icon } from '../dom.js';

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { store, editor, t }) {
  const list = h('ul', { class: 'list', role: 'listbox', 'aria-label': t('panel.canvases') });
  const report = (e) => editor.toast(e?.message ?? String(e), true);
  el.append(
    h('div', { class: 'section-title' }, t('panel.canvases')),
    list,
    h(
      'div',
      { class: 'panel-actions' },
      h('button', { class: 'btn btn-small', type: 'button', onclick: () => openTargets(null, { mode: 'new-canvas' }).catch(report) }, icon('plus'), t('canvases.add')),
      h('button', { class: 'btn btn-small', type: 'button', onclick: () => openTargets(null, { mode: 'canvas', canvas: store.get().canvas }).catch(report) }, t('canvases.manage')),
    ),
  );

  function render() {
    const { summary, canvas } = store.get();
    list.replaceChildren(
      ...(summary?.canvases ?? []).map((s) => {
        const targets = summary.targets.filter((x) => x.canvas === s.id).length;
        return h(
          'li',
          {
            class: 'list-item',
            role: 'option',
            tabindex: '0',
            'aria-selected': String(s.id === canvas),
            onclick: () => editor.setContext({ canvas: s.id }),
            onkeydown: (e) => e.key === 'Enter' && editor.setContext({ canvas: s.id }),
            ondblclick: () => openTargets(null, { mode: 'canvas', canvas: s.id }).catch(report),
          },
          h('span', { class: 'grow' }, s.id),
          h('span', { class: 'muted small' }, `${s.page.count} ${t('panel.canvases.pages')} · ${targets} ${t('panel.canvases.targets')}`),
        );
      }),
    );
  }

  const unsub = store.select((s) => [s.summary, s.canvas], render);
  render();
  return { unmount: () => unsub() };
}
