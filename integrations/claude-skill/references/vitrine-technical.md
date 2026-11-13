# Vitrine Studio – technical reference for working over MCP

Distilled from building 62 gallery templates through the `vitrine-studio` MCP server
(Nov 2026, including the MCP update of 13.11.2026). The authoritative reference is always the
server itself: `docs_get` topics `overview` (incl. panorama recipe), `elements`, `ops`,
`scopes`, `project`, `markup`, `targets`, `checks` (all validate codes: level, how measured,
threshold, `allow`-able, typical fixes – coming with "Paket D") and the tool descriptions. If something here contradicts `docs_get`, `docs_get` wins.

**Is the current build running?** `apply_ops` accepts a `file` parameter. If it doesn't, the
app is an older build: ask the user to rebuild and restart it, or fall back to inline `ops`
and the workarounds marked *(old build)*.

---

## 1. The model

- **One project = one folder** (`project.yaml` + `screenshots/`, `assets/`, `fonts/`, …). The app
  has **one open project at a time**; every project-scoped tool works on it. Without one, tools
  answer `no_project`. Never edit `project.yaml` by hand while the app has it open.
- **Canvas** = one device class (`phone`, `tablet`, `tablet-land`, …) with `page.aspect`
  (e.g. `1320:2868`), `page.count`, page `ids` and canvas `backgrounds`. Pages sit side by side
  on **one continuous canvas**.
- **Units (lu):** every page is **1000 lu wide**; page height = 1000 × h/w of the aspect
  (phone 1320:2868 → 2172.7 lu). Page *n* (0-based) spans x = 1000·n … 1000·(n+1) in
  canvas coordinates; element `at` is relative to its home `page`.
- **Store gaps:** in the store gallery the pages are separated by a gap. Vitrine renders the
  gallery with the real gap and cuts it out of anything that crosses a seam. Gap widths and
  gallery corner radii per store come from `catalog_galleries` – always query, values seen in
  Nov 2026 only as orientation: gaps iPhone ≈ 45, Play phone ≈ 71, iPad ≈ 40, Play tablet
  landscape ≈ 22.5 lu; corners iPhone 125 lu, iPad ≈ 20 lu, Play = 4 % of page height
  (≈ 71 lu phone/tablet portrait, ≈ 22.5 lu landscape).
- **Vertical positions in variants:** `at.y` (base and override) always refers to the
  **canvas** page height, not the target's. If a target page is Δ lu taller, `anchor_y`
  converts: `top` keeps y, `bottom` adds Δ, `center` adds Δ/2, `stretch` scales y (and
  background heights) by the ratio of page heights. Write y for the canvas page and steer
  other formats with `anchor_y`.
- **Variant** = per-target adaptation of a canvas (different page height, device positions,
  assets). Elements hold base values; a variant holds **overrides** per element.
- **Target** = one export (store slot): canvas + optional variant + pixel size + screenshot
  folder + gallery preset + `device_map` + store metadata.
- **Languages:** project languages with fallback chains (`en` default). Texts are **string
  keys**; each language gets its value via `set_string`. Images can be limited with
  `visible_in.langs` (de covers de-AT etc.).
- **Styles:** named `colors` (referenced as `@name`), `text` styles, `spans` styles,
  `shadows`, `fonts` (fallback chains).

## 2. Elements

Common fields: `id` (unique per canvas), `kind`, `page`, `at {x,y}`, `width`, `height`, `z`,
`rotation` (degrees, around the element centre or `pivot`), `opacity`, `shadow` (name from
`styles.shadows` or inline), `clip` (`page` = cut at the page edge, `none` = may cross seams),
`anchor_y` (`top`/`bottom`/…; matters when variants change page height), `visible`,
`visible_in {langs, targets}`, `allow: [check codes]`.

