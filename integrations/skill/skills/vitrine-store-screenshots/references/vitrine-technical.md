# Vitrine Studio over MCP – technical reference

How Vitrine Studio works when you drive it through its MCP server (`vitrine-studio`). The
authoritative reference is the server itself: `docs_get` topics `overview` (incl. the panorama
recipe), `elements`, `ops`, `scopes`, `project`, `markup`, `targets` and `checks` (every
validate code: level, how it is measured, threshold, whether it can be allowed, typical fix),
plus the tool descriptions. If something here contradicts `docs_get`, `docs_get` wins.

**App version.** `apply_ops` accepts a `file` parameter in current versions. If it doesn't,
the user's Vitrine Studio is older than this skill: ask them to update it, and meanwhile use
inline `ops` in smaller batches.

---

## 1. The model

- **One project = one folder**: `project.yaml`, `i18n/<lang>.yaml`, `screenshots/`, `assets/`,
  `fonts/`. The app has **one open project at a time**; every project-scoped tool works on it
  (without one, tools answer `no_project`). Every change is saved at once and is one undo step.
  The user may edit in the app at the same time – re-read `project_summary` when an id is
  unknown.
- **Canvas** = one device class (`phone`, `tablet`, …) with `page.aspect` (e.g. `1320:2868`),
  `page.count`, page `ids` (name the export files) and canvas `backgrounds`. Pages sit side by
  side on **one continuous canvas**.
- **Units (lu):** every page is **1000 lu wide**; page height = 1000 × h/w of the aspect
  (1320:2868 → 2173 lu). Element `at` is the element centre relative to the top-left of its
  home `page` (0-based).
- **Store gaps:** the store shows a gap between images; Vitrine renders the gallery with that
  gap and cuts it out of anything crossing a seam. Gap and corner radius come from the
  target's gallery profile – read them from `layout_report.gallery` (`gap_lu`, `radius_lu`,
  `measured: no` = estimate) or `catalog_galleries`. Current values are estimates, not yet
  measured in the store apps: App Store iPhone gap 45 lu, corner 125 lu; iPad gap 40 lu,
  corner 20 lu; Play gap and corner 4 % of the image height (≈ 71–87 lu on phone pages,
  depending on the size).
- **Vertical positions in variants:** `at.y` (base and override) always refers to the
  **canvas** page height, not the target's. If a target page is Δ lu taller, `anchor_y`
  converts: `top` keeps y, `bottom` adds Δ, `center` adds Δ/2, `stretch` scales y (and the
  height of images, shapes and backgrounds) by the ratio of page heights.
- **Variant** = per-format adaptation of a canvas. Elements hold base values; a variant holds
  **overrides** per element and an optional `content_scale`, which scales **device elements
  only** (size around their centre, and their shadows) – texts, images, shapes and
  backgrounds keep their size.
- **Target** = one export (store slot): canvas + optional variant + pixel size + screenshot
  folder + gallery profile + `device_map` + store slot (`docs_get {topic: "targets"}`).
- **Languages:** `languages.default` plus `available`, with fallback chains (fr-CA → fr →
  default). Texts reference **string keys**; each language gets its value with `strings_set`.
  Elements can be limited with `visible_in {langs, targets}` (`de` also covers `de-AT`).
- **Styles:** named `colors` (used as `@name`), `text` styles, `spans` styles, `shadows`,
  `fonts` (fallback chains per family), `frame` (default frame style).

## 2. Elements

Common fields: `id` (unique in the project), `kind`, `page`, `at {x,y}`, `width`, `height`,
`z`, `rotation` (degrees, clockwise, around the centre or `pivot`), `opacity`, `shadow` (name
from `styles.shadows` or inline), `clip` (`page` = cut at the page edge, default for text;
`none` = may cross seams), `anchor_y`, `visible`, `visible_in`, `allow: [check codes]`.
All fields: `docs_get {topic: "elements"}`.

