# 0012 – Imported device frames (Apple Product Bezels)

- Status: accepted (iPhone 15+ rules verified with a real package on
  2026-11-10; iPad / Mac rules still unverified)
- Date: 2026-11-10

## Context

KONZEPT §4.2: Apple's product bezels may be used in marketing material but
must not be redistributed by an open-source project. The user downloads
them; Vitrine Studio imports them app-wide, recognises device/colour/orientation
from file names via data (`crates/core/data/frame-import/apple.yaml`),
detects the screen automatically, and projects only reference
`frame: imported:apple`. On a machine without the package the parametric
frame is drawn and validation says so.

## Research (2026-11-10, no Apple files downloaded)

| Finding | Confidence | Source |
|---|---|---|
| One `.dmg` per device family (`Bezel-iPhone-17.dmg`, `Bezel-iPad-Pro-(M5).dmg`, `Bezel-MacBook-Air-M5.dmg`, `Bezel-iMac-M4.dmg`, `Bezel-Apple-Watch-Series-11-2025.dmg`, …); no ZIP; older families are removed from the page | high | <https://developer.apple.com/design/resources/> |
| DMGs contain PSDs + PNGs and licence RTFs; ~600 MB per family | medium | <https://github.com/extratone/bezels>, iphone-ticker.de |
| PNG names `<Model> - <Colour> - <Orientation>.png` with ASCII hyphens, e.g. `iPhone 14 Pro - Space Black - Portrait.png` | medium | <https://github.com/joshluongo/frameme> |
| Phones/iPads have Portrait and Landscape PNGs, Macs none | medium | <https://github.com/viticci/frames-cli>, <https://github.com/jonnyjackson26/device-frames-media> |
| Bezel = native screen + ~100 px margin (17 Pro Max 1520×3068, screen 1320×2868 at 100,100); screen area transparent | medium / high | device-frames-media, <https://github.com/weirdapps/mockups>, <https://github.com/arturs105/device-framer> |
| Island/notch opaque inside the transparent screen; no drop shadows in the PNGs | low–medium | inferred from tools that flood-fill the screen |
| Licence: mock-ups for Apple-platform apps; no redistribution, no embedding in software, no modification as clip art; use "as is" | high | [licence PDF](https://developer.apple.com/support/downloads/terms/apple-design-resources/Apple-Design-Resources-License-20230621-English.pdf), <https://developer.apple.com/app-store/marketing/guidelines/> |

iPad and Mac name patterns (`iPad Pro 13 - M4 - Space Black - Portrait`,
`MacBook Pro 14 - M4 - Space Black`) are educated guesses. Rules carry
`verified: <date>` once checked; the importer and GUI warn when a file
matched an unverified rule and offer manual assignment for everything the
rules miss.

### Real package (2026-11-10, `Bezel-iPhone-18.dmg`, checked by the user)

- The DMG has a **software licence agreement** (SLA): `hdiutil attach`
  without an answer on stdin fails with `hdiutil: attach canceled`.
  Vitrine Studio never accepts it for the user (no piped `Y`).
- Contents: PNGs `iPhone 18 Pro Max - <Colour> - <Portrait|Landscape>.png`
  and `iPhone 18 Pro - <Colour> - <Portrait|Landscape>.png`, colours
  Black, Burgundy, Glacier, Silver; Photoshop files; `License.rtf`
  (17 non-PNG files in all).
- Detected screens: Pro Max 1320×2868 at 75,66 (portrait) / 2868×1320 at
  66,76 (landscape); Pro 1206×2622 at 72,69 / 2622×1206 at 69,72; i.e.
  offsets of 66–76 px, not the ~100 px of older packages.
- The first importer kept one file per model × colour × orientation and
  dropped the 18 Pro as "superseded" by the Pro Max. But the 18 Pro screen
  (1206×2622) is exactly App Store Connect's required 6.3" size and the
  Pro Max (1320×2868) the 6.9" size: both are needed.

## Decision

### Rules (data)

`frame-import/<source>.yaml`: ordered rules, first match wins.
`pattern` is matched against the normalised file name (no folder, no
`.png`, case-insensitive, en/em dash → `-`, typographic quotes → `"`, one
space) with placeholders `{color}` (non-empty, never contains ` - `),
`{orientation}` (`Portrait`/`Landscape`) and `{*}` (any). Rule fields:
`model` (catalog id), `orientation` (when the name has none, Macs),
`priority` (tie-break of the automatic bezel choice, and between duplicate
files of one vendor model), `verified` (date checked against a real
package), `note`. The **vendor model** (`iPhone 18 Pro`) is the file name
before ` - <colour>`.
`color_aliases` map vendor names to ids; otherwise the colour is slugified
(`Black Titanium` → `black-titanium`). Device generations map onto the
parametric models: island (14 Pro, 15+, Air), notch (X–13, 14/14 Plus,
16e), home (SE, 8), `ipad-modern` (Pro/Air/mini/A16/10th gen), `ipad-home`
(≤ 9th gen), `macbook`, `imac`. Watch, TV, Studio Display: no models yet.

User assignments (GUI "unmatched files") are stored per source in
`<frames>/<source>/user-mapping.yaml` (normalised name → `{model, color,
orientation}` or `null` = ignore) and take precedence over the rules.

### Packages and analysis (`core::frame_import`)

Folder (recursive), `.zip` (`zip` crate, deflate via `flate2` +
`miniz_oxide`, pure Rust) or `.dmg` (macOS: `hdiutil attach -readonly
-nobrowse -noautoopen -mountpoint <tmp>`, detached on drop; elsewhere an
error asking to extract first). Hidden files, `__MACOSX`, `._*` are
ignored; Photoshop files are counted as "ignored", licence files
(`*licen[cs]e*.rtf|txt|pdf|…`) are listed, other non-PNGs counted as
skipped. A DMG is checked with `hdiutil imageinfo -plist` first (key
`Software License Agreement`); with an SLA, or when `attach` reports
"attach canceled" (stdin is closed), the error `PackageError::
LicenseAgreement` says: open the image in Finder, accept the licence,
then import the mounted volume (e.g. `/Volumes/Bezel-iPhone-18`). The
GUI shows this as guidance with a folder-picker button.

When the package has a licence file, plan, CLI and GUI say: "These frames
are subject to Apple's licence (License.rtf in the package). They are
stored only on this computer and never in projects.", with its path; the
file name is recorded per frame in `index.yaml` (`license`). Each matched PNG is decoded (`png`)
and analysed: 4-connected flood fill from the image centre over alpha <
128; the region must not touch the border. Screen rect = bounding box;
corner radius = mean over the four diagonals (`d / (1 − 1/√2)`); mask =
region grown by 2 px (the screenshot reaches under the anti-aliased bezel
edge) plus a 1 px feather at 50 %. Opaque parts inside (island) stay out
of the mask, the bezel covers them anyway. Sanity check: screen aspect vs
the catalog's display aspect (±2 %) → warning in the plan. Every vendor
model is installed; only duplicates of the same vendor model × colour ×
orientation are "superseded". `import(…, apply)` returns the plan (dry
run) or installs, analysing again one by one to keep memory low.

### App-wide store

```text
<app data>/frames/<source>/index.yaml
<app data>/frames/<source>/user-mapping.yaml
<app data>/frames/<source>/<model>/<colour>/<orientation>/<vendor-model>.png       (bezel, unchanged)
<app data>/frames/<source>/<model>/<colour>/<orientation>/<vendor-model>.mask.png  (8-bit grey)
e.g. apple/iphone-island/black/portrait/iphone-18-pro.png
```

`index.yaml` records model, colour id + label, orientation, `apple_model`
(vendor model name), file, mask, size, screen rect, radius, priority,
source file name, package name, `license` (licence file name), import
date. The file name is the vendor-model slug, not the screen size: two
vendor models can share a screen size (iPhone 17 and 17 Pro) yet differ in
bezel. Stores of the first layout (`<orientation>.png`, no `apple_model`)
are read as they are (`apple_model` derived from `source_file`) and moved
to the new layout by `FrameStore::migrate`, which runs before every
install / remove; missing files drop out of the index.
`<app data>` is the GUI's data folder (`directories`), or
`$VITRINE_APP_DIR/data` for GUI and CLI. Never inside a project;
`.gitignore` also blocks `**/frames/apple/`, `*.dmg`, `Bezel-*/`.

### Model, resolve, validation

No new field: `FrameStyle` already has `imported:<source>` (ADR 0002), on
the device (`frame:`) or as project default (`styles.frame`). Resolve
(`ResolveOptions.frames`, `ValidateOptions.frames`: an optional
`FrameIndex`) looks up model × colour × orientation (no colour: the
model's default colour if installed, else the first installed). Among the installed
vendor models of that slot `select_bezel` picks:

1. the element's `bezel:` (vendor model, case-insensitive or as slug,
   e.g. `bezel: iPhone 18 Pro`; variant overrides patch it like any
   field) if installed, else an info `imported_bezel_fallback` and on;
2. the screen size equal to the target's page size (pages targets; either
   orientation): 1206×2622 → iPhone 18 Pro, 1320×2868 → Pro Max;
