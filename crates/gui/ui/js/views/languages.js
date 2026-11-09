// "All languages" view (one page in every language, overflows marked).
// Placeholder until task 2.7.

import { h } from '../dom.js';
import { t } from '../i18n.js';

export function mountLanguages(root) {
  const el = h('div', { class: 'canvas-empty' }, t('view.languages.soon'));
  root.append(el);
  return { unmount: () => el.remove() };
}
