# Vitrine Studio – Architecture

The binding description of the system: what Vitrine Studio is, the concepts
and rules of its model, how a project becomes pixels and files, and how the
front ends share one core. It replaces the German concept `docs/KONZEPT.md`
and the 33 architecture decision records `docs/adr/0000–0032` (both moved
to `docs/archive/` on 2026-11-13). §14 maps every former ADR number and
KONZEPT section to the section here.

When this document and the code disagree, the code wins and this document
is fixed. Current state and owner decisions in progress: `docs/plan/STATUS.md`.
How to work in the repo: `AGENTS.md`.

Contents: 1 Purpose · 2 Concepts · 3 Project format · 4 Model and ops ·
5 Resolve · 6 Text · 7 Rendering · 8 Validation · 9 Export and import ·
10 Store upload · 11 Templates and gallery · 12 Application layer and front
ends · 13 Cross-cutting · 14 Decision index · 15 Recording decisions

---

## 1. Purpose, scope, non-goals

Vitrine Studio is an open-source (GPL-3.0-or-later) desktop app, written in
Rust, for App Store and Play Store screenshots: device frames, texts,
translations, panorama sets, store upload and an MCP interface. A layout is
designed **once** and rendered for every language, device class and store.

Users: app developers and designers who maintain store listings in several
languages and sizes, their CI pipelines, and AI assistants working on their
behalf over MCP.

### Principles

1. **Design once, output everywhere.** A device, text or background exists
   exactly once, however many export images it runs across.
2. **Local and open.** A project is a folder of readable YAML files and
   images. No account, no cloud, Git-friendly. Network is used only for
   store uploads, the template gallery and the update check.
3. **Headless first.** Everything the GUI does, the CLI and the MCP server
   can do, because all three go through the same core (Operations API,
   resolve, renderer).
4. **One renderer.** GUI preview, export and MCP `render_preview` come from
   the same Rust pipeline, so line breaks and positions in the preview are
   exactly those of the export.
5. **Fast.** Preview updates well below 50 ms per page at preview size; a
   full export of a project takes seconds (measured: 4 targets × 4
   languages, 66 files ≈ 1 s in release; incremental re-export ≈ 3 ms).

### Front ends

| Front end | Binary | Role |
|---|---|---|
| GUI | `vitrine-gui` (Tauri 2) | the editor; also hosts the MCP endpoint |
| CLI | `vitrine` (clap) | headless render/export/validate/import/upload, CI |
| MCP | `vitrine mcp` | stdio proxy to the running GUI, or headless server |

Plus `vitrine-api` (`crates/api-server`), the backend of
api.vitrinestudio.app (template gallery, GitHub login) – not part of the
desktop app.

### Non-goals (for now)

- App preview videos.
- Capturing screenshots (simulator/device control): fastlane snapshot,
  screengrab and UI tests do that; Vitrine Studio imports their output.
- 3D perspective (Rotato-style tilt) – an optional phase after 1.0. 2D
  rotation of every element exists from the start.
- Cloud sync, team features, accounts (the gallery login is only for
  publishing templates).
- Mirrored RTL layouts, Display P3 output, images with per-language asset
  variants (see §2 and §7).

### System at a glance

```text
                 ┌────────────── vitrine-gui (Tauri) ─┐   vitrine (CLI)
 web view UI ───▶│ wrappers, dev bridge, updater, menu│        │
                 └──────────────┬─────────────────────┘        │
     MCP clients ─▶ vitrine-mcp (tools, HTTP host, stdio proxy)│
                                │                              │
                         vitrine-app (session, commands, upload, settings)
                                │                              │
              ┌─────────────────┼───────────────────┐          │
        vitrine-core      vitrine-render      vitrine-publish ◀┘
  (model, ops, resolve,  (tiny-skia raster,  (App Store Connect,
   layout, validate,      frames, export,     Google Play, gallery
   catalogs, templates)   quality checks)     client, keychain)
                                ▲
                   vitrine-api-server (gallery backend; core + render)
```

| Crate | Responsibility | Network |
|---|---|---|
| `core` | model, YAML IO, Operations API, edit intents, resolve, geometry, text layout and fonts, validation, catalogs, colour profiles, import/exchange, fastlane paths, frame import + packs, templates | never |
| `render` | rasterizer, parametric frames, glyph outlines, export, pixel design checks, frame-pack SVG, template previews and sprites | never |
| `publish` | store REST clients, gallery client, keychain, CI credentials | yes |
| `app` | transport-agnostic application layer for GUI and MCP (no Tauri) | via `publish` |
| `mcp` | MCP server over `app`, HTTP host, stdio proxy | proxy: loopback only |
| `cli` | `vitrine` binary; direct core/render paths, `app` for upload/MCP | via `publish` |
| `gui` | Tauri binary, dev bridge, frontend in `crates/gui/ui/` | updater only |
| `api-server` | gallery backend (axum, SQLite) | serves; calls GitHub |

---

## 2. Core concepts and glossary

### Canvas, not single images

A **canvas** is a continuous row of *n* equal **pages**. Backgrounds and
devices live in canvas coordinates and are cut into pages at export. A
device spanning two pages exists once; a gradient runs across all pages.

```text
┌─────────┬─────────┬─────────┬─────────┐
│ page 0  │ page 1  │ page 2  │ page 3  │   one canvas
│ ┌─────┐ │         │         │         │
│ │title│ │  ┌──────┼──┐      │         │   phone spanning pages 1+2
│ └─────┘ │  │ phone│  │      │         │   (exists once)
│ background runs across every page and gap ─────────▶│
└─────────┴─────────┴─────────┴─────────┘
```