3. the screen size equal to the screenshot's pixel size;
4. the closest screen aspect (screenshot, else page) within ±2 %, near-ties
   (0.1 %) by priority;
5. the largest screen (info `imported_bezel_fallback` when there was a
   choice).

Ties: priority, screen area, then name. Target before screenshot (the task
first proposed the reverse): projects usually render one screenshot set
(e.g. 1206×2622) into both the 6.9" and the 6.3" slots, and each slot
should show its own device; the screenshot fits either bezel (aspects
differ by < 0.1 %) and the aspect check still warns about wrong devices.

Found → `ResolvedDevice.imported` (files, size, screen, `bezel`, `choice`,
`automatic`, installed `bezels`, `swatch` and installed `colors` for the
inspector) and the element height follows the bezel's aspect; the colour
may be a vendor colour that is not in the catalog palette. A colour that
is not installed for the model draws the default (else first) installed
colour with info `imported_color_fallback`. Missing → parametric
`detailed` frame plus info `imported_frame_missing` (and a vendor colour
name is only an info; the palette default is drawn).

Colours (user test 2026-11-10): ops accept a device colour that is in the
model's palette, a `#hex`, or – when the element's effective frame (own
`frame`, project `styles.frame`, or a variant override's `frame`) is
`imported:*` – any colour slug (`burgundy`); which colours are installed
is app-wide, so ops do not check it. Switching the frame source of a
device (`Editor::patch`) or the project default (`set_default_frame_ops`,
GUI `styles_patch`) maps the colour to the closest colour of the new
source in the same undo step (`frame_import::remap_color`: redmean
distance of the housing colours – measured swatch or catalog body colour –,
else shared name words, else cleared so the default applies); mapping was
chosen over clearing because it keeps the light/dark intent. Op order keeps
every step valid: styles first when entering an imported source, colours
first when leaving it.

Swatch: at import `analyze::body_swatch` samples scanlines across the
middle 60 % of each side from the image edge inwards (first opaque pixel
skipped, then 40 % of the edge-to-screen distance), drops near-black glass
pixels (luma < 28) when the rest is ≥ 1/12 of the samples, and averages
the middle half by luma. Stored as `swatch: "#RRGGBB"` per frame in
`index.yaml`; stores without it are filled in by `FrameStore::
fill_swatches` (GUI `frames_list`, CLI `frames list`).
Models no rule covers (browser, Android) silently stay parametric. The
screenshot aspect check uses the detected screen rect.

### Render

`RenderContext::with_frames(index)` adds a second asset cache rooted at the
frames root. An imported device is composed in device pixels: the
screenshot (cover/contain/stretch as usual) in the scaled screen rect,
multiplied by the scaled mask, then the bezel (Lanczos3 in export,
bilinear in previews; cached per file × size) source-over on top. The
composite is drawn with the element transform; its alpha is the shadow
source (the existing alpha-shadow path). Unreadable frame files fall back
to the parametric frame with an asset issue. The export cache hashes the
frame files' mtimes.

### Front ends

CLI `vitrine frames import <path> [--dry-run] [--format json]`, `frames
list`, `frames remove [--model] [--color] [--orientation] [--bezel]
[--all]`;
`render`/`export`/`validate` load the installed index. GUI: app settings →
Frame packages (plan table, manual assignment, installed frames with
thumbnails, remove), commands `frames_import_plan`, `frames_import_apply`,
`frames_list` (with installed colours + swatches per model),
`frames_remove`, `frames_thumbnail`; project settings default frame
Detailed / Stylized / Apple (imported); inspector frame source "Apple
(imported)" with availability hint, the installed colours as the only
colour swatches, and a "Bezel" picker ("Auto · 18 Pro Max" + the
installed vendor models) when a device has several.

### Tests

A synthetic package generator (`frame_import::synthetic`, own PNGs with an
opaque rounded frame, transparent rounded screen and an opaque island,
Apple-like names, two iPhone sizes "16 Pro" / "16 Pro Max" plus a smaller
duplicate, PSD / licence / other dummy files, plus a zip with `__MACOSX`
junk) drives tests for matching, both sizes kept, superseding, unmatched /
user mapping, screen detection (±1 px), store layout and migration, bezel
selection (target, screenshot, aspect, largest, pin, variant override),
SLA error mapping, licence notice, resolve fallback, validation info,
rendering (screenshot only inside the mask, island and frame on top) and
the GUI and CLI commands. `cargo run -p vitrine-core --example
synthetic_bezels -- [--full-size] <dir|file.zip>` writes it for manual
tests (`--full-size`: real screen sizes 1320×2868 / 1206×2622).

## Consequences

- Vendor images never touch the repo or projects; projects render on any
  machine (parametric fallback).
- iPhone 15+ rules are verified (iPhone 18 package); iPad / Mac rules
  still need a real package; new packages need only YAML.
- Several sizes per device take more disk space (≈ 2 × per family) but
  give each store slot its own device.
- Apple Watch / TV / displays need catalog models first.
- `imported:<source>` other than `apple` (own frame packs, KONZEPT §4.2
  item 3) can reuse store, analysis and render; they need a `frame.yaml`
  importer instead of name rules.
