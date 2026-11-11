---
title: Editor basics
description: "Canvases, pages and gaps, targets, variants, languages and the difference between project and app settings."
weight: 20
icon: canvas
---

## Canvas, pages and gaps

A **canvas** is a strip of pages. A page is always **1000 lu** (logical units)
wide; its height follows the aspect ratio. All sizes are in lu, so a layout is
independent of the export resolution.

- Every element has a **home page**; its position is relative to that page,
  and it may reach into the neighbours – a phone spanning two screenshots
  exists exactly once.
- The canvas has **gaps** between the pages like the store gallery, so
  continuous backgrounds and devices line up when the store shows them.
  Anything inside a gap is left out of the exported images. The editor shows
  the gaps hatched.
- Views: **Canvas** (the whole strip), **Pages**, **Store gallery** and **All
  languages** side by side.

## Targets

A **target** is one output: a store preset (e.g. App Store iPhone 6.9″,
Google Play phone, iPad 13″, Mac, Apple Watch, Wear OS, the Play feature
graphic, App Store header and search-results creatives) or a custom size,
rendered from one canvas.

- **Anchors** (`top`, `bottom`, `center`, `stretch`) adapt one layout to other
  aspect ratios, e.g. iPhone 1:2.17 and Play 1:2.
- **Grouping** (`group: 2`) turns two portrait pages into one landscape image –
  an iPad landscape set from the phone canvas.
- **Banner from this canvas** creates a feature graphic or App Store creative
  from a range of pages in one click.
- Watch and Wear OS targets are *screenshot only*: each page exports the
  device's screenshot alone, as the stores expect.

Manage everything in one place: menu → **Canvases & targets…**

## Variants

A **variant** stores different position, size, rotation or home page of
elements for one format – the iPad landscape layout, say – while texts,
screenshots and styles stay shared. A target picks its variant; the top-bar
*Variant* picker only changes what you see and edit. Overrides are marked
purple in the inspector.

## Languages

Every target is rendered in every project language. Missing texts and
screenshots come from a **fallback chain** (`fr-CA → fr → en`), per-language
**overrides** tweak a text for one language (marked teal). Details:
[Languages & translation](../languages/).

## Project settings vs. app settings

- **Project settings** travel with the folder: languages, styles, fonts and
  fallbacks, default frame style, store mapping.
- **App settings** belong to your computer: store accounts (keys in the system
  keychain), imported Apple bezels and frame packs, AI assistant (MCP)
  settings, templates.

## Working in the editor

- Drag to move (with snapping), handles to resize and rotate, double-click a
  text to edit it inline, drop an image onto a device to set its screenshot.
- Right-click for cut/copy/paste, arrange, group, lock and hide.
- Plain drag on empty space pans, ⇧ + drag selects, ⌘/Ctrl + wheel zooms,
  ⌘0 fits, ⌘1 shows export pixels. Press `?` for every shortcut.
- One history for everything – including your AI assistant's edits.
