# Vitrine Studio overview

Vitrine Studio designs App Store / Play Store screenshots: one layout, rendered
automatically for every target (store size) and language.

## Concepts
- **Project** = a folder: `project.yaml` (canvases, elements, styles, targets),
  `i18n/<lang>.yaml` (translations), `screenshots/<lang>/[<variant>/]<key>.png`,
  `assets/` (images), `fonts/`. Every tool change is saved at once and is
  one undo step (history_undo).
- **Canvas** = N equally sized pages side by side. Backgrounds run
  across all pages. A page is **1000 lu** wide; its height follows the canvas
  aspect (`1320:2868` → 2173 lu). All positions and sizes are in lu, never px.
- **Elements**: device, text, image, shape, background, group. Each has a home
  page (`page`, 0-based); `at` is the element centre relative to the top-left
  corner of that page. Elements may reach into neighbouring pages (a device
  spanning two pages exists once). Texts usually use margins
  (`box: {top, left, right}`) and are clipped to their page.
- **Gaps**: the store shows a gap between images (target gallery profile);
  content inside a gap is cut from page exports.
- **anchor_y** (top | bottom | center | stretch): which page edge an element
  keeps its distance to when a target's page height differs.
- **Targets** (from presets, catalog_targets): export size, store slot,
  gallery gaps, `group` (2 = two portrait pages form one landscape image),
  optional variant. See docs_get {topic: "targets"}.
- **Variants**: per-canvas overrides for one format (iPad landscape …).
  **Language overrides** (`langs.<lang>`) adjust a text for one language.
  See docs_get {topic: "scopes"}.
- **Texts** reference translation keys; strings live per language
  (strings_get / strings_set); missing ones fall back (fr-CA → fr → default).
- **Devices** show the screenshot `screenshots/<lang>/[<variant>/]<key>.png`
  of their `screenshot` key; a missing one is drawn as a placeholder.

## Workflow
1. project_open {dir} or project_create {dir, name, template, langs}.
2. project_summary → canvases, pages, element ids, targets, languages.
3. Change: element_add / element_update / element_move / element_resize /
   element_rotate, strings_set, styles_update, canvas_update, target_add,
   assets_import / screenshots_import, apply_ops for raw atomic changes.
4. After every visual change: render_preview and look at the image; also
   other targets and languages (view: gallery shows a whole target).
5. layout_report for exact boxes, font sizes, overflows; validate (0 errors
   before export); export.

## Units and geometry
- x grows to the right, y downward; rotation in degrees, positive = clockwise.
- Colours `#RRGGBB[AA]` or `@name` (styles.colors).
- Errors list the allowed fields and values; read them and retry.

## Layers and backgrounds
- Painting order: first the canvas `backgrounds`, then all elements sorted
  by `(z, list order)` – a `background` element is an ordinary layer in that
  order, not automatically at the bottom. An opaque image after it (same `z`)
  or with a higher `z` covers it. Give backgrounds a low `z` (e.g. -30) or
  add them with `index: 0`.
