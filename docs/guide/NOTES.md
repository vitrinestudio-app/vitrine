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
- Regional languages (`fr-CA`, `de-AT`, `pt-BR`): fall back automatically to their prefix language (`fr-CA → fr → default`); translate only the keys that differ (example: fr-CA has 2 Québec texts, "jardin communautaire", "cellulaire"). No `languages.fallback` entry needed for prefixes; use it only for non-prefix chains.
- Fallback severities: text from the default language = warning "not translated" (fails `validate --strict`); text from a prefix/configured fallback = info; screenshot from any fallback = info. A partial screenshot set per language is fine (missing ones come from the fallback chain).
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
- XLIFF/CSV export writes one file per non-default language; a sparse regional language (fr-CA) gets all keys it lacks with `--missing-only`.
- French typography: put a no-break space (U+00A0) before `:` `;` `?` `!` and inside « guillemets »; also to keep words together on one line ("le bon\u00a0moment"). Line breaking respects it.
- YAML: a translation containing `: ` (colon + space) or starting with `"`, `[`, `*` must be quoted (`"…"`); the GUI/strings import write correct YAML automatically.
- Hyphenation tip: auto-fit shrinks first and hyphenates only at `shrink_to`. If a long compound makes a headline shrink too much in one language, raise `shrink_to` for that language with a language override (`langs: {de: {fit: {shrink_to: 78, max_lines: 2}}}`) so hyphenation kicks in earlier; widen the box (`box`) if the break lands at an ugly syllable (it takes the last pattern point that fits: "Mischkul-turpartner" vs "Mischkultur-partner"). Soft hyphen `\-` forces the exact point. Example: `title-companions` in the example project.
- Validation info `hyphenated` lists every text that was hyphenated automatically per target × language – check those renders.

### Apple device frames (Apple Design Resources "Product Bezels")
- User downloads the bezels themselves from developer.apple.com/design/resources; Arrangia never ships or uploads them (Apple licence; License.rtf in the package).
- The DMGs contain a licence agreement: **open the DMG in Finder, accept the licence, then import the mounted volume** (`/Volumes/Bezel-iPhone-18`). Direct DMG import fails with "attach canceled".
- Package contents: PNGs per model/colour/orientation (`iPhone 18 Pro Max - Black - Portrait.png`), Photoshop files (ignored), License.rtf.
- iPhone 18 package (verified 2026-11-10): models Pro and Pro Max; colours Black, Burgundy, Glacier, Silver; screen exactly 1320×2868 (Pro Max) / 1206×2622 (Pro).
- Frames are stored app-wide (app data dir), never in the project; projects only say `frame: imported:apple`. Missing on another machine → parametric frame + info.
- CLI: `arrangia frames import <path> [--dry-run -v]`, `frames list`, `frames remove`.
- Both Pro and Pro Max bezels are kept; the bezel is picked per target size (else screenshot size), or pinned per device ("Bezel" in the inspector: "Auto · 18 Pro Max").
- Use them for the whole project: Project settings → General → Frame style → "Apple (imported)" (`styles.frame: imported:apple`); a hint shows when no Apple frames are installed. Per device: inspector → Frame.
- Colours come from the package (Black, Burgundy, Glacier, Silver for iPhone 18), not from Arrangia's palette. With an Apple frame the inspector shows only the installed colours, as swatches in the real housing colour (measured from each bezel at import: outer metal rim, black glass border excluded; stored as `swatch` in the store's `index.yaml`, filled in for older imports the first time the frame list is shown).
- Switching the frame source (device or project default) changes the colour too, in the same undo step: the closest colour of the new source by housing colour (Black titanium → Black, Natural titanium → Silver; Burgundy → Black titanium back). Round trips can change the colour (White titanium → Silver → Natural titanium).
- A colour that is not installed (e.g. project from another machine) → the default/first installed colour is drawn, info `imported_color_fallback`. No Apple frame installed at all → built-in detailed frame with the default colour, info `imported_frame_missing`.

