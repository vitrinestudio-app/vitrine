---
title: Migrating from fastlane frameit
linkTitle: fastlane migration
description: "Keep snapshot, screengrab, deliver and supply – replace frameit with Vitrine Studio."
weight: 50
icon: arrow
---

Vitrine Studio replaces **frameit** in a fastlane setup. Capturing
(snapshot, screengrab) and uploading (deliver, supply) can stay as they are.

## Step by step

1. **Import your captures.** Pick the whole `fastlane/` folder:

   ```sh
   vitrine import screenshots fastlane/
   ```

   `screenshots/` is read as snapshot output, `metadata/android/<locale>/images/<type>Screenshots/`
   as screengrab output (phone, 7″/10″ tablets, wear, tv). snapshot names like
   `iPhone 15 Pro Max-01Home.png` become the key `home`.

2. **Design once.** Build the layout in Vitrine Studio. Texts from
   `title.strings` / `keyword.strings` go into the project's strings, one file
   per language.

3. **Export the fastlane layout.**

   ```sh
   vitrine export --layout fastlane --out fastlane
   ```

   App Store targets go to `screenshots/<asc-locale>/<nn>_<target>.png`,
   Google Play targets to `metadata/android/<play-locale>/images/<type>/`, the
   feature graphic to `images/featureGraphic.png`.

4. **Drop `frameit` from your Fastfile.** Delete old `*_framed.png` files first –
   deliver uploads only those when it finds them. `upload_to_app_store` and
   `upload_to_play_store` then upload the folders unchanged.

## What the export checks

- deliver detects the screenshot set from the pixel size, so two targets of one
  size class in one locale folder are a **conflict** (the first wins, the other
  is reported). Same for two targets with one Play image type.
- More than 10 (Apple) or 8 (Google Play) images in a set → warning.
- Left out with a note: targets without a store slot (web, blog), App Store
  header and search-results creatives, sizes deliver rejects, languages without
  a store locale.

## Or skip deliver for screenshots

Vitrine Studio can [upload directly](../store-upload/) to App Store Connect
and Google Play – with a plan you confirm before anything is sent.