| kind | Purpose | Notes |
|---|---|---|
| `background` | Page surface | `fill` = colour, gradient or `{kind: image, asset, fit}` (`cover`, `contain`, `stretch`, `tile`). Sorts by z like everything else – an **opaque image with a higher z (or later in the list) covers it**; give it z −30 or so. `fit: tile`: 1 px / 1 SVG unit = 1 lu. |
| `image` | Assets (PNG/JPEG/SVG) | `asset: assets/…`, `fit`, size. `clip_to: <shape id>` cuts the image to a rect/ellipse on the same canvas (hidden shapes work as pure masks; give the shape the shadow). |
| `shape` | `rect` (with `radius`), `ellipse`, `line` | `fill`, `stroke`, `shadow`. |
| `text` | Copy | `text: <string key>`, `style`, `box {top,left,right,bottom}` margins or `at` + `width` (+ `height`), `align`, `valign`, `fit {shrink_to, max_lines}`, `font_size`, `stroke`, `glow`, `shadow`, `hyphenate`. Spans via markup `[name]…[/]`. |
| `device` | Screenshot in a frame | `model` (`catalog_devices`: `iphone-island`, `ipad-modern`, `android-punchhole`, `android-tablet`, `macbook`, `browser`, `generic-laptop`, `none` = frameless rounded screen, …), `screenshot: <key>`, `width` (height follows), `orientation`, `color`, `frame`. Frame and screen rects per model for width 1000: `catalog_devices`. |
| `group` | Children moved/rotated/scaled together | `at` = offset (not scaled), `rotation` around the children's centre (+ `pivot`), `scale` 0.1–10 (positions, sizes, font sizes, strokes, shadows, radii all scale). One `{scale, at}` override per group in a variant replaces overrides on every child. |

Text details:
- Markup in translations: `**bold**`, `_italic_`, `[span]text[/]`, `\n` hard break, `\-` soft
  hyphen (`docs_get {topic: "markup"}`).
- Span styles: `color, weight, italic, font, background, stroke, glow, shadow`, plus `padding`
  (number or `{x,y}`) and `radius` (≥ half the box height → pills). Padding grows the box, it
  **does not move text or change wrapping** – put spaces around the span or reduce x padding;
  each wrapped line gets its own box.
- Auto-fit (`fit.shrink_to`) handles most long headlines; automatic hyphenation can be turned
  on per text style or element (`hyphenate`).
- No vertical writing mode, no path/polyline element, no image tint, no backdrop blur.
  Routes, vines and curved lines: SVG assets.

## 3. Fonts

- **Bundled** (always available, no files needed): Inter, Inter Display, Montserrat, Nunito,
  Oswald (sans serif); Playfair Display, Lora (serif); Bebas Neue (caps-only display);
  Caveat, Dancing Script (handwriting, short texts only); JetBrains Mono. `catalog_fonts` →
  `bundled_info` gives each family's weight range and whether a real italic exists.
- **Inter** ships only Regular (400) and SemiBold (600), no italic: weight 700 is drawn with
  SemiBold, 800–900 are emboldened synthetically, italic is slanted synthetically. For bold
  display headlines use **Inter Display** (real Bold 700 and Black 900) or a variable family
  (Montserrat, Nunito, Oswald, Playfair Display, Lora …: any weight in their range).
- **Script fallbacks are automatic:** Noto Sans Arabic, Hebrew, Devanagari, Bengali, Tamil and
  Thai are bundled and used for those scripts. Chinese, Japanese and Korean have no bundled
  font: they use an installed system font, or a font in `fonts/` set as fallback
  (`styles_update {fonts: {"Inter": {fallback: ["Noto Sans JP"]}}}`).
- **Installed system fonts** can be named directly (`catalog_fonts` → `installed`, when the app
  setting allows it), but other computers and CI may not have them (validate
  `system_font_used`). The user can store the used system fonts in the project – in the app
  (Project settings → "Store used system fonts in the project") or with the CLI
  `vitrine fonts embed`; there is no MCP tool for this.
