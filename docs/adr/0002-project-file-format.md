# 0002 – Project file format (`project.yaml`, `i18n/*.yaml`)

- Status: accepted
- Date: 2026-11-09

## Context

KONZEPT §3 defines the project as a folder of human-editable YAML files that
must be diffable, Git-friendly and readable by people and Claude. Phase 1
(task 1.1/1.2) builds the typed model in `arrangia-core` (`model`, `io`,
`ops`). This ADR records the conventions of the file format and how the
model maps to it. The sample in `crates/core/tests/data/sample/` is the
reference; its `project.yaml` is canonical (load → save reproduces it byte
for byte, tested).

## Decision

### Library and emitter settings

- **serde-saphyr 2.1** for reading and writing (no fallback to
  `serde_norway` needed).
- **YAML 1.2 core schema** on both sides: written with `yaml_12` (no
  directive), read with `strict_booleans`. Only `true`/`false` are booleans;
  `y`, `no`, `on`, `1320:2868` stay strings (no "Norway problem", language
  code `no` works).
- Block style for the structure; **flow style for small value types** so the
  file stays terse: points (`at`, `pivot`), `page`, `box`, text `fit`,
  `size`, `available`, gradient `stops`, text/span styles, shadow layers,
  `stroke`, `visible_in`, `region`, `store`, `device_map`, gallery overrides,
  `langs` and variant overrides. serde-saphyr writes flow maps without inner
  padding: `at: {x: 1000, y: 2172}` (the concept shows `{ x: 1000 }`; both
  parse). Long strings are never folded (`prefer_block_scalars: false`).
- Two serde-saphyr 2.1.1 quirks are worked around in `model/fmt.rs`: a
  `FlowMap` nested in a `FlowMap` leaves the emitter in flow mode for the
  following block collections (we only emit the outermost flow wrapper),
  and `FlowMap` around an enum variant is ignored (`Region` serializes
  itself as a one-entry map).
- Numbers without fractional part are written as integers (`width: 820`,
  not `820.0`).

### Canonical form

- Field order = struct field order (stable). **Defaults are omitted**
  (`rotation: 0`, `opacity: 1`, `visible: true`, empty lists/maps …).
  Exceptions: `page` of an element is always written (central concept), and
  `format`, `revision`, `name`, `languages` always appear.
- Properties whose default depends on the kind (`anchor_y`, `clip`) are
  `Option`s: an explicitly written default (`anchor_y: top`) is kept.
- **Maps are written sorted by key** (`BTreeMap`): palette colours, styles,
  variants, overrides, fallbacks, translations. Lists (strips, elements,
  backgrounds, targets) keep their order; element order is painting order.
- Unknown fields are errors everywhere (`deny_unknown_fields`), reported
  with file, line, column and a snippet.

### Units, coordinates, references

- `format: arrangia/1` first; unknown formats are rejected.
- **Ids are strings**: ASCII letters, digits, `-`, `_`, `.`, starting with a
  letter or digit. Element ids are unique across the whole project (ops
  address elements by id alone); strip, target ids are unique in the
  project, variant ids within their strip. Translation keys are flat
  strings and may contain dots (`hero.title`).
- **Page ids**: `page: {aspect, count, ids: [hero, planner]}`; `ids` is
  optional and may be shorter than `count`, missing ids default to
  `page-01`, `page-02` … (1-based). Page ids are unique within the strip and
  name the export files (`<nn>_<page-id>.png`). Elements still refer to
  pages by index, so renaming a page id never breaks a reference.
- **`page` is a 0-based index** (home page). Positions are relative to the
  top-left corner of the home page, in **lu** (page width 1000 lu).
- `at` is the element centre for devices, images, shapes and free text
  boxes. `pivot` is an offset from the element centre (default centre).
- **Rotation in degrees, positive = clockwise**, y down (ADR 0001). Gradient
  angles follow CSS (0 = to top, 90 = to right, default 180).
- **Colours**: `#RRGGBB` / `#RRGGBBAA` (written upper-case) or `@name`
  referencing `styles.colors` (palette entries are literal colours only).
  Wherever a fill is expected, a colour string is shorthand for
  `{ kind: color, color: … }`.
