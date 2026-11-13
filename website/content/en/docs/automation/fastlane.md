---
title: Using Vitrine Studio with fastlane
linkTitle: Using with fastlane
description: "Vitrine Studio in a fastlane lane: snapshot and screengrab capture, Vitrine Studio designs the screenshots, deliver and supply upload – step by step."
weight: 30
aliases: ["/docs/fastlane/"]
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
| frameit's device frames | built-in frames, or Apple's bezels imported once ([Devices & frames](../../devices-frames/)) |
| `*_framed.png` next to the captures | images in `export/` or written straight into fastlane's folders |

## Step by step

1. **Create a project** (app or `vitrine new`) next to your app, e.g. in
   `store-screens/`.

2. **Give the captures their own folder.** deliver uploads every image in
   `fastlane/screenshots/<locale>/`, supply every image in
   `fastlane/metadata/android/<locale>/images/<type>/` – and by default these
   are also the folders snapshot and screengrab save their captures in. Once
   Vitrine Studio writes the finished images there, the store would receive
   both. So let snapshot and screengrab write below `fastlane/captures/`,
   in their usual structure:

   ```ruby
   # fastlane/Snapfile
   output_directory("./fastlane/captures/screenshots")

   # fastlane/Screengrabfile
   output_directory("fastlane/captures/metadata/android")
   ```

   (Or pass `output_directory:` to `capture_screenshots` /
   `capture_android_screenshots` in the lane.) Captures you already have can
   simply be moved there.

3. **Import your captures** from that folder:

   ```sh
   vitrine import screenshots fastlane/captures --dry-run   # check the plan
   vitrine import screenshots fastlane/captures --save-mapping
   ```

   `screenshots/` is read as snapshot output, `metadata/android/<locale>/images/<type>Screenshots/`
   as screengrab output (phone, 7″ and 10″ tablets, wear, tv). `de-DE`
   becomes `de`, `iPhone 15 Pro Max-01Home.png` becomes the key `home`, and
   frameit's own files are skipped. `--save-mapping` remembers the mapping so
   the next import (after the next snapshot run) needs no questions. In the
   app: ☰ → *Import screenshots…*.

4. **Design once.** Lay out the canvas, put the headlines from your
   `title.strings` into the texts (one key per screen, one file per language –
   or import them as CSV, see [translators](../../languages/#working-with-translators-csv-and-xliff)).

5. **Export the fastlane layout** into your fastlane folder:

   ```sh
   vitrine export --layout fastlane --out fastlane
   ```

   App Store targets go to `screenshots/<asc-locale>/<nn>_<target>.png`,
   Google Play targets to `metadata/android/<play-locale>/images/<type>/`, the
   feature graphic to `images/featureGraphic.png`.

6. **Take `frameit` out of the lane** – the finished images now come from
   Vitrine Studio. `upload_to_app_store` and `upload_to_play_store` then
   upload the folders as usual. With `overwrite_screenshots: true`, deliver
   replaces the screenshots in App Store Connect instead of adding to them;
   supply replaces the screenshots of each image type it uploads.

A lane could look like this (`sh` runs inside `fastlane/`, hence the `../`):

```ruby
lane :screenshots do
  capture_screenshots                     # snapshot → fastlane/captures/screenshots
  sh("vitrine", "--project", "../store-screens", "import", "screenshots", "captures")
  sh("vitrine", "--project", "../store-screens", "export", "--layout", "fastlane", "--out", ".")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
end
```

### If the captures stay in the upload folders

You don't have to move them right away – nothing breaks:

- The import recognises Vitrine Studio's own images and skips them
  ("exported by Vitrine Studio"): every export lists its files in
  `.vitrine-export.json`, and files named like the export (`01_<target>.png`,
  `01_<locale>.png`) are skipped too.
- The export warns about every image in the folders it writes to that did
  not come from Vitrine Studio – raw captures, older `*_framed.png` files –
  because deliver and supply would upload them as well.
  `vitrine export --layout fastlane --out fastlane --remove-foreign` deletes
  exactly those images after the export; snapshot and screengrab capture them
  again on their next run. Other files are never touched.

## What the export checks

- deliver detects the screenshot set from the pixel size, so two targets of one
  size class in one locale folder are a **conflict** (the first wins, the other
  is reported). Same for two targets with one Play image type.
- More than 10 (Apple) or 8 (Google Play) images in a set → warning.
- Images in the upload folders that Vitrine Studio did not export → warning
  (see above). When a locale folder holds `*_framed.png` files, deliver
  uploads only those – the export points that out as well.
- iPad 12.9″ (2nd generation) images get deliver's `_app_ipad_pro_129` marker
  in their name.
- Left out with a note: targets without a store slot (web, blog), App Store
  header and search results creatives (they are not part of the screenshot
  sets in the fastlane folder layout), sizes
  deliver rejects, languages without a store locale.

## fastlane in CI

fastlane runs the same lane on your Mac and in CI – on Bitrise, Codemagic,
CircleCI, GitHub Actions, GitLab CI, Jenkins and others. On the CI machine,
install the CLI first ([Installing the CLI in CI](../../automation/#installing-the-cli-in-ci));
the lane stays the same. This one checks the project before exporting and
fails the build on warnings too:

```ruby
lane :screenshots do
  capture_screenshots
  sh("vitrine", "--project", "../store-screens", "import", "screenshots", "captures")
  sh("vitrine", "--project", "../store-screens", "validate", "--strict")
  sh("vitrine", "--project", "../store-screens", "export", "--layout", "fastlane", "--out", ".")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
end
```

(`--out .` writes `fastlane/screenshots/` and `fastlane/metadata/android/`.)
The CI machine has no Apple bezels of its own – see
[Device frames in CI](../../automation/#device-frames-in-ci).

## The fastlane plugin (draft)

{{< callout status >}}
The plugin `fastlane-plugin-vitrine` is a **draft**: it is not published yet
and has not been run inside fastlane. Until it is, the `sh` calls above do the
same.
{{< /callout >}}

It wraps the CLI in three actions:

| Action | Does |
|---|---|
| `vitrine_import` | imports the captures from `fastlane/captures` (else `fastlane/`; or `source:`), with `lang_map`, `device_map`, `key_map`, `save_mapping`, `dry_run`; stops the lane when files cannot be mapped |
| `vitrine_validate` | checks the project; stops the lane on errors (on warnings too with `strict: true`) |
| `vitrine_export` | exports the fastlane layout into `fastlane/` and points deliver and supply to it; stops the lane when the upload folders hold images Vitrine Studio did not export (`foreign_images: "warn"` only reports them, `"remove"` deletes them) |

```ruby
lane :screenshots do
  capture_screenshots                     # Snapfile: output_directory("./fastlane/captures/screenshots")
  vitrine_import(project: "store-screens")
  vitrine_validate(project: "store-screens", strict: true)
  vitrine_export(project: "store-screens")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
end
```

Paths in the actions are relative to your project root (one level above
`fastlane/`).

## Upload the screenshots with Vitrine Studio

deliver and supply keep uploading your app binary and metadata. The
screenshots can be handled by Vitrine Studio if you like: it
[uploads directly](../../store-upload/) to App Store Connect and Google Play,
with a plan you confirm before anything is sent, and incrementally (unchanged
sets are skipped). In the lane, you then leave the screenshots out of deliver
and supply:

```ruby
upload_to_app_store(skip_screenshots: true)          # binary and metadata as before
upload_to_play_store(skip_upload_screenshots: true)
```

How to run the upload from CI: [Uploading from CI](../../automation/#uploading-from-ci).
