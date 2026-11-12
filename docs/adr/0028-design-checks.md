# 0028 – Design checks: layout and pixel findings

- Status: accepted
- Date: 2026-11-12

## Context

Building 50 gallery templates through MCP
(`docs/research/2026-11-12-mcp-test-results.md`, section 3 "P1") showed
that most visible mistakes passed `validate` and `layout_report`
silently: headlines touching tilted devices, white text on a bright spot
of a photo, UI cards covering a screen's status bar, half status bars at
a cut page edge, shapes split into two halves at a seam (pulled apart by
the store gap), square shapes in the corners the store rounds, devices
too small, backgrounds leaving a strip after a variant changed the page
height, pages that vanish into the store background. `layout_report`
compared axis-aligned boxes only (a text over a panorama counted as a full
overlap, transparent cut-outs hid real collisions) and did not know where
a device's screen is.

`core` has no renderer (ADR 0001/0006), but these questions need device
geometry (`render::frames`), glyph outlines and the actual pixels behind a
text.

## Decision

### Where the checks live

- **Layout checks in `core::validate`** (`checks::design_checks`, first
  resolved language of a pages target): `device_small` (largest device of
  a page < `Thresholds::device_small`, 50 %), `page_not_covered`
  (unrotated image / shape covering ≥ 85 % of its page and three edges,
  missing the fourth by a strip ≤ 15 %), `seam_split` (two clipped
  elements of the same kind and size centred on the two sides of a page
  border with a store gap).
- **Rendered checks in `render::quality`**, plugged into `validate`
  through the new trait `core::validate::SceneInspector`
  (`ValidateOptions::inspector`, `None` = skipped). `validate` fills in
  target, language and home page, so the findings look like all others
  (codes, levels, element ids; CLI text/json/github/gitlab/junit, GUI
  list, MCP `validate` and `render_preview` issues).
- Front ends: CLI `vitrine validate` (on; `--no-pixel-checks`), app
  command `validate` (`pixel_checks`, default true; the upload plan and
  export pre-checks pass `false`), MCP `validate` (`pixel_checks`) and
  `render_preview` (one target × language).

### How the rendered checks work

Small partial renders of the prepared scene (`quality::probe`): an element
drawn alone without shadow, glow, outline and highlights is its pixel
mask; the scene painted up to and including a text (with its own effects)
is what surrounds its glyphs. Text probes use one fixed scale (0.5 px per
lu) so the asset cache reuses resampled pictures (a 10× speed-up over
per-text scales); a full validation of a template (6 targets × 2
languages) takes 1–5 s in release, one `render_preview` view ~0.3 s.

- `low_contrast` (warn, per language): the glyph colour is the exact
  (un-premultiplied) fill, mixed with the backdrop for `opacity` < 1; the
  backdrop is a ring of max(3 lu, 0.15 em) around the glyphs. Per 1 em
  cell the 30 % quantile of the per-pixel WCAG ratio counts (catches a
  bright spot behind a word, ignores hairlines and grids); the worst cell
  is reported with its page-relative position and a direction ("lighten
  the text or darken what is behind it"). Needed: `min_contrast` 4.5,
  large text `min_contrast_large` 3.0. **Large** = ≥ 80 lu or bold ≥ 62
  lu: WCAG large is 24 px (18.66 px bold) and a store shows a 1000 lu page
  about 300 pt wide. Texts with `opacity` < 0.5 are treated as ghost
  decoration and skipped.
- `text_near_device` (per language): glyph mask vs. device mask (the
  rendered frame, so tilted devices, imported bezels and laptops count
  exactly). Glyphs behind a device → warn; glyphs crossing a frame edge →
  warn; the whole text on the device → nothing (deliberate); closer than
  `device_distance` (16 lu) → info. Elements between device and text
  (a card the text sits on) remove their area from both masks. Display
  type ≥ 3 × the large size (240 lu) is skipped (layered on purpose).
- `screen_header_cut` (info): a straight device whose screen is cut by an
  image top edge (or its clip rect) inside the top bar (14 % of a phone
  screen, 9 % of tablet / landscape screens).
- `screen_header_covered` (info): a non-device element drawn above a
  device covering ≥ 25 % of that bar.
- `seam_overlap` (info): an element reaching from its home page into
  another page and drawn **over** an element of that page by ≥ 576 lu²
  (≈ 24 × 24). Elements drawn on top of the reaching one (a text on a
  bridging tile), page backgrounds (images / shapes covering ≥ 90 % of a
  page) and members of a common group are left out.
- `element_in_corner` (info): an element (not a device) covering ≥ 3 % of
  the zone the store's corner radius cuts off, unless it fills the whole
  corner square (scrims, full-bleed pictures round like the page).
- `page_blends_with_store` (info, per target): ≥ 60 % of an image's edge
  pixels are below 1.3:1 against the gallery's light `background` or the
  new `background_dark` (galleries.yaml, **not yet measured**: App Store
  #000000, Google Play #131314).

### Intended designs: `allow` and `checks`

- Element field `allow: [codes]` (common field, variant-overridable, a
  group's list applies to its children): those findings are not reported
  for the element. Only codes in `validate::ALLOWABLE` (design findings:
  the new ones plus `text_clipped`, `text_in_gap`,
  `text_outside_safe_area`, `element_outside`, `hyphenated`,
  `variant_suggested`, `screenshot_upscale`); errors and missing content
  can never be switched off. Unknown or non-allowable entries → warn
  `unknown_allow`.
- Project section `checks:` (op `set_checks`): `device_distance`,
  `min_contrast`, `min_contrast_large`, `off: [codes]` (project-wide).

### Geometry for assistants

App command `scene_report` (`render::quality::scene_report`) and MCP
`layout_report`: per device the rotated frame corners and the screen
(unrotated box, rotated quad, corner radius, top-bar height) in page lu
with the elements covering it (share of screen / top bar); the gallery
(gap in lu and px, corner radius, `measured`), seams (gap per page border
and `next_page_offset`), and overlaps measured on drawn pixels (texts with
other elements; `seam: true` for non-text elements across a page border).
Page backgrounds are left out of overlaps.

### Seams

A shape meant to continue across a page border is **one** element with
`clip: none`: its position is relative to its home page and the next
page's origin includes the target's gap, so it stays continuous behind
the gap in every target (KONZEPT §3.3). No extra "continue on next page"
feature is needed; `seam_split` points at the two-halves workaround that
breaks. The remaining uncertainty is the store's real gap (native store
apps not measured; `measured: no` in reports).

## Consequences

- Validation of a project costs a few seconds more; `--no-pixel-checks` /
  `pixel_checks: false` keep the old speed. Export does not run them.
- Tuned on 14 of the 50 templates: findings per template dropped from
  5–200 (first version) to 4–57 per template (median ~12), most of the rest are WCAG contrast
  misses of small eyebrows and deliberate layouts the author can `allow`.
- Thresholds are code defaults plus `checks:`; new stores' dark
  backgrounds are data.

## Open questions

- The dark store backgrounds and the native apps' gap / corner radius
  still need measuring on devices.
- Contrast of text over device screenshots compares with the screenshot
  pixels; text inside the screenshot itself is not checked.