- **Anchors** `anchor_y: top | bottom | center | stretch`; default
  `stretch` for `background` elements, `top` otherwise (geometry per
  ADR 0001).
- **`clip`** `page | none`; default `page` for text, `none` for all other
  kinds.
- `visible: false` hides an element; `visible_in: {targets: […], langs: […]}`
  restricts it (empty list = no restriction).
- Shadow presets `soft`, `hard`, `none` always exist and may be redefined in
  `styles.shadows`; `styles.frame` sets the project frame style
  (`stylized`, `detailed`, `imported:<source>`).

### Elements

One flat map per element, tagged by `kind` (`background`, `device`, `text`,
`image`, `shape`, `group`). The Rust model is typed (`Element` with common
fields + `ElementKind` enum), but serialization goes through a flat
`ElementWire` struct listing every field, because serde's `flatten`
disables `deny_unknown_fields`. Fields that exist but do not belong to the
kind are reported as `field `model` is not allowed on text elements`.
Shared keys with kind-dependent types: `fit` (`cover|contain|stretch` for
devices/images, `{shrink_to, max_lines}` for text) and `color` (colour
variant name or `#hex` for devices, colour for text). A device has `width`
only; its height follows from the catalog model.

Groups hold `children`; children keep their own home page and position, the
group's `at`/rotation/opacity apply on top (resolved in task 1.4).

### Variants and overrides

`variants: {name: {content_scale, overrides: {element-id: patch}}}`. A patch
(`ElementPatch`, shared with `Op::UpdateElement`) has the same fields as an
element except `id`, `kind`, `children`: a missing field is unchanged,
`null` resets it to its default, a value replaces it. Overrides may touch
any property (in practice geometry, `visible`, `box`, `anchor_y`); the
element with the override applied must be valid. Content (screenshots,
texts, styles) is normally shared; device swaps belong to the target's
`device_map`. Removing an element removes its overrides; undo restores them.

### Targets

`kind: pages` (default, omitted) or `banner`. `group`, `variant`, `size`,
`screenshots`, `gallery` (preset id, or `{preset, gap, radius}` to override
the catalog values), `device_map`, `format`/`quality`, `langs`, `store`
(`{apple: {display_type}}` / `{google: {image_type}}`, strings checked
later against the catalog). Banner-only: `region` (`{pages: [a, b]}`
inclusive, or `{rect: {page, x, y, width, height}}`), `fit`, `gaps`
(`none | as_store`). The rules are checked on load and in ops.

### Translations

`i18n/<lang>.yaml` is a flat map key → text, sorted by key. Texts keep the
inline markup of §3.5 verbatim (parsed in task 1.5). Languages without any
string have no in-memory map; their files are left alone.

### Revision and saving

- `revision: u64` in `project.yaml`, incremented by every save.
- `Project::save` reads the revision on disk; if it differs from the loaded
  one it fails with `Conflict` and writes nothing (optimistic locking,
  §8.3). Otherwise it writes changed `i18n/*.yaml` files, then
  `project.yaml` with the new revision — each atomically (temp file in the
  same folder, fsync, rename). There is a small race window between the
  check and the rename; acceptable for GUI + MCP on one machine.
- Every save rewrites `project.yaml`, so a string-only change also bumps the
  revision and is visible to watchers of that file.

### Load strictness

Loading fails on syntax/type errors, unknown fields, wrong element fields,
unsupported `format`, duplicate ids and self-contained target/language
rules. Dangling references (unknown style, colour, page out of range,
unknown target …) do **not** fail loading so a hand-edited project can
still be opened; `core::validate` reports them and ops refuse to create new
ones.

## Consequences

- The concept's §3.7 example loads as is (spacing and key order differ
  after the first save: sorted maps, flow maps without padding).
- Adding a field means adding it to the struct (and for elements to the
  `element_fields!` list, which keeps `ElementWire` and `ElementPatch` in
  sync); field position defines its position in the file.
- If serde-saphyr fixes the nested-flow bug, the guard in `model/fmt.rs`
  can stay (it is harmless) or be removed.
