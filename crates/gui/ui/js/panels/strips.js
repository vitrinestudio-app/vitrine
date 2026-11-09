// Strips panel: the project's strips; choosing one shows it with its first
// target. (Strip settings follow in task 2.6.)

import { h } from '../dom.js';

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { store, editor, t }) {
  const list = h('ul', { class: 'list', role: 'listbox', 'aria-label': t('panel.strips') });
  el.append(h('div', { class: 'section-title' }, t('panel.strips')), list);

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
