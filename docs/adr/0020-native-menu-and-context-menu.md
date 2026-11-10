# 0020 – Native menu bar and web-view context menu

- Status: accepted
- Date: 2026-11-10

## Context

The user asked (2026-11-10) for the app menu's entries in the operating
system's menu bar as well ("ours and on top"), and for the web view's
context menu ("Reload Image", "Inspect Element") to disappear. Until now
the app had only the in-app menu (☰ button in the top bar, `ui/js/menu.js`)
plus Tauri's default macOS menu (English, no app actions).

## Decision

### Native menu bar: macOS only

- On macOS the GUI installs its own menu bar: application menu (About,
  App settings… ⌘,, Services, Hide, Hide Others, Show All, Quit), File
  (the in-app menu's entries), Edit (Undo/Redo, Cut/Copy/Paste/Select
  All), Window (Minimize, Zoom, Close Window; registered as the Window
  menu) and Help (allow-listed store setup docs via `open_url`; registered
  as the Help menu).
- Windows and Linux keep only the in-app menu: Tauri draws a native menu
  inside the window there, which would sit directly above our own top bar
  and duplicate it visibly.
- **The UI owns the content.** `menu.js` builds a spec (structure,
  localized labels, shortcuts in the UI notation `mod+shift+z`, enabled
  state) from the same item list as the in-app menu and sends it with the
  GUI-only Tauri command `menu_set` (`crates/gui/src/native_menu.rs`; no
  `arrangia-app` counterpart, the dev bridge has no menu). Rust validates
  the spec on every OS, builds the menu on macOS and answers `{native}`.
  It is re-sent when a project is opened or closed (enabled state); a UI
  language switch reloads the page and so rebuilds it. Until the first
  `menu_set` Tauri's default macOS menu is shown.
- **No duplicated logic.** Our items emit the event `menu-action` with the
  item id; the UI runs the same handler as the in-app menu (ignored while a
  modal dialog is open, like the app shortcuts). Predefined items (Hide,
  Quit, Cut, Copy …) are handled by the OS.
- **One owner per shortcut.** Where the native menu is active, the
  shortcuts of its own items (⌘N, ⌘O, ⌘E, ⌘,, ⌘Z, ⇧⌘Z) belong to it and the
  JS key handlers skip them (`menuOwns` in `shortcuts.js`). Everywhere else
  (Windows, Linux, dev bridge) the JS handlers own them as before. Undo /
  Redo from the menu act on the focused text field (`execCommand`) while
  typing, else on the project history. ⌘A, ⌘D, ⌘G, zoom etc. stay JS-only.

### Context menu

- A document-level `contextmenu` listener (`main.js`) suppresses the web
  view's menu except in text fields (textarea, contenteditable, text-like
  inputs) and on selected text, where copy/paste/spelling are useful.
  Custom context menus (none yet) call `preventDefault` themselves and are
  unaffected.
- Debug opt-in: ⌥/Alt + right-click shows the native menu anywhere
  ("Inspect Element" exists only in debug builds, which enable the dev
  tools). No query parameter: the Tauri window has no URL parameters.

## Consequences

- New native items need an i18n key in both catalogs (`native.*`) and an
  entry in `menu.js`; app actions stay in the shared `handlers`.
- The native menu can only be checked in the real macOS app (not in the
  dev bridge).
- Help links must be in `HELP_URLS` (`arrangia_app::commands::os`); user
  guide links can be added there once the guide is online.
