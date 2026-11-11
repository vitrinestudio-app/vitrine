---
title: Keyboard & mouse
description: "Every shortcut, mouse gesture and modifier key of the editor."
weight: 170
icon: keyboard
---

The same list is in the app: press <kbd>?</kbd> (when not typing), click
**?** in the bottom bar, or ☰ → *Keyboard & mouse…*. On Windows and Linux,
<kbd>⌘</kbd> is <kbd>Ctrl</kbd> and <kbd>⌥</kbd> is <kbd>Alt</kbd>. On a
trackpad, two-finger scrolling works like the scroll wheel.

## Navigation & zoom

| Keys / gesture | Action |
|---|---|
| Scroll | move the view up and down (trackpad: any direction) |
| <kbd>Shift</kbd> + scroll | move the view sideways |
| <kbd>⌘</kbd> + scroll, pinch | zoom around the pointer |
| Drag on an empty area | move the view (in every view) |
| <kbd>Space</kbd> + drag, middle-button drag | move the view |
| Scrollbars | drag to move the view (they appear when the content doesn't fit) |
| <kbd>⌘</kbd><kbd>+</kbd> / <kbd>⌘</kbd><kbd>−</kbd> | zoom in / out |
| <kbd>⌘</kbd><kbd>0</kbd> | fit to window |
| <kbd>⌘</kbd><kbd>1</kbd> | actual size (export pixels) |

## Selecting

| Keys / gesture | Action |
|---|---|
| Click | select an element (a group as a whole) |
| Double-click | select the element inside its group (a text opens for editing) |
| <kbd>⌘</kbd> + click | select the element under the pointer directly, even inside a group |
| <kbd>Shift</kbd> + click | add to or remove from the selection |
| <kbd>Shift</kbd> + drag on an empty area | select with a frame (adds to the selection) |
| <kbd>⌘</kbd><kbd>A</kbd> | select all elements of the canvas (not locked ones) |
| <kbd>Esc</kbd> | clear the selection, or cancel a running drag |

## Moving & arranging

| Keys / gesture | Action |
|---|---|
| Drag an element | move it (snaps to page edges, centres and other elements) |
| <kbd>Shift</kbd> while moving | only horizontally or vertically |
| <kbd>⌥</kbd> while moving or resizing | no snapping |
| <kbd>Shift</kbd> on a resize handle | keep the aspect ratio (not for texts) |
| Rotation handle | rotate (snaps at 0°, ±5°, ±15°, ±45°, ±90°) |
| <kbd>Shift</kbd> while rotating | free angle in 0.1° steps |
| Arrow keys | nudge by 1 lu |
| <kbd>Shift</kbd> + arrow keys | nudge by 10 lu |
| <kbd>⌘</kbd><kbd>C</kbd> / <kbd>⌘</kbd><kbd>X</kbd> / <kbd>⌘</kbd><kbd>V</kbd> | copy, cut, paste elements (pasted where the originals are = offset copies) |
| <kbd>⌘</kbd><kbd>D</kbd> | duplicate |
| <kbd>⌘</kbd><kbd>G</kbd> | group |
| <kbd>⇧</kbd><kbd>⌘</kbd><kbd>G</kbd> | ungroup |
| <kbd>⌫</kbd>, <kbd>Delete</kbd> | delete the selection |
| <kbd>[</kbd> / <kbd>]</kbd> | send backward / bring forward |
| <kbd>Shift</kbd>+<kbd>[</kbd> / <kbd>Shift</kbd>+<kbd>]</kbd> | send to back / bring to front |
| <kbd>⌘</kbd><kbd>Z</kbd> | undo |
| <kbd>⇧</kbd><kbd>⌘</kbd><kbd>Z</kbd>, <kbd>⌘</kbd><kbd>Y</kbd> | redo |
| Right-click | context menu: on an element (cut, copy, paste, arrange, lock, hide …) or on an empty area (paste on this page, select all on this page) |
| <kbd>Shift</kbd>+<kbd>F10</kbd>, menu key | context menu of the selection |
| Drop files onto a device | its screenshot for the current language |
| Drop files elsewhere | import dialog (screenshots, images, fonts) |

## Editing texts

| Keys / gesture | Action |
|---|---|
| Double-click a text on the canvas | edit it in the current language |
| <kbd>Enter</kbd> | apply (clicking elsewhere applies too) |
| <kbd>Shift</kbd>+<kbd>Enter</kbd> | line break |
| <kbd>Esc</kbd> | cancel editing |
| <kbd>⌘</kbd><kbd>Enter</kbd> | apply the text field in the inspector |
| Click, <kbd>Enter</kbd> | translations table: edit a cell (<kbd>Enter</kbd> applies, <kbd>Shift</kbd>+<kbd>Enter</kbd> breaks the line, <kbd>Esc</kbd> cancels) |

## Inspector & layers

| Keys / gesture | Action |
|---|---|
| Drag a number field's label sideways | change the value (<kbd>Shift</kbd>: bigger steps) |
| <kbd>↑</kbd> / <kbd>↓</kbd> | in a number field: one step up / down |
| <kbd>Shift</kbd>+<kbd>↑</kbd> / <kbd>Shift</kbd>+<kbd>↓</kbd> | in a number field: bigger steps |
| <kbd>Enter</kbd> | apply the typed value |
| <kbd>Esc</kbd> | discard the typed value |
| <kbd>Shift</kbd> + click | layers: select a range |
| <kbd>⌘</kbd> + click | layers: add to or remove from the selection |
| <kbd>↑</kbd> / <kbd>↓</kbd> | layers: select the next / previous layer |
| Double-click, <kbd>Enter</kbd>, <kbd>F2</kbd> | layers: rename |
| Drag | layers: change the order |

## App & dialogs

| Keys / gesture | Action |
|---|---|
| <kbd>?</kbd> | this overview (not while typing) |
| <kbd>⌘</kbd><kbd>N</kbd> | new project |
| <kbd>⌘</kbd><kbd>O</kbd> | open project |
| <kbd>⌘</kbd><kbd>E</kbd> | export |
| <kbd>⌘</kbd><kbd>,</kbd> | app settings |
| <kbd>Esc</kbd> | close a dialog |
| <kbd>Enter</kbd> | confirm a question dialog |
| Drop a project folder onto the start screen | open it |
| <kbd>⌥</kbd> + right-click | the web view's own context menu (for debugging) |

On macOS, the app's menu entries are also in the menu bar (*Vitrine Studio*,
*File*, *Edit*, *Window*, *Help*). While you type in a text field,
<kbd>⌘</kbd><kbd>Z</kbd> undoes your typing; elsewhere it undoes the last
project change.
