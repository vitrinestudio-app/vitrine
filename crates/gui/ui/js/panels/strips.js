// Strips panel: the project's strips; choosing one shows it with its first
// target. "New strip…" and "Manage…" open the strips & targets dialog
// (create, rename, duplicate, remove, variants); the strip's own settings
// (pages, backgrounds, variants) are in the inspector with nothing selected.

import { openTargets } from '../dialogs/targets.js';
import { h, icon } from '../dom.js';

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { store, editor, t }) {
  const list = h('ul', { class: 'list', role: 'listbox', 'aria-label': t('panel.strips') });
  const report = (e) => editor.toast(e?.message ?? String(e), true);
  el.append(
    h('div', { class: 'section-title' }, t('panel.strips')),
    list,
    h(
      'div',
      { class: 'panel-actions' },
      h('button', { class: 'btn btn-small', type: 'button', onclick: () => openTargets(null, { mode: 'new-strip' }).catch(report) }, icon('plus'), t('strips.add')),
      h('button', { class: 'btn btn-small', type: 'button', onclick: () => openTargets(null, { mode: 'strip', strip: store.get().strip }).catch(report) }, t('strips.manage')),
    ),
  );

  function render() {
    const { summary, strip } = store.get();
    list.replaceChildren(
      ...(summary?.strips ?? []).map((s) => {
        const targets = summary.targets.filter((x) => x.strip === s.id).length;
        return h(
          'li',
          {
            class: 'list-item',
            role: 'option',
            tabindex: '0',
            'aria-selected': String(s.id === strip),
            onclick: () => editor.setContext({ strip: s.id }),
            onkeydown: (e) => e.key === 'Enter' && editor.setContext({ strip: s.id }),
            ondblclick: () => openTargets(null, { mode: 'strip', strip: s.id }).catch(report),
          },
          h('span', { class: 'grow' }, s.id),
          h('span', { class: 'muted small' }, `${s.page.count} ${t('panel.strips.pages')} · ${targets} ${t('panel.strips.targets')}`),
        );
      }),
    );
  }

  const unsub = store.select((s) => [s.summary, s.strip], render);
  render();
  return { unmount: () => unsub() };
}
