# 0031 – `clip_to` for images, padded span highlights

- Status: accepted
- Date: 2026-11-13

## Context

The second MCP template round (`docs/research/2026-11-13-mcp-test-results2.md`)
asked twice for the same two things:

- **Images cut to a panel or card.** Faces in comic panels, slides in a
  frame, figures at a panel edge were built as an image *under* a second
  SVG overlay with a hole (one overlay per page and per Play height, 36 in
  one template). Wish: `clip_to: <element>`.
- **Pills and highlights in text.** A coloured box behind a word (`NEW`,
  a tag, a marker stroke) needed an extra shape plus a separately measured
  text element per language. Wish: span `padding` / `radius`. A separate
  speech-bubble element was declined by the owner.

## Decision

### `clip_to` (image elements only)

```yaml
- {id: panel, kind: shape, page: 1, at: {x: 500, y: 900}, width: 820, height: 1100,
   shape: rect, radius: 60, fill: "#FFE14D"}
- {id: face, kind: image, page: 1, at: {x: 520, y: 880}, width: 900,
   asset: assets/face.png, clip_to: panel}
# mask only: the shape is not drawn
- {id: hole, kind: shape, page: 2, at: {x: 500, y: 700}, width: 600, height: 600,
   shape: ellipse, visible: false}
```

- `clip_to: <element id>` is a field of **image** elements (variant
  overridable, not per language). The target must be a **shape** of the
  same canvas with `shape: rect` (with its `radius`) or `ellipse`. No
  polygons or paths (owner scope); `line` and other kinds are invalid.
- The mask is the shape's **resolved geometry** for the target: variant
  overrides, anchors, `content_scale`-independent sizes, group offsets and
  rotation, its own rotation and pivot – exactly where the shape would be
  drawn. Fill, stroke, opacity and shadow of the shape do not matter.
- The shape is **drawn as usual**. To use it only as a mask, hide it
  (`visible: false`, or a `visible_in` that excludes the target): hidden
  shapes still act as masks. This needs no new "mask" flag and keeps the
  visible-or-not choice per variant.
- The image's page clip (`clip: page`) still applies (intersection). The
  image's own shadow is clipped too: a shadow belongs on the shape.
- Since only images clip and only shapes are targets, cycles cannot occur.

Resolve: when a canvas uses `clip_to` anywhere (element or override),
hidden shapes (and hidden groups' shapes) are resolved quietly as well and
dropped after the images got their `ResolvedImage::clip_shape`
(`ResolvedClipShape`: element, page, shape, radius, centre, size,
rotation, pivot). Render: the outline is filled into a `tiny_skia::Mask`
in canvas pixels (intersected with the page clip) and the image layer is
drawn through it.

Validation (resolve diagnostics → findings):

- `invalid_clip_to` (warning): unknown id, not on this canvas, not a
  rect/ellipse shape – the image is drawn unclipped.
- `clip_to_other_page` (info, `allow`-able): the shape's home page differs
  from the image's. It works, but moving pages separates the two.

Operations keep references consistent:

- `rename_element` renames every `clip_to` (base elements and variant
  overrides); refused if a dangling `clip_to` already names the new id
  (undo could not tell them apart).
- `remove_element` is **refused** (`StillReferenced`) while an image
  outside the removed subtree is clipped to it – the same rule as removing
  a target still named by `visible_in`. The editor's delete
  (`Editor::remove`, GUI delete, MCP `element_remove`) first emits
  `Project::clip_to_release_ops` (base: `clip_to: null`; override: field
  reset) in the same transaction, so one undo restores everything.
  `canvas_remove` releases all references of the canvas first.
- Duplicate, paste and `copy_canvas` remap `clip_to` to the copied shape
  when the shape is copied along; otherwise the copy keeps the original
  reference.
- Adding or updating an element does not check the reference (an image
  may be added before its shape in one transaction, a paste may contain
  both in any order); validation reports leftovers.

### Span `padding` and `radius`

```yaml
styles:
  spans:
    tag: {color: "#FFFFFF", weight: 800, background: "@accent",
          padding: {x: 14, y: 2}, radius: 40}
    mark: {background: "#FFE14D99", padding: 6}
```

- `padding`: one number (all sides) or `{x, y}`, lu; `radius`: lu,
  clamped to half the box height when drawn (a large value = pill).
  Both apply to the existing span `background` box only.
- **Padding does not change the line layout.** The box grows around the
  glyph run box; glyphs, line breaks, auto-fit and `layout_report` stay as
  without padding. This matches how inline backgrounds are usually done
  for vertical padding and keeps one layout per text for all span
  styles; horizontal room comes from spaces in the text (documented).
  Making horizontal padding take space would need inline boxes in the
  parley layout and would change line breaking per span style.
- A span wrapping over lines gets **one box per line fragment** (runs of
  one fragment with the same paint still merge into one box), each padded
  and rounded on all four corners.
- The padded boxes are part of the text's drawn bounds (preview culling,
  layers). Text checks (overflow, clipping, gaps) keep using the glyph
  layout.

## Consequences

- Comic panels, framed slides, round avatars and shaped photo crops are
  one image plus one shape, per variant without extra SVGs.
- Pills and markers are one span style; translations keep working.
- Design checks still look at the image rectangle, not the clipped area
  (e.g. `screen_header_covered`, `seam_overlap` for clipped images) – a
  possible refinement.
- The GUI offers a simple shape picker for `clip_to` in the image
  inspector and X/Y padding + radius for span styles in Project settings;
  no on-canvas mask editing.
