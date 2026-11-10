// Editor controller: loads the project into the store, runs edit commands
// and tells the views what to re-render. Views never call edit commands
// directly; they call `editor.apply(...)` (or the shortcuts in actions.js)
// so history, overlay, summary and invalidation stay consistent.
//
// Events (`editor.on(name, fn)`):
//   invalidate  { all: true } | { rects: [Rect lu] } – pixels to re-render
//   reset       {}                                    – view context changed
//                                                      (target/variant/lang/strip):
//                                                      rebuild everything
//   highlight   { ids }                               – briefly outline these
//                                                      elements (assistant edits)

import { api } from './api.js';
import { bounds, inflate, union } from './geometry.js';
import { t } from './i18n.js';
import { currentTarget, store, viewArgs } from './store.js';

const listeners = new Map();

function emit(name, payload) {
  for (const fn of listeners.get(name) ?? []) fn(payload);
}

/** Extra lu around changed elements (shadows, anti-aliasing). */
const DIRTY_MARGIN = 40;

/** Canvas rects covered by `ids` in `overlay` (null if one is unknown). */
function rectsOf(overlay, ids) {
  if (!overlay) return null;
  const out = [];
  for (const id of ids) {
    const el = overlay.elements.find((e) => e.id === id);
    if (el) {
      if (el.kind === 'background') return null;
      out.push(bounds(el.quad));
      continue;
    }
    const g = overlay.groups.find((x) => x.id === id);
    if (g) {
      out.push(g.bounds);
      continue;
    }
  }
  return out;
}

let validateTimer = 0;
let overlaySeq = 0;

export const editor = {
  on(name, fn) {
    if (!listeners.has(name)) listeners.set(name, new Set());
    listeners.get(name).add(fn);
    return () => listeners.get(name).delete(fn);
  },

  /** Loads the open project (or returns false when none is open). */
  async load() {
    let summary;
    try {
      summary = await api.summary();
    } catch (e) {
      if (e.code === 'no_project') return false;
      throw e;
    }
    const s = store.get();
    const strip = summary.strips.some((x) => x.id === s.strip) ? s.strip : summary.targets[0]?.strip ?? summary.strips[0]?.id;
    const target = summary.targets.some((x) => x.id === s.target && x.strip === strip)
      ? s.target
      : summary.targets.find((x) => x.strip === strip)?.id ?? null;
    const lang = summary.languages.available.includes(s.lang) ? s.lang : summary.languages.default;
    store.set({ summary, strip, target, lang });
    const [history] = await Promise.all([api.historyState(), this.refreshOverlay()]);
    store.set({ history });
    emit('reset', {});
    this.scheduleValidation();
    return true;
  },

  /** Switches the view context (any of strip, target, lang, variant). */
  async setContext(patch) {
    const s = store.get();
    const next = { ...patch };
    if ('strip' in patch && patch.strip !== s.strip) {
      const target = s.summary.targets.find((x) => x.strip === patch.strip);
      next.target = target?.id ?? null;
      next.variant = undefined;
    }
    if ('target' in patch && patch.target !== s.target && !('variant' in patch)) next.variant = undefined;
    store.set({ ...next, selection: [], hover: null });
    await this.refreshOverlay();
    emit('reset', {});
    this.scheduleValidation();
  },

  /** Fetches the overlay of the current view context. */
  async refreshOverlay() {
    const s = store.get();
    if (!s.target) {
      store.set({ overlay: null });
      return null;
    }
    const seq = ++overlaySeq;
    const overlay = await api.overlay(viewArgs(s));
    if (seq !== overlaySeq) return overlay;
    const ids = new Set([...overlay.elements.map((e) => e.id), ...overlay.groups.map((g) => g.id)]);
    store.set({ overlay, selection: store.get().selection.filter((id) => ids.has(id)) });
    return overlay;
  },

  /**
   * Runs an edit (a promise of an `edit_*`, `undo` or `redo` result) and
   * updates history, overlay, summary and pixels. `touched`: ids the edit
   * is about (their old and new bounds are re-rendered); `select`: ids to
   * select afterwards (`'created'` = the ids the edit created).
   */
  async apply(call, { touched = [], select } = {}) {
    const before = store.get().overlay;
    let result;
    try {
      result = await call;
    } catch (e) {
      this.toast(`${t('error.edit')}: ${e.message}`, true);
      emit('invalidate', { rects: [] });
      return null;
    }
    store.set({ history: result.history });
    if (!result.applied) {
      emit('invalidate', { rects: [] });
      return result;
    }
    const [overlay, summary] = await Promise.all([this.refreshOverlay(), api.summary()]);
    store.set({ summary });
    if (select === 'created' && result.created?.length) store.set({ selection: result.created });
    else if (Array.isArray(select)) store.set({ selection: select });

    const c = result.changes;
    const broad = c.languages || c.styles || c.strings.length || c.targets.length || c.fonts || c.assets?.length;
    const ids = [...new Set([...c.elements, ...touched])];
    let payload = { all: true };
    if (!broad && ids.length) {
      const a = rectsOf(before, ids);
      const b = rectsOf(overlay, ids);
      if (a && b) payload = { rects: [...a, ...b].map((r) => inflate(r, DIRTY_MARGIN + Math.max(r.w, r.h) * 0.05)) };
    } else if (!broad && !c.strips.length) {
      payload = { rects: [] };
    }
    emit('invalidate', payload);
    this.scheduleValidation();
    return result;
  },

  undo() {
    return this.apply(api.undo());
  },

  redo() {
    return this.apply(api.redo());
  },

  /**
   * Shows a step made by someone else through the backend session
   * (`project-edited` with an assistant actor, ADR 0015): same refresh as
   * `apply`, or a full reload when targets, strips or languages changed.
   */
  async remoteEdit(ev) {
    const c = ev.changes ?? {};
    const summary = store.get().summary;
    if (!summary) return null;
    const newStrip = (c.strips ?? []).some((id) => !summary.strips.some((x) => x.id === id));
    if (c.targets?.length || c.languages || newStrip) {
      await this.load();
      return null;
    }
    const history = await api.historyState();
    const changes = { elements: [], strings: [], targets: [], strips: [], ...c };
    return this.apply(Promise.resolve({ applied: true, revision: ev.revision, history, changes }));
  },

  /** Briefly outlines `ids` on the canvas. */
  highlight(ids) {
    if (ids?.length) emit('highlight', { ids });
  },

  /** Reacts to `project-changed` from the backend (external edit, conflict, assets). */
  async externalChange(change) {
    if (change.reason === 'assets') {
      emit('invalidate', { all: true });
      this.scheduleValidation();
      return;
    }
    await this.load();
    if (change.reason === 'conflict') this.toast(t('project.conflict'), true);
  },

  scheduleValidation() {
    clearTimeout(validateTimer);
    validateTimer = setTimeout(async () => {
      const s = store.get();
      if (!currentTarget(s)) return;
      try {
        const r = await api.validate(s.target, s.lang, s.validationPreview !== false);
        store.set({ validation: { errors: r.errors, warnings: r.warnings, infos: r.infos } });
      } catch {
        store.set({ validation: null });
      }
    }, 400);
  },

  /** Short message at the bottom of the window. */
  toast(message, error = false) {
    const el = document.getElementById('toast');
    el.textContent = message;
    el.classList.toggle('error', error);
    el.hidden = false;
    clearTimeout(el._timer);
    el._timer = setTimeout(() => {
      el.hidden = true;
    }, error ? 5000 : 2500);
  },

  /** Union of rects (helper for views). */
  union,
};