| Term | Meaning |
|---|---|
| **Project** | folder with `project.yaml`, translations, screenshots, assets; one or more canvases |
| **Canvas** | *n* equal pages side by side (`page: {aspect, count, ids}`); typically one per device class. Was called "strip" until 2026-11-11 (old `strips:` key is refused with a hint) |
| **Page** | a slice of the canvas; normally one export image, with grouping several pages form one image |
| **lu** | logical unit. A page is **1000 lu wide**; its height follows the aspect (1320:2868 → 2172.7 lu). All sizes are lu; pixels come only from the target |
| **Home page** | `page` (0-based) of an element; its position is relative to that page's top-left corner. Elements may reach into neighbours |
| **Gap** | space between export images as the store shows them (from the target's gallery profile); content in a gap is left out of page exports |
| **Target** | renders one canvas at a pixel size, optionally with a store slot, device swap, page grouping, variant |
| **Variant** | per-canvas geometry overrides for one format (`tablet-landscape`); content stays shared |
| **Language** | BCP-47-like code with a fallback chain |
| **Element** | background, device, text, image, shape, group |
| **Asset** | screenshot, image, font, frame |
| **Banner** | a target that renders one continuous image of a canvas region |

### Coordinates

- y grows downward. Rotation in degrees, **positive = clockwise** (CSS /
  tiny-skia). Gradient angles are CSS-like (0 = to top, 90 = to right,
  default 180).
- `at` is the element **centre** (devices, images, shapes, free text boxes)
  relative to the home page's top-left. `pivot` is an offset from the
  default pivot (the centre; for auto-height texts the centre of the laid
  out content).
- Page origin on the canvas for a target with gap `g` lu and grouping `k`:
  `page_x(i) = i·1000 + ⌊i/k⌋·g`, `page_y = 0`. Columns of one export image
  are adjacent; gaps exist only between images.

Because elements hang on their home page, a different gap or grouping only
shifts the neighbour page; transitions stay correct in every target.

### Gaps and gallery profiles

Stores show screenshots with gaps and rounded corners. The canvas is built
**with** the gaps, so a device or diagonal band crossing a page border lines
up when the store shows the images. Gallery profiles
(`crates/core/data/galleries.yaml`) hold `gap`, `radius` (number or `{x,y}`),
`background`, `background_dark`, `basis` (`width` = 1000 lu per exported
image width; `height` for Google Play), `measured` (`false` = placeholder,
`web-css` = measured from the store website, `fixed`). A target may
override `gap`/`radius`; `gallery: none` or no gallery = no gaps. Web values
were measured 2026-11-09; native store apps and the dark backgrounds are
**not yet measured**.

### One layout, many formats

A target's page height follows its pixel aspect: `H = k · 1000 · h / w`
(`k` = `group`). Three tools adapt one layout:

1. **Vertical anchors** `anchor_y`: with `ΔH = H − H0` (H0 = canvas page
   height): `top` keeps `y`; `bottom` → `y + ΔH`; `center` → `y + ΔH/2`;
   `stretch` → `y · H/H0` and heights scale. Default `stretch` for
   `background` elements, `top` for all others.
2. **Page grouping** `group: 2`: two portrait pages become the two columns
   of one landscape image (6 phone pages → 3 iPad landscape images).
   Margins are relative to the column.
3. **Variants** with overrides and `content_scale` (devices only).

```text
phone:  [ P0 ] ␣ [ P1 ] ␣ [ P2 ] ␣ [ P3 ]
iPad:   [ P0 | P1 ]   ␣   [ P2 | P3 ]       (group: 2, variant tablet-landscape)
```

Plus `device_map` per target (`{iphone-island: android-punchhole}`): the
same canvas renders iPhone frames for iOS and Android frames for Play.

### Clip

`clip: page | none`; default `page` for texts, `none` for everything else.
A clipped element is cut at its home page (the column when grouped). A
shape meant to continue across a border is **one** element with
`clip: none`; never two halves.

### Languages and fallback chains

- `languages: {default, available, fallback}`. Chain for a language: the
  language, its shorter prefixes (`de-AT` → `de`), the explicit
  `fallback` entry of its most specific language that has one (again with
  prefixes), repeated, finally the default language; no duplicates,
  cycles stop. `pt-BR` with `pt: es` → `pt-BR, pt, es, en`.
- Texts resolve along the chain. Screenshots along the chain too (§5).
- The default language is set per project; new projects take the app
  setting `default_project_language`. Before changing it the app lists texts
  and screenshots that would lose their source (`default_language_impact`).

### Screenshot keys and variant folders

A device shows `screenshots/<lang>/[<variant>/]<key>.(png|jpg|jpeg)`.
`screenshot` is a key, never a path. The variant folder is the target's
`screenshots` value (catalog variants such as `ios-phone`, `android-phone`,
`ios-tablet`, `watch`, `wear`, or any own folder name), else the mapped
model's default variant. A file directly in the language folder serves
every variant.

### Styles and tokens

`styles` holds named colours (`@name`), text styles, span styles, shadow
presets (`soft`, `hard`, `none` built in, redefinable), the project frame
style and per-family font fallbacks. Element values override style values.

### Devices and frames

Device elements name a catalog model (`devices.yaml`: `iphone-island`,
`iphone-notch`, `iphone-home`, `android-punchhole`, `iphone-duo`,
`iphone-duo-folded`, `watch-ultra`, `watch`, `ipad-modern`, `ipad-home`,
`android-tablet`, `macbook`, `generic-laptop`, `imac`, `generic-monitor`,
`browser`, `none`). Width is set, height follows the model. Frame sources
(§7.3): parametric `stylized` / `detailed` (default `detailed`), imported
vendor bezels `imported:apple`, own frame packs `pack:<id>`.

---

## 3. Project format

### Folder

```text
my-app/
├── project.yaml            canvases, elements, styles, targets, languages, store, import, checks
├── i18n/<lang>.yaml        flat key → text, sorted by key
├── screenshots/<lang>/[<variant>/]<key>.png|jpg
├── screenshots/samples.yaml  marks template sample screenshots (optional)
├── assets/                 images for image elements and image fills
├── fonts/                  project fonts (TTF/OTF/TTC); fonts/system/ = stored installed fonts
├── frames/<id>/            own frame packs (frame.yaml + PNG/SVG)
├── frames/imported/        stored copies of imported vendor bezels (private repos only)
├── export/                 output (gitignored)
└── .cache/                 upload log + images, compiled frame packs, ops files (gitignored)
```

`vitrine new` / `project_new` write `.gitignore` with `export/` and
`.cache/`. YAML, not SQLite, because projects must be diffable, versionable
and readable by people and assistants. SQLite is only used for app-wide
recent projects (`app.sqlite`) and the per-project upload log.

### Canonical YAML (`core::io`, `core::model`)

- `serde-saphyr`, YAML 1.2 core schema: only `true`/`false` are booleans,
  so `no` (Norwegian) and `1320:2868` stay strings.
- **Canonical form**: field order = struct order; defaults omitted
  (exceptions: an element's `page`, and `format`, `revision`, `name`,
  `languages`); kind-dependent defaults (`anchor_y`, `clip`) are `Option`s so
  an explicit default survives; maps sorted by key; lists keep their order
  (element order = painting order); flow style for small values
  (`at: {x: 1000, y: 2172}`); integers without `.0`. Load → save
  reproduces a canonical file byte for byte (tested).
- `format: vitrine/1` first; other formats are refused (`arrangia/1` with a
  one-line migration hint).
- `deny_unknown_fields` everywhere, errors with file, line, column. Element
  fields are listed once in `element_fields!` (`crates/core/src/model/element.rs`),
  which generates the flat `ElementWire` and `ElementPatch`; a field that
  does not belong to the kind is "not allowed on text elements".
- **Ids**: ASCII letters, digits, `-`, `_`, `.`, starting with a letter or
  digit. Element ids are unique in the whole project; canvas and target ids
  in the project; variant ids per canvas. Page ids
  (`page: {ids: [hero, plan]}`, default `page-01`…) name export files;
  elements refer to pages by index.
- **Colours** `#RRGGBB`/`#RRGGBBAA` (written upper-case) or `@name`. A
  colour string is shorthand for `{kind: color, color}` wherever a fill is
  expected.
- **Load is lenient about references**: syntax, types, unknown fields,
  duplicate ids and self-contained rules fail; dangling references
  (unknown style, colour, page, target) load and are reported by
  validation. Ops refuse to create new dangling references.

### Elements (fields)

Common: `id`, `kind`, `page`, `lock_page`, `locked`, `at`, `width`,
`height`, `rotation`, `pivot`, `anchor_y`, `opacity`, `z`, `clip`,
`visible`, `visible_in: {targets, langs}` (empty list = no restriction;
`visible_in: {}` is never written), `allow: [codes]` (§8).

| Kind | Specific fields |
|---|---|
| `background` | `fill` – fills its home page exactly (per-page colour/photo) |
| `device` | `model`, `orientation`, `frame`, `bezel`, `color`, `fold` (`open`/`folded`), `chrome: browser`, `url`, `appearance` (`light`/`dark` browser chrome), `screenshot`, `fit` (`cover`/`contain`/`stretch`), `shadow` |
| `text` | `text` (key), `style`, `box: {top, left, right, bottom}` (margins) or free box (`at` + `width` [+ `height`]), `align`, `valign`, `fit: {shrink_to, max_lines}`, `font_size`, `color`, `stroke`, `glow`, `shadow`, `hyphenate`, `langs: {<lang>: patch}` |
| `image` | `asset` (PNG/JPEG/SVG path), `width`, `height`, `fit`, `shadow`, `clip_to` |
| `shape` | `shape` (`rect`/`ellipse`/`line`), `radius`, `fill`, `stroke`, `shadow` |
| `group` | `children`, `scale` (0.1–10) |

`locked` is editor-only (canvas hit-testing skips it; no render effect,
always written to the base). `lock_page` keeps the home page while
dragging. Images have **no** per-language asset variants (KONZEPT
proposed them); language-specific images are separate elements with
`visible_in.langs`. A `{lang}` placeholder in asset paths is proposed, not
built.

### Canvases and fills

`canvases[]: {id, page: {aspect, count, ids?}, backgrounds: [fill…],
elements, variants: {<id>: {content_scale, overrides: {<element>: patch}}}}`.
`backgrounds` are **layers** over the whole canvas (pages and gaps), bottom
to top – not one per page (`background_hidden` warns when an opaque layer
hides lower ones). Fills: colour, `linear-gradient {angle, stops}`,
`radial-gradient {center, radius, stops}`, `image {asset, fit:
cover|contain|tile, focus}` (`tile` = 1 px or SVG unit per lu).

### Targets

`targets[]: {id, kind: pages|banner, canvas, size: [w, h], group, variant,
screenshots, gallery: <preset> | {preset, gap, radius}, device_map, format:
png|jpeg, quality, langs, store, screenshot_only, safe_area}`; banners add
`region: {pages: [a, b]} | {rect: {page, x, y, width, height}}`, `fit:
cover|contain`, `gaps: none|as_store`. Store slots: `{apple: {display_type,
placement?: header|search_results|universal}}` or `{google: {image_type}}`.
`screenshot_only` (watch, Wear OS) exports each page's largest device's
screenshot alone. `safe_area` (insets in % of the image) is drawn as a
dashed box and checked (`text_outside_safe_area`). Presets come from
`crates/core/data/targets.yaml` (31 presets: iPhone 6.9″…4.7″, Duo, iPad,
Mac, watches, App Store creatives, Play phone/tablet/Chromebook/Wear/feature
graphic, web banners).

### Other sections

- `store:` – store mapping (§10): `{apple: {account, app_id, bundle_id,
  platform}, google: {account, package_name}, locales: {apple: {…}, google:
  {…}}}`. Changed only through op `set_store` (not via MCP). The retired
  key `apple.upload_flow` (decision 0034) is still read and ignored; the
  canonical writer drops it.
- `import:` – saved screenshot-import mappings
  (`import.fastlane.{devices, locales, keys}`, op `set_import`, §9.4).
- `checks:` – design-check thresholds and `off: [codes]` (op `set_checks`, §8).
- `screenshots/samples.yaml` – sample screenshots copied from a template,
  with size + SHA-256; a file is a sample only while its content matches.

### Revisions and atomic save

- `revision: u64` in `project.yaml`, incremented by every save.
- `Project::save` re-reads the revision on disk; a different one is a
  `Conflict` and nothing is written (optimistic locking). Otherwise changed
  `i18n/*.yaml` are written, then `project.yaml`, each atomically (temp file
  in the same folder, fsync, rename). `i18n` files of languages without
  strings are deleted.
- The project on disk is the source of truth: every applied transaction is
  saved immediately (no "unsaved" state). A transaction that changes nothing
  is not saved (no revision bump, no undo step).

---

## 4. Model and Operations API

### Ops (`core::ops`)

**All model changes go through the Operations API.** GUI, CLI and MCP never
modify model fields directly.

```rust
// JSON: {"op": "update_element", "id": "title", "patch": {"font_size": 80}}
pub enum Op { SetName, SetLanguages, SetStyles, SetImport, SetChecks, SetStore,
  SetString, AddCanvas, UpdateCanvas, RemoveCanvas, AddElement, UpdateElement,
  RemoveElement, MoveElement, AddVariant, RemoveVariant, SetVariantContentScale,
  SetVariantOverride, AddTarget, UpdateTarget, RemoveTarget, RenameElement,
  RenameCanvas, RenameTarget, RenameVariant, MoveElementTo, ResetOverrideField,
  SetDefaultLanguage, AddLanguage, RemoveLanguage, RenameStringKey,
  SetLangOverride, ResetLangOverrideField, RenameStyle, … }
```

- `Project::apply(op) -> Result<inverse op>` validates, applies and returns
  the inverse. Op reference for assistants: `crates/mcp/src/ops.md`.
- A **transaction** (`History::apply`, `apply_ops`) is a list of ops applied
  atomically – all succeed or nothing changes – and is **one undo step**.
- **History** is in memory (`ops::history`, 500 steps), per session.
  External edits clear it. KONZEPT's optional `.cache/history.sqlite` was
  never built.
- **References stay consistent**: renames follow every reference
  (`rename_element` → overrides and `clip_to`; `rename_canvas` → targets;
  `rename_target` → `visible_in`; `rename_variant` → targets;
  `rename_string_key` → all languages, elements and overrides;
  `rename_style` → styles, elements, overrides, backgrounds and `[span]` tags
  in translations). Removing a target named by `visible_in` or a style in
  use is refused; `remove_element` is refused while an image outside the
  removed subtree is `clip_to` it (the editor's delete first emits
  `clip_to_release_ops` in the same transaction). Removing an element
  removes its overrides (the inverse restores them). `remove_language`
  refuses the default language and languages named by `visible_in` or
  target `langs`; its inverse carries the translations and language
  overrides.
- Catalog values (device models, colours, gallery presets, store slots) are
  checked by ops; loading accepts unknown ones and validation reports them.

### Patches and overrides

- `ElementPatch` has every element field except `id`, `kind`, `children`:
  missing = unchanged, `null` = reset to the kind's default (base only),
  value = replace.
- **Overrides never hold `null`.** Variant and language overrides only set
  values; "back to the base" is the absence of the field. In variant/lang
  scope a `null` in an edit removes the field from the override; override
  ops drop `null` fields; a hand-written `null` is dropped on load (info
  `override_null`). Switching something off per variant needs an explicit
  value (`shadow: none`, `visible: false`, `stroke.width: 0`).
- `set_variant_override` / `set_lang_override` **replace** the whole
  override; lost fields are reported as `dropped_override_fields`. Edit
  intents merge instead (`ElementPatch::merged`).
- **Language overrides** (`langs.<lang>` on text elements) allow only
  `LANG_OVERRIDE_FIELDS`: `at, width, height, anchor_y, style, box, align,
  valign, fit, color, font_size, stroke, glow, shadow, hyphenate`. Page,
  rotation, pivot, z, clip, opacity and visibility are shared by all
  languages.

### Edit intents (`core::edit`)

Gestures arrive in canvas coordinates ("drop here", "drag this handle");
the model stores home pages, anchor-relative positions, margins, group
offsets and overrides. `Editor::new(project, &EditContext, ResolveOptions)`
translates intents into `Vec<Op>` = one undo step, never applying anything.
`EditContext {target, lang, variant, scope}`: `variant` missing = the
target's own, `null` = base, a name = that variant; `scope: all | lang`.

- **Solve against resolve.** Positions are found by applying candidate ops
  to a scratch copy, resolving and reading the element back; a Newton
  iteration (finite-difference Jacobian, ≤ 25 steps, residual < 1e-9 lu)
  adjusts `at` (or a common margin shift for margin boxes). Anchors, gaps,
  grouping, `content_scale`, group transforms and pivots are honoured
  because resolve applies them – no second implementation. Results are
  rounded to 1e-9 lu.
- **Home page** follows the reference point (centre of the drawn box; top
  edge centre for auto-height margin boxes) unless `lock_page`.
- **Resize** keeps the opposite handle fixed; sizes are divided by
  `content_scale`, stretch factors and group `scale`. Devices take width
  only.
- **Overrides**: with an edit variant, field writes go into the merged
  variant override; in `lang` scope text fields go into the view language's
  override. **Structural** changes (order, grouping, duplicate, remove,
  renames) always change the base.
- Intents: `move_to/by`, `resize`, `rotate_to`, `set_pivot`,
  `set_home_page`, `reorder_z`, `duplicate`, `remove`, `group`/`ungroup`
  (ungroup of a scaled group is refused), `arrange` (layer tree, keeps the
  drawn position), `patch` (inspector fields, base or override),
  `reset_override`, `copy_item`/`paste` (session clipboard), canvas copy
  (`edit::copy_canvas`), `overlay` (hit-test data, §12.2),
  `default_language_impact`. Ids: `unique_id` gives `base`, `base-2`…
- Model changes that need several ops are built in Rust (an intent or an
  app command), never in JS.

---

## 5. Resolve pipeline

`core::resolve::resolve(project, target, lang, &ResolveOptions) →
ResolvedScene`. Only an unknown target, canvas or invalid language code is
an error; everything else becomes a diagnostic. The scene is a flat,
z-ordered list of `ResolvedElement`s in canvas lu plus canvas backgrounds,
span styles and diagnostics; renderer, validation, overlays and edit
intents consume only this (no anchor, gap or style logic outside core).

`ResolveOptions` carries the catalog, an `AssetLookup` (file existence,
image dimensions from the header, colour notes, sample marks; `FsAssets`
in production), an optional `FrameIndex` (§7.3) and a `TextMeasure`.

### Order of application

1. **Frame geometry** (`core::geometry`, `CanvasLayout`, `Viewport`): page
   height from target size and `group`; gap from the gallery profile
   (converted with `basis`); export images are always `k` columns (a
   partial last image gets empty columns, info `partial_image`); each
   `OutputImage` carries its source rect, columns and canvas → px viewport,
   shared by export, gallery preview and GUI.
2. **Variant**: the target's variant patches copies of the elements
   (`Element::patched`); unknown variants or non-applying overrides are
   warnings. The project is never mutated.
3. **Language overrides**: for text elements the overrides of the
   language's prefixes first, the most specific last (`de`, then `de-AT`).
   `languages.fallback` entries are not followed – an override belongs to a
   language, not to the text it falls back to.
4. **Visibility**: `visible`, `visible_in.targets` (target id),
   `visible_in.langs` (language or a prefix of it).
5. **Anchors and boxes**: `canvas = (page_x(page) + x, anchor_y(y))`.
   Margin boxes: left/right relative to the page (column), `top` and
   `H0 − bottom` follow the anchor; without `bottom` the box reaches the page
   bottom (`auto_height`, `valign: top`). Free boxes: centred at `at`,
   width default 840 lu, `valign: center`. Page backgrounds fill their page.
6. **Devices**: `device_map` swaps the model (width kept, height from the
   new model's outer size in the effective orientation); colour = element
   colour if the model has it, `#hex`, else the default colour; frame
   `element.frame → styles.frame → detailed`. `aspect: free` models
   (browser, `none`) take the screenshot's aspect when its size is known.
   `fold` picks `iphone-duo` / `iphone-duo-folded`. Imported/pack frames
   select a bezel (§7.3). `content_scale` scales **devices only** (size,
   pivot offset, shadow) around their `at`.
7. **Groups** are flattened (`apply_group`): children keep their own page,
   anchor and position; the group's `at` is an **unscaled offset**; the
   group pivot is the centre of the bounding box of all children's
   reference points (visible or not) plus `pivot`. Order: scale around the
   pivot, rotate around the pivot, add `at`. `scale` multiplies every child
   length (sizes, font sizes, `shrink_to`, outlines, glows, shadows, radii,
   strokes, tile sizes); texts are laid out at the scaled size. Rotations
   add, opacities multiply, `z` adds; nested scales multiply
   (`ResolvedElement::scale`). Clip rects are not transformed.
8. **`clip_to`**: images resolve their mask shape's final geometry
   (variant, anchors, group transform, rotation); hidden shapes are resolved
   quietly when a canvas uses `clip_to`.
9. **Styles**: `@colour` against `styles.colors` (unknown → magenta
   `#FF00FF`, warning); text style = defaults (Inter, 400, 48 lu, line
   height 1.2) ← named style ← element `color`/`font_size`; shadow presets
   (project wins; `soft` = two layers σ 48/y 46/42 % + σ 10/y 10/30 %,
   `hard` = σ 4/y 16/35 %).
10. **Texts and screenshots** along the language chain (texts report their
    source language; screenshots record path, language, variant and level
    `lang_variant | lang | fallback_variant | fallback | missing`; missing →
    placeholder with the key).
11. **Painting order**: stable sort by effective `z` (list order within
    equal z; groups flatten in place).

Banners keep the canvas's own page height (anchors are no-ops), gap 0 for
`gaps: none` or the gallery's gap for `as_store`; `fit: cover` keeps the
horizontal centre and the top (focus (0.5, 0)), `contain` centres and lets
the canvas background extend.

Diagnostics have level, stable code, element id and message; validation
turns them into findings (§8).

---

## 6. Text

### Markup (`core::markup`)

| Source | Meaning |
|---|---|
| `**…**` | bold = CSS `bolder` of the current weight |
| `_…_` | italic; CommonMark flanking, so `snake_case` stays literal |
| `[name]…[/]` | span style `styles.spans.name`; may nest, innermost wins |
| `\n`, newline | hard break |
| `\-`, U+00AD | soft hyphen |
| `\*` `\_` `\[` `\]` `\\` | literal character |

The parser never fails: unbalanced markers render literally and are
reported (`invalid_markup`); unknown span styles render unstyled.

### Styles and spans

Text styles: `font, weight, italic, size, line_height, tracking (em),
color, transform (uppercase/lowercase), shadow, stroke, glow, hyphenate`.
Span styles: `font, weight, italic, color, background, padding, radius,
stroke, glow, shadow`. A span `background` draws a highlight box per line
fragment; `padding` (number or `{x, y}`) and `radius` (clamped to half the
box height: a pill) grow and round it **without changing the line layout**
(horizontal room comes from spaces in the text).

### Layout (`core::layout`, parley)

- `vitrine_core::layout` shapes, wraps and fits with **parley** (fontique,
  harfrust shaping, ICU4X segmentation incl. `complex-scripts` dictionaries
  for Thai/Lao/Khmer/Burmese). It never touches pixels: layout is in lu,
  unhinted, so preview and export break identically. `render::text` draws
  the glyph outlines (skrifa) as tiny-skia paths.
- Alignment is applied by core (parley lines aligned `Left`), so the
  soft-hyphen glyph counts and RTL lines are not aligned twice. Alignment
  is **physical** (`left` is left in every language; RTL languages set
  `align: right` per language). Korean breaks at spaces (`keep-all`).
- Soft hyphens are break opportunities; a line ending at one draws a hyphen
  glyph and must fit including it.
- **Auto-fit**: lay out at the style size `S`; if it does not fit
  (`max_lines`, box height, line width), try `shrink_to` `m`; if that fits,
  binary search the largest `m + k·0.25 lu` (`FIT_STEP`) that fits; if not,
  (hyphenation, then) emergency break at `m`. Every returned layout was
  actually checked.
- **Hyphenation** (opt-in `hyphenate: true` on style, element or language
  override): only when the text does not fit at `shrink_to`. Own Liang
  implementation (`layout::hyphen`) over hyph-utf8 patterns bundled for
  en-us, en-gb, de-1996, fr, es, it, nl, pt, da, nb/no, fi (words ≥ 6
  characters, 2 left / 3 right). Swedish is excluded (LPPL-only patterns,
  GPL-incompatible). Words with a manual `\-` are left alone. Reported as
  info `hyphenated`.
- **Emergency break** (break inside a word without hyphen) only as last
  resort at `shrink_to`, reported `emergency_break`.
- `LayoutReport`: box, content rect, chosen size, lines, overflow,
  diagnostics (rounded to 0.01 lu).

### Effects

`stroke {color, width, position: outside|center, join}`, `glow {color,
radius, opacity, spread}`, `shadow <preset>`. Cascade: defaults ← text
style ← element ← span style (a span that sets any effect gets its own set;
`width: 0`, `opacity: 0`, `shadow: none` switch inherited ones off).
Lengths are absolute lu (auto-fit shrinks the glyphs, not the outline).
Draw order per text: shadows → span highlights → glow → outside outline
(below the fill) → fill → centre outline. Validation uses the *ink* rect
(glyphs + hard outline); culling and layers use the *visual* rect
(including glow and shadow).

### Fonts

- **Resolution order** for a family a style names: bundled → project
  `fonts/` → stored `fonts/system/` → installed system fonts. Bundled
  families win name collisions; project fonts win over installed ones.
- **One builder** for GUI, CLI and MCP:
  `core::layout::setup::project_library(root, project, system)`
  (`RenderContext::for_project`). Tests and golden images use only bundled
  fonts (`RenderContext::new`).
- **Bundled** (SIL OFL 1.1, embedded, registered lazily on first use;
  manifest `core::layout::bundled`): Inter, Inter Display, Montserrat,
  Nunito, Oswald, Playfair Display, Lora, Bebas Neue, Caveat, Dancing
  Script, JetBrains Mono, plus script fonts Noto Sans Arabic, Hebrew,
  Devanagari, Bengali, Tamil, Thai. Variable `wght` files with weight-range
  matching; missing italics are synthesised (14° oblique), synthetic bold is
  an extra stroke.
- **Installed fonts** (`core::layout::system`): a process-wide index built
  by fontique's path scan of the platform font folders (no platform API, no
  C dependency), only when a project needs it (~1 s first scan on macOS).
  App setting `system_fonts` ("Use installed fonts", **on by default**;
  replaces the old opt-in `system_font_fallback`, still read as alias); CLI
  `--no-system-fonts`, env `VITRINE_SYSTEM_FONTS=off`.
- **Fallback chain per text**: family → `styles.fonts.<family>.fallback` →
  library fallbacks → Inter → bundled script fonts for scripts present →
  the language's CJK system preference (ja: Hiragino Sans, Yu Gothic,
  Meiryo, Noto Sans CJK JP…; zh-Hans: PingFang SC, Hiragino Sans GB…;
  zh-Hant/HK; ko: Apple SD Gothic Neo, Malgun Gothic…) → installed families
  covering most remaining characters. The text's language is the shaping
  locale.
