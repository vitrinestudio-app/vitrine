# fastlane screenshot conventions (snapshot, frameit, deliver, screengrab, supply)

Checked 2026-11-10 against fastlane `master`. The source files were read
through the jsDelivr mirror `https://cdn.jsdelivr.net/gh/fastlane/fastlane@master/<path>`.
Its content is byte-identical to `https://raw.githubusercontent.com/fastlane/fastlane/master/<path>`,
which is the URL cited below. No commit SHA is pinned; the mirror can lag
`master` by a few hours to days. Docs pages are the rendered
`fastlane/lib/fastlane/actions/docs/*.md` on docs.fastlane.tools.

Machine-readable values: `crates/core/data/fastlane.yaml`. Used by the
screenshot import (`--format fastlane`) and the export layout
`--layout fastlane`.

Why: many apps capture screenshots with fastlane snapshot (iOS) or
screengrab (Android), frame them with frameit and upload them with deliver
or supply. Vitrine Studio can replace frameit. It imports the captures and
exports folders that deliver/supply upload unchanged.

## deliver (iOS upload)

Source: `deliver/lib/deliver/loader.rb`.

- **Folder layout:** `<screenshots_path>/<locale>/*.{png,jpg,jpeg}`.
  - Glob with `File::FNM_CASEFOLD`, one level deep (no recursion).
  - `iMessage/<locale>/` and `appleTV/<locale>/` are expanded one level
    deeper (`EXPANDABLE_DIR_NAMES`).
  - `default` is accepted as a folder name, but screenshots in it get no
    language. The `default` fallback in the docs covers metadata only.
  - `android` and `fonts` are skipped.
- **Locale folders:** matched case-insensitively against
  `FastlaneCore::Languages::ALL_LANGUAGES`
  (`fastlane_core/lib/fastlane_core/languages.rb`).
  - That is exactly the 50 App Store Connect codes of our
    `store-locales.yaml` (`de-DE`, `en-US`, `fr-CA`, `ja`, `no`, `zh-Hans` …),
    and a unit test checks they are the same.
  - Unknown folders are a user error.
- **frameit output in the folder:** as soon as a locale folder contains any
  `*_framed.{png,jpg,jpeg}`, deliver keeps only the files whose path
  contains `framed` or `watch` and skips all others.
  - The Vitrine Studio export warns when it finds such leftovers in a deliver
    folder it writes to.
- **Extensions** (`app_screenshot_validator.rb`): `png`/`PNG`,
  `jpg`/`JPG`/`jpeg`/`JPEG`.
  - The extension must match the real format.
  - An unknown pixel size is `INVALID_SCREEN_SIZE` and aborts the whole
    upload. That is why the fastlane export layout never writes gallery
    previews into deliver folders.
