---
title: Screenshots
description: "Where your app screenshots go, how keys and screenshot variants work, importing whole folders (also fastlane), placeholders and colour profiles."
weight: 90
icon: image
---

## Keys: one name per screen

Each device element shows the screenshot with its **key** – a short name like
`home`, `planner` or `calendar` (inspector → *Screenshot*). The key is the
file name without extension. One key can have a file per language and per
device class, and Vitrine Studio picks the right one for each target.

## The folder layout

```text
screenshots/
  en/
    ios-phone/home.png        ← iPhone targets, English
    ios-phone/planner.png
    android-phone/home.png    ← Google Play phone targets
    ios-tablet/home.png       ← iPad targets
    phone-sync.png            ← no variant folder: serves every target
  de/
    ios-phone/home.png        ← German; missing ones come from en
```

`screenshots/<language>/<screenshot variant>/<key>.png` – PNG or JPEG.

- The **screenshot variant** folder comes from the target (*Screenshots* in the
  target form): `ios-phone`, `android-phone`, `ios-tablet`, `mac`, `watch`,
  `wear`, `web` … So the iPhone target shows iPhone captures and the Play
  target Android captures of the same screen, under the same key.
- A file **directly in the language folder** (`screenshots/en/home.png`)
  serves every variant – the easiest start, and fine when your iPhone and
  Android captures look alike.
- **Lookup order** per key: the language's variant folder, the language
  folder, then the same for each fallback language (`fr-CA → fr → en`).

## Adding screenshots in the app

- **Drop a file onto a device** on the canvas: it becomes that device's
  screenshot for the current language (the key and folder are taken care of).
- **Assets → Import…** (or drop files onto the Assets tab or anywhere on the
  canvas): choose *Kind* (*Screenshot*, *Image (assets/)*, *Font (fonts/)*),
  *Language*, *Variant* and *Key*. The variant is detected from the pixel size
  when possible.
- **Import screenshots…** (☰ menu, or **Import folder…** in the Assets tab):
  a whole folder at once – see below.

**Assets → Screenshots** lists every key per language and variant, marks
*missing* and *unused* ones, and which are borrowed from a fallback language.

## Importing a folder (and fastlane)

The import understands three layouts and detects them automatically:

- **Vitrine Studio folders** (`<lang>/<variant>/<key>.png`);
- **fastlane** – snapshot output (`de-DE/iPhone 17 Pro Max-01_home.png`) and
  screengrab output (`metadata/android/<locale>/images/phoneScreenshots/`);
  you can pick the whole `fastlane/` folder;
- **Loose files** in one folder.

What it does:

- Locales become project languages (`de-DE → de`; store codes like `iw-IL →
  he` are understood).
- Device names become screenshot variants – by your mapping or by pixel size
  (*Device → screenshot variant*, *Locale → language*). screengrab folders map
  to `android-phone`, `android-tablet` (7″/10″) and `wear`.
- Names become keys: `01_home.png` → `home`, `iPhone 15 Pro Max-01Home.png` →
  `home`.
- frameit leftovers and store graphics are **skipped with a reason**:
  `*_framed.png`, `Framefile.json`, `title.strings`, `keyword.strings`,
  frameit backgrounds, deliver metadata, Play's feature graphic and icons.

The dialog shows every file with its detected *Language*, *Variant* and *Key*
(*certain*, *probable*, *uncertain*), what will happen (*new*, *replace*,
*unchanged*) and which screenshots would still be *missing* afterwards. Rows
that need a decision are marked. **Save mapping to project** stores the
device, locale and key mappings in `project.yaml`, so the next import (e.g.
after a new snapshot run) maps the same way without questions.

{{< placeholder "The Import screenshots dialog with a fastlane folder: detected languages, variants and keys." >}}

On the command line:

```sh
vitrine import screenshots fastlane/ --dry-run      # show the plan
vitrine import screenshots fastlane/ --save-mapping
vitrine import screenshots shots/ --device-map "iPhone 17 Pro Max=ios-phone" --lang-map pt-PT=pt --key-map 01_home=home
```

More on replacing frameit: [Migrating from fastlane](../fastlane/).

## Missing screenshots and placeholders

A key without a file in any language of the chain shows a **placeholder** in
the editor, so you can design before you have captures. Validation reports it
as an error (`screenshot_missing`), and the store upload blocks every set that
still has one – so no placeholder ends up in the store by accident. (An export
still writes the image with the placeholder, which is handy for reviews.)
While designing, **Preview mode** in the
validation list (or `vitrine validate --preview`) treats them as warnings.

## Size and aspect

- A screenshot whose aspect differs from the device's display by more than a
  little gets a warning (`screenshot_aspect`, "wrong device?") – e.g. an
  iPhone capture in an Android frame. *Fit* (*Cover*, *Contain*, *Stretch*)
  decides how it fills the display.
- A screenshot enlarged by more than 15 % gets a warning
  (`screenshot_upscale`) – it may look soft. Use captures from the largest
  device of a class.

## Colour profiles (Display P3)

iPhone and Mac screenshots are usually in **Display P3**. Vitrine Studio works
in sRGB and converts images with a colour profile to sRGB when loading them
(Display P3, Adobe RGB, grey profiles …; PNG and JPEG), the same way in the
editor, the export, the upload and assistant previews. Images without a
profile are treated as sRGB. Very saturated P3 colours are clipped to the
nearest sRGB colour – the same as in any sRGB export.

If a profile can't be used (CMYK JPEG, HDR PNG, an unreadable profile), the
image is drawn as if it were sRGB and validation notes
`color_profile_unsupported` – re-save it as sRGB or Display P3 PNG.