| kind | Purpose | Notes |
|---|---|---|
| `background` | Page surface | `fill` = colour, gradient or `{kind: image, asset, fit: stretch/cover/tile}`. Sorts by z like everything else – an **opaque image with low z still covers it**. Use z −30 or so. `fit: tile`: 1 px / 1 SVG unit = 1 lu. |
| `image` | Assets (PNG/JPG/SVG) | `asset: assets/…`, `fit`, size. `clip_to: <shape id>` cuts the image to a rect/ellipse on the same canvas (radius, rotation, group scale respected; hidden shapes work as pure masks; the image's shadow is clipped too – give the shape the shadow). |
| `shape` | `rect` (with `radius`), `ellipse`, line | `fill`, `stroke {color, width}`, `shadow`. |
| `text` | Copy | `text: <string key>`, `style`, `font_size`, `align`, `valign`, `fit {shrink_to, max_lines}`, `stroke`, `glow`, `shadow`. Spans via markup `[name]…[/]`. |
| `device` | Screenshot in a frame | `model` (`iphone-island`, `ipad-modern`, `android-punchhole`, `android-tablet`, `macbook`, `browser`, `generic-laptop`, `none` = frameless rounded screen …), `screenshot: <key>`, `width`, `orientation: landscape`, `color`, `content_scale`. Frame and screen rects per model: `catalog_devices` (values for width 1000, scale by width/1000). |
| `group` | Children moved/rotated/scaled together | `at` = offset (not scaled), `rotation` around children centre + `pivot`, `scale` 0.1–10 (positions, sizes, font sizes, strokes, shadows, radii all scale; nested factors multiply). One `{scale, at}` override per group in a variant replaces overrides on every child. Ungroup only at scale 1. |

Text details:
- Span styles: `color, weight, italic, font, background, stroke, glow, shadow`, plus
  `padding` (number or `{x,y}`) and `radius` (capped at half the box height → pills).
  Padding **does not move text or change wrapping** – put spaces around the span or reduce x
  padding; each wrapped line gets its own box.
- Auto-fit handles most German headlines without per-language overrides. Hyphenation can be
  allowed per language.
- Bundled fonts: Inter (400/600 only, no italic – 700 is synthesised), Inter Display (700/900),
  Montserrat, Nunito, Oswald, Bebas Neue, Playfair Display, Lora, Caveat, Dancing Script,
  JetBrains Mono. Other OFL fonts: TTF + OFL.txt into the project `fonts/`; `catalog_fonts`
  shows them immediately. CJK or symbol fallback: `styles_update {fonts: {"Bangers":
  {fallback: ["Noto Sans JP"]}}}`.
- No vertical writing mode, no path/polyline element, no image tint, no backdrop blur. Vertical
  text: one character per line via `\n`; routes/vines/lines: SVG assets.

## 3. Screenshots and devices

- Folder layout: `screenshots/<lang>/<variant>/<key>.png`. A device references `<key>`;
  the target's `screenshots` field picks the variant folder. Language falls back along the chain.
- Catalogue variants and sizes used for templates:

  | Variant folder | Viewport @ scale | PNG |
  |---|---|---|
  | `ios-phone` | 440×956 @3 | 1320×2868 |
  | `android-phone` | 432×960 @2.5 | 1080×2400 |
  | `ios-tablet` | 1032×1376 @2 | 2064×2752 |
  | `android-tablet` | 800×1280 @2 | 1600×2560 |
  | `ios-tablet-land` | 1376×1032 @2 | 2752×2064 |
  | `android-tablet-land` | 1280×800 @2 | 2560×1600 |

  Own folders such as `landscape` are allowed (set `screenshots: <folder>` on the target);
  `validate` only flags folders that exist nowhere (typos).
- `device_map` on a target swaps models, e.g. `{iphone-island: android-punchhole}`,
  `{ipad-modern: android-tablet}`, `{macbook: generic-laptop}` – one design, both stores.
- iPad sets must not show iPhone frames: use `model: none` for a phone inside an iPad page;
  map it to a real Android phone on Play if wanted.
- `content_scale` in a variant scales **all** content including wide panoramas – prefer
  explicit overrides or group `scale`.

## 4. Variants and targets

Standard target set for a template (both stores, device frames):

| Target | Canvas | Variant | Size | Screenshots | device_map |
|---|---|---|---|---|---|
| `ios-iphone-69` | phone | – | 1320×2868 | ios-phone | – |
| `play-phone` | phone | `play` | 1080×1920 | android-phone | iphone-island → android-punchhole |
| `ios-ipad-13` | tablet | – | 2064×2752 | ios-tablet | – |
| `play-tablet-10` | tablet | `play-tablet` | 9:16 | android-tablet | ipad-modern → android-tablet |
| `ios-ipad-13-landscape` | tablet-land | – | 2752×2064 | ios-tablet-land | – |
| `play-tablet-10-landscape` | tablet-land | `play-land` | 2560×1600 | android-tablet-land | ipad-modern → android-tablet |

- **One target per store slot for upload.** Portrait and landscape tablet targets occupy the
  same slot (Play `tenInchScreenshots`, App Store iPad display type) and collide in the upload
  plan. A template that ships both orientations as canvases must have only one *upload* target
  per slot – decide which one, keep the other as an alternative the user switches to.
- Play pages are lower (9:16) → devices anchored at the bottom overlap headlines or leave gaps.
  Give every Play target its own variant with adjusted positions (`anchor_y`, `at`, `width`).
- **Overrides:**
  - `element_update {id, scope: "variant", variant, patch}` **merges** into the existing
    override – use this for single fields. Several patches, even across canvases, in one undo
    step: `element_update {updates: [...]}`.
  - `set_variant_override` (op) **replaces the whole override** of that element. Only for
    deliberate full rewrites. The result lists `dropped_override_fields` if fields vanished –
    always read it. `patch: null` removes the override.
  - Overrides can change position, size, rotation, assets (e.g. a Play-specific SVG), `scale`
    of groups, `clip_to`. Not in language overrides: `scale`, `clip_to` (language overrides
    are for texts).

## 5. Ops and tools

`apply_ops` is the workhorse: an ordered list of ops, **atomic** (all or nothing, one undo
step, clear error on the failing op). Edits that change nothing are not saved
(`applied: false`, revision unchanged).

```json
{"file": ".cache/ops/phone.json"}
{"file": ".cache/ops/phone.yaml", "label": "Phone: pages 1–3"}
{"ops": [...], "label": "..."}
```

- `file`: path relative to the project, `.json`/`.yaml`/`.yml`, ≤ 32 MB, content = ops array or
  `{ops, label}`. Exactly one of `ops`/`file`. The file stays; `.cache/` is not part of the
  project content and never goes into templates. Write the file with the shell / file commit,
  then call `apply_ops {file}` – this replaces inline batches of 10–50 KB.
- Common ops (exact fields: `docs_get ops`): `add_canvas`, `update_canvas`, `rename_canvas`,
  `add_variant`, `add_target`, `add_element`, `update_element`, `remove_element`
  (refused with `StillReferenced` while an image `clip_to`s it – use the `element_remove` tool,
  which resolves references), `rename_element` (updates `clip_to` references),
  `set_variant_override`, `set_string`, `set_styles`, `set_checks`.

Example ops (shape of real calls):

```json
{"op":"update_canvas","id":"phone","patch":{"count":6,"ids":["match","icebreak","date","verify","vibes","wall"],"backgrounds":["#FFFBF0"]}}
{"op":"add_target","target":{"id":"play-phone","kind":"pages","canvas":"phone","variant":"play","size":[1080,1920],"screenshots":"android-phone","gallery":"play-phone","device_map":{"iphone-island":"android-punchhole"},"store":{"google":{"image_type":"phoneScreenshots"}}}}
{"op":"add_element","canvas":"phone","element":{"id":"phone-match","kind":"device","page":0,"model":"iphone-island","screenshot":"discover","width":560,"at":{"x":680,"y":1814},"z":5,"shadow":"dev","clip":"page","rotation":5}}
{"op":"add_element","canvas":"phone","element":{"id":"eyebrow-match","kind":"text","page":0,"text":"match.eyebrow","style":"eyebrow","at":{"x":444,"y":125},"width":707,"height":49,"align":"left","valign":"center","font_size":38.9,"fit":{"shrink_to":31.1,"max_lines":1},"z":20}}
{"op":"set_string","lang":"de","key":"match.eyebrow","value":"FOLGE 1 · DAS MATCH"}
```

Tools by job:

| Job | Tools |
|---|---|
| Orientation | `docs_get`, `catalog_devices`, `catalog_fonts`, `catalog_galleries`, `catalog_targets`, `template_list` |
| Project | `project_create {dir, name, template: "minimal", langs}`, `project_open`, `project_summary` (long on big projects – prefer `element_get`, `element_list`, `layout_report {page}`) |
| Build | `apply_ops`, `element_update`, `element_remove`, `styles_update` (merges colours, text/span styles, fonts), `strings_set`, `target_add/update` |
| Check | `validate`, `layout_report` (boxes, overlaps, `devices[].covered_by[].share_of_screen`), `render_preview` |
| Output | `export`, `export_status`, `template_save`, `upload_plan` → user confirmation → `upload_execute` |
| History | `history_undo/redo/state` |

`assets_import` / `screenshots_import` work but return one line per file – for many files copy
them straight into `assets/` and `screenshots/…` with the shell; Vitrine picks them up at once.
Overwriting an asset file takes effect at the next render without any op.

**Generator pattern (recommended for anything bigger than a page):** keep a script
(`assets/src/build_ops.py`) that writes the full op list per canvas – base elements plus every
variant override – from a page plan. Rebuild = regenerate + `apply_ops {file}`. Keep the
previous output to diff when you want small, reviewable changes.

## 6. Validation

`validate` runs every target × language with pixel checks in seconds (`pixel_checks: false`
= layout only, faster); `render_preview` returns the findings for the rendered target too.
Templates ship with **0 errors, 0 warnings**. Full code reference: `docs_get {topic: "checks"}`.

| Code | Means |
|---|---|
| `text_near_device` | Text closer than `device_distance` (16 lu) to a device; text on a carrier (card, bubble, plate behind it) does not count |
| `low_contrast` | Text below `min_contrast` (4.5) / `min_contrast_large` (3), measured on rendered pixels; the text's own shadow, glow, outline and span marker count. The message names the source (own effect or `<element>` underneath) |
| `text_clipped`, `text_in_gap`, `text_outside_safe_area`, `hyphenated` | Text fit and placement |
| `screen_header_covered` | ≥ 25 % of the screen's header (status + title) covered |
| `screen_header_cut` | Device cut **inside** the header (message names the cause: page edge, export image, clip). A header entirely outside the page is not reported |
| `screen_covered` | Element covers ≥ `screen_cover` (15 %) of the screen below the header |
| `device_cut` | Device loses > 8 lu at the left/right edge (rotation-exact); suggests how far to move. Top/bottom bleed is not reported |
| `device_small` | Device too small for the page |
| `seam_overlap` / `seam_split` | Seam-crossing element covers something on the neighbour page / is split badly |
| `element_in_corner` | Visible pixels in the rounded gallery corner; corners under opaque elements ignored, full-page background images deliberately not checked |
| `page_not_covered` | Estimate: a strip of the page with nothing designed underneath; the canvas background colour does not count as cover |
| `page_blends_with_store` | A large part (~60 %) of the page edge doesn't stand out from the store background; one big white sheet alone doesn't trigger it |
| `element_outside`, `background_hidden` | Element off the canvas / page background hidden |
| `invalid_clip_to` (warn), `clip_to_other_page` (info) | Broken / cross-page clip references |
| `variant_suggested`, `screenshot_upscale`, `unknown_screenshot_variant`, `override_null` | Setup hints |

- Per element: `allow: [codes]` (on a group it applies to its children too) for intended exceptions (giant cropped type, a device peeking in
  from the side, a prop deliberately crossing a sunken device). Prefer fixing over allowing.
- Per project: `{"op": "set_checks", "checks": {"device_distance": 16, "min_contrast": 4.5,
  "min_contrast_large": 3, "screen_cover": 0.15, "off": [...]}}`.
- `layout_report` gives the measured geometry behind the checks: `devices[].screen`, `quad`,
  `header`, `covered_by[].share_of_screen`, `overlaps`, `seams` (with `next_page_offset` to map
  a canvas point onto the next page) and `gallery`.
- Keep rotated text boxes narrow; a wider box changes wrapping and can move a line onto a dark
  edge (real finding, not a false alarm).

**Still only visible by eye:**
- motifs baked into full-page images (square paper corners in a rounded store corner, panel
  edges that don't continue at the same height across a seam);
- decoration that looks like a system icon (image placeholder, warning triangle, spinner);
- image on a same-coloured surface, white on white;
- an image overlapping a headline (only `layout_report.overlaps`);
- prop scale relative to the device (a teapot 4 cm tall next to a phone);
- taste, composition, generated-image artefacts.

## 7. Preview and export

- `render_preview {target, lang, view: "gallery"}` – full store gallery with gaps and rounded
  corners; `background: "dark"` (or a colour) for the dark store; `pages: [2, 3]` or page ids
  renders a full-height excerpt (use it for wide landscape galleries); `view: "image"` for
  single pages; `out_file` writes a PNG into the project. Leave `format` out unless needed.
- `export {out_dir, pages: false, html: false, wait_seconds: 120}` writes one `_gallery.png` per
  target × language – the most reliable way to produce README previews. Long exports continue in
  the background → poll `export_status {job}`. Unchanged pages are skipped on re-export.

## 8. Templates and gallery upload

- `template_save` stores canvases, variants, styles, all languages, image assets and OFL fonts;
  screenshots become placeholders (sample screenshots can be included). Licence is CC0-1.0 →
  every asset must be CC0-compatible (own SVG/HTML, generated images on a plan granting rights).
- Gallery listing fields per template: `slug` (folder = permanent id), `name`, `style` (1–2),
  `color` (1–2), `use` (1–3) – **fixed tag ids only**, device tags are set by the CLI – and a
  plain English `description` (look, suitable apps, what's included).
- Uploads to the stores always: `upload_plan` → show the user → explicit yes → `upload_execute`.

## 9. Proven composition techniques

- **Across a seam: one element with `clip: none`** – never two halves with `clip: page`
  (twins only line up when the gap is 0). Vitrine cuts the real gap for every target.
- **Panorama (recipe from `docs_get overview`):**
  1. a picture across all pages as **canvas `backgrounds`** image fill – the canvas backgrounds
     are layers over the whole canvas (all pages and gaps); what falls into a store gap simply
     drops out on export; or
  2. **one wide element with `clip: none`** reaching from its home page into the neighbours –
     bands, waves, routes, a laptop across two pages. `seam_split` reports twin halves.
  Transparent if it sits above page backgrounds. Don't compute gaps yourself – check each
  target with `render_preview {view: "gallery"}` (Play has other gap widths; `group: 2`
  landscape sets behave differently; `layout_report.seams[].next_page_offset` maps points).
- **Per-page slices only when pages are deliberately different** (own art per page and
  variant). Then the scene must be computed in global coordinates including each target's gap,
  otherwise motifs jump at the seams.
- **Rounded inner frames** (notebook pages, passe-partouts): radius = store corner radius −
  frame inset, per target variant; check by eye or script – `element_in_corner` ignores motifs
  inside full-page images.
- **Tilted devices:** `rotation` ±3–10° with shadow; `device_cut` reports side overflow.
- **Cards, tickets, polaroids, speech bubbles:** group (shape + image + text) → one `{scale, at}`
  per Play variant; images inside panels via `clip_to`; pills/markers via span
  `padding`/`radius`.
- **Text effects:** `stroke`, hard `shadow` and `glow` on text styles; a dark glow can rescue
  contrast on busy backgrounds.
- **Props:** generate one consistent set (same light direction, angle, material), cut out,
  size them true-to-scale relative to the device width, keep them on frames and away from UI.

## 10. Working on a remote Mac (Claude cloud session + desktop bridge)

- Build files (screens, SVGs, ops JSON) in the cloud container, transfer with
  `device_commit_files` (≤ ~10 MB per file; tar bundles, unpack with `tar -xOf … > target`),
  verify with `md5sum`. Use fresh staging file names – re-using one can deliver stale content.
- Deleting can be blocked on the user's machine: move leftovers to `_unused/` and truncate with
  `: > file`.
- Fonts: `npm pack @fontsource/<name>` (+ WOFF→TTF with fontTools) instead of raw downloads.
- Fictional app screens: HTML/CSS rendered with Playwright/Chrome at the sizes in §3, per
  language; look at a contact sheet before importing.