- Canvas `backgrounds` is a list of LAYERS over the whole canvas (bottom to
  top, all pages and gaps), not one fill per page: with several opaque
  colours only the last one shows (validate warns `background_hidden`).
  A colour or photo per page = one `background` element per page (it fills
  its home page and follows every target's page height); a gradient or
  panorama across all pages = one canvas background (or a wide image).
- `fill: {kind: image, fit: tile}` repeats the image at its natural size:
  1 px (or 1 SVG user unit) = 1 lu, starting at the top-left of the filled
  area. Scale the tile by exporting the image at the wanted lu size.
- Transparent PNG/SVG panoramas can sit above coloured page backgrounds;
  an opaque photo hides everything below it.

## Panoramas and seams
Page x positions depend on the target: page `n` starts at canvas x
`n·1000 + ⌊n/group⌋·gap` (gap = the target's store gap in lu,
`layout_report.gallery.gap_lu`; App Store iPhone 45 lu, Play phone 71–87
lu by size, `group: 2` has a gap only between images). Content in a gap is cut
out, so anything drawn **continuously on the canvas** continues exactly
behind every gap in every target. Never slice a motif per page and
compute gaps yourself. Three ways, simplest first:

1. **Canvas background image** – one wide picture behind everything:
   ```yaml
   canvases:
   - id: phone
     backgrounds:
     - {kind: image, asset: assets/panorama.jpg, fit: cover, focus: {x: 0.5, y: 0.6}}
   ```
   Mapped onto the whole canvas of each target (all pages + gaps, the
   target's page height). `cover` scales it to fill and crops the excess
   (`focus` picks what stays); a target with another gap or page height
   crops / scales it slightly differently, so a feature does not stay at
   the same spot of a page. Cannot be overridden per variant. Best for
   skies, gradients, textures, scenery without page-exact features.
2. **One wide image element** – when you need control (z order, size,
   per-variant override):
   ```yaml
   - id: scene
     kind: image
     page: 0
     clip: none            # images default to none; keep it so
     z: -30
     anchor_y: stretch     # height follows each target's page height
     asset: assets/scene.svg
     fit: stretch          # or cover
     at: {x: 1600, y: 1086.5}   # centre; x = width / 2 starts it at page 0's left edge
     width: 3200           # ≥ pages·1000 + gaps of the widest target (3 pages: 3000 + 2·~90)
     height: 2173          # the canvas page height
   ```
   Its width in lu is the same in every target; only the page origins
   move with the gap, so the part of the motif seen on page n shifts by
   n × (gap difference) between targets (≈ 26–42 lu per page between iPhone
   and Play). Leave 50–100 lu of calm margin around anything that must
   sit at an exact page position. A variant can override it in one go
   (`{asset, width, height, at}` – e.g. a separate Play or landscape
   picture).
3. **Bridging pieces** for exact motifs at seams (a phone lying across
   the border, a ribbon, a card): one element homed on the left page with
   `clip: none`, reaching into the next page. Its position is relative to
   its home page, so it stays continuous behind the gap whatever the gap:
   ```yaml
   - id: ribbon
     kind: shape
     shape: rect
     page: 1
     clip: none
     at: {x: 1000, y: 1500}   # centred on the border of pages 1 and 2
     width: 600
     height: 80
     fill: "@accent"
   ```
   Combine with one `background` element per page for per-page colours.

Slice per page only when pages really differ per variant (e.g. a separate
composition for iPad landscape) – then put one `background` element (or
image) per page with its own asset and override `fill` / `asset` per
variant; the slices no longer join at seams, so let the seam fall on calm
areas or cover it with a bridging piece.

Checking: `layout_report.seams[]` lists each border with a gap
(`after_page`, canvas `x`, `gap_lu`, `next_page_offset` = −(1000 + gap):
a point x on `after_page` is x + `next_page_offset` on the next page);
`render_preview {view: "gallery"}` per target. validate: `seam_split`
(two clipped halves of one shape at a border – use ONE element instead),
`seam_overlap` (a bridging piece covers something of the next page), see
docs_get {topic: "checks"}.

## Groups, variants and anchors
- A group's `at` is an OFFSET added to all children (default {x: 0, y: 0}),
  not a centre; `width`/`height` of a group are not used. Children keep
  their own page-relative `at`. Moving a group in a variant = override its
  `at` (one override instead of one per child); `rotation` turns all
  children around the centre of their bounding box (+ `pivot`). `scale`
  (0.1..10) scales all children around that same point like a transform –
  positions, sizes, font sizes (text re-laid out, sharp), strokes, radii,
  shadows, glows; `at` stays an unscaled offset. A card/ticket built as a
  group gets smaller for a format with ONE variant override
  `{scale: 0.85}` (+ `at` if it should move) instead of one per child.
- anchor_y applies after overrides: `at.y` (base AND variant override) is
  always written for the CANVAS page height (`page.aspect`, e.g. 2173 lu
  for 1320:2868) – never for the target's page height, even in an override
  used only by a Play or landscape target. For a target whose page is `Δ`
  lu higher (Δ < 0 = lower), `bottom` adds Δ, `center` Δ/2, `stretch`
  scales y (and the height of images, shapes and backgrounds) by the
  height ratio, `top` keeps y. Example: canvas 2173 lu, Play page 1500 lu
  (Δ = −673), device `anchor_y: bottom`: to place its centre at y 1300 on
  the Play page, write `at.y: 1973` (1300 + 673) in the Play override.
  Height and width values are not anchored (only `stretch` scales height).
  layout_report shows the resulting boxes in target page lu.
- Variant `content_scale` scales DEVICE elements only (size around their
  centre, and their shadows); texts, images, shapes and backgrounds keep
  their size (images with `fit: cover` in a page background only follow
  the page height).

## Devices
- catalog_devices lists per model the colour variants (`colors`, use as
  `color`) and `frame`: the parametric frame of an element 1000 lu wide per
  orientation – height, screen rect [x, y, w, h] from the element's
  top-left corner, corner radii. Multiply by width/1000; `at` is the centre.
- `model: none` draws the bare screenshot with rounded corners (radius
  10 % of the short side) – useful in an iPad set where Apple wants no
  iPhone frame. Its height follows the screenshot's aspect.
- `shadow` works on texts too (`shadow: soft` or a styles.shadows name,
  also in a text style and as a language override).

## Checking the result
- render_preview {view: "gallery"} shows the store's gaps and rounded
  corners (gallery profile, catalog_galleries); `background: "dark"` (or
  `"#1C1C1E"`) shows the store's dark mode – pages close to the store
  colour blend in. Wide galleries are rendered lower to stay within 4096 px;
  `pages: [2, 4]` (numbers or page ids) shows only the images from the
  first to the last of those pages, larger;
  use `format: "jpeg"` for small transfers and `out_file` to save a PNG
  into the project (e.g. for a README).
- export runs in the background when it takes longer than `wait_seconds`
  (default 45): poll export_status {job}. `pages: false` writes only the
  `_gallery.png` previews (fast check of whole sets).
