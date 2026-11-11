---
title: Import & export
description: "Bring screenshots in from any folder or fastlane, export images in Vitrine Studio's or fastlane's layout."
weight: 40
icon: folder
---

## Importing screenshots

Vitrine Studio reads its own folder layout, fastlane snapshot/screengrab
output and flat folders:

- Locale folders like `de-DE` become `de`; store locales such as `iw-IL` map to
  `he`.
- Device names map to variants by your mapping or by pixel size;
  `01_home.png` becomes the key `home`.
- frameit leftovers (`_framed.png`, `Framefile.json`, `title.strings`) and
  store graphics are skipped – with a reason.
- The mapping can be saved to `project.yaml` so re-imports run unattended.

```sh
vitrine import screenshots fastlane/ --dry-run
vitrine import screenshots shots/ --device-map "iPhone 17 Pro Max=ios-phone" --save-mapping
```

In the app: *Import screenshots…* – or drop images onto the canvas.

## Exporting

```sh
vitrine export --project my-app                         # every target × language
vitrine export --target ios-iphone-69 --lang de --force # one target, re-render everything
```

- Files: `export/<target>/<lang>/<nn>_<page-id>.png`, banners as
  `banner.png`, plus a `_gallery.png` preview.
- An `index.html` overview lists every image by language and target – it opens
  offline, light and dark (`--no-html` to skip it).
- Exports are **incremental** (unchanged images are skipped), always opaque
  and sRGB. Screenshots in Display P3 or other profiles are converted once.
- Re-exporting removes images the export wrote earlier but no longer produces
  – never other files.

## fastlane layout

`vitrine export --layout fastlane --out fastlane` writes the folders
`deliver` and `supply` upload: `screenshots/<asc-locale>/` and
`metadata/android/<play-locale>/images/<type>/`. See
[Migrating from fastlane frameit](../fastlane/).
