// Tiny state store: one plain state object, shallow `set`, and
// subscriptions with selectors. No framework, no proxies.
//
//   store.get()                       → current state (treat as read-only)
//   store.set({ selection: ['a'] })   → shallow merge, notifies subscribers
//   store.subscribe(fn)               → fn(state, prev) on every change
//   store.select(sel, fn)             → fn(sel(state), sel(prev)) when the
//                                       selected value changes (=== or
//                                       shallow array equality)
// Every subscribe/select returns an unsubscribe function.

/** The state shape used by the editor (documented defaults). */
export const initialState = {
  /** `project_summary` result (null = no project open). */
  summary: null,
  /** Current strip id. */
  strip: null,
  /** Current target id. */
  target: null,
  /** Current content language. */
  lang: null,
  /**
   * Variant shown and edited: `undefined` = the target's own variant,
   * `null` = the base elements, a string = that variant of the strip.
   */
  variant: undefined,
  /** `canvas` | `pages` | `gallery` | `languages`. */
  view: 'canvas',
  /** Selected element ids (groups by their id). */
  selection: [],
  /** Element id under the pointer. */
  hover: null,
  /** Canvas zoom (screen px per lu) and pan (screen px of canvas lu 0,0). */
  zoom: 0.25,
  panX: 0,
  panY: 0,
  /** `overlay` result for the current view context. */
  overlay: null,
  /** `{ can_undo, can_redo, undo: [labels], redo: [labels] }`. */
  history: { can_undo: false, can_redo: false, undo: [], redo: [] },
  /** `{ errors, warnings, infos }` of the current target × language, or null. */
  validation: null,
  /** Left sidebar tab: `strips` | `layers` | `assets` | `translations`. */
  sidebarTab: 'strips',
  /** Short status line (bottom bar). */
  status: '',
  /** Element ids whose home page is locked while dragging (UI only, task 2.6). */
  pageLocks: [],
};

function same(a, b) {
  if (a === b) return true;
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((v, i) => v === b[i]);
  }
  return false;
}

export function createStore(initial = initialState) {
  let state = { ...initial };
  const listeners = new Set();

  return {
    get: () => state,
    set(patch) {
      const prev = state;
      const next = { ...state, ...(typeof patch === 'function' ? patch(state) : patch) };
      if (Object.keys(next).every((k) => same(next[k], prev[k]))) return;
      state = next;
      for (const fn of [...listeners]) fn(state, prev);
    },
    subscribe(fn) {
      listeners.add(fn);
      return () => listeners.delete(fn);
    },
    select(selector, fn) {
      const listener = (s, prev) => {
        const a = selector(s);
        const b = selector(prev);
        if (!same(a, b)) fn(a, b);
      };
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

/** The app-wide store. */
export const store = createStore();

/** View context `{target, lang, variant}` for commands (variant omitted = target's own). */
export function viewArgs(state = store.get()) {
  const args = { target: state.target, lang: state.lang };
  if (state.variant !== undefined) args.variant = state.variant;
  return args;
}

/** Target summary of the current target. */
export function currentTarget(state = store.get()) {
  return state.summary?.targets.find((t) => t.id === state.target) ?? null;
}

/** Strip summary of the current strip. */
export function currentStrip(state = store.get()) {
  return state.summary?.strips.find((s) => s.id === state.strip) ?? null;
}

/** The variant actually shown (resolves `undefined` to the target's own). */
export function effectiveVariant(state = store.get()) {
  if (state.variant !== undefined) return state.variant;
  return currentTarget(state)?.variant ?? null;
}
