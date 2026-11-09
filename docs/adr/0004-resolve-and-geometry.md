# 0004 – Resolve and canvas geometry

- Status: accepted
- Date: 2026-11-09

## Context

Phase 1 task 1.4 turns a project, a target and a language into a concrete
scene that `core::layout` and the renderer can use without knowing YAML
concepts (KONZEPT §6.1). The Phase 0 spike proved the page → canvas
formulas (ADR 0001) inside `arrangia-render`; ADR 0002 left several
resolve-time questions open (group transforms, `content_scale`, defaults).
This ADR records how `core::geometry` and `core::resolve` answer them.

## Decision

### Modules and API

- `core::geometry` holds the page → canvas mapping, ported from
  `render/src/geometry.rs` without rendering dependencies: `Vec2`, `Rect`,
  `CanvasLayout` (page height, gap, grouping, `page_x`, `page_rect`,
  `image_rect`, `gap_rects`, `anchor_y`/`unanchor_y`,
  `to_canvas`/`from_canvas`) and `Viewport` (`exact`, `fit_width`,
  `cover`, `contain`, `visible`, `to_px`). The render crate switches to it
  in task 1.6; until then the spike copy stays.
- `core::resolve::resolve(project, target_id, lang, &ResolveOptions)
  -> Result<ResolvedScene, ResolveError>`. `ResolveOptions` carries the
  catalog and an `AssetLookup`: file existence plus optional image
  dimensions (`dimensions(path) -> Option<(u32, u32)>`, default `None`).
  Implementations: `FsAssets` (reads only the image header via the
  `imagesize` crate), `NoAssets`, `BTreeSet<String>` (no dimensions),
  `BTreeMap<String, (u32, u32)>` (tests). Only an unknown target, an unknown strip or an
  invalid language code are errors; everything else becomes a diagnostic.
- `ResolvedScene` is `Serialize` (not `Deserialize`): frame, canvas-wide
  backgrounds, a flat z-ordered element list in canvas lu, resolved span
  styles and diagnostics. Catalog entries (`Device`, `ColorVariant`) ride
  along for the renderer but are not serialized (looked up by name).

### Frame geometry

- Page (column) height `H = k · 1000 · h / w` from the target size and
  `group: k` (ADR 0001). The reference height `H0` is the strip's aspect.
- Export images are always `k` columns wide, also the last one when the
  page count is not a multiple of `k` (it gets empty columns; info
  `partial_image`). The canvas extends to the right edge of the last image.
- Gallery values (gap, corner radius) are in profile lu (`basis: width` =
  width of one exported image, `height` = its height) and are converted
  with `GalleryProfile::page_lu_per_unit(image px, k)`. A target `gap`
  override with an unknown preset assumes the width basis (warning
  `unknown_gallery`). No gallery = no gaps.
- **Banners** keep the strip's own page height (anchors are a no-op).
  `gaps: none` (default) → gap 0; `as_store` → the target's gallery
  converted for page images in the strip aspect (no gallery: warning
  `no_gallery`, no gaps). `region: {pages: [a, b]}` covers
  `page_x(a) … page_x(b) + 1000` (clamped, warning on overflow), `rect` is
  relative to its page. `fit: cover` (default) keeps the horizontal centre
  and the **top** (focus `(0.5, 0)`, headlines live there); `contain`
  centres the region and the canvas background extends into the margins.
- Each `OutputImage` carries its source rect, its columns and a
  `Viewport` (canvas lu → px), so export, gallery preview and GUI share
  one mapping.

### Anchors and boxes

- Positions follow ADR 0001: `canvas = (page_x(page) + x, anchor_y(y))`.
  `stretch` additionally scales explicit heights (shapes, images, free
  text boxes); devices only move (their height follows the model).
- Text boxes from margins: left/right relative to the home page (column);
  both vertical edges are y coordinates of the reference page
  (`top`, `H0 − bottom`) and follow the anchor. So a `top`-anchored box
  keeps its height and its distance to the top; a `bottom`-anchored one
  keeps its distance to the bottom. Without `bottom` the box reaches to the
  page bottom and `auto_height` is set; default `valign: top`.
- Free text boxes (no `box`): centred at `at`, `width` (default 840 lu,
  info), optional `height`; without height `auto_height` with a zero-high
  rect centred at `at`; default `valign: center`. Default `align: left`.
- **Pivot of auto-height texts** (amended in task 2.6): the default
  rotation pivot of an `auto_height` box (margin box without `bottom`,
  free box without `height`) is the centre of its laid-out content
  (`PlacedText::content_rect`), not the centre of the frame rect. The
  frame of a margin box reaches to the page bottom, so a rotated title
  used to swing around the middle of the page. `pivot` stays an offset,
  now from that default point (`ResolvedText::default_pivot`;
  `ResolvedElement::pivot` = default + offset). Fixed-height boxes keep
  the box centre. Without text, margin boxes use the centre of the top
  edge and free boxes `at`. Resolve needs layout for this, so
  `ResolveOptions::text` takes a `TextMeasure` (the render context or a
  `TextLayouter` with the project's fonts; without one, a per-thread
  layouter with the bundled fonts). The pivot follows the content, so it
  differs slightly per language; renderer, overlay and edit intents read
  it from resolve and stay consistent. Group pivots still use the frame
  centre of margin texts as reference point (see *Groups*).
- `clip: page` → the home page rect (the column when grouped).
- Page backgrounds (`kind: background` elements) fill their home page
  rect; canvas backgrounds cover the canvas rect (pages and gaps).

