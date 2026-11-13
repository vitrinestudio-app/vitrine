# Vitrine Studio UI (web view)

Vanilla ES modules, no framework, no npm, no build step. The same files
run in the Tauri window and, via the dev bridge, in a normal browser
(`cargo run -p vitrine-gui --features devbridge --bin vitrine-devbridge --
--project tests/fixtures/example --copy`). Debug query parameters:
`?theme=dark|light` (force a theme), `?lang=en|de` (UI language),
`?guide=first` (show the quick guide on project open like a first start;
the dev bridge never shows it by itself).
Right-click shows no web-view menu except in text fields and on selected
text (`main.js`); ⌥/Alt + right-click shows it anywhere ("Inspect
Element" in debug builds).

## Layout

```
index.html            shell skeleton (top bar, sidebars, view, bottom bar)
styles/base.css       design tokens (CSS variables), light/dark themes, base
styles/controls.css   buttons, pickers, segmented control, tabs, lists, badges
styles/shell.css      app grid
styles/canvas.css     canvas view, SVG overlay classes, pages/gallery views
styles/inspector.css  inspector rows, form controls, fill editor, canvas settings
styles/layers.css     layers tree
i18n/{en,de}.json     UI strings (flat key → string; keep both in sync)
js/main.js            entry: theme, i18n, load project, shell, shortcuts
js/transport.js       invoke/listen over Tauri IPC or the dev bridge
js/api.js             typed wrappers for every command the UI uses
js/store.js           tiny state store + view-context helpers
js/editor.js          controller: load, setContext, apply edits, invalidate
js/actions.js         selection actions (delete, duplicate, group, z-order…)
js/shortcuts.js       keyboard map → named handlers (+ DOM copy/cut/paste events)
js/context-menu.js    right-click / ⇧F10 menu (`openContextMenu({x, y, items})`)
js/geometry.js        2D helpers for canvas lu geometry
js/dom.js             h()/s() element helpers, inline SVG icons, picker()
js/shell.js           top bar (☰ + icon + name = one menu button), sidebar tabs,
                      inspector (+ resize handle: `--inspector-w`, localStorage
                      `vitrine.inspector.width`), view switch, zoom
js/views/viewport.js  shared zoom/pan viewport of all centre views (fit mode,
                      wheel/pinch, pan, overlay scrollbars, per-view memory) + pixmap slots with
                      loading placeholder (`makeSlot`/`paintSlot`, `densityFor`)
js/views/canvas.js    canvas: pixmaps, hit test, gestures, previews (zoom via viewport.js)
js/views/overlay.js   SVG overlay drawing (pure function of a model)
js/views/snap.js      snapping lines/box/angle
js/views/{pages,gallery,languages}.js   other centre views
js/views/textedit.js  inline text editor of the canvas (double-click)
js/panels/*.js        sidebar/inspector panels (see below); importer.js =
                      asset import dialog + drag & drop routing
js/panels/fields.js   form controls: scrub numbers, colours, selects, fill editor
js/panels/canvas-settings.js  canvas settings (inspector with no selection)
js/menu.js            app menu (top-bar button), its macOS menu-bar mirror
                      (`menu_set`, see below), ⌘N/⌘O/⌘E/⌘, shortcuts
js/add-menu.js        top-bar "Add" menu → `element_add` on the visible page
js/help/gestures.js   THE list of shortcuts, mouse gestures and modifiers
                      (grouped; `FEATURES` flags for pending gesture changes)
js/help/sheet.js      "Keyboard & mouse" sheet generated from it (platform
                      modifier names), `?` key, bottom-bar "?" button
js/help/guide.js      quick guide "How Vitrine Studio is organised" (Help menu,
                      start screen, once on first project open: setting `guide_seen`)
js/start.js           start screen (#welcome), new-project dialog, folder drop
js/assistant.js       AI assistants over MCP (ADR 0015): status, top-bar indicator +
                      activity popover, toasts (client display name), canvas glow
                      trigger, projects opened by an assistant
js/updates.js         updates (ADR 0026: daily check, "Check for updates…", install
                      dialog; GUI-only `update_*` commands) + app settings rows for
                      updates and "Install command line tool"
js/dialogs/modal.js   <dialog> helper (openModal, choiceDialog, confirmDialog,
                      promptDialog) + form controls (row, input, select…)
js/dialogs/*.js       targets.js = canvases & targets editor (canvas pane: new /
                      rename / duplicate / remove, variants; target add and
                      form), variants.js = "New variant" dialog + remove
                      confirmation, validation popover, export, project
                      and app settings; stored-frames.js = project settings
                      "Frames" tab (imported frames stored in the project for
                      CI, licence warning; ADR 0027); stores.js = store accounts (app
                      settings) + store mapping tab (project settings);
                      upload.js = "Upload to stores…" (plan table → confirm →
                      progress → result/log) + MCP upload confirmation dialog;
                      templates.js = template picker cards (New project,
                      insert), "Save as template…" (app menu), canvases
                      dialog "From template" (template_* commands, ADR 0023);
                      image-preview.js = image preview of screenshots/images
                      (`asset_thumbnail` + `asset_info`, ←/→, used-by chips)
styles/templates.css  template cards, previews, reports
js/dialogs/template-gallery.js  New project → "Template gallery" tab (online
                      gallery: lazy list, search + facet filters, previews via
                      `gallery_preview`, download + create; ADR 0025) and its
                      styles/template-gallery.css
styles/{dialogs,targets,start,settings}.css   styles of the above
styles/{textedit,translations,assets}.css      text editor, translations, assets
styles/assistant.css  indicator, activity popover, toasts, glow, settings section
styles/upload.css     upload dialog, plan table, MCP upload confirmation
styles/help.css       keyboard & mouse sheet, quick guide, "?" button
```

Dialogs use `openModal({ title, body, footer, size })` from
`dialogs/modal.js`; model changes go through a backend command that
builds the ops (e.g. `target_patch`, `styles_patch`) and then
`editor.apply(Promise.resolve(result))`. Long-running work inside a
dialog calls `m.setBusy(true)` (blocks Escape, the close button and
backdrop clicks; `m.close()` still works), shows a busy bar and closes
the dialog with an `editor.toast` when done ("Save as template").

## Conventions

- **State** lives in `store` (`js/store.js`): `summary`, `canvas`,
  `target`, `lang`, `variant` (`undefined` = target's own, `null` = base,
  string = that variant), `view`, `selection` (ids), `hover`, `zoom`,
  `panX`, `panY`, `overlay`, `history`, `validation`, `sidebarTab`.
  Read with `store.get()`, change with `store.set(patch)`, react with
  `store.select(selector, fn)` (fires when the selected value changes;
  arrays compare shallowly). Every subscription returns an unsubscribe.
- **Commands** only through `api.*` (`js/api.js`); add a wrapper there
  for a new command. Pass the view context with `viewArgs()` from
  `store.js`.
- **Model changes** only through `editor.apply(api.someEdit(...), {
  touched, select })`. It updates history, overlay and summary, emits
  `invalidate` (rects in lu or `all`) so views re-render only what
  changed, and schedules validation. Never patch `summary`/`overlay` by
  hand. Switching target/lang/variant/canvas: `editor.setContext(patch)`
  (emits `reset`).
- **Views** (`js/views/*.js`) export `mountX(root)` returning at least
  `{ unmount() }`; they listen to `editor.on('invalidate' | 'reset')`.
  Zoomable views also return `fit`, `zoomIn`, `zoomOut`, `zoomTo`,
  `zoomActual`, `percent` and the `fitMode` getter (the shell's zoom
  controls and ⌘+/⌘−/⌘0/⌘1 act on `shell.view`). They build on
  `createViewport(el, { key, content, unit, … })` from
  `views/viewport.js`: world units (lu, or export px in the gallery) map
  to screen px by `zoom`/`pan`; `fit()` enters **fit mode** (re-fit on
  every resize of the view element, e.g. window or sidebar), any manual
  zoom/pan leaves it; `refresh(contentKey)` re-fits in fit mode or when
  the content changed; zoom state is remembered per `key`. Rendered
  images are slots (`makeSlot` → loading spinner until the first
  `paintSlot`; re-renders keep the old bitmap) requested at
  `densityFor(zoom, rect, cap)` (zoom × devicePixelRatio in √2 steps).
  `applyWorld` sets the world transform and `--px` (one screen px in world
  units) for hairlines inside the zoomed world.
- **No inline styles in markup** (Tauri CSP `style-src 'self'`): use CSS
  classes; setting `el.style.prop` from JS is fine. SVG presentation
  attributes are fine.
- **Theme**: only CSS variables from `styles/base.css`; define new
  colours there for both themes (light default, dark via
  `prefers-color-scheme` or `data-theme`).
- **Top-bar pickers** (canvas, target, language, variant) end with a
  "+ …" entry (value `__add__`) that opens the place where that thing is
  created (`addCanvas`/`addTarget`/`addLanguage`/`addVariant` actions
  passed from `main.js` to `mountShell`); keep the four consistent.
- **Canvases/targets/variants** removed or renamed (also by undo/redo):
  `editor.apply` re-validates the view context (`validContext` in
  `editor.js`) before fetching the overlay, so dialogs need not fix it.
- **Strings**: every visible string via `t('key')`; add keys to both
  `i18n/en.json` and `i18n/de.json`.
- **Gestures**: a new or changed shortcut, mouse gesture or modifier
  (shortcuts.js, views, panels, menu) updates its entry in
  `js/help/gestures.js` (+ i18n `help.g.*`) in the same change, so the
  "Keyboard & mouse" sheet stays complete. Only list what exists; pending
  changes go behind a `FEATURES` flag.
- **Icons**: add path data to `ICONS` in `js/dom.js` (16×16, stroked).

## Panels

A panel is a module in `js/panels/` exporting

```js
/** @param {HTMLElement} el  @param {PanelContext} ctx */
export function mount(el, { store, editor, api, t, viewArgs }) {
  // build DOM into `el`, subscribe to the store / editor events …
  return { unmount() { /* unsubscribe */ } };
}
```

`ctx` comes from `panelContext()` in `js/panels/index.js`. Left sidebar
tabs are listed in `SIDEBAR_TABS` there (`{ id, icon, title, module }`);
the shell mounts the active tab's panel into the sidebar body and calls
`unmount()` when switching. The inspector (`panels/inspector.js`) is
mounted once into the right sidebar. Panels use the shared classes
`section-title`, `list`/`list-item`, `props` (dl grid), `placeholder`,
`btn`, `chip`.

The inspector (`panels/inspector.js`) reads one element with
`api.elementGet` (base / override / effective values) and writes with
`api.elementPatch(view, ids, patch, strings)`: plain YAML field values,
core decides base vs variant override; never build override ops in JS.
Rows take the YAML field name so overridden fields are marked and get a
reset button. Controls from `panels/fields.js` commit on Enter/blur
(arrow keys and label scrubbing for numbers) and carry a `data-key`, so a
re-render after an edit keeps focus and unsaved typing. With nothing
selected it shows the canvas settings. Layers (`panels/layers.js`)
reorders with `api.arrange` (HTML5 drag & drop) and renames with
`api.rename`. The home-page lock is the model field `lock_page` (written
with `elementPatch`; core keeps the page on drags). `store.langScope`
(inspector toggle "Only for <lang>") adds `scope: 'lang'` to
`viewArgs()`: text edits then go into the language override of the view
language; language-overridden fields (`lang_overridden` from
`element_get`) are marked teal with their own reset
(`api.resetOverride(view, id, field, true)`).

## Canvas gestures and context menu

Plain drag on empty canvas space pans (`vp.panStart(e, false)`: becomes a
pan after 3 px, a plain click clears the selection), Shift+drag draws the
marquee, Space/middle-drag pans anywhere. Cursor classes on
`.canvas-view`: `over-empty` (grab), `shift` (crosshair on empty space),
`marquee`, `panning` (grabbing). Locked elements (`locked`, overlay
`locked` on elements and groups; `isLocked`/`idLocked` in `actions.js`)
are skipped by hit tests and the marquee, except for the context menu.
The canvas context menu builds its items from `actions` (copy/cut/paste
use the Rust element clipboard `edit_copy`/`edit_cut`/`edit_paste`,
`store.clipboard` holds its count; lock/hide go through `element_patch`);
items show shortcuts via `keyLabel`. Scrollbars come with every
`createViewport` (`.vp-scroll`, colours `--scroll-*` in `base.css`); a
view whose content changes without `refresh()` calls
`updateScrollbars()`.

## Native menu bar (macOS)

ADR 0020. `menu.js` mirrors the in-app menu (`ITEMS`) into the macOS menu
bar (the in-app menu also ends with the Help entries `HELP_ITEMS`: quick
guide, keyboard & mouse; the native Help menu lists them above the store
docs links): `initNativeMenu(handlers)` sends a spec (labels via `t()`, shortcuts
in `mod+shift+z` notation, enabled state) with the GUI-only command
`menu_set` (`crates/gui/src/native_menu.rs`); clicks come back as
`menu-action` events and run the same `handlers`. Re-sent on project
open/close; a language switch reloads the page. Windows/Linux answer
`{native: false}`, the dev bridge has no menu. Shortcut owner: the native
menu owns its items' shortcuts (⌘N ⌘O ⌘E ⌘, ⌘Z ⇧⌘Z) and the JS key
handlers skip them (`menuOwns` in `shortcuts.js`). New labels go under
`native.*` in both catalogs.