- **Stored system fonts** (`layout::stored`, "Store used system fonts in the
  project"): copies only the faces the styles pick into `fonts/system/` with
  `fonts.yaml` (licence classified from the name table: `ofl`,
  `restricted`, `other`, `unknown`) and a README licence notice. Folder =
  option; not an op (no undo); not an MCP tool; never in templates.
- Findings: `unknown_font`, `missing_glyphs`, `system_font_used`,
  `system_font_fallback`, `system_fonts_stored`.

---

## 7. Rendering

### Pipeline (`vitrine-render`)

```text
project + i18n + assets
   │ core::resolve(target, lang)       ResolvedScene (lu)
   │ render::prepare(scene)            texts laid out once → PreparedScene
   │ render::rasterize(image, scale)   tiny-skia pixmap (page, group, banner, canvas, view)
   ├─▶ GUI preview (binary, cached)
   ├─▶ MCP render_preview (PNG/JPEG)
   ├─▶ validation pixel checks (§8)
   └─▶ export (flatten alpha, encode PNG/JPEG)
```

- **tiny-skia** for paths, gradients, patterns, masks, AA. Pure Rust, no C.
- `RenderContext` (one per project, `Send + Sync`): asset cache, font
  library, layouter pool, frame index. `prepare` lays out every text once;
  any number of output images of a prepared scene rasterize in parallel.
- `Scale::{Target, Factor, MaxEdge, Width}`; `Quality::Preview` resamples
  bilinearly, `Export` with Lanczos3. Rendering is deterministic (no
  clocks, no randomness; caches only change speed).
- Elements draw with `view · translate(center + pivot) · rotate ·
  translate(−pivot)`; outside the viewport they are culled. Opacity and
  clipping use an offscreen layer covering only the element's pixel bounds;
  a clip mask is only built when the element crosses its clip rect. Page
  images show exactly their source rect, so gap content is omitted without
  special cases.
- **Shadows and blur**: own 3-pass box blur on a (downsampled) alpha mask;
  offsets in canvas space (they do not rotate). Image shadows follow the
  image's alpha (like CSS `drop-shadow`).
- **Images** are decoded once (`raster::SourceImage::decode`), resampled to
  the exact destination size (fast_image_resize, Lanczos3 for export) and
  mapped with a bilinear pattern for rotation/sub-pixel scale. SVG through
  resvg (no text, no embedded rasters). Screenshot resolution affects
  quality only, never layout; upscaling > 1.15× warns.
- **`clip_to`**: the image layer is drawn through a mask filled from the
  shape outline (rect with radius or ellipse), intersected with the page
  clip; the image's own shadow is clipped too.
- **Caches**: decoded assets keyed by path + mtime, LRU 1.5 GiB; resampled
  pixmaps keyed by (path, mtime, size, mode, filter), cleared above 768 MB;
  a rebuilt render context keeps decoded images. Asset paths must be
  relative without `..`.

### Colour management

Everything renders in sRGB; model colours are sRGB. Every decoded raster
image (screenshots, assets, imported bezels) is converted to sRGB **once
at decode** with `moxcms` (`core::colorspace`): precedence PNG `cICP` →
`iCCP`/JPEG ICC → `sRGB` chunk → untagged (= sRGB); sRGB-equivalent
profiles are identity (bit-identical, goldens unchanged); relative
colorimetric, out-of-gamut clips; 16-bit converted before reduction to 8.
`gAMA`/`cHRM` alone are ignored. Unusable profiles (corrupt ICC, CMYK, HDR
PQ/HLG, unknown cICP) render unconverted with info
`color_profile_unsupported`. Exports: PNG with `sRGB` chunk, JPEG untagged.
No Display P3 output (would need a second colour pipeline; revisit after
1.0 as a per-target option).

### Device frames: four sources

No vendor graphics ship with Vitrine Studio or live in this repository.

1. **Parametric frames** (`render::frames`, data in `devices.yaml`): own
   vector artwork (housing with continuous corners, buttons, cut-outs,
   browser chrome, laptop base, watch crown) in styles `stylized` and
   `detailed`, any colour. Device-local lu: the outer box (incl. base or
   stand) has exactly the resolved element size. Landscape = the portrait
   frame rotated; screenshots are never rotated. Without a screenshot the
   screen shows a placeholder with the key. Guide:
   `docs/dev/adding-devices.md`.
2. **Imported vendor bezels** (`core::frame_import`, `frame: imported:apple`):
   the user downloads Apple's Product Bezels and imports a folder, zip or
   mounted DMG (DMGs with a licence agreement must be opened in Finder first;
   Vitrine never accepts an SLA). Rules in
   `crates/core/data/frame-import/apple.yaml` (ordered patterns with
   `{color}`, `{orientation}`, `{*}`, `skip:` rules, colour aliases,
   `verified` dates) map files to model × colour × orientation × vendor
   model; unmatched files are assigned by the user
   (`user-mapping.yaml`). The screen is found by flood fill from the centre
   over transparent pixels (mask grown 2 px + feather), the housing swatch is
   measured. Stored app-wide in `<app data>/frames/<source>/` with
   `index.yaml`, masks and the package's licence file. **Every vendor model
   is kept** (iPhone 18 Pro and Pro Max: the 6.3″ and 6.9″ slots each need
   their own). `select_bezel`: pinned `bezel:` → screen size = target page
   size → = screenshot size → closest aspect (±2 %) → largest. Missing →
   parametric `detailed` + info `imported_frame_missing`. Colours accept any
   slug with an external frame; switching frame source remaps the colour to
   the closest housing colour in the same undo step.
3. **Own frame packs** (`frame: pack:<id>`, format `docs/guide/frame-packs.md`):
   folder `<id>/` with `frame.yaml` (`format: vitrine-frame/1`, frames with
   file, colour, orientation, bezel, `models` (empty = any device), `screen`
   + `radius` or `mask` or detection, `cut_screen`, SVG `raster` size) in the
   project's `frames/` or app-wide `<app data>/frames/packs/`. Compiled into
   bezel + mask + index with a stamp (`.cache/frames/<id>/` resp.
   `.compiled/`); SVG via `render::frame_packs::ResvgRasterizer`. Project
   packs win over app packs. Pack id `imported` is reserved.
4. **Stored copies in the project** (`frame_import::embed`,
   `frames/imported/<source>/`, same layout as the app store + README +
   `embed.yaml {all_sizes}`): only the bezels the targets draw, so CI
   renders like the owner's Mac. Lookup **project copy → app store →
   parametric**; a project copy wins over a newer local import ("update" is
   offered, never automatic). `select_bezel` is monotone, so the subset picks
   the same bezel. Apple's licence allows this in **private repositories
   only**; every surface warns, templates refuse it, it is not an MCP tool.
   Infos `imported_frame_not_stored`, `imported_frames_stored`,
   `stored_frames_stale`.

One entry point builds the index for GUI, CLI and MCP:
`vitrine_render::frame_packs::frame_index(frames_root, project)`. An
imported or pack device is composed in device pixels (screenshot × mask,
bezel on top) and drawn with the element transform; its alpha is the shadow
source. The GUI watcher does not reload frames when `frames/` changes on
disk (re-open the project).

### Golden tests

`crates/render/tests/golden.rs` renders every output image of every target
of `tests/fixtures/example` (en; de for the iPhone set and blog header; fr
for iPhone and iPad) at 25 % and the gallery previews at 320 px and compares
with `tests/fixtures/example/golden/` (a pixel differs above 8/255; an
image fails above 0.5 % differing pixels). Bundled fonts only.
`VITRINE_UPDATE_GOLDEN=1` rewrites; visible renderer or fixture changes
update the references in the same commit. `RENDER_REVISION` (export hash)
is bumped when output changes.

---

## 8. Validation and design checks

`core::validate::validate(project, &ValidateOptions) → ValidationReport`.
One report with stable codes for CLI, GUI, MCP and the upload plan; front
ends only format it.

- `Finding {level, code, target, lang?, page?, element?, message}`; levels
  `error` (blocks export with exit 1 and uploads), `warn`, `info`. 81 codes
  in `FindingCode` (snake case, never renamed). The full table with
  thresholds and fixes is `crates/mcp/src/docs/checks.md` (a test requires
  every code there); this section gives the rules.
- For each target and each of its languages: resolve → its diagnostics →
  lay out every text (same helper as the renderer) → element checks. Store
  rules and the variant suggestion run once per target. `lang` is set only
  for language-dependent findings, so file and geometry findings appear
  once per target.
- `ValidateMode::export | preview`: missing screenshots are errors when
  exporting, warnings while editing.

### Levels (main rules)

| Situation | Level |
|---|---|
| unknown device model, unknown `@colour`, page out of range, text missing in every language, text overflow (per element × language), store size/format/count violations, missing screenshot (export mode), unknown store locale code | error |
| text from the default language (not translated), emergency break, unbalanced markup, unknown span/font, missing glyphs, text in a gap, text clipped, screenshot aspect off by > 2 % (> 5 % under `fit: cover`, or any with `contain`/`stretch`), upscale > 1.15×, element outside every page, store mapping conflicts (missing/duplicate locale, slot conflicts) | warn |
| text from a prefix or configured fallback, screenshot fallback, screenshot aspect off by 2–5 % under `fit: cover` (only a thin edge cropped, e.g. iPhone captures on a Play target's Android frame), hyphenated, variant suggested (page height differs > 15 % without a variant), colour profile unsupported, imported frame missing, sample screenshot | info |

Store rules (`targets.yaml` `rules`, found via presets with the target's
slot): accepted formats, exact sizes or ranges with aspect limits, counts.
Exports are always opaque, so "no alpha" holds by construction. Store
mapping (§10) adds `store_locale_*`, `store_no_slot`,
`store_slot_duplicate`. A check that an App Store project contains the
*required* sets (iPhone 6.3″, iPad 13″) does not exist yet.

### Design checks

Two kinds of measurement:

- **Layout checks** in `core::validate::checks` (no pixels):
  `device_small` (largest device < 50 % of its page), `page_not_covered`
  (page-filling picture leaving a strip ≤ 15 %, quiet when something below
  fills it), `seam_split` (two halves of one shape at a seam).
- **Pixel checks** in `render::quality`, plugged in through the core trait
  `SceneInspector` (core stays render-free): small partial renders of the
  prepared scene at a fixed 0.5 px/lu. An element drawn alone without
  effects is its mask; the composite up to a text is its backdrop.
  `low_contrast` (WCAG: 4.5:1, large text ≥ 80 lu or bold ≥ 62 lu 3:1; 30 %
  quantile per 1-em cell; texts with opacity < 0.5 skipped),
  `text_near_device` (glyphs behind or crossing a frame warn; closer than
  16 lu info; a text fully on a device is fine), `screen_header_covered`,
  `screen_header_cut`, `screen_covered` (≥ 15 % of the screen below the top
  bar), `device_cut` (> 8 lu lost at a left/right edge), `seam_overlap`,
  `element_in_corner`, `page_blends_with_store` (against the gallery's
  light/dark background).
- **Language scope**: texts are checked in every language; device, page,
  corner, store-background, seam and store-rule checks only on the **first
  language of each target**.
- **Cost**: ~0.3 s per target × language in release. On by default in CLI
  `validate` (`--no-pixel-checks`), app command and MCP `validate`
  (`pixel_checks`) and `render_preview` (one view); off in export
  pre-checks and upload plans.

### Intended designs: `allow` and `checks`

- Element field `allow: [codes]` (common, variant-overridable; on a group it
  covers the children) silences findings that name that element. Only codes
  in `validate::ALLOWABLE` (design findings plus `text_clipped`,
  `text_in_gap`, `text_outside_safe_area`, `element_outside`, `hyphenated`,
  `variant_suggested`, `screenshot_upscale`, `clip_to_other_page`); errors
  and missing content never. Bad entries → `unknown_allow`.
- Project `checks:` (op `set_checks`): `device_distance`, `min_contrast`,
  `min_contrast_large`, `screen_cover`, `off: [codes]`. Other thresholds are
  code constants.
- `scene_report` (app) / MCP `layout_report` expose the same geometry:
  device frames and screens (rotated quads, top-bar height, covering
  elements), gallery gap/radius with `measured`, seams with
  `next_page_offset`, pixel-measured overlaps.

---

## 9. Export and import

### Export (`render::export`)

- Target × language pairs (target `langs`, else all available) are
  resolved and prepared in parallel (rayon); then every file is rendered,
  encoded and written in parallel.
- **Standard layout** (`export/`): `<target>/<lang>/<nn>_<page-id>.<ext>`,
  grouped `<nn>_<id1>+<id2>.<ext>`, `banner.<ext>`, `_gallery.png`. Names
  are fixed (KONZEPT's name templates were not built).
- PNG: 24-bit RGB flattened onto white, `sRGB` chunk,
  `Compression::Balanced`; JPEG with the target's `quality` (default 90).
  **Never an alpha channel.**
- **Gallery preview** (`_gallery.png`): output images at ≤ 1200 px height,
  placed at their canvas x (so content across a gap lines up), with the
  profile's gap, (elliptical) corner radius and background. Standard layout
  only; `pages: false` / `--gallery-only` writes only previews.
- **Incremental**: per file a hash of the serialized scene, catalog data of
  its devices, mtimes of referenced assets, fonts and frame files, format
  and `RENDER_REVISION`, stored in the manifest `<out>/.vitrine-export.json`
  (also target, language, kind, size per file). Unchanged files are skipped
  (`--force` re-renders; a missing file is re-rendered).
- **Stale cleanup**: a re-export deletes files of the same targets ×
  languages it no longer produces – only files listed in the manifest.
- **`index.html`** (default on; `--no-html`): self-contained overview of all
  current images by language → target, with lightbox; works offline.
- `ExportReport`: files (size, bytes, ms, skipped), issues, errors, timings,
  `foreign`. Hooks for progress and cancel (GUI `export-progress`, MCP
  progress notifications).

### fastlane layout

`--layout fastlane` (default folder `export/fastlane/`), conventions in
`crates/core/data/fastlane.yaml`, paths and conflicts in `core::fastlane`:

- App Store targets → deliver `screenshots/<asc-locale>/<nn>_<target>.png`
  (iMessage under `screenshots/iMessage/`; iPad 2048×2732 with slot
  `APP_IPAD_PRO_129` gets deliver's 2nd-gen marker); Google Play → supply
  `metadata/android/<play-locale>/images/<type>/<nn>_<locale>.png`, feature
  graphic `images/featureGraphic.png`.
- Left out with a note: targets without a store slot, App Store creatives,
  sizes deliver rejects, languages without a store locale, gallery previews.
- deliver detects the set by pixel size: two targets of one size class in a
  locale folder conflict – the first claim wins, the later one is an error.
- **Captures and exports in separate folders** (fastlane uploads every
  image of those folders): recommended snapshot/screengrab
  `output_directory` below `fastlane/captures/`; import reads that.
  The export lists **foreign** images in the deliver/supply folders it writes
  (`ExportReport::foreign`, one warning per folder) and deletes them only
  with `--remove-foreign` (CLI only) after a complete export.

### Screenshot import (`core::import`)

Plan, then apply; never moves or deletes source files.

- `plan_screenshot_import` / `plan_files` only read; `apply` copies
  atomically and skips identical destinations.
- **Formats**: auto, Vitrine folders (`<lang>/[<variant>/]<key>`), fastlane
  (snapshot `<locale>/<Device Name>-<screen>.png`, screengrab
  `metadata/android/<locale>/images/<type>Screenshots/`, whole `fastlane/`
  folders), flat.
- **Detection**: language (locale map → exact → case-insensitive → shorter
  subtags → store-locale map); variant (device map or folder/id → catalog
  screenshot sizes → display aspect ±2 %); key (key map → screen name
  without numeric prefix, slugified, matched to used keys). Each row has a
  confidence and notes; actions `new | overwrite | unchanged | duplicate |
  unresolved`; ignored files (frameit `_framed`, `Framefile.json`,
  `.strings`, metadata, Play graphics) are listed with a reason. `missing`
  lists keys still without an own file per language × variant.
- **Own exports are never imported**: files listed in a
  `.vitrine-export.json` (in the source or up to three folders above) and, in
  fastlane folders, files named like the fastlane export layout.
- Mappings can be saved to `project.yaml` `import.fastlane` (op
  `set_import`, one undo step) so re-imports run unattended. Apply re-plans
  with the same arguments (the backend stays stateless).

### Translation exchange (`core::exchange`)

- Export CSV (`key, <default>, <langs>…` + optional `max_chars, context,
  notes`; delimiter, BOM for Excel), XLIFF 1.2 and 2.0 (one file per target
  language, source = default language). Markup becomes inline codes (bold,
  italic, spans as paired codes, breaks/soft hyphens as placeholders) so CAT
  tools protect it; `markup: plain` writes it as text. Round trip is
  lossless except one overlap corner case.
- Import: any mix of files → `ImportPlan` (new/changed/removed/unchanged
  per language, unknown keys, markup issues, per-file errors);
  `check_layout` compares overflow/emergency-break findings before and
  after; applied as **one transaction**; `conflicts()` detects strings
  edited in between. "Unchanged" compares the parsed rendering, not bytes.

---

## 10. Store upload

**Never upload without a plan and an explicit confirmation – also not via
MCP.** Everything lives in `vitrine_app::upload` (it needs rendering,
validation, settings, keychain and the clients; `core` stays network-free,
`publish` holds only clients).

```text
readiness → plan (dry run + confirm token) → confirm → execute → log
```

### Accounts, credentials, mapping

- **Accounts are app-wide** (`settings.yaml` `store_accounts`: id, kind,
  label, Apple issuer/key id, Google client e-mail – public facts only),
  never in projects. Only `store_account_import|remove` change them.
- **Secrets only in the system keychain** (`publish::credentials`, `keyring`
  4 + `keyring-core`): service `app.vitrinestudio`, user
  `<account id>/<kind>`, password = the `.p8` PEM or service-account JSON.
  Import validates the key (P-256 EC for Apple, `service_account` JSON for
  Google) without echoing secrets. `VITRINE_KEYRING=mock` = in-memory;
  unit tests use `SecretStore::mock()`; the dev bridge uses a mock unless
  `--system-keychain`.
- **CI credentials** (`publish::env_credentials`, read only by `vitrine
  upload`): `VITRINE_ASC_KEY_ID|_ISSUER_ID|_KEY|_KEY_PATH`,
  `VITRINE_PLAY_SERVICE_ACCOUNT[_PATH]`; all-or-nothing per store, win over
  the project's account, never touch the keychain. GUI, dev bridge and MCP
  never read them.
- **Project mapping** `store:` (§3) with locale map defaults from
  `crates/core/data/store-locales.yaml` (Apple 50 / Google 86 codes):
  explicit map → the language itself if it is a store code → default for
  the tag → default for the primary subtag.
- **Connection test** (`store_test::StoreClientTester`, GUI and dev bridge
  only): read-only (`GET /v1/apps?limit=1`; Google token fetch). App picker
  (`store_apps_list`) for Apple; Google has no app list, so
  `store_package_check` opens and deletes a draft edit.

### Readiness and plan

- `upload_readiness` (no rendering, keychain or network): ordered steps
  with a `fix` place – app settings (`no_account`,
  `account_missing_for_project`), project settings (`project_not_mapped`,
  locale problems), targets (`no_store_slots`, `store_slot_conflict`).
- `upload_plan {store?, targets?, langs?, remote?, force?}`: every slotted
  target × language; the incremental export renders into
  `<project>/.cache/upload/images`; SHA-256 per image; compared with the
  log's last successful set → `new | replace | unchanged | skip`.
  `remote: true` also reads the store (Apple app → editable version →
  asset library + ref data → localizations → placements; Google a
  throw-away edit) and refines actions and steps.
- **Blocking problems** block the whole plan: validation errors (export
  mode), store-mapping conflicts, two targets in one set, file too large,
  missing account/app, sample screenshots, an Apple slot without
  `asset-library.yaml` mapping (`no_placement_mapping`), and with the
  remote check: no editable App Store version, no asset library access
  (`asset_library_unavailable`), no placement group in the ref data
  (`no_placement_group`). Narrow with `--target`/`--lang`.
- **Confirm token** `upl_<32 hex>`: bound to a fingerprint (project folder,
  revision, `store:` section, selection, per-set image hashes), valid 30
  minutes, once; stored with the plan in the log (CLI plan and execute are
  separate processes). Execute re-builds the local plan; any saved change
  → `not_confirmed`. The stored plan is executed, never a client-supplied
  one.

### Execution

- One run at a time on its own thread (current-thread tokio); `wait` for
  CLI/tests, events for GUI/MCP (`upload-progress`, `upload-finished`).
  Watchdog: heartbeat 30 s, a run without progress for 10 min fails; runs
  of a crashed app show as `interrupted`. Rate limits pause a set (≤ 15
  min, 2 pauses). Cancel works in every phase.
- **Apple** only through the **App Asset Library** (`upload::library`,
  `AssetLibraryFlow`: `open`, `inspect`, `replace`; decision 0034): images
  in the per-app library with a content marker in `referenceName`
  (`<stem> [<sha256[..12]>]`, the library has no checksums), reused when
  present; placements on the version localization created per route
  (placement type + group resolved **at runtime** from
  `appAssetLibraryRefData` via `crates/core/data/asset-library.yaml`),
  ordered with an ordering request, verified; images still processing are
  placed anyway (Apple allows it) and polled together up to 15 min per
  set (still processing = `uploaded_processing`, confirmed later by a
  remote plan); a `FAILED` asset fails the set before any placement
  changes; old Vitrine assets are deleted after their placements.
  Every Apple slot (iPhone, iPad, Mac, Watch, TV, Vision, iMessage, Duo,
  header / search-results creatives) has a mapping row; a slot without a
  ref-data group is blocking (`no_placement_group`), a key without library
  access fails the store once (`asset_library_unavailable`). The
  deprecated screenshot-set API (`appScreenshotSets`, ASC API ≤ 4.5.0) and
  the `store.apple.upload_flow` choice were removed (decision 0034).
  - "No editable App Store version" is one short store-level message with a
    link to App Store Connect; the store's sets are skipped.
- **Google Play** (Android Publisher v3): one edit for all languages and
  image types: per set `deleteall` → upload in order; then commit
  (`changesNotSentForReview` on request). Any failure or cancel deletes the
  edit – nothing changes in Play.
- **Resume** = a new plan: done sets with the same hashes are unchanged;
  everything else is replaced as a whole set.
- **Log** `<project>/.cache/uploads.sqlite`: plans, runs (heartbeat, report),
  sets, files (store ids, status), events (method, redacted URL, status,
  timing – never headers, bodies, tokens, keys or signed URLs).

### Store clients (`vitrine-publish`)

Plain REST with `reqwest` + rustls, `jsonwebtoken` (aws-lc-rs): Apple ES256
JWTs (15 min, cached, re-signed on 401), Google service-account OAuth
(token URL from config, never from the JSON file). Errors are typed
(`Auth`, `NotFound`, `Conflict`, `RateLimited`, `Validation`, `Processing`,
`Transport`, `Timeout`, `Cancelled`); retries with jitter (creating POSTs
retry only when no duplicate can result); `Retry-After` honoured up to 120
s; cancellation tokens; an observer gets redacted HTTP events. Bearer tokens
go only to the configured origin. Endpoints are overridable for tests
(`VITRINE_ASC_BASE_URL`, `VITRINE_PLAY_BASE_URL`, `VITRINE_PLAY_TOKEN_URL`,
loopback only, CLI and dev bridge); tests use wiremock only.

### MCP policy (`upload::policy`)

| Caller | Requirement |
|---|---|
| user (GUI button; CLI `--confirm <token>` + prompt or `--yes`) | confirm token |
| assistant via the GUI-hosted MCP | token **and** the user's "Confirm upload" click in the app (`upload-confirm-requested` → `upload_confirm`; 5 min timeout); a one-time grant bound to the token |
| assistant via `vitrine mcp --headless` | refused unless app setting `mcp_allow_headless_upload` (default off) |

Assistants never see key material, cannot import accounts and cannot
change the mapping (`apply_ops` refuses `set_store`). CI uploads are plan +
`execute --confirm <token> --yes` in one job; without a terminal and
without `--yes` execute refuses (exit 2).

---

## 11. Templates and gallery

### Package `.vitrine-template` (`core::template`)

A zip of a project subset plus manifest; the extracted layout is what the
app-wide store `<app data>/templates/<id>/` keeps.

```text
template.yaml                  manifest (format vitrine-template/1, id, name, description,
                               author, tags {style, color, use}, version, license: CC0-1.0,
                               canvases, fonts, fonts_required, samples, generator)
project.yaml                   canvases + their targets + variants + styles; no store:/import:
i18n/<lang>.yaml               only the keys the canvases use
screenshots/<lang>/<key>.png   generated placeholders, or sample screenshots (JPEG q60)
assets/…  fonts/…  previews/<canvas>.png
```

- **Licence**: templates are always CC0; fonts only under the OFL (checked
  in the name table; licence file kept or generated from the copyright
  line + OFL text). Non-OFL fonts are refused unless
  `skip_unlicensed_fonts`, then listed in `fonts_required`; installed
  system fonts are always listed (`not_in_project`). `fonts/system/` and
  `frames/imported/` never go in.
- **Safe reading**: client limit 250 MB packed / 500 MB unpacked / 50 MB per
  file / 1000 files / depth 6 (sizes counted while reading); relative paths
  only, no `..`, hidden components, symlinks or case collisions; file kinds
  checked by content (PNG/JPEG magic, SVG without scripts/external refs,
  fonts parsed); the project subset must load and validate (content-
  dependent findings tolerated).
- **Tags**: fixed taxonomy `crates/core/data/template-tags.yaml` (`style`
  1–2, `color` 1–2, `use` 1–3; `devices` derived, never stored); local
  templates may leave facets empty, the gallery needs the minimums.
- **Save** (`extract`): selected canvases with targets, all styles, used
  strings, assets (moved under `assets/`), OFL fonts, placeholders for every
  screenshot key (or sample screenshots with
  `SaveOptions::sample_screenshots`, re-encoded as JPEG q60 unless
  transparent), previews rendered with parametric frames only.
- **New project** (`instantiate`): all canvases; texts mapped per chosen
  language (same → primary subtag → a template language with that subtag
  → template default, reported); language-only elements follow that
  mapping or are removed; **placeholders are not copied** (validation keeps
  asking for screenshots; no placeholder can reach a store); samples are
  copied and marked in `screenshots/samples.yaml` (info `sample_screenshot`;
  the upload plan blocks them).
- **Insert** into an open project (`insert`): renames planned with ops on a
  scratch copy, then **one transaction**: styles (colours → shadows → spans
  → text styles; equal = shared, different = renamed `<name>-2` by default
  or `use_project`), ids (`-2`, `-3`…), texts (shared when equal in every
  language, else `<template id>.<key>`), files (identical reused, else
  `-2`). The project's frame style and font settings win.
- **Built-ins** `phone` (default; iPhone 6.9″ + 6.3″, Play, blog banner)
  and `minimal`, embedded from `crates/core/templates/<id>/`, same format,
  previews rendered on demand.

### Gallery backend (`crates/api-server`, api.vitrinestudio.app)

- axum 0.8 + rusqlite + files on disk, Docker behind the owner's reverse
  proxy, configuration via `VITRINE_API_*`. API reference `docs/api/API.md`,
  deployment and GDPR notes `docs/api/DEPLOY.md`. One module per area under
  `src/routes/` (`templates`, `admin`, `auth`); schema version via
  `PRAGMA user_version` with idempotent migration and a start-up backfill.
- **Reads are public** (list with facet filters, detail, download,
  previews, WebP sprites with ETag, `/v1/tags`; generous per-client rate
  limit; CORS only for configured origins). **Uploads need a login.**
- **Uploads** are re-checked with the same core code, screenshots replaced
  by generated placeholders (samples kept only with
  `?confirm_own_screenshots=true`), previews and sprites rendered by the
  server (parametric frames only, `VITRINE_API_RENDER_CONCURRENCY`), stored
  as `pending`. 413 `too_large` above `VITRINE_API_MAX_UPLOAD_MB` (default
  50).
- **GitHub login**: device flow for app/CLI (proxied by the server, which
  enforces GitHub's interval and own limits) and web flow with state cookie
  + PKCE for the website. No scope; the GitHub token is dropped after
  `GET /user`. Own session tokens `vss_…` stored as SHA-256 hashes (90
  days). Static upload tokens for the org `vitrinestudio-app` (official
  templates) and testers. Ownership by numeric GitHub id, else login.
- **Moderation**: admin bearer token from the environment; approve, reject
  (= delete row, reports and files at once), rerender. Retention: rejected
  templates and resolved reports are not kept; rate-limit counters and
  expired sessions are purged by a housekeeping task.

### Gallery client

`vitrine_publish::gallery` (reqwest + rustls; core stays network-free):
`api_url` (explicit https/loopback URL, `VITRINE_API_URL` loopback only,
else production), list/detail/preview/download with size caps,
upload, device login, admin calls. `GalleryError::Unreachable` for
offline/DNS/proxy errors so front ends stay quiet. Session token in the
keychain (`gallery/<api host>`). App commands `gallery_list`,
`gallery_download` (same safe import; installs into the app-wide store,
kept for offline use), image `gallery_preview`; GUI tab "Template gallery"
in the New project dialog; CLI `vitrine gallery
login|logout|list|upload|download|admin`. Tokens are never command-line
arguments (`--token-env VAR`).

---

## 12. Application layer and front ends

### 12.1 `vitrine-app` (no Tauri dependency)

- **Commands** (`commands/`, one module per area): plain functions
  `fn(&AppState, Args) -> Result<T, CommandError>` with serde types,
  `dispatch(name, json)` from the `command_table!` (JSON commands and
  image commands returning pixels). Errors `{code, message, details}`
  (`no_project`, `invalid_args`, `not_found`, `op_rejected`, `conflict`,
  `load`, `io`, `render`, `busy`, `unreachable`, `remote`, …). A new command
  goes into `command_table!` **and** the GUI's wrapper lists
  (`crates/gui/src/main.rs`).
- **Session** (one open project per `AppState`): project + history + folder
  + `RenderContext` + preview cache. `apply_ops`/`undo`/`redo` work on
  copies, save (revision check, atomic), commit only on success; a conflict
  reloads and returns `conflict`. Results report revision, step label and
  a diff of what changed.
- **Watcher** (`notify`, 150 ms debounce): model files reload (own writes
  recognised because the folder loads to the in-memory project); external
  edits replace the model, clear the history and emit `project-changed
  {reason: external}`; unparsable files emit `project-error` and keep the
  model; asset changes drop affected previews; font changes rebuild the
  render context.
- **Events** through an `EventSink` trait (Tauri emit, bridge SSE, MCP
  progress relay): `project-edited {action, actor, label, revision,
  changes}` for every applied/undone/redone step; `project-changed` only
  for reloads; `project-opened`/`-closed`; `export-progress`;
  `upload-*`; `mcp-status`.
- **Actor** (`Actor::User | Assistant {client, name}`, thread-local scope or
  `dispatch_as`): assistant steps get history labels prefixed with the
  client's display name ("Claude Desktop: Move title").
- **View state** (`ViewState {canvas, target, lang, variant, selection}`):
  pushed by the GUI (`view_set`), read by `view_get` / MCP `selection_get`.
- **Preview cache**: key = target, language, view, size, quality, format and
  a hash of the resolved scene incl. asset mtimes; LRU 256 MiB of encoded
  data; `interactive` renders at half size while dragging.
- **Settings**: `settings.yaml` (UI language, default project language and
  frame style, window, `system_fonts`, MCP switches and port, store
  accounts, `mcp_allow_headless_upload`, `guide_seen`, update check) in the
  platform config folder; recent projects in `app.sqlite`. App data via
  `directories` (`app.vitrinestudio`); `VITRINE_APP_DIR` puts everything
  under `<dir>/config` and `<dir>/data`.
- Also: `dialogs` (trait; Tauri dialog plugin from Rust, fixed answers in
  the bridge/tests), `stores`, `upload`, `claude_desktop` (order-keeping
  JSON merge with backup; `VITRINE_CLAUDE_DESKTOP_CONFIG` redirects),
  `cli_install` (macOS symlink `/usr/local/bin/vitrine`, Windows user PATH;
  `VITRINE_CLI_LINK_DIR`), `open_url` (exact allow-list), `about`.

### 12.2 GUI (`vitrine-gui`, Tauri 2)

- **The GUI never renders in the browser.** It shows Rust pixmaps and draws
  only interaction overlays (selection, handles, guides, hatched gaps,
  safe areas) as HTML/SVG, from the `overlay` command (page/gap/image
  rects; per element quad, handles, pivot, overridden fields, text content
  rect, fitted size, overflow).
- **Preview transport**: `tauri::ipc::Response` binary (PNG `Fast`/`Up` by
  default, raw RGBA optional) with a 40-byte header (magic `ARPX`, size,
  format, canvas origin, px-per-lu); the bridge sends the same as HTTP with
  `X-*` headers. `ui/js/transport.js` hides the difference.
- **Edit gestures** go through `overlay` + `edit_*` commands (view context
  `{target, lang, variant, scope}`, one undo step each);
  `render_preview_edit` renders drag previews from a scratch copy, never
  saved. New elements come from `element_add` (defaults built in Rust).
- **Frontend** `crates/gui/ui/`: vanilla ES modules, no framework, no npm,
  no build step; tiny store, editor controller, views, panels with
  `mount(el, ctx)`, dialogs via `dialogs/modal.js`; conventions in
  `crates/gui/ui/README.md`. UI strings in `i18n/en.json` and `de.json`.
- **Security**: the web view gets only `core:default`; every file/system
  access is a custom command. The CSP forbids inline styles and scripts.
- **Native menu** (macOS only): the UI builds a menu spec and sends it via
  the GUI-only command `menu_set`; items emit `menu-action` to the same
  handlers as the in-app ☰ menu; the native menu owns its shortcuts there.
  Windows/Linux keep the in-app menu only. The web-view context menu is
  suppressed except in text fields (⌥/Alt + right-click = debug).
- **Updater** (`crates/gui/src/updater.rs`, `tauri-plugin-updater`, from
  Rust only): `latest.json` from GitHub Releases, signed (minisign,
  `requireSignedVersion`), daily check setting, the user confirms install;
  installed bundles only (deb/rpm get a notice).
- **Dev bridge** (`vitrine-devbridge`, feature `devbridge`, never bundled):
  minimal blocking HTTP/1.1 on 127.0.0.1 serving `ui/`, `POST
  /cmd/<name>` → `dispatch`, `GET /events` SSE; `--copy`, `--app-dir`,
  `--pick-*`, `--mcp`. No CSP there – check the real app after touching
  resource loading.

### 12.3 CLI (`vitrine`)

One module per command: `new`, `render`, `export`, `validate` (text, json,
github, gitlab, junit; `--strict`, `--preview`), `strings
export|import`, `import screenshots`, `frames import|list|remove|embed|unembed`,
`fonts [embed|unembed]`, `template …`, `gallery …`, `upload
plan|execute|log|refdata`, `mcp`, `install-cli`. Render/export/validate use
core + render directly with the same font library and frame index as the
app; upload and MCP go through `vitrine-app`. Exit codes: **0 ok, 1 project
problems, 2 could not run**. `vitrine mcp` never prints to stdout outside
the protocol. The CLI edits projects only where a command implies it
(import mapping, strings import, template insert, frames/fonts embed).

### 12.4 MCP (`vitrine-mcp`)

- **One handler, two hosts.** `VitrineMcp` over an `Arc<AppState>` (rmcp
  `=3.9.0`, exact pin). Headless: own state. **GUI-hosted**: the GUI's live
  `AppState`, so assistant edits appear live, are undoable and labelled;
  omitted `target`/`lang` default to what the user is looking at.
- **In-app endpoint** (`host`, feature `http`): Streamable HTTP on
  `127.0.0.1` (preferred port setting or random), path `/mcp`, bearer token
  (32 random bytes, new per start, constant-time compare), loopback Host
  and Origin checks (DNS rebinding), discovery file `<app data>/mcp.json`
  (mode 0600, removed on exit). Setting `mcp_enabled` (default on) switches
  it live.
- **`vitrine mcp`** (stdio, what Claude Desktop starts): finds the running
  GUI via the discovery file → transparent JSON-RPC proxy (keeps the
  client's `clientInfo`, replays `initialize` after a GUI restart); none →
  starts the GUI (`mcp_autostart_gui`, default on; ≤ 20 s); else or with
  `--headless` → own session on `--project`.
- **Tools** in `src/tools/<area>.rs`, one `#[tool_router]` block each
  (~65 tools; list in `docs/guide/NOTES.md`). Arguments are
  `schemars::JsonSchema` structs with `deny_unknown_fields`. Every body runs
  in `VitrineMcp::run`: client name → `spawn_blocking` → `Actor::scope` →
  `sync_with_disk` (catches external edits; a broken `project.yaml` gives
  `load`) → app commands by name; never the model. Results:
  `structuredContent` + a text block, PNG image blocks; errors as tool
  errors with code and hint (argument errors too).
- MCP edits default to scope `base` (not the target's variant). Batch
  edits: `element_update {updates}`, `apply_ops {ops | file}`.
- **Docs for assistants**: server instructions `src/instructions.md`, op
  reference `src/ops.md`, `docs_get {topic}` topics `src/docs/*.md`
  (overview, elements – generated from the model –, ops, project, markup,
  targets, scopes, checks), also as resources `vitrine://docs/<topic>`;
  resources `vitrine://project/project.yaml`, `vitrine://project/i18n/{lang}.yaml`,
  `vitrine://catalog/{devices,targets}`.
- **Boundaries**: no network except the upload tools; writes only inside the
  project and to paths passed explicitly; the frame store, stored frames and
  fonts, store accounts and the mapping are read-only.
- "Add to Claude Desktop" writes `mcpServers.vitrine-studio = {command:
  <abs path of vitrine>, args: ["mcp"]}` after confirmation, with a backup.
  The settings section is client-neutral ("AI assistants (MCP)"); Claude
  setup rows keep their names.

---

## 13. Cross-cutting

### Security and network boundaries

| Component | Network |
|---|---|
| `core`, `render` | none, ever |
| `publish` | App Store Connect, Google Play, gallery API |
| `app` | only through `publish` (upload, connection test, gallery) |
| `mcp` | in-app endpoint on 127.0.0.1; proxy talks to it; tools offline except upload |
| `gui` | updater (one GET of `latest.json`), allow-listed `open_url` |
| `api-server` | serves the API; calls GitHub for login |

No telemetry. Secrets only in the keychain (or environment for CI uploads),
never in projects, logs or settings. Paths into projects are validated
(no `..`, project-relative op files and asset paths).

### Licences

- Code **GPL-3.0-or-later** (`LICENSE`, set once in `[workspace.package]`);
  contributions need the licence-style CLA (`CLA.md`, contributors keep
  copyright and grant the owner a broad licence incl. relicensing; signed via
  a CLA bot – not activated yet). No SPDX headers per file. `publish = false`
  until crates.io names are settled.
- **Integrations are MIT** (`integrations/`: GitHub Action, GitLab
  components, fastlane plugin, agent skill): glue that only runs the CLI or
  talks to the MCP server.
- Name, logo and icons (`assets/brand/`) are not GPL; forks rebrand.
- Dependencies and bundled data need a GPL-3-compatible licence (allow-list
  in `deny.toml`, `cargo deny check licenses` in CI); notices via
  cargo-about. Fonts OFL (each family's `OFL.txt` ships), hyphenation
  patterns MIT/BSD/permissive (`assets/hyphenation/LICENSES.md`).
- **No vendor graphics** in the repo or packages: parametric frames only;
  vendor bezels are imported by the user (stored project copies: private
  repositories only).
- Templates and gallery content are CC0 (fonts keep the OFL).

### Store values are data

Store sizes, display types, rules, gallery gaps, locales, asset-library
mappings and fastlane names live in `crates/core/data/*.yaml`, each with a
source URL and verification date; unmeasured values are marked as such.
Never hard-coded.

### UI i18n

The GUI is English and German (JSON catalogs, every key in both). Website
and user guide are English and German. Code, comments, repo docs and commit
messages are English.

### Platforms

macOS, Windows, Linux. Platform-gated code must be lint-clean on every OS
(symmetric `cfg` branches; cross-clippy commands in `AGENTS.md`). Paths via
`Path::join`; LF line endings. DMG import only on macOS. The app is not
sandboxed (projects are arbitrary folders shared with the CLI).

### Performance notes

- Preview of a page at 400 px: ~1–4 ms warm, 5–30 ms cold; export page
  1320×2868 ≈ 80 ms; PNG encoding dominates export (Balanced chosen).
- Text layout is per scene and language, not per render; bundled fonts
  register lazily (~1 µs library build).
- Debug builds of our crates are 7–15× slower; long MCP sessions should use
  a release GUI.
- Validation with pixel checks costs ~0.3 s per target × language.

### Versioning, release, updater

- Version = `[workspace.package] version` (0.1.0) only; tag `v<version>`
  must match.
- `.github/workflows/release.yml`: tag → draft release (owner publishes);
  macOS universal `.app`/`.dmg` (Developer ID + notarization when secrets
  exist), Windows NSIS per-user setup (**unsigned for now**, owner decision
  2026-11-13; SignPath path dormant), Linux AppImage/deb/rpm; standalone CLI
  archives (incl. Linux arm64 and static musl); container image
  `ghcr.io/vitrinestudio-app/vitrine-cli` (Alpine + musl binary). The CLI
  ships inside the app as a Tauri sidecar. Version-free asset names for
  `releases/latest/download/…`. Owner checklist: `docs/dev/releasing.md`.
- Updater manifests are signed with the Tauri updater key (public key in
  `tauri.conf.json`); losing the private key strands existing installs.

### Testing

Golden images (§7), insta snapshots (YAML, resolve, layout), proptests
(geometry round trips, op/inverse pairs, edit intents), YAML round trips,
store clients against wiremock, MCP over stdio and through the proxy, upload
against fake stores, API server in-process with a fake GitHub. No real
network, keychain, app data or Claude Desktop config in tests. Test map:
`AGENTS.md`.

---

## 14. Decision index

This table replaces the ADR files (archived in `docs/archive/adr/`). Code
comments and docs that say "ADR 00NN" resolve here. Status: **current**
(holds as written), **partly** (core decision holds, details changed as
noted), **superseded** (replaced; see the later row).

| ADR | Title | Decision that holds today | Status | § |
|---|---|---|---|---|
| 0000 | Record architecture decisions | Decisions were ADR files; since 2026-11-13 they are sections of this document plus the decision log (§15) | superseded | 15 |
| 0001 | Render pipeline (Phase 0 spike) | tiny-skia raster, parley text layout (cosmic-text dropped after a measured comparison), skrifa outlines as paths, own box blur, fast_image_resize Lanczos3, png without oxipng; page geometry formulas; rotation positive = clockwise. Changed: fonts no longer bundled-only (0028); gap values partly measured | partly | 2, 7 |
| 0002 | Project file format | serde-saphyr, YAML 1.2 core schema, canonical form, flat `element_fields!`, ids, lenient references, revision + atomic save | current | 3 |
| 0003 | Text markup and layout | markup grammar, soft hyphens, emergency break last, auto-fit binary search (0.25 lu), core alignment, text effects cascade and draw order, layout report. Changed: "system fonts never loaded" (0028), italics now partly real (bundled families) | partly | 6 |
| 0004 | Resolve and canvas geometry | `core::geometry` + `core::resolve` → `ResolvedScene`; anchors, boxes, auto-height pivots, `content_scale` devices-only around `at`, device swap and free-aspect devices, language chain, screenshot lookup, group offsets/pivots, z order. Open question "per-language overrides" solved by 0009 | current | 5 |
| 0005 | Validation | one report, stable codes, levels, store rules from data, aspect 2 %, upscale 1.15, variant suggestion 15 %. Extended by 0016, 0029, 0031 | current | 8 |
| 0006 | Rasterizing resolved scenes and export | `RenderContext`/`prepare`/`rasterize`, layers and masks, alpha shadows, resample once, export names, PNG Balanced, gallery preview, incremental manifest. Changed: decoded cache LRU 1.5 GiB (scaled cache still 768 MB), manifest also lists target/lang/kind for stale cleanup, fastlane layout | partly | 7, 9 |
| 0007 | Edit intents in core | `core::edit`: gestures → ops solved against resolve (Newton), override vs structural rules, new rename/move/language ops, overlay, default-language impact. Since then: `lang` scope (0009), `locked` field exists, clipboard intents | current | 4 |
| 0008 | GUI session, commands, preview transport | command functions + dispatch, session with atomic saves and conflict reload, watcher, preview cache, binary transport (`ARPX` header, PNG Fast), dev bridge, settings + `app.sqlite`. Moved to `vitrine-app` (0013); built-ins now `core::template::builtin` (0023) | partly | 12 |
| 0009 | Language overrides, font fallbacks, hyphenation | `langs` on texts (allowed fields), resolution base → variant → language, ops, `lang` edit scope, `styles.fonts` fallbacks, own Liang hyphenation (opt-in, at `shrink_to`), `lock_page`, `rename_style`; amendment: overrides never hold `null`. Changed: system-font fallback is now the default-on `system_fonts` (0028) | partly | 4, 6 |
| 0010 | Screenshot import | plan → apply (copy only), formats and detection, `import.fastlane` mapping; amendment 2026-11-13: own exports never imported (0032) | current | 9 |
| 0011 | Translation exchange | CSV + XLIFF 1.2/2.0, markup as inline codes, plan with layout check, one transaction | current | 9 |
| 0012 | Imported device frames (Apple bezels) | user-imported, rules as data, screen detection, app-wide store, all vendor models kept, bezel selection, colour remap, parametric fallback. Changed: "never in projects" → private stored copies (0027) | partly | 7 |
| 0013 | Shared application crate, actors, view state | `vitrine-app` without Tauri; thread-local actor; `project-edited` vs `project-changed`; `ViewState`. `Actor::Claude` became `Actor::Assistant {client, name}` (0015 amendment) | current | 12 |
| 0014 | MCP server design | rmcp exact pin, one handler two hosts, `VitrineMcp::run`, compact structured results, MCP defaults to base scope, resources, instructions, `docs_get` (amendment). Changed: upload tools now exist (0018), GUI hosting real (0015) | partly | 12 |
| 0015 | GUI-hosted MCP, stdio proxy, Claude Desktop | in-app Streamable HTTP endpoint (loopback, token, Host/Origin), discovery file, transparent stdio proxy with GUI autostart, live aids, "Add to Claude Desktop"; amendment: neutral wording, `Actor::Assistant` | current | 12 |
| 0016 | Store credentials and store config | app-wide accounts, keychain secrets, `store:` mapping + `set_store`, locale defaults as data, store validation, read-only MCP; amendments: setup UX (help, app picker, package check), CI credentials from env | current | 10 |
| 0017 | Store clients | direct REST clients, verified endpoints, JWT/OAuth, typed errors, retry/cancel/observer/progress, wiremock tests. Its "implement only screenshot sets" decision was amended by 0019; the screenshot-set calls were removed (0034) | partly | 10 |
| 0018 | Upload plan and executor | plan/confirm token/execute/log, Apple per set, Google one edit, resume = new plan, MCP policy table; amendments: readiness checklist, processing model + watchdog, asset-library flow hook, CI uploads | current | 10 |
| 0019 | App Asset Library upload flow | library client, runtime group resolution via `asset-library.yaml`, content markers, `upload_flow` auto/asset_library/screenshot_sets (default `auto`). Superseded in part by 0034: the library is the only Apple flow, `upload_flow` retired | partly | 10 |
| 0020 | Native menu bar and context menu | macOS native menu from the UI's spec (`menu_set`), one owner per shortcut, no web-view context menu; amendments: menu order, help entries, website docs link | current | 12 |
| 0021 | Colour management | convert to sRGB once at decode with moxcms (not qcms), precedence cICP → ICC → sRGB, info for unusable profiles, sRGB exports, no P3 output | current | 7 |
| 0022 | Licence GPL-3.0-or-later with CLA | GPL + licence-style CLA, brand excluded, no SPDX headers, dependency/font/hyphenation review (Swedish out); amendment: integrations MIT | current | 13 |
| 0023 | Templates | `.vitrine-template` format, safe reading, extract/instantiate/insert rules, built-ins, gallery backend; amendments: sample screenshots, tag facets, sprites, JPEG samples, raised limits. Its "not done yet: GUI browse gallery" is done (2026-11-13) | current | 11 |
| 0024 | Own frame packs | `frame.yaml` + PNG/SVG, folder = id, `pack:<id>`, compiled cache, resvg rasterizer, project wins over app; pack id `imported` reserved (0027) | current | 7 |
| 0025 | API server and GitHub login | renamed `api-server`, device + web flow, own hashed sessions, ownership by GitHub id, public reads; amendments: schema version, sprites, filters, admin CLI, 413 limits. Its "GUI dialog is a separate step" is done | current | 11 |
| 0026 | Packaging, signing, updates | GitHub Actions + pinned Tauri CLI, universal macOS, NSIS per-user, AppImage/deb/rpm, CLI sidecar + install-cli, updater, licence files, version single source; amendment: arm64/musl CLI + container image. Changed: Windows ships unsigned (owner 2026-11-13, SignPath dormant); the updater public key is no longer a placeholder | partly | 13 |
| 0027 | Imported frames stored in the project | `frames/imported/<source>/` in app-store layout, project copy → app store → parametric, private repos only, no MCP tool | current | 7 |
| 0028 | System fonts, stored fonts, scripts | bundled → project → stored → installed, `system_fonts` default on, fallback chain incl. script fonts and CJK per language, `fonts/system/`, templates `fonts_required`, parley `complex-scripts`, physical RTL alignment | current | 6 |
| 0029 | Design checks | layout checks in core, pixel checks in `render::quality` via `SceneInspector`, `allow` + `checks:`, first-language scope, `scene_report`; amendment: `device_cut`, `screen_covered`, cause in messages | current | 8 |
| 0030 | Group scale | `scale` on groups (0.1–10), origin = group pivot, `at` unscaled, applied in resolve to every child length, ungroup of scaled groups refused | current | 5 |
| 0031 | `clip_to` and span boxes | `clip_to` on images → rect/ellipse shape of the canvas, hidden shapes mask, refs kept by ops; span `padding`/`radius` grow the highlight without changing layout | current | 4, 5, 6, 7 |
| 0032 | fastlane captures and exports in separate folders | captures below `fastlane/captures/`, import skips own exports, export reports foreign images and deletes them only with `--remove-foreign` | current | 9 |

### KONZEPT sections

Code comments also cite `KONZEPT §n`. The German concept is archived at
`docs/archive/KONZEPT-de.md`; its sections map as follows.

| KONZEPT | Topic | Here |
|---|---|---|
| §1 | goal, principles, non-goals | 1 |
| §2 | canvas idea, glossary | 2 |
| §3.1 | project folder | 3 |
| §3.2 | lu, home page, rotation | 2 |
| §3.3 | gaps, gallery profiles | 2 |
| §3.4 | anchors, grouping, variants | 2, 5 |
| §3.5 | elements, markup | 3, 6 |
| §3.6 | styles | 2, 6 |
| §3.7 | example `project.yaml`, `device_map` | 3, 5 |
| §4 | device catalog, frame sources, screenshot in frame | 2, 7 |
| §5 | languages, texts, screenshots, import | 2, 5, 9 |
| §6 | renderer, libraries, export, presets | 7, 9 |
| §7 | store upload | 10 |
| §8 | MCP | 12.4 |
| §9 | user interface | 12.2 |
| §10 | workspace, Operations API, stack, licence | 1, 4, 13 |
| §11 | quality and tests | 7, 13 |
| §12 | roadmap | `docs/plan/` |
| §13 | decisions, open questions | 14, `docs/plan/STATUS.md` |

### Where the concept and the code differ (code wins)

- Text layout uses parley + fontique (not cosmic-text + fontdb); colour
  management uses moxcms and was built in Phase 6 (not qcms "later"); no
  oxipng.
- The undo history is in memory only (no `.cache/history.sqlite`).
- Image elements have no per-language asset variants; use `visible_in.langs`.
- Export file names are fixed, not templated.
- The macOS "document package" registration was not done; projects are
  plain folders.
- Apple uploads use only the App Asset Library (§10, decision 0034), not
  the set API the concept described.
- `vitrine mcp` is a proxy to the GUI-hosted server, headless only as
  fallback; the tool list grew far beyond the first version.
- Frames have four sources, not three (stored project copies, §7).
- Installed system fonts are first-class and on by default (the concept had
  only bundled + project fonts).
- 3D perspective moved to phase 8 (after 1.0).

---

## 15. Recording decisions

- A new or changed decision **updates the relevant section** of this
  document in the same commit as the code, and adds a dated row to the
  decision log below. No new ADR files.
- Numbering continues from the old ADRs so references stay unique: 0033 is
  the first row of the log (this document itself), the next decision takes
  the number after the last row. Code comments may cite "decision 0034"
  like the old "ADR 0029".
- A superseded decision keeps its row; the newer row says what it replaces,
  and the §14 status column is updated if an old ADR is affected.
- Decisions taken with the owner and still to confirm are tracked in
  `docs/plan/STATUS.md`; once settled, they land here.

### Decision log

| No. | Date | Decision | § |
|---|---|---|---|
| 0033 | 2026-11-13 | ARCHITECTURE.md replaces `docs/KONZEPT.md` and the ADR files as the binding description; both archived under `docs/archive/`; future decisions are recorded here | 14, 15 |
| 0034 | 2026-11-13 | The App Asset Library is the only Apple upload flow (owner verified real uploads with it). The screenshot-set flow (`appScreenshotSets`/`appScreenshots`, MD5 checksums) and the `store.apple.upload_flow` setting are removed; old projects with `upload_flow: auto\|asset_library\|screenshot_sets` still load (key ignored, dropped on save); logged plans with `flow: screenshot_sets` still display. Replaces the `upload_flow` part of 0019 | 3, 10 |
