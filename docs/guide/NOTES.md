# User guide – collected notes

Raw notes for the future user guide / website (arrangia.app). **Not prose yet.**
Language: English (German translation later). Planned format: mdBook under
`docs/guide/` (Phase 6). Every feature adds its user-facing facts here as bullet
points under the matching chapter. Keep entries short, concrete, with commands.

## Outline (planned chapters)
1. Getting started (install, first project, tour of the editor)
2. Concepts (canvas/strip, home page, gaps, anchors, page grouping, variants, targets, languages & fallbacks)
3. How-to guides (screenshots import, translating, Apple frames, exporting, store sizes, banners)
4. Reference (CLI, project.yaml fields, validation findings, keyboard shortcuts, markup)
5. Limits & FAQ

---

## 1. Getting started
- CLI quick start: `arrangia new <dir> [--template phone|minimal] [--lang en --lang de]` → `arrangia validate` → `arrangia export`.
- GUI: `arrangia-gui [project-dir]` opens a project directly; otherwise start screen (new / open / recent).
- Project = folder: `project.yaml`, `i18n/<lang>.yaml`, `screenshots/<lang>/<variant>/<key>.png`, `assets/`, `fonts/`, `export/` and `.cache/` (both gitignored).
- Every edit is saved immediately (no "Save"); external edits (e.g. by Claude via MCP, a text editor, git checkout) are reloaded live.

## 2. Concepts
- Page width is always 1000 lu (logical units); height follows the aspect. Sizes in lu → export resolution independent.
- Home page: each element's position is relative to its page; elements may reach into neighbouring pages (a phone spanning two screenshots exists once).
- Gaps: stores show screenshots with gaps; Arrangia builds the canvas with those gaps so continuous elements line up in the store; content inside a gap is not exported. Gap values per gallery profile; app-store web values measured from apps.apple.com CSS, native store apps still placeholders.
- Anchors (`top|bottom|center|stretch`) adapt one layout to other aspect ratios (e.g. iPhone 1:2.17 vs Play 1:2).
- Grouping `group: 2`: two portrait pages → one landscape image (iPad landscape from the phone canvas).
- Variants: per-format overrides (position, size, rotation…) while content stays shared. Inspector marks overrides purple.
- Language overrides: per-language text tweaks (font size, box, style, fit, effects); marked teal in the inspector; `de-AT` inherits `de`.
- Rotation sign: positive = clockwise (CSS convention).

## 3. How-to notes

### Store sizes (verified in App Store Connect web app, 2026-11-10)
- Required iPhone set: "iPhone with Dynamic Island (medium display)" = 6.3" = 1206×2622 → `APP_IPHONE_61`. All other iPhone sizes optional (scaled by Apple).
- 6.9" 1320×2868 / 1290×2796 → `APP_IPHONE_67` (no `_69` type exists).
- 1260×2736 and 1512×2316 are listed on Apple's help page but rejected by the ASC uploader → Arrangia does not offer them.
- iPad 13" 2064×2752 / 2048×2732 → `APP_IPAD_PRO_3GEN_129` (required if the app runs on iPad).
- Mac: 16:10 only (1280×800, 1440×900, 2560×1600, 2880×1800).
- Google Play: long side ≤ 2× short side, 320–3840 px, 2–8 per device type, ≥ 4 at ≥ 1080 px for recommendations.
- iPhone Duo exists in ASC behind a feature flag; sizes not public yet.

### Screenshots import
- Supports Arrangia folder layout, fastlane snapshot output (`de-DE/iPhone 17 Pro Max-01_home.png`), flat folders.
- Locale `de-DE` → `de` automatically; device name → variant by mapping or pixel size; `01_home` → key `home`.
- `_framed.png` (frameit) and `screenshots.html` are skipped.
- Mapping can be saved to `project.yaml` (`import.fastlane`) so re-imports run unattended.
- CLI: `arrangia import screenshots <dir> [--dry-run] [--device-map "NAME=VARIANT"] [--save-mapping]`.

### Translating
- Markup: `**bold**`, `_italic_`, `[style]…[/]`, `\n`, soft hyphen `\-`.
- Soft hyphen: invisible unless the line breaks there; then a visible hyphen.
- Emergency break (word split without hyphen) is reported as a warning.
- Automatic hyphenation: opt-in (`hyphenate: true`), only kicks in when text doesn't fit even at `shrink_to`. Bundled languages: en-us, en-gb, de (1996), fr, es, nl, it, da, pt, nb/no, fi. Swedish missing (pattern licence LPPL-only, pending project licence decision).
- Exchange: CSV (Excel variants with BOM/`;` detected) and XLIFF 1.2/2.0; markup becomes protected inline tags so CAT tools don't break it; import shows a dry-run diff and post-import overflow check; one undo step.
- CSV context column gives element/page/style and an approximate max length.

### Apple device frames (Apple Design Resources "Product Bezels")
- User downloads the bezels themselves from developer.apple.com/design/resources; Arrangia never ships or uploads them (Apple licence; License.rtf in the package).
- The DMGs contain a licence agreement: **open the DMG in Finder, accept the licence, then import the mounted volume** (`/Volumes/Bezel-iPhone-18`). Direct DMG import fails with "attach canceled".
- Package contents: PNGs per model/colour/orientation (`iPhone 18 Pro Max - Black - Portrait.png`), Photoshop files (ignored), License.rtf.
- iPhone 18 package (verified 2026-11-10): models Pro and Pro Max; colours Black, Burgundy, Glacier, Silver; screen exactly 1320×2868 (Pro Max) / 1206×2622 (Pro).
- Frames are stored app-wide (app data dir), never in the project; projects only say `frame: imported:apple`. Missing on another machine → parametric frame + info.
- CLI: `arrangia frames import <path> [--dry-run -v]`, `frames list`, `frames remove`.
- (In progress) both Pro and Pro Max bezels are kept; selection by screenshot/target size; manual pin per element.

### Text effects
- Outline (stroke), glow, shadow on text styles, span styles and elements. Precedence span > element > style. Disable inherited: `width: 0`, `opacity: 0`, `shadow: none`. Sizes in lu (don't shrink with auto-fit).

### Images
- PNG/JPEG/SVG. Shadows of transparent images follow the alpha (like CSS drop-shadow). Text inside SVGs is not rendered yet.

### Export
- Files: `export/<target>/<lang>/<nn>_<page-id>.png`, grouped `<nn>_<id1>+<id2>`, `banner.png`, `_gallery.png`.
- Always opaque (no alpha), sRGB. Incremental: unchanged images are skipped (`--force` to re-render).

## 4. Reference notes
- CLI exit codes: 0 ok, 1 project problems (validation errors / export errors), 2 command could not run.
- `validate --strict` fails on warnings; `--preview` treats missing screenshots as warnings.
- Keyboard: arrows nudge 1 lu (⇧ 10), ⌘D duplicate, ⌘G / ⇧⌘G group/ungroup, `[` `]` z-order, ⌘Z/⇧⌘Z, ⌥ disables snapping while dragging, ⇧ free rotation / axis lock, ⌘N/⌘O/⌘E/⌘, menus.
- Dev bridge (developers only): run the UI in a browser – see CLAUDE.md.

## 5. Limits & FAQ
- Colour management (Display P3 → sRGB) not yet.
- RTL layouts not mirrored yet.
- Native store app gallery gaps are estimates.
