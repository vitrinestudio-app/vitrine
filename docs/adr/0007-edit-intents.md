# 0007 – Edit intents in core

- Status: accepted
- Date: 2026-11-09

## Context

Phase 2 (task 2.1) adds a GUI editor; Phase 4 adds MCP tools that edit
the canvas. Both start from gestures in canvas coordinates ("drop the
phone here", "drag this handle", "rotate to 15°") while the model stores
home pages, anchor-relative positions, margins, group offsets and
variant overrides (KONZEPT §3.2–3.5, ADR 0004). Translating one into the
other needs the whole geometry of `core::resolve`. Doing it in JavaScript
(GUI) and again in the MCP server would duplicate it, drift from the
renderer and break the rule that only the Operations API changes the
model. Some changes also had no op yet (renames, regrouping, resetting a
single override field, languages).

## Decision

### `core::edit`

- `Editor::new(project, &EditContext, ResolveOptions)`;
  `EditContext { target, lang, variant }` is serde-friendly. `variant`
  missing = the target's own variant, `null` = the base elements (the
  target is then shown without its variant), a name = that variant.
- Intents return `Vec<Op>` = **one undo step**, never apply anything:
  `move_to`/`move_by` (`MoveOptions { keep_page }`), `resize(id,
  ResizeSpec { width, height }, fixed: Handle)`, `rotate_to`,
  `set_pivot`, `set_home_page`, `reorder_z`, `duplicate`, `remove`,
  `group`/`ungroup`, `reset_override`; plus `reference_point`,
  `overlay`, free functions `unique_id`, `reset_override`,
  `move_point`, `local_to_canvas` and `default_language_impact`.
- Errors are `EditError` (unknown element/variant, element of another
  strip, not drawn, unsupported, unsolvable, op/resolve errors).

### Mapping rules

- **Solve against resolve.** A position is found by applying candidate
  ops to a scratch copy, resolving the target and reading the element
  back; a Newton iteration (finite-difference Jacobian, ≤ 25 steps,
  residual < 1e-9 lu) adjusts the position parameters. Parameters are
  `at` for devices, images, shapes, free texts and groups (offset), and a
  common shift of the margins for margin text boxes. Anchors, gaps,
  grouping (columns), `content_scale`, group offsets and rotations and
  pivots are honoured because resolve applies them; there is no second
  implementation. The map is affine except inside rotated groups (their
  pivot follows the children), where Newton converges in a few steps; a
  singular case (child of a group rotated by 180°) is `Unsolvable`.
  Results are rounded to 1e-9 lu to keep the YAML free of float noise.
- **Reference point** (`move_point`): the centre of the drawn, rotated
  box (`center + pivot − R·pivot`); for margin text boxes without
  `bottom` (they reach to the page bottom) the centre of the top edge;
  for groups the centre of the axis-aligned bounds of their drawn
  descendants. Hidden elements (`visible`, `visible_in`, variant
  overrides) are measured as if visible.
- **Home page** follows the reference point: the page (column) whose
  rect contains its x, else the nearest one (gaps, outside the canvas).
  `keep_page` locks it. Groups keep their page (it only matters for an
  empty group); page backgrounds only change their page.
- **Resize** keeps a handle of the unrotated box fixed (usually the one
  opposite the dragged handle). Sizes are canvas lu as seen in the
  target: device width is divided by `content_scale`, heights of
  `stretch` elements by the stretch factor. Devices take a width only
  (aspect locked); images, shapes and free texts width and height; margin
  text boxes move their margins (the fixed side keeps its margin; a box
  without `bottom` gets one only when a height is given). Groups and page
  backgrounds are not resizable.
- **Rotation** is the canvas angle (enclosing groups included), stored
  normalised to `(-180, 180]`. `set_pivot` takes a canvas point and can
  keep the drawn position (otherwise a rotated element jumps).
- **Painting order** (`reorder_z`) changes the list order among siblings;
  `z` stays an inspector field.
- **Group/ungroup:** a new group has no offset or rotation, so nothing
  moves; members must be siblings and keep their order. Ungroup folds the
  group's offset and rotation into the children's base (solved for the
  target without variant), multiplies opacity and adds `z`. Overrides of
  the group itself are dropped (undo restores them).

### Override rules

- With an edit variant, element **fields** go into the variant override,
  merged into the existing one (`ElementPatch::merged`): position, page,
  size, rotation, pivot — every field of `ElementPatch` may be
  overridden; the base stays untouched.
- **Structural** changes always change the base: painting order,
  grouping, duplicate, remove, renames. They are not per format.
  `duplicate` copies the overrides of the element (and its children)
  under the new ids and offsets their position overrides too.
- `reset_override(field)` → `Op::ResetOverrideField`; without a field the
  whole override is removed. The overlay lists overridden fields per
  element so the GUI can mark and reset them.

### Ids

- `unique_id(project, base)` returns `base` if unused, else `base-2`,
  `base-3`, …; a trailing `-N` is replaced (`phone-2` → `phone-3`).
  Duplicating a group renames every child the same way. Groups default to
  `group`, `group-2`, ….

### New ops

`RenameElement` (overrides follow), `RenameStrip` (targets follow),
`RenameTarget` (`visible_in` in elements and overrides follows),
`RenameVariant` (targets follow), `MoveElementTo` (into/out of a group,
reorder; no position compensation), `ResetOverrideField`,
`SetDefaultLanguage`, `AddLanguage` (optionally restoring translations),
`RemoveLanguage` (refuses the default language and languages named by
`visible_in` or target `langs`; drops its translations, which the
inverse carries; `languages.fallback` entries stay since they may name
languages outside `available`), `RenameStringKey` (all languages, text
elements and overrides; refuses an existing target key). Saving deletes
`i18n/<lang>.yaml` files of languages without strings, otherwise a
removed language would come back on the next load.

### Overlay

`Editor::overlay(&mut TextLayouter) -> Overlay` (serde): canvas, page
(column) rects with ids, gap rects, output image rects, and per drawn
element id, kind, enclosing groups, home page, z, rotated quad, unrotated
rect, rotation, pivot point, move point, 8 resize handles, aspect lock,
overridden fields, clip, `in_gap` and for texts the laid-out content
rect/quad, line count, fitted font size and overflow (`core::layout`, so
lines match the render). Auto-height text boxes use the layout box as
their drawn box. Groups get members, bounds and move point.

### Default language impact (§5.0)

`default_language_impact(project, new_default, catalog, assets)` lists
texts (every key used by a text element or override, in every available
language) and screenshots (every device screenshot of every target in
its languages, with the target's screenshot variant or the mapped
model's default) that resolve today but would have no source with the
new default.

## Consequences

- GUI and MCP send gestures, not field values; the result is always what
  resolve/render show. Intents cost a few resolves (each is cheap; no
  layout) and a project clone per evaluation — fine for interactive use,
  a cache can come later if profiling asks for it.
- Proptests check that `move_to(p)` followed by resolve puts the
  reference point on `p` within 1e-6 lu for random elements, anchors,
  targets (gaps, grouping), groups and variants, and that resize and
  rotate land exactly and undo restores the project.

## Open questions

- Group pivots cannot be set on the canvas yet (`set_pivot` refuses
  groups): resolve does not expose the group pivot (see ADR 0004).
- Ungroup with a variant that overrides the group's position: the
  children keep the base compensation only.
- No `locked` flag in the model; the overlay has none.
- Snapping (page edges, centres, other elements) belongs to the GUI
  layer on top of `overlay`; it may move into core for MCP later.
