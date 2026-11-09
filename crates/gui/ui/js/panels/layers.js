// Layers panel – placeholder until task 2.6. See ui/README.md for the
// panel contract (`mount(el, ctx)` → `{ unmount }`).

import { h } from '../dom.js';

/** @param {HTMLElement} el  @param {import('./index.js').PanelContext} ctx */
export function mount(el, { t }) {
  el.append(h('div', { class: 'section-title' }, t('panel.layers')), h('p', { class: 'placeholder' }, t('panel.soon')));
  return { unmount() {} };
}
