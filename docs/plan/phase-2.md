# Phase 2 – GUI editor: task list

Scope: KONZEPT.md §9 and §12, Phase 2. Done when a project can be created,
designed and exported completely in the GUI.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## Architecture (decided)
- **Session layer in Rust, transport-agnostic.** `vitrine-gui` gets a library
  part (`session`, `commands`) with plain Rust functions taking/returning serde
  types. Tauri commands are thin wrappers. GUI, CLI and MCP keep going through
  the core Operations API.
- **Edit intents in `core`** (`core::edit`): GUI gestures are translated into
  ops in core, not in JS – e.g. "move element to canvas point" (→ home page +
  anchor-relative `at`, or a variant override when a variant is active),
  "rotate", "resize", "set home page". MCP reuses them in Phase 4.
- **Rendering:** previews come from `vitrine-render`, per output image (page
  or page group) and as canvas tiles, cached and invalidated per op. Pixels
  travel as binary `tauri::ipc::Response` (raw RGBA or PNG, measured), never
  Base64. Overlays (selection, handles, guides, gap hatching) are HTML/SVG.
- **Persistence:** every op is saved atomically right away (no unsaved state),
  revision check against external edits; `notify` watches the project folder
  and reloads live.
- **Frontend:** vanilla ES modules, no framework/npm/build step; a tiny store
  (state + subscribe), one module per panel; UI strings via `ui/i18n/{en,de}.json`.
- **Dev harness for verification:** an optional HTTP bridge (dev-only binary)
  exposes the same commands so the unchanged UI can run in a normal browser
  with a JS transport shim. Used for automated/visual checks in Chrome; not
  shipped.

## 2.1 Core edit intents + missing ops (`core`)
- [x] `core::edit`: move (canvas point → home page/at, honours anchors, gaps,
      grouping and the active variant → override), resize, rotate, set pivot,
      reorder z, duplicate, set home page explicitly
- [x] Hit-test/overlay data: per element its canvas-lu quad (rotated box),
      handles, page/column rects, gap rects for a target × lang
