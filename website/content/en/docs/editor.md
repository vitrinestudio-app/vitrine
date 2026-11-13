---
title: The editor
description: "Views, selecting and arranging, the element kinds, inspector and layers, the context menu, copy and paste, locking, variants and undo."
weight: 40
icon: canvas
---

## What you see: the four pickers

The pickers in the top bar decide **what** the centre view shows and **where
your edits go**:

| Picker | Meaning |
|---|---|
| **Canvas** | which canvas you design (e.g. `phone`, `mac`) |
| **Target** | which output you look at – size, frames, gaps (e.g. `ios-iphone-69  (1320×2868)`) |
| **Language** | which language's texts and screenshots are shown and edited |
| **Variant** | which layout: the target's own variant, the **Base** layout, or another variant |

Each picker ends with a "+" entry: **New canvas…**, **Add target…**, **Add
language…**, **New variant…**.

The Variant picker only changes what you **look at and edit** – which variant
a target exports is set in the target itself (see [Variants](#variants)).

## Views

Switch in the bottom bar:

- **Canvas** – the whole row of pages with hatched gaps; this is where you
  edit. A dashed rectangle shows a target's safe area, if it has one.
- **Pages** – each output image of the target as it will be exported (two
  pages side by side for grouped targets, the region for banners).
- **Store gallery** – the images as the store shows them, with its gaps and
  rounded corners. Banners have no gallery.
- **All languages** – every language as a row, to compare translations;
  overflowing texts are marked. Click an image to open it on the canvas.

**Zoom and pan** work the same in every view: scroll to move (<kbd>Shift</kbd>
+ scroll sideways), <kbd>⌘</kbd>/<kbd>Ctrl</kbd> + scroll or pinch to zoom
around the pointer, drag on empty space (or <kbd>Space</kbd> + drag, or the
middle mouse button) to pan. <kbd>⌘</kbd><kbd>+</kbd> / <kbd>⌘</kbd><kbd>−</kbd>
zoom, <kbd>⌘</kbd><kbd>0</kbd> fits, <kbd>⌘</kbd><kbd>1</kbd> shows actual size
(one export pixel per screen pixel). After *Fit* the view stays fitted when you
resize the window, until you zoom or pan yourself. Scrollbars appear when the
content doesn't fit.

On Windows and Linux, read <kbd>⌘</kbd> as <kbd>Ctrl</kbd> and <kbd>⌥</kbd>
as <kbd>Alt</kbd> throughout this guide.

{{< shot src="images/gallery-view.jpg" alt="The Store gallery view: six iPhone images of a pop-art dating app side by side with the store's gaps and rounded corners." caption="The **Store gallery** view (*Pop-Art Panels* template, iPhone 6.9″)." >}}

## Elements

**Add** in the top bar puts a new element on the **visible page**:

| Kind | What it is |
|---|---|
| **Device** | a phone, tablet, laptop, desktop, watch or browser window showing a screenshot – see [Devices & frames](../devices-frames/) |
| **Text** | a headline or label; the text itself lives in the translations |
| **Image** | a logo, badge, illustration – PNG, JPEG or SVG from the project's `assets/` folder |
| **Shape** | rectangle (with corner radius), ellipse or line, with fill and outline |
| **Page background** | a fill (colour, gradient or image) for one page, on top of the canvas background |

**Groups** combine elements so they move and rotate together:
<kbd>⌘</kbd><kbd>G</kbd> groups the selection, <kbd>⇧</kbd><kbd>⌘</kbd><kbd>G</kbd>
ungroups.

Backgrounds that run across **all** pages belong to the canvas: deselect
everything and edit **Canvas backgrounds** in the inspector.

A **page background** fills its whole home page, so a click on the page
selects the element in front of it, not the background. Select it by
**double-clicking an empty spot** of the page, with right-click → *Select
background "…"*, in **Layers**, or in the canvas settings (nothing selected)
under **Page backgrounds**, which also has *Add page background*. The
inspector then edits its *Home page*, *Fill* (colour, gradient or image),
*Opacity*, *Layer (z)*, *Clip* and visibility.

## Selecting

- **Click** selects an element (a group as a whole).
- **Double-click** selects the element inside its group – on a text it opens
  the text for editing; on an empty spot of a page it selects the page's
  background element.
- <kbd>⌘</kbd> + click selects the element under the pointer directly, even
  inside a group.
- <kbd>Shift</kbd> + click adds to or removes from the selection.
- <kbd>Shift</kbd> + drag on empty space draws a selection frame (adds).
- <kbd>⌘</kbd><kbd>A</kbd> selects all (except locked elements),
  <kbd>Esc</kbd> clears the selection.

## Moving and arranging

- **Drag** to move. Elements snap to page edges, centres and other elements;
  hold <kbd>⌥</kbd> to switch snapping off, <kbd>Shift</kbd> to move only
  horizontally or vertically.
- **Resize** with the handles; <kbd>Shift</kbd> keeps the aspect ratio (not
  for texts).
- **Rotate** with the handle above the selection; it snaps at 0°, ±5°, ±15°,
  ±45° and ±90°, <kbd>Shift</kbd> rotates freely in 0.1° steps. Positive
  angles turn clockwise.
- **Arrow keys** nudge by 1 lu, with <kbd>Shift</kbd> by 10 lu.
- <kbd>[</kbd> / <kbd>]</kbd> send backward / bring forward,
  <kbd>Shift</kbd>+<kbd>[</kbd> / <kbd>Shift</kbd>+<kbd>]</kbd> to the back /
  front.
- <kbd>⌘</kbd><kbd>D</kbd> duplicates, <kbd>⌫</kbd> or <kbd>Delete</kbd>
  deletes.

When you drag an element onto another page, its **home page** follows. To keep
it (e.g. a phone that spans two pages but belongs to the first), use the lock
next to **Home page** in the inspector.

## The context menu

Right-click an element (or press <kbd>Shift</kbd>+<kbd>F10</kbd> / the menu
key) for: **Cut**, **Copy**, **Paste**, **Duplicate**, **Delete**, **Bring to
front**, **Bring forward**, **Send backward**, **Send to back**, **Group** /
**Ungroup**, **Lock** / **Unlock** and **Hide**. It acts on the whole
selection.

Right-click empty space for **Paste on page N**, **Select all on page N**,
**Select all** and **Fit to window**.

{{< shot src="images/context-menu.jpg" alt="The context menu on a selected phone with its editing and arranging commands." caption="The canvas context menu on a selected phone." >}}

### Copy and paste

<kbd>⌘</kbd><kbd>C</kbd>, <kbd>⌘</kbd><kbd>X</kbd>, <kbd>⌘</kbd><kbd>V</kbd>
copy, cut and paste elements – also between canvases of the same project.

- Pasted where the originals still are, the copies move 40 lu down and right
  (further with every paste).
- **Paste on page N** keeps the position relative to the page, so you can
  repeat a layout on another page.
- Copies keep their variant overrides (for variants the canvas has).
- The clipboard belongs to the open project; it is not the system clipboard
  and is emptied when you close the project.

### Lock and hide

- **Lock** makes an element unclickable on the canvas – clicks and selection
  frames go through it, so you can work on what's behind a large shape. The
  **Layers** tab shows a lock button that unlocks it. Locking only affects
  the editor, never the rendered images.
- **Hide** switches the element off (*Visible* in the inspector). The Layers
  tab has an eye button to show it again.

## The inspector

With an element selected, the inspector shows its settings. The most important:

- **Layout:** *Home page*, *Centre* (position of the centre on the home page,
  in lu), *Size*, *Rotation*, *Pivot*, *Anchor*.
- **Anchor** (*Top*, *Bottom*, *Centre*, *Stretch*) decides what happens when
  a target's page is taller or shorter than the canvas page: the element keeps
  its distance to that edge (*Stretch* scales with the page). Phones at the
  bottom of a page → *Bottom*.
- **Appearance:** *Opacity*, *Layer (z)*, *Clip* (texts are clipped to their
  *Home page* by default), *Visible*, *Shown in targets*, *Shown in
  languages* (e.g. a badge only in the App Store targets).
- **Device:** *Model*, *Frame*, *Colour*, *Orientation*, *Screenshot* (the
  key), *Fit*, *Shadow* … – see [Devices & frames](../devices-frames/).
- **Text:** *Key*, *Text*, *Style*, *Box* (*Margins* to the page edges or a
  *Free box*), *Align*, *Vertical*, *Auto-fit* (*Shrink down to*, *Maximum
  lines*), *Hyphenation*, *Colour*, *Font size*, *Outline*, *Glow*, *Shadow*.
  Below, *Laid out* shows the font size and line count the text actually got,
  or *Text overflows its box*.
- **Image:** *Asset* (the eye button opens the image preview), *Fit*, *Shadow*. **Shape:** *Shape*, *Radius*, *Fill*,
  *Stroke*. Fills can be a colour, a linear or radial gradient, or an image.

Number fields: drag the label sideways to scrub the value (<kbd>Shift</kbd>
for bigger steps), <kbd>↑</kbd>/<kbd>↓</kbd> step, <kbd>Enter</kbd> applies,
<kbd>Esc</kbd> discards. Colours accept `#RRGGBB`, `#RRGGBBAA` or a project
colour (`@accent`).

With **nothing selected**, the inspector shows the **canvas settings**: page
*Aspect* and *Page height*, number of *Pages* (add or remove at the end),
*Page ids* (they name the exported files), *Canvas backgrounds*, *Page
backgrounds* and the *Variants*.

The inspector is wider than before and **resizable**: drag its left edge
(double-click it for the default width). The width is remembered.

{{< shot src="images/inspector-text.jpg" alt="The inspector with a selected headline: layout, text key and content, style, font size, colour, box, alignment and auto-fit." caption="The inspector with a headline selected." >}}

## Layers

The **Layers** tab lists the canvas's elements in painting order (top = in
front). Click to select, <kbd>Shift</kbd> + click for a range,
<kbd>⌘</kbd> + click to toggle, <kbd>↑</kbd>/<kbd>↓</kbd> to walk, drag to
reorder, double-click / <kbd>Enter</kbd> / <kbd>F2</kbd> to rename (the
element id). Groups expand and collapse; eye and lock buttons show hidden
and locked elements.

## Variants

A **variant** stores per-format differences of a canvas – e.g. the phones a bit
lower in the iPad landscape set – as **overrides**, while texts, screenshots and
styles stay shared.

- The Variant picker defaults to the **target's own variant** (`… · target`).
  While a variant is shown, the inspector says *Edits become overrides of …*:
  every move, resize or change goes into that variant only. Overridden fields
  are marked **purple**, with *Reset to base* per field and *Reset all*.
- Switch the picker to **Base** to edit the shared layout.
- **New variant…** (Variant picker, canvas settings or *Canvases & targets*):
  name, *Based on* (the base layout or a copy of another variant), *Content
  scale* (scales devices and images in this variant) and *Shown by* (targets
  that use it from now on).

Which variant a target **exports** is set in the target (*Canvases & targets* →
target → *Variant*). See [Targets & store formats](../targets/).

## Canvases

The **Canvases** tab lists the project's canvases with their pages and
targets; **New canvas…** and **Manage…** open *Canvases & targets* (also in
the ☰ menu). There you can:

- create a canvas – *Empty*, *Headline + device* (a headline and a device on
  every page) or a *Copy of a canvas* – with a page format from a store preset
  and, in the same step, the matching target (*Also add this target*);
- rename, duplicate or remove a canvas (removing also removes its targets);
- insert canvases *From template* and *Save as template…* – see
  [Templates](../templates/);
- add store banners with **Banner from this canvas**.

A canvas is only shown and exported through its targets – a new canvas needs
at least one.

## Undo and history

Everything you do is one undo step: <kbd>⌘</kbd><kbd>Z</kbd> undoes,
<kbd>⇧</kbd><kbd>⌘</kbd><kbd>Z</kbd> or <kbd>⌘</kbd><kbd>Y</kbd> redoes. The
undo button's tooltip names the step. Changes are saved to disk immediately –
there is no Save.

The history also contains edits by an [AI assistant](../ai-mcp/), labelled
with its name ("Claude Desktop: Move title"). Changes from outside (text
editor, `git checkout`) reload the project.

## Validation

The badge in the top bar checks the current target and language continuously:
**OK**, or the number of errors and warnings. Click it for the list – filter by
level or code, switch between languages, **Show on the canvas** jumps to the
element. **Preview mode** treats missing screenshots as warnings while you
design (export still checks them strictly). The check also looks at the
rendered pages: text contrast against what is really behind it, text touching
devices, covered or cut screen top bars, overlaps across page borders, the
store's rounded corners. Common findings, and how to mark a deliberate one
with `allow`, are explained in [Troubleshooting](../troubleshooting/).

{{< shot src="images/validation.jpg" alt="The validation list for the iPhone target: German and French texts that fall back to English, and one French title that overflows." caption="The validation list with **Preview mode** on: fallbacks are warnings, the overflowing French title is an error." >}}

## Project settings and app settings

| | Project settings (☰ → *Project settings…*) | App settings (☰ → *App settings…*, <kbd>⌘</kbd><kbd>,</kbd>) |
|---|---|---|
| Belongs to | the project folder – travels with it, shared via Git | your computer – all projects |
| Contains | *General* (name, default *Frame style*), *Languages*, *Styles*, *Fonts*, *Stores* | interface language, defaults for new projects, system fonts, *Frame packages*, store accounts, *AI assistants (MCP)*, *Updates*, *Command line tool* |

## Help in the app

The ☰ menu opens from the ☰ button, the app icon or the project name in the
top bar (also with <kbd>Enter</kbd>/<kbd>Space</kbd> when focused; arrow keys
move through the entries).

- **Keyboard & mouse…** (☰ → Help, the **?** button in the bottom bar, or
  <kbd>?</kbd>) – every shortcut and gesture. Also on
  [Keyboard & mouse](../shortcuts/).
- **How Vitrine Studio is organised…** – the one-screen overview.
- **Download Apple Product Bezels** (top of the Help menu) opens Apple's
  Design Resources page – see [Devices & frames](../devices-frames/).
