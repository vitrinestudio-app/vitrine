// "Add" button of the top bar: Device / Text / Image / Shape / Background.
// The element is built in Rust (`element_add`, defaults per target) and
// centred on the page in the middle of the visible canvas (else the page
// of the selection, else the first page); the new element is selected.

import { api } from './api.js';
import { h, icon } from './dom.js';
import { editor } from './editor.js';
import { t } from './i18n.js';
import { store, viewArgs } from './store.js';

const KINDS = ['device', 'text', 'image', 'shape', 'background'];

/** `getCanvas()`: the mounted canvas view or null (other views). */
export function createAddMenu(getCanvas) {
  const button = h('button', { class: 'btn btn-small add-btn', type: 'button', title: t('add.title'), 'aria-haspopup': 'menu' }, icon('plus'), t('add.button'));
  let popup = null;

  function close() {
    popup?.remove();
    popup = null;
    document.removeEventListener('mousedown', onDown, true);
    document.removeEventListener('keydown', onKey, true);
  }
  function onDown(e) {
    if (!popup?.contains(e.target) && !button.contains(e.target)) close();
  }
  function onKey(e) {
    if (e.key === 'Escape') {
      e.stopPropagation();
      close();
    }
  }
  function page() {
    const visible = getCanvas()?.visiblePage?.();
    if (visible != null) return visible;
    const st = store.get();
    const sel = st.overlay?.elements?.find((e) => e.id === st.selection[0]);
    return sel?.page ?? 0;
  }
  async function add(kind) {
    const r = await editor.apply(api.elementAdd(viewArgs(), kind, page()), { select: 'created' });
    if (r?.applied && store.get().view !== 'canvas') store.set({ view: 'canvas' });
  }
  button.addEventListener('click', () => {
    if (popup) return close();
    popup = h(
      'div',
      { class: 'popover menu', role: 'menu' },
      KINDS.map((kind) =>
        h(
          'button',
          {
            type: 'button',
            role: 'menuitem',
            class: 'menu-item',
            onclick: () => {
              close();
              add(kind);
            },
          },
          icon(kind),
          h('span', { class: 'grow' }, t(`add.${kind}`)),
        ),
      ),
    );
    document.body.append(popup);
    const r = button.getBoundingClientRect();
    popup.style.top = `${r.bottom + 4}px`;
    popup.style.left = `${r.left}px`;
    document.addEventListener('mousedown', onDown, true);
    document.addEventListener('keydown', onKey, true);
  });
  return { el: button, close };
}