### Text effects
- Outline (stroke), glow, shadow on text styles, span styles and elements. Precedence span > element > style. Disable inherited: `width: 0`, `opacity: 0`, `shadow: none`. Sizes in lu (don't shrink with auto-fit).

### Images
- PNG/JPEG/SVG. Shadows of transparent images follow the alpha (like CSS drop-shadow). Text inside SVGs is not rendered yet.

### Export
- Files: `export/<target>/<lang>/<nn>_<page-id>.png`, grouped `<nn>_<id1>+<id2>`, `banner.png`, `_gallery.png`.
- Always opaque (no alpha), sRGB. Incremental: unchanged images are skipped (`--force` to re-render).

### Claude / MCP (Phase 4)
- Main path: start Arrangia → App settings → "Claude / MCP" → "Add to Claude Desktop…" (shows file + entry, asks, backup `claude_desktop_config.json.bak-<UTC timestamp>`) → restart Claude Desktop. Entry: `mcpServers.arrangia = {command: "<abs path>/arrangia", args: ["mcp"]}`. Config file: macOS `~/Library/Application Support/Claude/claude_desktop_config.json`, Windows `%APPDATA%\Claude\claude_desktop_config.json`, Linux `~/.config/Claude/claude_desktop_config.json`. Already present → "Already set up"; CLI moved → updated in place; broken JSON → nothing written.
- The app hosts the MCP endpoint (setting "Allow Claude to connect", default on, switches live): Streamable HTTP on `127.0.0.1:<random port>/mcp` (optional fixed port setting), bearer token (new per start), loopback Host/Origin only. Discovery file `<app data>/mcp.json` (port, token, pid; 0600; removed on exit).
- `arrangia mcp` (stdio, what Claude Desktop starts): running app found → forwards to it (Claude edits what you see); none → starts the app (setting "Start on demand", default on; waits ≤ 20 s); else / `--headless` → own session on `--project` (default `.`, if it holds a `project.yaml`), Claude uses `project_open` / `project_create`.
- Claude Code: `claude mcp add arrangia -- <path>/arrangia mcp` (recommended) or the HTTP command with port + token from the settings (valid until the app restarts). Headless/CI: `arrangia mcp --headless --project <dir>`.
- Live aids: top-bar sparkle indicator (grey = off, ready, orange with client name = Claude connected; tooltip), click → activity list (time, label, click selects, undo for the latest step); Claude's changes are outlined on the canvas for 1.5 s and shown as small toasts "Claude: …" (click selects). Projects Claude opens/creates open in the app (start screen → editor).
- In the app, tools default to what you look at: omitted `target` = the current target (if it shows the strip), omitted `lang` = the current language. `view_get` / `selection_get` read the view and selection ("make this bigger").
- Client names: `claude-ai` = Claude Desktop, `claude-code` = Claude Code; a client counts as connected while it has a session or was active in the last 5 min.
- Every change by Claude is one undo step labelled `Claude: …` in the shared history (undo in the app or with `history_undo` / `history_redo`).
- Claude sees the result: `render_preview` returns a PNG (view `image` = one exported image, `canvas` = whole strip with hatched gaps, `gallery` = store gallery; `max_edge` default 1200) plus the errors/warnings of that target × language.
- External edits (text editor, git checkout, the GUI) are picked up before every tool call; a `project.yaml` that does not load gives error `load` and nothing is written.
- No network; files are written only inside the project folder and to paths given explicitly (`export.out_dir`, `strings_export.path`). Installed device frames are read-only for Claude (`frames_list`).
- Coordinates for Claude: lu, page 1000 lu wide; `element_move` x/y = element centre relative to the top-left of a page (default: home page), or `dx/dy`. Rotation degrees, positive = clockwise.
- Edit scope: `base` (default, all targets/languages), `variant` (format override, e.g. iPad landscape), `lang` (per-language text override).
- Tools: project_open, project_create, project_summary, view_get, selection_get · catalog_devices, catalog_targets, catalog_galleries, catalog_fonts · strip_add, strip_update, strip_variant, target_add, target_update, target_remove, language_add, language_remove · element_list, element_get, element_add, element_update, element_remove, element_move, element_resize, element_rotate, element_reorder, element_group, element_ungroup, element_rename, apply_ops · strings_get, strings_set, strings_missing, string_key_rename, strings_export, strings_import · styles_get, styles_update · assets_list, assets_import, screenshots_import, frames_list · render_preview, layout_report, validate, export · history_undo, history_redo, history_state.
- Resources: `arrangia://project/project.yaml`, `arrangia://project/i18n/<lang>.yaml`, `arrangia://catalog/devices`, `arrangia://catalog/targets`, `arrangia://docs/ops` (op reference for `apply_ops`).
- `export` sends MCP progress notifications (one per file) when the client passes a progress token.
- Example prompts: "Create an Arrangia project in ~/Screens/Fernwood (en, de) for a garden planner, three iPhone pages with headline + phone each, show me the gallery"; "Translate everything missing into French, then check page 2 in French for overflows"; "Select the title in the app and say: make this 20 % smaller"; "Export iPhone and iPad for all languages".

## 4. Reference notes
- CLI exit codes: 0 ok, 1 project problems (validation errors / export errors), 2 command could not run.
- `validate --strict` fails on warnings; `--preview` treats missing screenshots as warnings.
- Text overflow (`text_overflow`) is an **error** (exit 1) – only the affected language/target; other languages still validate clean.
- `render --gallery --max-edge N`: N is the image *height* of the gallery, not its long side.
- Keyboard: arrows nudge 1 lu (⇧ 10), ⌘D duplicate, ⌘G / ⇧⌘G group/ungroup, `[` `]` z-order, ⌘Z/⇧⌘Z, ⌥ disables snapping while dragging, ⇧ free rotation / axis lock, ⌘N/⌘O/⌘E/⌘, menus.
- Dev bridge (developers only): run the UI in a browser – see CLAUDE.md.

## Contributor notes
- Built-in device frames are own parametric artwork (devices.yaml + render/frames); how to add devices + ready-made Claude prompt: docs/dev/adding-devices.md.

## 5. Limits & FAQ
- Colour management (Display P3 → sRGB) not yet.
- RTL layouts not mirrored yet.
- Native store app gallery gaps are estimates.
