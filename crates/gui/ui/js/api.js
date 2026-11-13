// Typed wrappers over the transport (`invoke`). Every backend command the UI
// uses goes through here, so argument shapes live in one place. JSDoc types
// mirror the Rust serde types in crates/app/src/commands.
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
  /** Tells the backend what the user looks at (`{canvas, target, lang, variant?, selection}`; read by MCP `view_get`). */
  viewSet: (view) => invoke('view_set', view),
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
  // ----- AI assistants / MCP (ADR 0015) -----
  /** In-app MCP endpoint: `{hosted, enabled, running, url, port, clients, token?, generic, claude_code, cli_path, …}`. */
  mcpStatus: () => invoke('mcp_status'),
  mcpSetEnabled: (enabled) => invoke('mcp_set_enabled', { enabled }),
  /** What "Add to Claude Desktop" would write (`{config_path, state, entry, preview, cli_path, …}`). */
  claudeDesktopPlan: () => invoke('claude_desktop_plan'),
  /** Writes the entry (only after the user confirmed). */
  claudeDesktopInstall: () => invoke('claude_desktop_install', { confirm: true }),
  settingsSet: (settings) => invoke('settings_set', { settings }),
  // ----- store accounts & project store mapping (Phase 5.1, ADR 0016) -----
  /** `{accounts, keychain: {service, backend, error?}}` – never any key material. */
  storeAccountsList: () => invoke('store_accounts_list'),
  /** `{kind, path, label?, issuer_id?, key_id?}`: validates the key file, stores it in the keychain. */
  storeAccountImport: (args) => invoke('store_account_import', args),
  storeAccountRemove: (id) => invoke('store_account_remove', { id }),
  storeAccountTest: (id) => invoke('store_account_test', { id }),
  /** Apple apps visible to the account's key `{account, apps: [{id, name, bundle_id, sku?, primary_locale?}], cached}` (cached per session; `refresh` asks again). */
  storeAppsList: (account, refresh = false) => invoke('store_apps_list', { account, refresh }),
  /** Play package check (temporary edit, deleted again) → `{package_name, title?, default_language?}`. */
  storePackageCheck: (account, packageName) => invoke('store_package_check', { account, package_name: packageName }),
  /** Opens an allow-listed help link in the system browser. */
  openUrl: (url) => invoke('open_url', { url }),
  // ----- command line tool + updates (ADR 0026) -----
  /** `{method, cli_path, cli_exists, installed, location?, conflict, note?}` */
  cliInstallStatus: () => invoke('cli_install_status'),
  /** Installs (or `remove`s) `vitrine` for terminals; the user confirmed. */
  cliInstall: (remove = false) => invoke('cli_install', { confirm: true, remove }),
  /** GUI-only: `{current_version, mode, reason, auto_check, checked_at, releases_url}`. */
  updateStatus: () => invoke('update_status'),
  /** GUI-only: `{checked, mode, reason, update?}`; never installs. */
  updateCheck: (auto = false) => invoke('update_check', { auto }),
  /** GUI-only: installs the checked update and restarts (after the user confirmed). */
  updateInstall: () => invoke('update_install'),
  /** `{store, effective, locales, accounts, missing_accounts}`. */
  projectStoreGet: () => invoke('project_store_get'),
  /** `{apple?: obj|null, google?: obj|null, locales?: {apple?, google?}}` → one undo step. */
  projectStorePatch: (patch) => invoke('project_store_patch', patch),
  catalogTargets: () => invoke('catalog_targets'),
  catalogGalleries: () => invoke('catalog_galleries'),
  catalogDevices: () => invoke('catalog_devices'),
  catalogFonts: () => invoke('catalog_fonts'),
  targetGet: (id) => invoke('target_get', { id }),
  targetAddFromPreset: (preset, canvas, opts = {}) => invoke('target_add_from_preset', { preset, canvas, ...opts }),
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
  // ----- store upload (Phase 5.5, ADR 0018) -----
  /** Dry run `{store?, targets?, langs?, remote?, force?}` → plan with `confirm_token`. */
  uploadPlan: (args) => invoke('upload_plan', args),
  /** Setup checklist `{ready, steps: [{code, store?, message, fix, blocking}]}` (no rendering, no network). */
  uploadReadiness: (store = null) => invoke('upload_readiness', store ? { store } : {}),
  /** Starts the run of a planned upload (returns at once; progress via `upload-progress`). */
  uploadExecute: (token, opts = {}) => invoke('upload_execute', { token, ...opts }),
  /** `{run, pending_confirmations}`. */
  uploadStatus: () => invoke('upload_status'),
  uploadCancel: () => invoke('upload_cancel'),
  uploadLog: (args = {}) => invoke('upload_log', args),
  uploadRefdata: (args = {}) => invoke('upload_refdata', args),
  /** The user's answer to an assistant's upload request. */
  uploadConfirm: (id, approved) => invoke('upload_confirm', { id, approved }),

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
  /** Element clipboard of the open project (in memory, Rust side). */
  copy: (view, ids) => invoke('edit_copy', { ...view, ids }),
  cut: (view, ids) => invoke('edit_cut', { ...view, ids }),
  /** `page`: home page of the first pasted element (default: the copied pages). */
  paste: (view, page) => invoke('edit_paste', { ...view, ...(page != null ? { page } : {}) }),
  clipboard: () => invoke('edit_clipboard'),
  group: (view, ids) => invoke('edit_group', { ...view, ids }),
  ungroup: (view, ids) => invoke('edit_ungroup', { ...view, ids }),
  /** `langOverride`: reset the view language's override instead of the variant's. */
  resetOverride: (view, id, field, langOverride = false) =>
    invoke('edit_reset_override', { ...view, id, field, ...(langOverride ? { lang_override: true } : {}) }),

  // ----- inspector, layers, canvas settings (task 2.6) -----
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
  /**
   * New element of `kind` (device | text | image | shape | background),
   * centred on `page`, defaults built in Rust; `opts`: { model, asset, text }.
   */
  elementAdd: (view, kind, page, opts = {}) => invoke('element_add', { ...view, kind, page, ...opts }),
  /** Into group `parent` (null = top level) at `index` (null = top). */
  arrange: (view, id, parent, index) => invoke('edit_arrange', { ...view, id, parent, index }),
  canvasGet: (canvas) => invoke('canvas_get', { canvas }),
  /** `patch`: any of `{ aspect: 'w:h', count, ids: [..], backgrounds: [fill] }`. */
  canvasPatch: (canvas, patch) => invoke('canvas_patch', { canvas, ...patch }),
  /**
   * `change`: `{ action: 'add'|'remove'|'rename'|'scale', id, to?, content_scale? }`;
   * add also takes `from` (variant to copy) and `targets` (they show it).
   * Removing detaches the targets showing it.
   */
  canvasVariant: (canvas, change) => invoke('canvas_variant', { canvas, change }),
  /**
   * New canvas (one undo step): `{ id?, aspect?, count?, start: 'empty'|'starter',
   * target_preset?, target_size?, duplicate_of?, copy_targets?, copy_strings? }`
   * → `{ canvas, targets, … }`.
   */
  canvasAdd: (args) => invoke('canvas_add', args),
  canvasRename: (canvas, to) => invoke('canvas_rename', { canvas, to }),
  /** Removes the canvas with its elements and the targets showing it. */
  canvasRemove: (canvas) => invoke('canvas_remove', { canvas }),
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
  /** `{path, bytes, width, height, format, color_space, color_note}` of a screenshot or image asset. */
  assetInfo: (path) => invoke('asset_info', { path }),
  pickFiles: (args = {}) => invoke('dialog_pick_files', args),
  /** Native save dialog (`null` when cancelled); `filters`: [{ name, extensions }]. */
  saveFile: (title, defaultName, filters = []) => invoke('dialog_save_file', { title, default_name: defaultName, filters }),

  // ----- translation exchange (3.4) -----
  /** Writes CSV/XLIFF (`format`: csv | xliff20 | xliff12) to `path` (file or folder). */
  stringsExport: (format, langs, path, options = {}) => invoke('strings_export', { format, langs, path, options }),
  /** Dry run: diff, markup issues and layout findings of importing `paths`. */
  stringsImportPlan: (paths, options = {}) => invoke('strings_import_plan', { paths, options }),
  /** Applies a plan from `stringsImportPlan` as one undo step. */
  stringsImportApply: (plan) => invoke('strings_import_apply', { plan }),

  // ----- screenshot import (3.3) -----
  /** Plan of a folder (`dir`) or `files`: `{ format?, mapping?, overrides?, include_framed? }`. */
  importPlan: (args) => invoke('import_plan', args),
  /** Re-plans with the same arguments and copies; `save_mapping` stores the mapping. */
  importApply: (args) => invoke('import_apply', args),
  catalogFonts: () => invoke('catalog_fonts'),

  // ----- imported frame packages (3.5) -----
  framesList: () => invoke('frames_list'),
  /** Plan of importing a folder / .zip / .dmg (nothing written). */
  framesImportPlan: (path) => invoke('frames_import_plan', { path }),
  /** `assignments`: [{ name, model?, color?, orientation? }] (no model = ignore). */
  framesImportApply: (path, assignments = []) => invoke('frames_import_apply', { path, assignments }),
  /** `args`: { model?, color?, orientation? } (none = all). */
  framesRemove: (args = {}) => invoke('frames_remove', args),
  /** `source`: `apple` (default) or `pack:<id>`. */
  framesThumbnail: (file, maxEdge, source = 'apple') => invoke('frames_thumbnail', { file, max_edge: maxEdge, source }),
  // ----- own frame packs (6.4, ADR 0024) -----
  /** `opts`: { target?: 'app' | 'project', id?, dry_run? }; error details `{kind: 'not_a_pack'}` for other packages. */
  framesPackImport: (path, opts = {}) => invoke('frames_pack_import', { path, ...opts }),
  framesPackRemove: (id, target = 'app') => invoke('frames_pack_remove', { id, target }),
  // ----- imported frames stored in the project (ADR 0027) -----
  /** Dry run of storing the used frames: { stored, up_to_date, items, missing, bytes_now, bytes_after, folder_bytes, … }. */
  framesEmbedStatus: (allSizes) => invoke('frames_embed_status', allSizes == null ? {} : { all_sizes: allSizes }),
  /** `opts`: { all_sizes?, dry_run? }; returns the applied plan. */
  framesEmbed: (opts = {}) => invoke('frames_embed', opts),
  /** Removes `frames/imported/`; returns the bytes freed. */
  framesUnembed: () => invoke('frames_unembed'),
  // ----- system fonts stored in the project (ADR 0028) -----
  /** Dry run of storing the used system fonts: { stored, up_to_date, items, restricted, system_families, bytes_now, bytes_after, folder_bytes, system_enabled }. */
  fontsEmbedStatus: () => invoke('fonts_embed_status'),
  /** Stores / updates the used system fonts in fonts/system/; returns the applied plan. */
  fontsEmbed: () => invoke('fonts_embed', {}),
  /** Removes fonts/system/; returns the bytes freed. */
  fontsUnembed: () => invoke('fonts_unembed'),
  /** Rescans the installed system fonts; returns the number of families. */
  fontsRefresh: () => invoke('fonts_refresh'),
  // ----- banner from this canvas (6.2) -----
  /** Store banner proposals for `canvas`: [{ preset, label, store, size, pages, existing }]. */
  canvasBannerOptions: (canvas, page) => invoke('canvas_banner_options', page == null ? { canvas } : { canvas, page }),
  /** `args`: { canvas, preset, pages?, page?, fit?, variant?, id? } → { created: [id] }. */
  targetAddBanner: (args) => invoke('target_add_banner', args),

  // ----- templates (6.3, ADR 0023) -----
  /** Built-in and installed templates (`source`: builtin | installed). */
  templateList: () => invoke('template_list'),
  /** Tag taxonomy: `{ facets: [{ id, label: {en, de}, min, max, derived, values: [{ id, label }] }] }`. */
  templateTags: () => invoke('template_tags'),
  /** `{ name, id?, description?, author?, tags?: {style, color, use}, sample_screenshots?, canvases?, replace?, file? }` → `{ template, report, file? }`. */
  templateSave: (args) => invoke('template_save', args),
  templateImport: (path, replace = false) => invoke('template_import', { path, replace }),
  templateExport: (id, path) => invoke('template_export', { id, path }),
  templateRemove: (id) => invoke('template_remove', { id }),
  /** Dry run of an insert: `{ template, canvases?, styles? }` → report. */
  templateInsertPlan: (args) => invoke('template_insert_plan', args),
  /** Inserts canvases of a template (one undo step) → apply result + `report`. */
  templateInsert: (args) => invoke('template_insert', args),
  /** Preview PNG of a template canvas (default: the first). */
  templatePreview: (id, canvas) => invoke('template_preview', canvas ? { id, canvas } : { id }),

  // ----- template gallery (online, ADR 0025; errors `unreachable`, `remote`) -----
  /** `{ query?, style?, color?, use?, devices?, offset?, limit? }` → `{ templates, offset, limit, has_more }`. */
  galleryList: (args = {}) => invoke('gallery_list', args),
  /** Downloads and installs a gallery template into the app-wide store → template summary. */
  galleryDownload: (id, replace = false) => invoke('gallery_download', { id, replace }),
  /** Preview PNG of a gallery template canvas (rendered by the server). */
  galleryPreview: (id, canvas) => invoke('gallery_preview', canvas ? { id, canvas } : { id }),
};