- **Own fonts** (TTF/OTF) go into the project's `fonts/` (`assets_import {kind: "font"}`) and
  are usable by family name at once. Brand or commercial fonts are the user's responsibility:
  make sure their licence allows this use (and sharing, if the project or a template is
  shared).
- Prefer bundled families unless the user asks; max two families per set.

## 4. Screenshots and devices

- Folder layout: `screenshots/<lang>/<variant>/<key>.png` (the variant folder is optional).
  A device element references `<key>`; the target's `screenshots` field picks the variant
  folder; the language falls back along the chain (validate info `screenshot_fallback`).
- Catalog screenshot variants include `ios-phone`, `android-phone`, `ios-tablet`,
  `android-tablet`, `mac`, `web`, `watch` (`catalog_devices`, `catalog_targets`). Own folders
  such as `landscape` are allowed (`screenshots: landscape` on the target); validate only
  flags names with no such folder (`unknown_screenshot_variant`).
- **Import:** `screenshots_import {dir, format?: auto|vitrine|fastlane|flat, mapping?:
  {devices, locales, keys}, dry_run, save_mapping}` maps fastlane device names → variant,
  locale folders (`de-DE`) → project language, screen names (`01_home`) → key.
  `save_mapping: true` stores the mapping in `project.yaml` for the next import. Single files:
  `assets_import {files: [{path, kind: "screenshot", key, lang, variant}]}`.
  `assets_list` shows the files, which device elements use each key and the screenshot
  variants the targets need.
- **Samples** from a template are listed in `screenshots/samples.yaml` (validate info
  `sample_screenshot`). Export works; the upload plan blocks every set that still uses one.
  Replacing the file un-marks it.
- A missing screenshot is drawn as a placeholder; validate reports `screenshot_missing`
  (error, a warning with `validate {preview: true}` while editing). Other useful findings:
  `screenshot_aspect` (wrong device or folder), `screenshot_upscale` (may look blurry).
- `device_map` on a target swaps models, e.g. `{iphone-island: android-punchhole}`,
  `{ipad-modern: android-tablet}`, `{macbook: generic-laptop}` – one design, both stores.
- iPad sets must not show iPhone frames: use `model: none` for a phone inside an iPad page.

## 5. Targets and variants

Typical target set for both stores (presets from `catalog_targets`; add with
`target_add {preset, canvas}`; `requirement` says which sets a store requires):

| Store slot | Preset | Size (portrait) | Screenshots | Notes |
|---|---|---|---|---|
| App Store iPhone 6.3″ | `ios-iphone-63` | 1206×2622 | ios-phone | marked required |
| App Store iPhone 6.9″ | `ios-iphone-69` | 1320×2868 | ios-phone | optional, the largest |
| App Store iPad 13″ | `ios-ipad-13` | 2064×2752 | ios-tablet | required if the app runs on iPad |
| Play phone | `play-phone` | 1080×1920 (9:16) | android-phone | `device_map` → Android frames, own variant |
| Play 10″ tablet | `play-tablet-10` | 1440×2560 | android-tablet | optional |
| Play feature graphic | `play-feature-graphic` | 1024×500 | – | required; `banner_add` |

- **One upload target per store slot.** Portrait and landscape targets of the same device
  share a slot and collide in the upload plan (readiness `store_slot_conflict`, validate
  `store_slot_duplicate`). Keep the other as a non-upload alternative.
- iPad landscape from portrait pages: `group: 2` (two pages form one image) plus a variant.
- Play pages are lower (9:16) → devices anchored at the bottom overlap headlines or leave gaps.
  Give the Play target its own variant (`canvas_variant`) with adjusted `at`, `width`,
  `anchor_y`; validate suggests one when page heights differ > 15 % (`variant_suggested`).
- **Overrides:**
  - `element_update {id, scope: "variant", variant, patch}` **merges** into the existing
    override – use it for single fields. Several patches, also across canvases, in one undo
    step: `element_update {updates: [...]}`.
  - The op `set_variant_override` **replaces the whole override**; the result lists lost fields
    in `dropped_override_fields` – always read it. `element_reset_override` removes overrides.
  - Language overrides (`scope: "lang"`) are for texts: `at, width, height, anchor_y, style,
    box, align, valign, fit, color, font_size, stroke, glow, shadow, hyphenate`.