- [x] Missing ops: rename ids, move element into/out of a group, reorder,
      reset variant override field, set default language (with "what loses
      its fallback" report, §5.0), add/remove language
- [x] Tests incl. proptest: move → resolve puts the element where it was dropped
- ADR 0007 (edit intents); open: group pivots on the canvas, ungroup
  with variant overrides of the group

## 2.2 Session + commands (`gui` lib)
- [x] `Session`: open/new/close project, op apply with history (undo/redo),
      atomic save per op, revision conflict → reload, file watcher (`notify`)
      with debounce and external-change events
- [x] Commands: project (open/new/summary/recent), apply_ops, undo/redo,
      render (image/tile/gallery, size, quality) as binary, overlay data,
      validate, export with progress events + cancel, strings get/set/missing,
      assets list/import/remove (copy into project, screenshot variants),
      catalog (devices, targets, galleries, fonts), settings
      (overlay data and `edit_*` intents follow with 2.1/2.4)
- [x] Preview cache keyed by (canvas, target, lang, image, scale, scene hash);
      invalidate on op/external change; drag mode = low-res renders
- [x] App settings (YAML in the platform config dir): default language for
      new projects, frame style default, UI language, recent projects
      (SQLite `rusqlite` bundled, as in the concept), window state
- [x] File dialogs via Rust (`tauri-plugin-dialog` used from Rust only;
      web view keeps `core:default`)
- [x] Unit tests for session/commands without Tauri

## 2.3 Dev HTTP bridge
- [x] Dev-only binary serving `ui/` + `POST /cmd/<name>` → same command
      functions; events via SSE; JS transport shim chooses Tauri or HTTP
- [x] Documented in CLAUDE.md; never part of the release bundle

## 2.4 App shell + canvas view
- [x] Layout per §9.1: top bar (project, canvas, target, language, variant,
      validation badge), left (canvases, layers, assets, translations), centre
      canvas, right inspector, bottom view switch (canvas | pages | store
      gallery | all languages grid)
- [x] Canvas: zoom/pan (wheel, trackpad pinch, space-drag, fit), preview
      images positioned in canvas lu, gaps hatched, content in gaps
      semi-transparent, page labels, low-res while interacting
- [x] Light/dark theme following the OS; keyboard shortcuts; en/de UI

## 2.5 Direct manipulation
- [x] Select (click, shift-click, marquee), move, resize (aspect-locked for
      devices), rotate with snapping (0°, ±5°, ±15°, ±45°, ±90°; shift = free)
- [x] Snapping to page edges/centres, margins, other elements; guides
- [x] Home page follows the element centre on drop; manual override
      (inspector: home page select + lock toggle → `keep_page` while dragging)
- [x] Variant view: edits stored as overrides, marked + resettable
- [x] Arrow-key nudging, delete, duplicate, z-order
- Commands `overlay`, `edit_*`, `render_preview_edit` (drag feedback:
  overlay ghost at once + previews of the affected images from a scratch
  copy, ~10 ms per image in release, self-paced ≤ 15 fps); render
  commands take a `variant`, `render_canvas` a `region` (gaps).
  Open: manual home page lock in the UI
  (`edit_set_page` exists; inspector in 2.6), group rotation handles.

## 2.6 Inspector + layers
- [x] Inspector per element kind (common: page, at, size, rotation, pivot,
      opacity, z, anchor, clip, visibility; device: model, style, colour,
      orientation, screenshot key, fit, shadow, chrome/url; text: key, style,
      box/margins, align, fit, max lines; image; shape; background stops)
- [x] Layers list with drag reorder, visibility, groups
- [x] Canvas settings (page aspect, count, page ids, backgrounds), variants
- Core: auto-height texts rotate around their laid-out content (ADR 0004
  amended; `ResolveOptions::text`). Commands `element_get`,
  `element_patch` (base vs override decided in core, optional strings in
  the same step), `edit_rename`, `edit_arrange`, `canvas_get`,
  `canvas_patch`, `canvas_variant`. Open: the page lock is UI state only
  (not saved); new keys get the text in the current language only.

## 2.7 Texts + translations
- [x] Double-click edit of the current language's text on canvas
- [x] Translation table (keys × languages, missing marked), add/rename keys
- [x] Overflow and emergency-break markers from validation on the canvas
  Done: inline editor (`views/textedit.js`) attached below the text's
  content quad (same width/rotation, so the live preview stays visible;
  Enter commits one `set_string`, Shift+Enter = line break, Esc cancels;
  toolbar `**`, `_`, `[span]…[/]`, `\-`; markup diagnostics via
  `markup_check`), live preview via `render_preview_string` (scratch copy);
  `element_info` (resolved element + own string); translations panel
  (`panels/translations.js`: filter, inline cells, add/rename/delete key,
  add/remove language, per-cell markers from `validate` of all targets);
  "All languages" view (`views/languages.js`: languages × output images,
  overflow markers, click → language + canvas + selection).

## 2.8 Assets
- [x] Assets panel: screenshots per language/variant, images, fonts;
      drag & drop import (detect variant by pixel size), placeholders listed
  Done: `panels/assets.js` (thumbnails via `asset_thumbnail`, usage from
  `assets_list.screenshot_keys`/`asset_usage`, missing keys per canvas ×
  target variant with import, remove with confirmation), import dialog
  `panels/importer.js` (`assets_probe`: detected variant and candidates,
  overridable; kind, language, key, replace), drops onto the panel or a
  device (device → its screenshot key, current language, target variant;
  Tauri `tauri://drag-drop` paths, browser `File`s via `assets_upload`).

## 2.9 Targets, validation, export
- [x] Targets editor (from presets, size, gallery, group, variant,
      device_map, banner region/fit/gaps, format)
- [x] Validation panel (grouped, click → select element/page)
- [x] Export dialog: targets × languages, progress, open folder
- Commands `target_get`, `target_add_from_preset`, `target_patch` (fields
  + rename, one undo step), `target_remove`, `dialog_pick_folder`,
  `reveal_path` (file manager via `std::process`); store-rule hints are
  validation findings of the target. The export matrix is split into one
  `export` call per distinct language set.

## 2.10 Project + app settings, start screen
- [x] Start screen: new (templates), open, recent
- [x] Project settings: default language (§5.0 warning), languages + fallback,
      frame style, colours/text styles/spans, fonts
- [x] App settings dialog
- Commands `project_rename`, `language_add`/`language_remove`,
  `languages_set` (order, fallback map), `default_language_impact`,
  `default_language_set`, `styles_get`, `styles_patch` (per-map entries,
  frame, rename incl. references inside the styles). App menu (top-bar
  button) + ⌘N/⌘O/⌘E/⌘, ; the UI language applies by reloading the web
  view (view context kept). Open: renaming a style referenced by elements
  is refused (no element rewrite yet); font import UI (fonts are listed,
  added via the `fonts/` folder).

## 2.11 Verification + docs
- [x] End-to-end scenario via the dev bridge in Chrome: new project → add
      device/text → translate → export, with screenshots reviewed
- [x] Tauri build smoke test on macOS (app starts, renders the example)
- [x] ADR(s): GUI architecture, preview transport (ADR 0008, amended for
      the bridge server); CLAUDE.md/README update
- Done: branches of 2.7/2.8 and 2.9/2.10 merged (one modal helper,
  `dialogs/modal.js`). Missing piece found by the scenario: there was no
  way to add elements; now the top bar's "Add" menu (`element_add`,
  defaults built in Rust, centred on the visible page). Scenario run in
  Chrome on the dev bridge, en + de UI, both themes: start screen → new
  project (phone template, en + de) → add text, edit inline (live preview)
  → German in the translations table → add device → import a screenshot
  (variant detected from the pixel size) → add target `ios-iphone-63` from
  its preset → validation panel, click → target, zoom, selection → export
  of 4 targets × 2 languages (26 files, checked visually). Fixed on the
  way: inspector kept "no screenshot file" after an import, validation
  badge not refreshed after asset changes, export summary label. Bridge:
  a reload after an export with pending previews stalled on stylesheets
  (tiny_http pool starvation, reproduced); replaced by a small
  `std::net` server (one request per connection). Tauri: the window was
  blank because the CSP blocked `fetch` of the i18n catalogs
  (`connect-src` without `'self'`); fixed, the app starts without
  panics, opens a project given on the command line
  (`vitrine-gui <dir>`), renders the example via the binary IPC
  transport (window captured with `screencapture -l`).

## Follow-ups (known gaps, not blocking Phase 2)
- Home-page lock (`store.pageLocks`) is UI state only, not persisted.
- A new string key gets its text only in the current language.
- Margins ↔ free-box conversion of texts in the inspector is approximate.
- Variants: no preset-on-create (content scale, overrides from a preset).
- Renaming colours/text styles that elements use is refused (no element
  rewrite yet).
- No font import UI (fonts are listed; add files to `fonts/`).
- Switching the UI language reloads the web view (view context kept).
- No native menu bar (app menu is a top-bar button).
- Targets editor: store-slot dropdown limited to the preset's values.
- Banner page ranges are 0-based in the UI.
- Translation and asset usage ignore variant overrides.
- Translations table re-validates all targets after each edit.
- Drag & drop in Tauri: the code paths start without errors, but a real
  file drop onto the window (asset import, folder → open project) is
  untested.
- Group pivot reference for margin texts (rotation of groups with
  margin-box texts).
- Add menu: new texts/devices land centred on the page and may cover
  existing elements; image needs an asset in `assets/` first.
- Some WebKit form fields lay out wider than in Chrome (panel bodies now
  clip horizontally instead of scrolling).
