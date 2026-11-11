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
    for (const e of o.elements) if (e.kind !== 'background' && !isLocked(o, e)) ids.add(e.groups?.[0] ?? e.id);
    store.set({ selection: [...ids] });
  },

  clearSelection: () => store.set({ selection: [] }),

  /** Copies the selection to the element clipboard (Rust side; no undo step). */
  async copy() {
    const ids = selection();
    if (!ids.length) return;
    try {
      const r = await api.copy(viewArgs(), ids);
      store.set({ clipboard: r.count });
    } catch (e) {
      editor.toast(e.message, true);
    }
  },

  /** Copy + delete in one undo step. */
  async cut() {
    const ids = selection();
    if (!ids.length) return;
    const r = await editor.apply(api.cut(viewArgs(), ids), { touched: ids, select: [] });
    if (r?.applied) store.set({ clipboard: ids.length });
  },

  /** Pastes the clipboard (onto home page `page` if given); selects the copies. */
  paste(page) {
    if (!store.get().clipboard) return null;
    return editor.apply(api.paste(viewArgs(), page), { select: 'created' });
  },

  /** Locks (`locked: true`) or unlocks the selection (`ids` default: the selection). */
  lock(on, ids = selection()) {
    if (ids.length) return editor.apply(api.elementPatch(viewArgs(), ids, { locked: on ? true : null }), { touched: ids });
  },

  /** Hides (`visible: false`, an override in a variant view) or shows the selection. */
  hide(on, ids = selection()) {
    if (ids.length) return editor.apply(api.elementPatch(viewArgs(), ids, { visible: on ? false : null }), { touched: ids, select: on ? [] : ids });
  },

  /** Selects the top-level elements whose home page is `page` (no backgrounds, nothing locked). */
  selectAllOnPage(page) {
    const o = store.get().overlay;
    if (!o) return;
    const ids = new Set();
    for (const e of o.elements) {
      if (e.kind === 'background' || e.page !== page || isLocked(o, e)) continue;
      ids.add(e.groups?.[0] ?? e.id);
    }
    store.set({ selection: [...ids] });
  },
};

/** Is overlay element `e` locked itself or inside a locked group? */
export function isLocked(o, e) {
  if (e.locked) return true;
  return (e.groups ?? []).some((g) => o.groups.find((x) => x.id === g)?.locked);
}

/** Is the id (element or group) locked, or inside a locked group? */
export function idLocked(o, id) {
  const e = o?.elements.find((x) => x.id === id);
  if (e) return isLocked(o, e);
  const g = o?.groups.find((x) => x.id === id);
  return Boolean(g && (g.locked || (g.groups ?? []).some((p) => o.groups.find((x) => x.id === p)?.locked)));
}