## 6. Ops and tools

`apply_ops` is the power tool: an ordered list of ops, **atomic** (all or nothing, one undo
step, a clear error naming the failing op). Edits that change nothing are not saved.

```json
{"file": ".cache/ops/phone.json", "label": "Phone: pages 1–3"}
{"ops": [...], "label": "..."}
```

- `file`: a path **relative to the project folder** (absolute paths, `..` and symlinks out are
  refused), `.json`/`.yaml`/`.yml`, ≤ 32 MB, content = the ops array or `{ops, label}`.
  Exactly one of `ops`/`file`. The file stays; keep it under `.cache/` (not project content)
  and diff the next version against it.
- **No file access?** If you cannot write files into the project folder (e.g. a chat client
  without a filesystem tool), pass inline `ops` in smaller batches – one page or one group of
  related elements per call – and prefer the specific tools (`element_add`,
  `element_update`, `strings_set`, `styles_update`).
- Common ops (exact fields: `docs_get {topic: "ops"}`): `add_canvas`, `update_canvas`,
  `add_variant`, `add_target`, `update_target`, `add_element`, `update_element`,
  `remove_element` (refused while an image `clip_to`s it – the `element_remove` tool resolves
  that), `rename_element` (overrides and `clip_to` follow), `set_variant_override`,
  `set_lang_override`, `set_string`, `set_styles`, `set_languages`, `set_checks`.

Example ops:

```json
{"op":"update_canvas","id":"phone","patch":{"count":6,"ids":["hero","plan","share","sync","stats","wall"],"backgrounds":["#FFFBF0"]}}
{"op":"add_element","canvas":"phone","element":{"id":"phone-hero","kind":"device","page":0,"model":"iphone-island","screenshot":"home","width":560,"at":{"x":680,"y":1814},"z":5,"shadow":"soft","anchor_y":"bottom","rotation":5}}
{"op":"add_element","canvas":"phone","element":{"id":"title-hero","kind":"text","page":0,"text":"hero.title","style":"headline","box":{"top":140,"left":80,"right":80},"fit":{"shrink_to":64,"max_lines":2},"z":20}}
{"op":"set_string","lang":"de","key":"hero.title","value":"Deine Woche **auf einen Blick**"}
```

Tools by job:

| Job | Tools |
|---|---|
| Orientation | `docs_get`, `catalog_devices`, `catalog_fonts`, `catalog_galleries`, `catalog_targets`, `template_list`, `view_get` / `selection_get` ("this page", "the selected text") |
| Project | `project_open`, `project_create`, `project_summary`, `template_insert`, `language_add` / `language_remove` |
| Files | `assets_list`, `assets_import`, `screenshots_import`, `frames_list` |
| Build | `apply_ops`, `element_add` / `element_update` / `element_move` / `element_resize` / `element_rotate` / `element_remove`, `canvas_add` / `canvas_duplicate` / `canvas_update` / `canvas_variant`, `target_add` / `target_update`, `banner_add`, `styles_update` |
| Texts | `strings_get`, `strings_set`, `strings_missing`, `strings_export`, `strings_import` |
| Check | `render_preview`, `layout_report`, `validate`, `element_get`, `element_list` |
| Output | `export`, `export_status`, `template_save` |
| Upload | `store_config_get`, `upload_plan`, `upload_execute`, `upload_status`, `upload_log` |
| History | `history_state`, `history_undo`, `history_redo` (your steps start with your client's name) |

## 7. Validation

`validate {targets?, langs?, preview?, pixel_checks?}` runs every target × language; the pixel
("design") checks render small partial images (`pixel_checks: false` = layout only, faster).
Fix every error before exporting or uploading (export reports the count).
`render_preview` returns only the errors and warnings of its view; most design findings
are **info** – call `validate` to see them. Every code with its threshold and fix: `docs_get {topic: "checks"}`.

| Code | Means |
|---|---|
| `low_contrast` (warn) | Text below `min_contrast` 4.5:1 (large text `min_contrast_large` 3:1), measured on rendered pixels; the text's own shadow, glow, outline and span marker count as backdrop. The message names the source. |
| `text_near_device` | Glyphs touching, crossing or hidden by a device frame (warn) or closer than `device_distance` 16 lu (info). Text on a carrier (card, bubble) between device and text is fine. |
| `text_overflow` (error), `text_clipped`, `text_in_gap`, `text_outside_safe_area`, `emergency_break`, `hyphenated` | Text fit and placement |
| `screen_header_covered` | An element covers ≥ 25 % of a screen's top bar |
| `screen_covered` | An element covers ≥ `screen_cover` (15 %) of the visible screen below the top bar |
| `screen_header_cut` | A page edge or clip cuts the device inside its top bar |
| `device_cut` | Device loses > 8 lu at the left/right edge (rotation-exact); suggests how far to move |
| `device_small` | Largest device on a page < 50 % of the page |
| `seam_overlap` / `seam_split` | A seam-crossing element covers something on the next page / twin halves at a seam |
| `element_in_corner` | Visible pixels in the store's rounded corner |
| `page_not_covered` | A strip of the page where only the canvas colour shows |
| `page_blends_with_store` | ≥ 60 % of a page's edge doesn't stand out from the light or dark store background |
| `variant_suggested`, `screenshot_upscale`, `unknown_screenshot_variant`, `sample_screenshot` | Setup hints |

- **Top bar** = the top 14 % of the screen height, or 9 % when the screen is wide (width >
  0.8 × height: landscape phones and tablets). Portrait iPads (3:4) use 14 %.
  `layout_report` gives it as `screen.header`.
- **Which language:** text checks run in every language. Device, page, corner, seam (non-text)
  and store checks – `screen_header_covered`, `screen_covered`, `screen_header_cut`,
  `device_cut`, `device_small`, `element_in_corner`, `page_not_covered`, `seam_split`,
  `page_blends_with_store`, `variant_suggested` – run only on the **first language of each
  target** (the first of the target's `langs`, default the order of `languages.available`).
  A longer German headline over a screen is therefore not caught when English comes first:
  run `validate {langs: ["de"]}` (it then runs on `de`), put the longest language first in the
  target's `langs` when that matters, and look at every language's gallery.
- **Allowing:** `allow: [code]` on an element silences findings *whose element is that
  element* (for `screen_*covered` the covering element; on a group for all children; in a
  variant override only for that variant's targets). `page_blends_with_store` and
  `variant_suggested` name no element, so `allow` can't silence them – only the project's
  `checks.off`. Prefer fixing over allowing.
- **Thresholds** (project-wide, `set_checks` replaces the whole section): `device_distance`
  16, `min_contrast` 4.5, `min_contrast_large` 3, `screen_cover` 0.15, `off: [codes]`.
  Everything else is fixed.
- `layout_report {target?, lang?, page?}` gives the geometry behind the checks: element boxes,
  text font sizes and overflow, per device `screen` (with `quad` and `header`) and
  `screen_covered_by[]` (`share_of_screen`, `share_of_header`), `overlaps`, `seams`
  (`next_page_offset` maps a canvas point onto the next page) and `gallery`.

**Only visible by eye:**
- motifs baked into full-page images (square corners in a rounded store corner, edges that
  don't continue at the same height across a seam);
- decoration that looks like a system icon (image placeholder, warning triangle, spinner);
- an image on a same-coloured surface, white on white;
- an image overlapping a headline (only `layout_report.overlaps` shows it);
- props over UI below the `screen_cover` threshold, prop scale relative to the device;
- taste, composition, generated-image artefacts, and anything in languages other than the
  first (see above).

## 8. Preview, export and upload

- `render_preview {target?, lang?, view?, image?, page?, variant?, max_edge?, pages?,
  background?, format?, out_file?, inline?}`: `view: "image"` (default, one output image as
  exported), `"canvas"` (whole canvas with hatched gaps) or `"gallery"` (all images with store
  gaps and corners). `background: "dark"` (or a colour) for the store's dark mode;
  `pages: [2, 3]` (numbers or page ids) renders only that run of images, larger;
  `format: "jpeg"` for smaller transfers; `out_file` writes the image into the project.
  Defaults: the target and language the user is looking at in the app, else the first target
  and the default language.
- `export {targets?, langs?, out_dir?, force?, gallery?, layout?, html?, pages?,
  wait_seconds?}` writes `<project>/export/<target>/<lang>/…` plus `_gallery.png` previews and
  an `index.html` overview. Incremental: unchanged files are skipped. `layout: "fastlane"`
  writes `export/fastlane/` folders for fastlane deliver/supply. `pages: false` writes only the
  gallery previews. After `wait_seconds` (default 45, max 600) the export continues in the
  background: poll `export_status {job}`.
- **Store upload** (outward-facing – it changes the public store listing):
  1. Setup is done by the user in the app: store accounts and keys in App settings → Stores,
     the project's mapping (app, package name, locales) in Project settings → Stores, store
     slots in the targets. `store_config_get` shows both (never key material); there is no
     tool to change them.
  2. `upload_plan {store?, targets?, langs?, remote?, force?}` is a dry run: sets to upload,
     unchanged and skipped sets, images, `problems`, a `readiness` checklist (each step with
     the place to fix it), `blocked`, and a `confirm_token` (valid 30 minutes, only for the
     unchanged project). `remote: true` also reads the store (network).
  3. Show the plan to the user. Only after their explicit yes: `upload_execute
     {confirm_token}`. The app then shows a "Confirm upload" dialog the user must click (the
     call waits up to 5 minutes, else `not_confirmed`). Without the app (headless) uploads
     are disabled unless the user turned on the setting for it.
  4. `upload_status` until the run ends. `uploaded_processing` is fine (Apple still
     processing; a later `upload_plan {remote: true}` confirms it). "No editable App Store
     version": the user creates a new version in App Store Connect first. Never retry a
     declined upload on your own. `upload_log` shows past runs.

## 9. Proven composition techniques

- **Across a seam: one element with `clip: none`** – never two halves with `clip: page`
  (twins only line up when the gap is 0). Vitrine cuts the real gap for every target.
- **Panorama** (recipe in `docs_get {topic: "overview"}`):
  1. one picture across all pages as a canvas `backgrounds` image fill – layers over the whole
     canvas (all pages and gaps); what falls into a store gap drops out on export; or
  2. one wide element with `clip: none` reaching from its home page into the neighbours –
     bands, waves, routes, a laptop across two pages (`anchor_y: stretch` keeps it at page
     height in every target).
  Don't compute gaps yourself – check each target with `render_preview {view: "gallery"}`
  (Play has other gap widths; `group: 2` sets behave differently).
- **Per-page slices only when pages are deliberately different** (own art per page and
  variant); then let seams fall on calm areas or cover them with a bridging piece.
- **Rounded inner frames** (notebook pages, passe-partouts): radius = store corner radius −
  frame inset, per target variant; check by eye – `element_in_corner` doesn't see motifs inside
  full-page images.
- **Tilted devices:** `rotation` ±3–10° with shadow; `device_cut` reports side overflow.
- **Cards, tickets, speech bubbles:** group (shape + image + text) → one `{scale, at}` override
  per Play variant; images inside panels via `clip_to`; pills/markers via span
  `padding`/`radius`.
- **Text effects:** `stroke`, hard `shadow` and `glow` on text styles; a dark glow can rescue
  contrast on busy backgrounds (a glow in the text colour makes it worse).
- **Generator script for big sets:** if you can run code and write files, keep a script that
  writes the full op list per canvas – base elements plus every variant override – from a page
  plan into `.cache/ops/`. Rebuild = regenerate + `apply_ops {file}`.
