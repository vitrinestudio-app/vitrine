// Editing actions on the current selection, shared by shortcuts, menus and
// panels. Each one is a single undo step (or nothing).

import { api } from './api.js';
import { editor } from './editor.js';
import { store, viewArgs } from './store.js';

const selection = () => store.get().selection;

export const actions = {
  undo: () => editor.undo(),
  redo: () => editor.redo(),

  remove() {
    const ids = selection();
    if (ids.length) return editor.apply(api.remove(viewArgs(), ids), { touched: ids, select: [] });
  },

  duplicate() {
    const ids = selection();
    if (ids.length) return editor.apply(api.duplicate(viewArgs(), ids), { touched: ids, select: 'created' });
  },

  group() {
    const ids = selection();
    if (ids.length) return editor.apply(api.group(viewArgs(), ids), { touched: ids, select: 'created' });
  },

  ungroup() {
    const o = store.get().overlay;
    const groups = selection().filter((id) => o?.groups.some((g) => g.id === id));
    if (!groups.length) return;
    const members = new Set();
    for (const id of groups) {
      // Direct children: members whose innermost enclosing group path ends with this group.
      for (const e of o.elements) {
        const i = e.groups?.indexOf(id) ?? -1;
        if (i >= 0) members.add(e.groups[i + 1] ?? e.id);
      }
    }
    return editor.apply(api.ungroup(viewArgs(), groups), { touched: groups, select: [...members] });
  },

  /** `order`: front | back | forward | backward. */
  reorder(order) {
    const ids = selection();
    if (ids.length) return editor.apply(api.reorder(viewArgs(), ids, order), { touched: ids });
  },

  selectAll() {
    const o = store.get().overlay;
    if (!o) return;
    const ids = new Set();
    for (const e of o.elements) if (e.kind !== 'background') ids.add(e.groups?.[0] ?? e.id);
    store.set({ selection: [...ids] });
  },

  clearSelection: () => store.set({ selection: [] }),
};
