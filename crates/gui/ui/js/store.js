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
  /** Current canvas id. */
  canvas: null,
  /** Current target id. */
  target: null,
  /** Current content language. */
  lang: null,
  /**
   * Variant shown and edited: `undefined` = the target's own variant,
   * `null` = the base elements, a string = that variant of the canvas.
   */
  variant: undefined,
  /** `canvas` | `pages` | `gallery` | `languages`. */
  view: 'canvas',
  /** Selected element ids (groups by their id). */
  selection: [],
  /** Element id under the pointer. */
  hover: null,
  /** Device id under a dragged screenshot / file (drop highlight). */
  dropTarget: null,
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
  /** Validation in preview mode (missing screenshots are warnings). */
  validationPreview: true,
  /** Left sidebar tab: `canvases` | `layers` | `assets` | `translations`. */
  sidebarTab: 'canvases',
  /** Short status line (bottom bar). */
  status: '',
  /** Inspector toggle "only for <lang>": text edits go into the language override. */
  langScope: false,
  /** Elements on the element clipboard (`edit_copy`/`edit_cut`; 0 = empty). */
  clipboard: 0,
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

/**
 * View context `{target, lang, variant, scope?}` for commands (variant
 * omitted = target's own; `scope: 'lang'` = text edits go into the
 * language override of `lang`).
 */
export function viewArgs(state = store.get()) {
  const args = { target: state.target, lang: state.lang };
  if (state.variant !== undefined) args.variant = state.variant;
  if (state.langScope) args.scope = 'lang';
  return args;
}

/** Target summary of the current target. */
export function currentTarget(state = store.get()) {
  return state.summary?.targets.find((t) => t.id === state.target) ?? null;
}

/** Canvas summary of the current canvas. */
export function currentCanvas(state = store.get()) {
  return state.summary?.canvases.find((s) => s.id === state.canvas) ?? null;
}

/** The variant actually shown (resolves `undefined` to the target's own). */
export function effectiveVariant(state = store.get()) {
  if (state.variant !== undefined) return state.variant;
  return currentTarget(state)?.variant ?? null;
}
