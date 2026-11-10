// Typed wrappers over the transport (`invoke`). Every backend command the UI
// uses goes through here, so argument shapes live in one place. JSDoc types
// mirror the Rust serde types in crates/gui/src/commands.
//
// `view` is the view context `{ target, lang, variant? }` from
// `viewArgs()` in store.js (variant omitted = the target's own variant,
// `null` = base elements).

import { invoke, listen, toImageBitmap } from './transport.js';

export { listen, toImageBitmap };

/**
 * @typedef {{x:number,y:number}} Vec2
 * @typedef {{x:number,y:number,w:number,h:number}} Rect
 * @typedef {{applied:boolean, revision:number, changes:object, history:object, created?:string[]}} EditResult
 * @typedef {{width:number,height:number,format:string,originX:number,originY:number,scale:number,data:ArrayBuffer}} Preview
 */

export const api = {
  appInfo: () => invoke('app_info'),
  summary: () => invoke('project_summary'),
  openDialog: () => invoke('dialog_open_project'),
  historyState: () => invoke('history_state'),
  undo: () => invoke('undo'),
  redo: () => invoke('redo'),
  /** @returns {Promise<object>} validation counts + findings */
  validate: (target, lang, preview = true) => invoke('validate', { targets: [target], langs: [lang], preview }),

  // ----- project, targets, settings, export (tasks 2.9/2.10) -----
  /** Full validation report (all targets × languages unless filtered). */
  validateAll: ({ targets, langs, preview = true } = {}) => invoke('validate', { targets, langs, preview }),
  projectNew: (args) => invoke('project_new', args),
  projectOpen: (dir) => invoke('project_open', { dir }),
  projectClose: () => invoke('project_close'),
  recentProjects: () => invoke('recent_projects'),
  recentRemove: (path) => invoke('recent_remove', { path }),
  pickFolder: (title) => invoke('dialog_pick_folder', { title }),
  revealPath: (path) => invoke('reveal_path', { path }),
  settingsGet: () => invoke('settings_get'),
  settingsSet: (settings) => invoke('settings_set', { settings }),
  catalogTargets: () => invoke('catalog_targets'),
  catalogGalleries: () => invoke('catalog_galleries'),
  catalogDevices: () => invoke('catalog_devices'),
  catalogFonts: () => invoke('catalog_fonts'),
  targetGet: (id) => invoke('target_get', { id }),
  targetAddFromPreset: (preset, strip, opts = {}) => invoke('target_add_from_preset', { preset, strip, ...opts }),
  /** `patch`: TargetPatch fields (null resets); `rename`: new id. */
  targetPatch: (id, patch = {}, rename) => invoke('target_patch', { id, patch, ...(rename ? { rename } : {}) }),
  targetRemove: (id) => invoke('target_remove', { id }),
  projectRename: (name) => invoke('project_rename', { name }),
  languageAdd: (lang) => invoke('language_add', { lang }),
  languageRemove: (lang) => invoke('language_remove', { lang }),
  languagesSet: ({ order, fallback }) => invoke('languages_set', { order, fallback }),
  defaultLanguageImpact: (lang) => invoke('default_language_impact', { lang }),
  defaultLanguageSet: (lang) => invoke('default_language_set', { lang }),
  stylesGet: () => invoke('styles_get'),
  /** `{colors?, text?, spans?, shadows?: {name: value|null}, frame?, rename?: {kind, from, to}}`. */
  stylesPatch: (patch) => invoke('styles_patch', patch),
  export: (args) => invoke('export', args),
  exportCancel: () => invoke('export_cancel'),

  /** Overlay (hit-test) data of the view. */
  overlay: (view) => invoke('overlay', view),

  // ----- rendering (resolve to a Preview) -----
  /** Output image `image` at `width` px. */
  renderImage: (view, image, width, opts = {}) => invoke('render_image', { ...view, image, width, ...opts }),
  /** Canvas `region` (lu) at `scale` px per lu. */
  renderRegion: (view, region, scale, opts = {}) =>
    invoke('render_canvas', { ...view, region, scale, overlay: false, ...opts }),
  renderGallery: (view, height) => invoke('render_gallery', { ...view, height }),
  /**
   * A gesture in progress (`edit`: {kind:'move_by'|'move'|'resize'|'rotate', …});
   * `interactive` (default true) renders at half `width`.
   */
  renderPreviewEdit: (view, edit, image, width, interactive = true) =>
    invoke('render_preview_edit', { ...view, edit, image, width, interactive }),

  // ----- edit intents (one undo step each; resolve to EditResult) -----
  move: (view, id, x, y, keepPage = false) => invoke('edit_move', { ...view, id, x, y, keep_page: keepPage }),
  moveBy: (view, ids, dx, dy, keepPage = false) => invoke('edit_move_by', { ...view, ids, dx, dy, keep_page: keepPage }),
  /** `fixed`: handle name (`nw`, `n`, … `center`) that stays in place. */
  resize: (view, id, { width, height }, fixed) => invoke('edit_resize', { ...view, id, width, height, fixed }),
  rotate: (view, id, degrees) => invoke('edit_rotate', { ...view, id, degrees }),
  setPage: (view, id, page, keepVisualPosition = true) =>
    invoke('edit_set_page', { ...view, id, page, keep_visual_position: keepVisualPosition }),
  /** `order`: `front` | `back` | `forward` | `backward`. */
  reorder: (view, ids, order) => invoke('edit_reorder', { ...view, ids, order }),
  duplicate: (view, ids, dx = 40, dy = 40) => invoke('edit_duplicate', { ...view, ids, dx, dy }),
  remove: (view, ids) => invoke('edit_remove', { ...view, ids }),
  group: (view, ids) => invoke('edit_group', { ...view, ids }),
  ungroup: (view, ids) => invoke('edit_ungroup', { ...view, ids }),
  resetOverride: (view, id, field) => invoke('edit_reset_override', { ...view, id, field }),

  // ----- inspector, layers, strip settings (task 2.6) -----
  /** One element: `{ base, override, effective, overridden, text, … }`. */
  elementGet: (view, id) => invoke('element_get', { ...view, id }),
  /**
   * Sets fields of `ids` (string or array; YAML names, `null` = default);
   * core writes the base or the view's variant override. `strings`:
   * `[{ lang, key, value }]` set in the same undo step.
   */
  elementPatch: (view, ids, patch, strings = []) =>
    invoke('element_patch', { ...view, ids: [ids].flat(), patch, strings }),
  rename: (id, to) => invoke('edit_rename', { id, to }),
  /** Into group `parent` (null = top level) at `index` (null = top). */
  arrange: (view, id, parent, index) => invoke('edit_arrange', { ...view, id, parent, index }),
  stripGet: (strip) => invoke('strip_get', { strip }),
  /** `patch`: any of `{ aspect: 'w:h', count, ids: [..], backgrounds: [fill] }`. */
  stripPatch: (strip, patch) => invoke('strip_patch', { strip, ...patch }),
  /** `change`: `{ action: 'add'|'remove'|'rename'|'scale', id, to?, content_scale? }`. */
  stripVariant: (strip, change) => invoke('strip_variant', { strip, change }),
  catalogDevices: () => invoke('catalog_devices'),
  catalogTargets: () => invoke('catalog_targets'),

  // ----- texts + translations (2.7) -----
  /** Raw ops as one undo step (`label`: history label). */
  applyOps: (ops, label) => invoke('apply_ops', label ? { ops, label } : { ops }),
  /** Sets (`value` string) or removes (`null`) a translation: one undo step. */
  setString: (lang, key, value, label) => invoke('apply_ops', { ops: [{ op: 'set_string', lang, key, value }], ...(label ? { label } : {}) }),
  /** Translation table; `lang` limits it to one language (keys only). */
  stringsGet: (lang) => invoke('strings_get', lang ? { lang } : {}),
  stringsMissing: () => invoke('strings_missing'),
  /** Validation of all targets (or `targets`) in `langs` (null = all). */
  validateMany: (targets = null, langs = null) => invoke('validate', { targets, langs, preview: true }),
  /** The resolved element in the view (+ own string, markup diagnostics). */
  elementInfo: (view, id) => invoke('element_info', { ...view, id }),
  markupCheck: (text) => invoke('markup_check', { text }),
  /** Output image with translation `key` set to `value` (uncommitted). */
  renderPreviewString: (view, key, value, image, width, interactive = true) =>
    invoke('render_preview_string', { ...view, key, value, image, width, interactive }),

  // ----- assets (2.8) -----
  assetsList: () => invoke('assets_list'),
  /** `kind`: screenshot | asset | font; opts: { lang, variant, key, replace }. */
  assetsImport: (paths, kind, opts = {}) => invoke('assets_import', { paths, kind, ...opts }),
  assetsUpload: (name, data, kind, opts = {}) => invoke('assets_upload', { name, data, kind, ...opts }),
  assetsProbe: ({ paths = [], sizes = [] }) => invoke('assets_probe', { paths, sizes }),
  assetsRemove: (paths) => invoke('assets_remove', { paths }),
  assetThumbnail: (path, maxEdge) => invoke('asset_thumbnail', { path, max_edge: maxEdge }),
  pickFiles: (args = {}) => invoke('dialog_pick_files', args),
  catalogFonts: () => invoke('catalog_fonts'),
};