- **Display type by pixel size** (`deliver/lib/deliver/app_screenshot.rb`,
  `DEVICE_RESOLUTIONS`, verbatim in `fastlane.yaml`):

  | Display type | Pixel sizes |
  |---|---|
  | `APP_IPHONE_67` (6.9") | 1260×2736, 1290×2796, 1320×2868 |
  | `APP_IPHONE_65` | 1242×2688, 1284×2778 |
  | `APP_IPHONE_61` (6.3") | 1179×2556, 1206×2622 |
  | `APP_IPHONE_58` | 1170×2532, 1125×2436, 1080×2340 |
  | `APP_IPHONE_55` | 1242×2208 |
  | `APP_IPHONE_47` | 750×1334 |
  | `APP_IPHONE_40` | 640×1096, 640×1136, 1136×600, 1136×640 |
  | `APP_IPHONE_35` | 640×920, 640×960, 960×600, 960×640 |
  | `APP_IPHONE_DUO` | 1398×2034, 2007×2853 |
  | `APP_IPAD_97` | 1024×748, 1024×768, 2048×1496, 2048×1536 and portrait |
  | `APP_IPAD_105` | 1668×2224 |
  | `APP_IPAD_PRO_3GEN_11` | 1488×2266, 1668×2420, 1668×2388, 1640×2360 |
  | `APP_IPAD_PRO_129` | 2048×2732 |
  | `APP_IPAD_PRO_3GEN_129` (13") | 2048×2732, 2064×2752 |
  | `APP_DESKTOP` | 1280×800, 1440×900, 2560×1600, 2880×1800 |
  | `APP_WATCH_SERIES_3/4/7/10` | 312×390, 368×448, 396×484, 416×496 |
  | `APP_WATCH_ULTRA` | 410×502, 422×514 |
  | `APP_APPLE_TV` | 1920×1080, 3840×2160 |
  | `APP_APPLE_VISION_PRO` | 3840×2160 |

  - Both orientations are listed unless noted otherwise.
  - Under `iMessage/<locale>/` the `IMESSAGE_APP_*` twins are used. They
    exist for iPhone 4.0" to 6.9" and the iPads, not for 3.5", Duo, Watch,
    TV, Mac or Vision.
  - **Ambiguous sizes:**
    - 2048×2732 is `APP_IPAD_PRO_3GEN_129` unless the path contains
      `app_ipad_pro_129`, or both `12.9` and `2nd generation`. This has been
      the default since fastlane 2.230.0 (docs.fastlane.tools/actions/deliver/);
      before that the file name needed `IPAD_PRO_3GEN_129` or
      `iPad Pro (12.9-inch) (3rd generation)`.
    - 3840×2160 is `APP_APPLE_TV` unless the path contains `vision`.
- **Order** (`deliver/lib/deliver/upload_screenshots.rb`): after the upload
  each set is reordered with `Naturally.sort(…, by: :file_name)` (natural
  sort by file name).
- **Limit:** 10 per display type and locale; remote screenshots count.
  `overwrite_screenshots` deletes the existing ones first.

## snapshot (iOS capture)

- **Output directory:** `fastlane/screenshots` when a `fastlane` folder
  exists, else `screenshots` (`snapshot/lib/snapshot/options.rb`).
- **Locale folders:** `<locale>/`.
- **File name:** `"\(simulator)-\(name).png"`
  (`snapshot/lib/assets/SnapshotHelper.swift`). The simulator name keeps
  its spaces and has "Clone N of " stripped, e.g.
  `iPhone 15 Pro Max-01Home.png`.
- **Report:** `screenshots.html` (`reports_generator.rb`).
- **No other suffixes:** no dark-mode suffix, and no launch-argument suffix
  outside the legacy pre-Xcode-9 path.

## frameit

Sources: `frameit/lib/frameit/runner.rb`, `editor.rb`, `strings_parser.rb`.

- **Output:** `<name>_framed.png`. Inputs are found by the recursive glob
  `**/*.{png,PNG}`.
- **`Framefile.json`:** searched one, two and four levels above the image
  (the docs say: in the `screenshots` folder).
  - `data[].filter` is a substring of the path.
  - `background` is a relative image path, typically `./background.jpg`.
- **`title.strings` / `keyword.strings`:** in the locale folder (searched
  up to three levels up). The key is a case-insensitive substring of the
  path; the files are UTF-8 or UTF-16.

## screengrab (Android capture)

- **Output directory:** `fastlane/metadata/android/<locale>/images/<device_type>Screenshots/`
  (`screengrab/lib/screengrab/runner.rb`).
  - `device_type` is one of `phone`, `sevenInch`, `tenInch`, `tv`, `wear`.
  - Locales use the `en-US` form.
- **File name:** `<name>_<System.currentTimeMillis()>.png`, or `<name>.png`
  with `use_timestamp_suffix: false` (`FileWritingScreenshotCallback.java`).

## supply (Android upload)

- **Constants** (`supply/lib/supply.rb`):
  - `IMAGES_TYPES = %w(featureGraphic icon tvBanner)`
  - `SCREENSHOT_TYPES = %w(phoneScreenshots sevenInchScreenshots tenInchScreenshots tvScreenshots wearScreenshots)`
  - `IMAGE_FILE_EXTENSIONS = "{png,jpg,jpeg}"`
  - The docs also list `promoGraphic`, but the code does not upload it.
- **Lookup** (`supply/lib/supply/uploader.rb`):
  - Single images: `<locale>/images/<type>.{png,jpg,jpeg}`; the last glob
    hit wins.
  - Screenshots: `<locale>/images/<type>/*.{png,jpg,jpeg}`, sorted by a plain
    `.sort` (byte order). The docs say "alphanumerical filename order".
- **Metadata path:** `./fastlane/metadata/android` (`supply/lib/supply/options.rb`).
- **Locale folders:** Play codes (`en-US`, `de-DE`; `supply init` writes the
  API's `listing.language`).
- **Limit:** supply enforces no count; Play allows 8 per type.

## Consequences for Vitrine Studio

**Import** (`vitrine import screenshots <dir> --format fastlane|auto`):
- A whole `fastlane/` folder can be picked: `screenshots/` is read as
  snapshot output, `metadata/android/` as screengrab output.
- The screengrab folder gives the variant: phone → `android-phone`,
  7"/10" → `android-tablet`, wear → `wear`, tv by pixel size. A device map
  entry for the folder name overrides this.
- The screengrab timestamp is dropped from the name.
- snapshot's `01Home` naming becomes the key `home`.
- Locale folders also match through the store locale maps
  (`iw-IL` → `he`, `zh-Hans`).
- Skipped with a reason: frameit configs, the backgrounds a
  `Framefile.json` names, `*_framed`, `screenshots.html`, `iMessage/` and
  `appleTV/`, deliver metadata, Play store graphics.

**Export** (`vitrine export --layout fastlane`): see `core::fastlane` and
the notes in `docs/guide/NOTES.md`.
- deliver: `screenshots/<asc-locale>/<nn>_<target>.<ext>`.
- supply: `metadata/android/<play-locale>/images/<type>/<nn>_<play-locale>.<ext>`
  and `images/<type>.<ext>`.
- Upload-set conflicts and count limits are reported.
