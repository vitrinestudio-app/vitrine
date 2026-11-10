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
js/views/canvas.js    canvas: pixmaps, zoom/pan, hit test, gestures, previews
js/views/overlay.js   SVG overlay drawing (pure function of a model)
js/views/snap.js      snapping lines/box/angle
js/views/{pages,gallery,languages}.js   other centre views
js/views/textedit.js  inline text editor of the canvas (double-click)
js/panels/*.js        sidebar/inspector panels (see below); modal.js =
                      confirm/prompt dialogs, importer.js = asset import
                      dialog + drag & drop routing
js/panels/fields.js   form controls: scrub numbers, colours, selects, fill editor
js/panels/strip-settings.js  strip settings (inspector with no selection)
```

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
- **No inline styles in markup** (Tauri CSP `style-src 'self'`): use CSS
  classes; setting `el.style.prop` from JS is fine. SVG presentation
  attributes are fine.
- **Theme**: only CSS variables from `styles/base.css`; define new
  colours there for both themes (light default, dark via
  `prefers-color-scheme` or `data-theme`).
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
`api.rename`. `store.pageLocks` lists elements whose home page stays put
while dragging on the canvas (UI state only).
