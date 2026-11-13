---
title: Export
description: "Export every target × language as PNG or JPEG, in Vitrine Studio's folder layout or fastlane's, with an index.html overview – incrementally, from the app or the command line."
weight: 100
icon: export
---

## Exporting from the app

**Export** in the top bar (or <kbd>⌘</kbd><kbd>E</kbd>, or ☰ → *Export…*):

1. Pick what to export in the grid of **targets × languages** – *All* toggles
   a row or column. Cells that a target doesn't export are marked *Not
   exported for this target*.
2. **Output folder:** empty = `export/` in the project folder; **Choose…** for
   another one.
3. **Layout:** *Standard* or *fastlane* (below).
4. **Options:** *Re-render everything* (ignore the cache), *Gallery previews*,
   *Overview page (index.html)*.
5. **Export.** Progress per batch; **Cancel** stops it.

The summary shows how many files were *Written* and *Unchanged*, the issues
found, and **Show in Finder** / **Show in Explorer**. **Export again** re-runs
the same selection; if everything was already up to date it says so (with a
button to *Re-render everything*). From the dialog, **Upload…** goes on to
the [store upload](../store-upload/).

## What you get

```text
export/
  index.html
  ios-iphone-69/
    en/01_plan.png
    en/02_beds.png
    …
    en/_gallery.png
    de/…
  ios-ipad-13-landscape/en/01_plan+beds.png     ← two pages per image
  play-feature-graphic/en/banner.png
  web-slides/en/01_plots.jpg
```

- One folder per target and language; files are numbered in page order and
  named after the **page ids** (*Canvas settings → Page ids*). Grouped pages
  join their ids with `+`; banners are `banner.png`.
- `_gallery.png` is a preview of the store gallery (not for upload; switch off
  with *Gallery previews*).
- PNG or JPEG as set in the target (*Format*, *quality*). Images are always
  **opaque** (no transparency) and **sRGB** (PNG with an sRGB tag) – what the
  stores expect.

## The overview page

`index.html` in the export folder lists every exported image by language and
target in page order, with file name, pixel size and file size. Click an image
to enlarge it, <kbd>←</kbd>/<kbd>→</kbd> to step through, <kbd>Esc</kbd> to
close. It is self-contained – it opens from the file system, needs no
internet, and has a light and dark look – handy to send around for review. It
lists all current files in the folder, also from earlier exports of other
targets.

{{< placeholder "The index.html overview page of an export, with the enlarged view." >}}

## Incremental exports

Vitrine Studio remembers what it wrote (`export/.vitrine-export.json`). The next
export renders only images whose input changed – a changed text, screenshot,
style or setting – and skips the rest. *Re-render everything* / `--force`
renders all of them.

When a target produces fewer or different images than in the last export
(say you deleted pages or renamed page ids), the export removes the images **it wrote earlier** for those targets and languages but no longer
produces. It never deletes other files in the folder.

`export/` and `.cache/` are listed in the project's `.gitignore` – exports are
build output.

## fastlane layout

Choose *Layout → fastlane* (or `--layout fastlane`) to write the folder
structure that fastlane's **deliver** and **supply** upload unchanged (default
folder `export/fastlane/`):

```text
screenshots/en-US/01_ios-iphone-69.png              ← deliver
screenshots/de-DE/…
metadata/android/en-US/images/phoneScreenshots/01_en-US.png   ← supply
metadata/android/en-US/images/featureGraphic.png
```

- Only targets with a store slot; no gallery previews (deliver would try to
  upload them).
- Locales come from the project's store languages (`de → de-DE`, Hebrew on
  Play `iw-IL`).
- Left out with a note: targets without a store slot (web, blog), App Store
  header and search results creatives, sizes deliver rejects, languages without
  a store locale.
- deliver recognises the screenshot set by pixel size, so two targets of the
  same size class in one locale folder are a **conflict** (the first wins, the
  other is reported). Same for two targets with one Play image type. More than
  10 (Apple) or 8 (Play) images in a set → warning.
- If the folder still contains frameit `*_framed.png` files, you get a warning
  – deliver would upload only those.

Step by step: [Using with fastlane](../fastlane/).

## Exporting from the command line

```sh
vitrine export                                        # everything, current folder
vitrine export --project my-app --target ios-iphone-69 --lang de
vitrine export --out ~/Desktop/shots --force          # another folder, re-render all
vitrine export --layout fastlane --out ../fastlane    # deliver/supply folders
vitrine export --no-gallery --no-html --format json   # for scripts
vitrine export --gallery-only --target play-phone     # only _gallery.png, a quick look
```

A full export of the example project (nine targets, four languages) takes
about two seconds in a release build. Projects with many full-size photos
take longer – most of the time goes into compressing the PNGs (about 25
seconds for 84 photo pages). `--gallery-only` writes just the gallery
previews and keeps the page files of earlier exports. Exit code 1 means some files could not be
exported; see [CLI reference](../cli/).

## Rendering single previews

`vitrine render` writes one PNG for a quick look – one image, the whole canvas
or the gallery – at any size:

```sh
vitrine render --target play-phone --page 2 --width 600 -o page2.png
vitrine render --canvas --lang de                     # the canvas with hatched gaps
vitrine render --gallery --max-edge 800 -o gallery.png   # gallery 800 px high
```

Without `-o` the file goes to `.cache/render/` and its path is printed.
