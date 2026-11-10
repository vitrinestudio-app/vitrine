# Arrangia UI (web view)

Vanilla ES modules, no framework, no npm, no build step. The same files
run in the Tauri window and, via the dev bridge, in a normal browser
(`cargo run -p arrangia-gui --features devbridge --bin arrangia-devbridge --
--project tests/fixtures/example --copy`). Debug query parameters:
`?theme=dark|light` (force a theme), `?lang=en|de` (UI language).

## Layout

```
index.html            shell skeleton (top bar, sidebars, view, bottom bar)
styles/base.css       design tokens (CSS variables), light/dark themes, base
styles/controls.css   buttons, pickers, segmented control, tabs, lists, badges
styles/shell.css      app grid
styles/canvas.css     canvas view, SVG overlay classes, pages/gallery views
styles/inspector.css  inspector rows, form controls, fill editor, strip settings
styles/layers.css     layers tree
i18n/{en,de}.json     UI strings (flat key → string; keep both in sync)
js/main.js            entry: theme, i18n, load project, shell, shortcuts
js/transport.js       invoke/listen over Tauri IPC or the dev bridge
js/api.js             typed wrappers for every command the UI uses
js/store.js           tiny state store + view-context helpers
js/editor.js          controller: load, setContext, apply edits, invalidate
js/actions.js         selection actions (delete, duplicate, group, z-order…)
js/shortcuts.js       keyboard map → named handlers
js/geometry.js        2D helpers for canvas lu geometry
js/dom.js             h()/s() element helpers, inline SVG icons, picker()
js/shell.js           top bar, sidebar tabs, inspector, view switch, zoom
js/views/viewport.js  shared zoom/pan viewport of all centre views (fit mode,
                      wheel/pinch, pan, per-view memory) + pixmap slots with
                      loading placeholder (`makeSlot`/`paintSlot`, `densityFor`)
js/views/canvas.js    canvas: pixmaps, hit test, gestures, previews (zoom via viewport.js)
js/views/overlay.js   SVG overlay drawing (pure function of a model)
js/views/snap.js      snapping lines/box/angle
js/views/{pages,gallery,languages}.js   other centre views
js/views/textedit.js  inline text editor of the canvas (double-click)
js/panels/*.js        sidebar/inspector panels (see below); importer.js =
                      asset import dialog + drag & drop routing
js/panels/fields.js   form controls: scrub numbers, colours, selects, fill editor
js/panels/strip-settings.js  strip settings (inspector with no selection)
js/menu.js            app menu (top-bar button), ⌘N/⌘O/⌘E/⌘, shortcuts
js/add-menu.js        top-bar "Add" menu → `element_add` on the visible page
js/start.js           start screen (#welcome), new-project dialog, folder drop
js/assistant.js       AI assistants over MCP (ADR 0015): status, top-bar indicator +
                      activity popover, toasts (client display name), canvas glow
                      trigger, projects opened by an assistant
js/dialogs/modal.js   <dialog> helper (openModal, choiceDialog, confirmDialog,
                      promptDialog) + form controls (row, input, select…)
js/dialogs/*.js       targets.js = strips & targets editor (strip pane: new /
                      rename / duplicate / remove, variants; target add and
                      form), variants.js = "New variant" dialog + remove
                      confirmation, validation popover, export, project
                      and app settings; stores.js = store accounts (app
                      settings) + store mapping tab (project settings);
                      upload.js = "Upload to stores…" (plan table → confirm →
                      progress → result/log) + MCP upload confirmation dialog
styles/{dialogs,targets,start,settings}.css   styles of the above
styles/{textedit,translations,assets}.css      text editor, translations, assets
styles/assistant.css  indicator, activity popover, toasts, glow, settings section
styles/upload.css     upload dialog, plan table, MCP upload confirmation
```

Dialogs use `openModal({ title, body, footer, size })` from
`dialogs/modal.js`; model changes go through a backend command that
builds the ops (e.g. `target_patch`, `styles_patch`) and then
`editor.apply(Promise.resolve(result))`.

## Conventions

- **State** lives in `store` (`js/store.js`): `summary`, `strip`,
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
  hand. Switching target/lang/variant/strip: `editor.setContext(patch)`
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
- **Top-bar pickers** (strip, target, language, variant) end with a
  "+ …" entry (value `__add__`) that opens the place where that thing is
  created (`addStrip`/`addTarget`/`addLanguage`/`addVariant` actions
  passed from `main.js` to `mountShell`); keep the four consistent.
- **Strips/targets/variants** removed or renamed (also by undo/redo):
  `editor.apply` re-validates the view context (`validContext` in
  `editor.js`) before fetching the overlay, so dialogs need not fix it.
- **Strings**: every visible string via `t('key')`; add keys to both
  `i18n/en.json` and `i18n/de.json`.
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
selected it shows the strip settings. Layers (`panels/layers.js`)
reorders with `api.arrange` (HTML5 drag & drop) and renames with
`api.rename`. The home-page lock is the model field `lock_page` (written
with `elementPatch`; core keeps the page on drags). `store.langScope`
(inspector toggle "Only for <lang>") adds `scope: 'lang'` to
`viewArgs()`: text edits then go into the language override of the view
language; language-overridden fields (`lang_overridden` from
`element_get`) are marked teal with their own reset
(`api.resetOverride(view, id, field, true)`).
