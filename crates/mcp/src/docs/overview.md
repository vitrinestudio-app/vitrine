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

## Groups, variants and anchors
- A group's `at` is an OFFSET added to all children (default {x: 0, y: 0}),
  not a centre; `width`/`height` of a group are not used. Children keep
  their own page-relative `at`. Moving a group in a variant = override its
  `at` (one override instead of one per child); `rotation` turns all
  children around the centre of their bounding box (+ `pivot`).
- anchor_y applies after overrides: `at.y` (base or variant override) is
  always written for the canvas' own page height; for a target whose page
  is `Δ` lu higher, `bottom` adds Δ, `center` Δ/2, `stretch` scales y (and
  the height of backgrounds) by the height ratio, `top` keeps y.
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
