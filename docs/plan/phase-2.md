# Phase 2 – GUI editor: task list

Scope: KONZEPT.md §9 and §12, Phase 2. Done when a project can be created,
designed and exported completely in the GUI.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## Architecture (decided)
- **Session layer in Rust, transport-agnostic.** `arrangia-gui` gets a library
  part (`session`, `commands`) with plain Rust functions taking/returning serde
  types. Tauri commands are thin wrappers. GUI, CLI and MCP keep going through
  the core Operations API.
- **Edit intents in `core`** (`core::edit`): GUI gestures are translated into
  ops in core, not in JS – e.g. "move element to canvas point" (→ home page +
  anchor-relative `at`, or a variant override when a variant is active),
  "rotate", "resize", "set home page". MCP reuses them in Phase 4.
- **Rendering:** previews come from `arrangia-render`, per output image (page
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
- [ ] `core::edit`: move (canvas point → home page/at, honours anchors, gaps,
      grouping and the active variant → override), resize, rotate, set pivot,
      reorder z, duplicate, set home page explicitly
- [ ] Hit-test/overlay data: per element its canvas-lu quad (rotated box),
      handles, page/column rects, gap rects for a target × lang
- [ ] Missing ops: rename ids, move element into/out of a group, reorder,
      reset variant override field, set default language (with "what loses
      its fallback" report, §5.0), add/remove language
- [ ] Tests incl. proptest: move → resolve puts the element where it was dropped

## 2.2 Session + commands (`gui` lib)
- [ ] `Session`: open/new/close project, op apply with history (undo/redo),
      atomic save per op, revision conflict → reload, file watcher (`notify`)
      with debounce and external-change events
- [ ] Commands: project (open/new/summary/recent), apply_ops, undo/redo,
      render (image/tile/gallery, size, quality) as binary, overlay data,
      validate, export with progress events + cancel, strings get/set/missing,
      assets list/import/remove (copy into project, screenshot variants),
      catalog (devices, targets, galleries, fonts), settings
- [ ] Preview cache keyed by (strip, target, lang, image, scale, scene hash);
      invalidate on op/external change; drag mode = low-res renders
- [ ] App settings (YAML in the platform config dir): default language for
      new projects, frame style default, UI language, recent projects
      (SQLite `rusqlite` bundled, as in the concept), window state
- [ ] File dialogs via Rust (`tauri-plugin-dialog` used from Rust only;
      web view keeps `core:default`)
- [ ] Unit tests for session/commands without Tauri

## 2.3 Dev HTTP bridge
- [ ] Dev-only binary serving `ui/` + `POST /cmd/<name>` → same command
      functions; events via SSE; JS transport shim chooses Tauri or HTTP
- [ ] Documented in CLAUDE.md; never part of the release bundle

## 2.4 App shell + canvas view
- [ ] Layout per §9.1: top bar (project, strip, target, language, variant,
      validation badge), left (strips, layers, assets, translations), centre
      canvas, right inspector, bottom view switch (canvas | pages | store
      gallery | all languages grid)
- [ ] Canvas: zoom/pan (wheel, trackpad pinch, space-drag, fit), preview
      images positioned in canvas lu, gaps hatched, content in gaps
      semi-transparent, page labels, low-res while interacting
- [ ] Light/dark theme following the OS; keyboard shortcuts; en/de UI

## 2.5 Direct manipulation
- [ ] Select (click, shift-click, marquee), move, resize (aspect-locked for
      devices), rotate with snapping (0°, ±5°, ±15°, ±45°, ±90°; shift = free)
- [ ] Snapping to page edges/centres, margins, other elements; guides
- [ ] Home page follows the element centre on drop; manual override
- [ ] Variant view: edits stored as overrides, marked + resettable
- [ ] Arrow-key nudging, delete, duplicate, z-order

## 2.6 Inspector + layers
- [ ] Inspector per element kind (common: page, at, size, rotation, pivot,
      opacity, z, anchor, clip, visibility; device: model, style, colour,
      orientation, screenshot key, fit, shadow, chrome/url; text: key, style,
      box/margins, align, fit, max lines; image; shape; background stops)
- [ ] Layers list with drag reorder, visibility, groups
- [ ] Strip settings (page aspect, count, page ids, backgrounds), variants

## 2.7 Texts + translations
- [ ] Double-click edit of the current language's text on canvas
- [ ] Translation table (keys × languages, missing marked), add/rename keys
- [ ] Overflow and emergency-break markers from validation on the canvas

## 2.8 Assets
- [ ] Assets panel: screenshots per language/variant, images, fonts;
      drag & drop import (detect variant by pixel size), placeholders listed

## 2.9 Targets, validation, export
- [ ] Targets editor (from presets, size, gallery, group, variant,
      device_map, banner region/fit/gaps, format)
- [ ] Validation panel (grouped, click → select element/page)
- [ ] Export dialog: targets × languages, progress, open folder

## 2.10 Project + app settings, start screen
- [ ] Start screen: new (templates), open, recent
- [ ] Project settings: default language (§5.0 warning), languages + fallback,
      frame style, colours/text styles/spans, fonts
- [ ] App settings dialog

## 2.11 Verification + docs
- [ ] End-to-end scenario via the dev bridge in Chrome: new project → add
      device/text → translate → export, with screenshots reviewed
- [ ] Tauri build smoke test on macOS (app starts, renders the example)
- [ ] ADR(s): GUI architecture, preview transport; CLAUDE.md/README update
