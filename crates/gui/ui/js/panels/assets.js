// Assets panel – placeholder until task 2.8. See ui/README.md for the
// panel contract (`mount(el, ctx)` → `{ unmount }`).

import { h } from '../dom.js';

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { t }) {
  el.append(h('div', { class: 'section-title' }, t('panel.assets')), h('p', { class: 'placeholder' }, t('panel.soon')));
  return { unmount() {} };
}
