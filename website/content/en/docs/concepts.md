---
title: How Vitrine Studio works
linkTitle: How it works
description: "Projects, canvases and pages, gaps, home pages, targets × languages × variants, lu units – the few ideas everything else builds on."
weight: 10
icon: formats
---

Store screenshots multiply quickly: several pages, several device classes,
several languages, two stores. Vitrine Studio builds on one idea: you design
**once**, and the app renders every image you need.

```text
Project ─┬─ Canvas "phone"  ── pages 1 … 6, backgrounds, devices, texts
         └─ Canvas "mac"    ── pages 1 … 4, backgrounds, devices, texts

Target × Language × Variant  →  finished images
(iPhone 6.9″, Google Play phone, iPad 13″ …) × (en, de, fr …)
```

## A project is a folder

A project is a plain folder: `project.yaml` (the layout), one text file per
language in `i18n/`, your app screenshots in `screenshots/`, plus optional
images, fonts and frames. No account, no cloud, nothing hidden – you can keep
it in Git, diff it and review it. Every change in the app is **saved
immediately**; there is no Save command. If the files change on disk (a text
editor, `git checkout`, an AI assistant), the open project reloads live.

Details: [Project file format](../file-format/).

## Canvas and pages

A **canvas** is a row of pages that you design as **one picture**. Each page
becomes one store screenshot. Because it is one picture:

- a background can run continuously across all pages (the canvas
  backgrounds are layers over the whole canvas; for a colour or photo per
  page, put a *Background* element on each page);
- a device can sit across the border between two pages and still exists only
  once – in the store it looks like one phone spanning two screenshots;
- you can see the whole set as it will appear in the store while you work.

A project can have several canvases – for example *phone*, *tablet* and
*mac* – each with its own page format.

{{< shot src="images/canvas-phone-en.jpg" alt="A canvas with six phone pages: a green gradient and a diagonal band run across all pages, one phone sits across the border of pages 3 and 4." caption="The example project's phone canvas: six pages, one continuous background, one phone across pages 3 and 4. The hatched stripes are the store gaps." >}}

## Gaps like in the store

The App Store and Google Play show screenshots side by side with a small gap
between them. The canvas has the **same gaps** (hatched in the editor), so a
device or a line that crosses a page border continues correctly in the store.
Anything that lies inside a gap is simply not part of any exported image.

The gap sizes come from the target's **gallery profile**. Values for the store
websites were measured; the values for the native store apps are still
estimates.

## Home page

Every element (device, text, image, shape) belongs to one **home page**. Its
position is relative to that page, and it may reach into the neighbouring
pages. Texts are clipped to their home page by default, so a headline never
leaks into the next screenshot.

## One unit: lu

A page is always **1000 lu** (logical units) wide; its height follows the page
aspect (an iPhone page is about 2173 lu high). All positions, sizes, font
sizes and margins are in lu. That's why one layout works for every export size:
the pixel size only comes from the target.

## Targets

A **target** is one output: "App Store iPhone 6.9″", "Google Play phone",
"iPad 13″ landscape", "Mac App Store", the Google Play feature graphic, a blog
header … It names the canvas it shows, the pixel size, the store slot, the
device frames to use, and the gallery profile. Most targets come from
[presets](../targets/) with the store's exact sizes.

A target can adapt the canvas without duplicating it:

- **Different aspect.** A canvas designed for iPhone (about 1:2.17) is also
  rendered for Google Play (1:2). Elements keep their distance to the top,
  the bottom or the centre of the page, depending on their **anchor**.
- **Grouping.** `Pages per image: 2` puts two portrait pages side by side into
  one landscape image – an iPad landscape set from the phone canvas. Tablets
  can of course have a canvas of their own; grouping is just a handy shortcut.
- **Device swap.** The Play target swaps the iPhone frames for Android frames.
- **Banner.** One image from a region of the canvas, e.g. the feature graphic or a blog post header.

## Variants

Sometimes a format needs a slightly different arrangement – the phone in the
iPad landscape set sits a bit lower, the band is less steep. A **variant**
stores exactly those differences (position, size, rotation, home page,
visibility …) as **overrides**. Texts, screenshots and styles stay shared. A
target shows one variant; everything not overridden comes from the base
layout.

## Languages

Every target is rendered in every project language. Texts live in one file per
language; screenshots in one folder per language. Anything missing comes from a
**fallback** – `fr-CA` falls back to `fr`, everything finally to the default
language – so a partial translation or a partial screenshot set still renders.
A **language override** tweaks a text element for one language only (a wider
box for the long German headline). See [Languages & translation](../languages/).

## Targets × languages × variants = your images

Put together: you edit one canvas; each target renders it at its size with its
variant, in each language. A project with 3 targets and 4 languages gives you
12 complete image sets – and when you change a headline, all of them change.

{{< shot src="images/ipad-gallery-de.jpg" alt="The iPad landscape gallery of the example project in German: two phone pages per landscape image." caption="The same canvas as the iPad 13″ landscape target in German: two pages per image, a variant moves the phones, a device swap draws iPads." >}}

## One renderer for everything

The editor preview, the exported files, the gallery previews, the store upload
and the previews an AI assistant gets all come from the same renderer. What you
see in the editor is exactly what ends up in the store.

## Three ways to work

- **The desktop app** – the visual editor. Start here.
- **The command line tool `vitrine`** – validate, render, export, translate,
  import and upload from scripts and CI. See the [CLI reference](../cli/).
- **AI assistants over MCP** – Claude and other assistants can design,
  translate, check and export in the project you have open, live. See
  [AI assistants](../ai-mcp/).

All three work on the same project folder and can be mixed freely.