### Variants and `content_scale`

- The target's variant patches elements with `Element::patched` on copies;
  the project is never mutated (tested). Unknown variant, overrides for
  unknown elements and overrides that do not apply are warnings; the
  element is then used unpatched.
- `content_scale` applies to **devices only**: size, rotation pivot offset
  and shadow (blur and offset). Not to images, shapes, texts or
  backgrounds: shapes are often decorative bands across pages and texts
  get their box from margins; an image that should shrink gets an
  override.
- The device scales **around its `at` point** (its centre, already
  anchored); the position is unchanged. A device whose centre sits on the
  page bottom keeps running out of the image by the same fraction, and a
  device on a page edge stays on it. This matches the spike
  (`translate · rotate · scale`). Scaling around an edge point (e.g. the
  bottom edge for `bottom` anchors) was considered and rejected: it moves
  the centre, which `at` and the overrides refer to.

### Devices

- `device_map` swaps the model (unknown replacement: warning, original
  kept; unknown model: warning, element skipped). The width stays, the
  height follows from the new model's outer size (laptop base and stand
  included) in the effective orientation (`orientation` or the model's
  natural one; unsupported → natural, warning).
- Colour: element colour if the (mapped) model has it, `#hex` as custom
  colour, otherwise the model's default colour (info when the model was
  swapped by `device_map`, since names differ between models; warning
  otherwise).
- Frame style: element `frame` → `styles.frame` → `detailed`.
- `aspect: free` models (browser, `none`) are flagged `free_aspect`. When
  the screenshot's pixel size is known (`AssetLookup::dimensions`), the
  display takes the screenshot's aspect: it keeps its width, its height
  changes and the body grows or shrinks by the same amount (the browser
  chrome above the display stays) — `Device::with_display_aspect`. The
  adapted copy is `ResolvedDevice::device`, and the element height follows
  it. For a rotated model the natural-orientation aspect is the inverse
  (the screenshot is never rotated). Without dimensions the catalog aspect
  is used. Deciding this in resolve (not in the renderer) keeps layout,
  validation, export and GUI handles consistent.

### Languages and assets

- Language chain: the language, its shorter prefixes (`de-AT` → `de`),
  then the explicit `languages.fallback` of the most specific entry of
  that block (again with prefixes), repeated, finally the default
  language; no duplicates, cycles stop. Texts report the language they came
  from (`text_fallback` info, `text_missing` warning; `text: None`).
  Markup is not parsed here.
- Screenshots: for each language of the chain
  `screenshots/<lang>/<variant>/<key>.<ext>` then
  `screenshots/<lang>/<key>.<ext>`, extensions `png`, `jpg`, `jpeg`. The
  variant is the target's `screenshots`, else the model's default variant
  from the catalog. The result records path, language, variant and the
  level (`lang_variant`, `lang`, `fallback_variant`, `fallback`,
  `missing`) and the pixel size if the lookup knows it; missing →
  placeholder with the key (warning).
- `visible_in.langs` matches the language or a prefix of it (`de` matches
  `de-AT`); `visible_in.targets` matches the target id.

### Styles

- Colours resolve `@name` against `styles.colors`; unknown names render
  magenta `#FF00FF` (warning).
- Text style = defaults (Inter, 400, 48 lu, line height 1.2) ← named style
  ← element `color`/`font_size`. Span styles are resolved once per scene.
- Shadow presets: project presets win; built-in `soft` = the spike's two
  layers (σ 48 / y 46 / 42 %, σ 10 / y 10 / 30 %), `hard` = one layer
  (σ 4 / y 16 / 35 %), `none` = no layers.

### Groups

- Children keep their own home page, anchor and position. The group's
  `at` is an **offset** added on top; its rotation turns the children
  around the group pivot: the centre of the bounding box of all children's
  reference points (`at` on the canvas; box centre for margin texts; page
  centre for page backgrounds), visible or not, so the pivot does not
  change with language or target visibility, plus the group's `pivot`
  offset. A child keeps its own pivot offset: its pivot point is rotated
  around the group pivot and moved by the offset; rotations add, opacities
  multiply, `z` adds. Clip rects are not transformed. The group's own
  `page` only serves a group without children; its anchor is unused.

### Order and diagnostics

- Painting order is a stable sort by effective `z` (list order within
  equal z; groups flatten in place).
- Diagnostics have a level (`info`, `warn`), a stable code, the element id
  (if any) and a message; duplicates are dropped. `core::validate` (1.8)
  reuses them and adds layout-dependent checks.

## Consequences

- Renderer, export and GUI consume `ResolvedScene` + `Viewport` only; no
  anchor, gap or style logic outside `core`.
- Snapshot tests (`tests/resolve.rs`) round numbers to 4 decimals so they
  do not depend on the last bits of floating point maths.
- Property tests cover the geometry: anchor round trips, gap independence
  of home-page-relative points, grouping columns.

## Open questions

- Group pivot: bounding box of child reference points is predictable but
  ignores child sizes; revisit when the GUI shows group handles.
- ~~`aspect: free` devices: should resolve read the screenshot size to
  derive the frame height?~~ Decided (task 1.8): yes, see *Devices*.
- Banner `fit: cover` focus is fixed at `(0.5, 0)`; a `focus` field on
  banner targets may be needed.
- `content_scale` for images (logos/badges next to devices) could become
  opt-in per element if overrides prove tedious.
- Per-language element overrides (KONZEPT §5.1) are not modelled yet.
