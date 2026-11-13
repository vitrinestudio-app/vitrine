---
title: Using Vitrine Studio with fastlane
linkTitle: Using with fastlane
description: "Vitrine Studio in a fastlane lane: snapshot and screengrab capture, Vitrine Studio designs the screenshots, deliver and supply upload – step by step."
weight: 105
icon: arrow
---

I used fastlane and frameit for my apps' store screenshots for years myself,
and I am very grateful to Felix Krause and everyone who keeps fastlane going
for their work. That's why Vitrine Studio is built to **fit into fastlane**:
snapshot and screengrab keep capturing, deliver and supply keep uploading –
Vitrine Studio takes over designing the screenshots in your lane, the job
frameit does there. If you like, it can upload the screenshots too.

## What maps to what

| With frameit | With Vitrine Studio |
|---|---|
| one framed image per screenshot | one canvas for the whole set; pages can share backgrounds and devices |
| `Framefile.json` (background, font, padding) | canvas backgrounds, text styles, device positions – designed visually |
| `title.strings`, `keyword.strings` | the texts in `i18n/<lang>.yaml` (one key per headline) |
| frameit's device frames | built-in frames, or Apple's bezels imported once ([Devices & frames](../devices-frames/)) |
| `*_framed.png` next to the captures | images in `export/` or written straight into fastlane's folders |

## Step by step

1. **Create a project** (app or `vitrine new`) next to your app, e.g. in
   `store-screens/`.

2. **Import your captures.** Pick the whole `fastlane/` folder:

   ```sh
   vitrine import screenshots fastlane/ --dry-run   # check the plan
   vitrine import screenshots fastlane/ --save-mapping
   ```

   `screenshots/` is read as snapshot output, `metadata/android/<locale>/images/<type>Screenshots/`
   as screengrab output (phone, 7″ and 10″ tablets, wear, tv). `de-DE`
   becomes `de`, `iPhone 15 Pro Max-01Home.png` becomes the key `home`, and
   frameit's own files are skipped. `--save-mapping` remembers the mapping so
   the next import (after the next snapshot run) needs no questions. In the
   app: ☰ → *Import screenshots…*.

3. **Design once.** Lay out the canvas, put the headlines from your
   `title.strings` into the texts (one key per screen, one file per language –
   or import them as CSV, see [translators](../languages/#working-with-translators-csv-and-xliff)).

4. **Export the fastlane layout** into your fastlane folder:

   ```sh
   vitrine export --layout fastlane --out fastlane
   ```

   App Store targets go to `screenshots/<asc-locale>/<nn>_<target>.png`,
   Google Play targets to `metadata/android/<play-locale>/images/<type>/`, the
   feature graphic to `images/featureGraphic.png`.

5. **Take `frameit` out of the lane** – the finished images now come from
   Vitrine Studio – and delete old `*_framed.png` files: deliver uploads only
   those when it finds any. `upload_to_app_store` and `upload_to_play_store`
   then upload the folders as usual.

A lane could look like this:

```ruby
lane :screenshots do
  capture_screenshots                     # snapshot, as before
  sh("vitrine", "import", "screenshots", "../fastlane", "--project", "../store-screens")
  sh("vitrine", "export", "--layout", "fastlane", "--out", "../fastlane", "--project", "../store-screens")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true)
end
```

## What the export checks

- deliver detects the screenshot set from the pixel size, so two targets of one
  size class in one locale folder are a **conflict** (the first wins, the other
  is reported). Same for two targets with one Play image type.
- More than 10 (Apple) or 8 (Google Play) images in a set → warning.
- iPad 12.9″ (2nd generation) images get deliver's `_app_ipad_pro_129` marker
  in their name.
- Left out with a note: targets without a store slot (web, blog), App Store
  header and search results creatives (they are not part of the screenshot
  sets in the fastlane folder layout), sizes
  deliver rejects, languages without a store locale.

## Upload the screenshots with Vitrine Studio

deliver and supply keep uploading your app binary and metadata. The
screenshots can be handled by Vitrine Studio if you like: it
[uploads directly](../store-upload/) to App Store Connect and Google Play,
with a plan you confirm before anything is sent, and incrementally (unchanged
sets are skipped). In the lane, you then leave the screenshots out of deliver
and supply:

```ruby
upload_to_app_store(skip_screenshots: true)          # binary and metadata as before
upload_to_play_store(skip_upload_screenshots: true)
```

How to run the upload from CI: [Uploading from CI](../ci/#uploading-from-ci).
