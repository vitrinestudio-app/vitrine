# 0009 – Imported device frames (Apple Product Bezels)

- Status: accepted (file-name rules unverified until checked with a real package)
- Date: 2026-11-10

## Context

KONZEPT §4.2: Apple's product bezels may be used in marketing material but
must not be redistributed by an open-source project. The user downloads
them; Arrangia imports them app-wide, recognises device/colour/orientation
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
`MacBook Pro 14 - M4 - Space Black`) are educated guesses. `apple.yaml`
has `verified: false`; the importer and GUI say so and offer manual
assignment for everything the rules miss.

## Decision

### Rules (data)

`frame-import/<source>.yaml`: ordered rules, first match wins.
`pattern` is matched against the normalised file name (no folder, no
`.png`, case-insensitive, en/em dash → `-`, typographic quotes → `"`, one
space) with placeholders `{color}` (non-empty, never contains ` - `),
`{orientation}` (`Portrait`/`Landscape`) and `{*}` (any). Rule fields:
`model` (catalog id), `orientation` (when the name has none, Macs),
`priority` (several files for one model × colour × orientation, e.g. 17 Pro
vs 17 Pro Max: higher priority, then larger image wins), `note`.
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
ignored; non-PNGs counted as skipped. Each matched PNG is decoded (`png`)
and analysed: 4-connected flood fill from the image centre over alpha <
128; the region must not touch the border. Screen rect = bounding box;
corner radius = mean over the four diagonals (`d / (1 − 1/√2)`); mask =
region grown by 2 px (the screenshot reaches under the anti-aliased bezel
edge) plus a 1 px feather at 50 %. Opaque parts inside (island) stay out
of the mask, the bezel covers them anyway. Sanity check: screen aspect vs
the catalog's display aspect (±2 %) → warning in the plan. `import(…,
apply)` returns the plan (dry run) or installs the winners, analysing them
again one by one to keep memory low.

### App-wide store

```text
<app data>/frames/<source>/index.yaml
<app data>/frames/<source>/user-mapping.yaml
<app data>/frames/<source>/<model>/<colour>/<orientation>.png       (bezel, unchanged bytes)
<app data>/frames/<source>/<model>/<colour>/<orientation>.mask.png  (8-bit grey)
```

`index.yaml` records model, colour id + label, orientation, file, mask,
size, screen rect, radius, source file name, package name, import date.
`<app data>` is the GUI's data folder (`directories`), or
`$ARRANGIA_APP_DIR/data` for GUI and CLI. Never inside a project;
`.gitignore` also blocks `**/frames/apple/`, `*.dmg`, `Bezel-*/`.

### Model, resolve, validation

No new field: `FrameStyle` already has `imported:<source>` (ADR 0002), on
the device (`frame:`) or as project default (`styles.frame`). Resolve
(`ResolveOptions.frames`, `ValidateOptions.frames`: an optional
`FrameIndex`) looks up model × colour × orientation (no colour: the
model's default colour if installed, else the first installed). Found →
`ResolvedDevice.imported` (files, size, screen) and the element height
follows the bezel's aspect; the colour may be a vendor colour that is not
in the catalog palette. Missing → parametric `detailed` frame plus info
`imported_frame_missing` (and a vendor colour name is only an info).
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

CLI `arrangia frames import <path> [--dry-run] [--format json]`, `frames
list`, `frames remove [--model] [--color] [--orientation] [--all]`;
`render`/`export`/`validate` load the installed index. GUI: app settings →
Frame packages (plan table, manual assignment, installed frames with
thumbnails, remove), commands `frames_import_plan`, `frames_import_apply`,
`frames_list`, `frames_remove`, `frames_thumbnail`; inspector frame source
"Apple (imported)" with availability hint and installed colour chips.

### Tests

A synthetic package generator (`frame_import::synthetic`, own PNGs with an
opaque rounded frame, transparent rounded screen and an opaque island,
Apple-like names, plus a zip with `__MACOSX` junk) drives tests for
matching, superseding, unmatched/user mapping, screen detection (±1 px),
store layout, resolve fallback, validation info, rendering (screenshot
only inside the mask, island and frame on top) and the GUI commands.
`cargo run -p arrangia-core --example synthetic_bezels -- <dir|file.zip>`
writes it for manual tests.

## Consequences

- Vendor images never touch the repo or projects; projects render on any
  machine (parametric fallback).
- The file-name rules must be verified with a real package (task 3.5
  "real package verified by the user"); new packages need only YAML.
- Apple Watch / TV / displays need catalog models first.
- `imported:<source>` other than `apple` (own frame packs, KONZEPT §4.2
  item 3) can reuse store, analysis and render; they need a `frame.yaml`
  importer instead of name rules.
