# Vitrine Studio ops (apply_ops)

`apply_ops {ops: [...], label?}` applies all ops atomically as one undo step:
either every op succeeds or nothing changes. Each op is a JSON object with an
`"op"` tag. Field values use the YAML names of `project.yaml` (docs_get
{topic: "project"} and {topic: "elements"}; the resource
`vitrine://project/project.yaml` has real examples). Prefer the specific
tools (element_update, strings_set …); use ops for multi-part changes that
must be one step. A failing op names its index: `op 2 of the transaction
failed: …`.

Big batches (a whole canvas, 10–50 KB of JSON): write them to a file inside
the project folder and call `apply_ops {file: ".cache/ops/phone.json",
label?}` instead of passing `ops` (exactly one of the two). The path is
relative to the project folder (absolute paths, `..` and symlinks leading out
are refused); `.json` or `.yaml`/`.yml`; the content is the ops array or
`{"ops": [...], "label": "..."}` (a `label` argument wins). The file is not
deleted – keep it (e.g. under `.cache/`) to diff the next version.

## Value shapes
- Ids: letters, digits, `-`, `_`, `.`; element ids are unique in the project.
- Point `{x, y}` in lu. Aspect `"w:h"`. Pixel size `[w, h]`.
- Colour `"#RRGGBB"`, `"#RRGGBBAA"` or `"@name"` (styles.colors).
- Fill: `"#0B3324"` | `{kind: color, color}` | `{kind: linear-gradient, angle, stops: [[colour, 0..1], …]}`
  (angle CSS-like: 0 = to top, 90 = to right, 180 = to bottom) |
  `{kind: radial-gradient, center?: {x, y} (fractions), radius?, stops}` |
  `{kind: image, asset, fit?: cover|contain|tile, …}`.
- ElementPatch: object of element fields (docs_get {topic: "elements"}).
  In `update_element` a `null` resets the base field to its default. In
  override patches (`set_variant_override`, `set_lang_override`) a `null`
  field is ignored: an override only sets values, and the patch REPLACES
  the whole override (fields left out are no longer overridden). The
  result lists such lost fields in `dropped_override_fields` (`[{canvas,
  variant, element, fields}]`) with a warning line. To change single fields
  of an override use element_update {id, scope: "variant", variant?,
  patch}, which merges.

## Elements
- `{"op": "add_element", "canvas": "phone", "element": {"id": "badge", "kind": "shape", "page": 0, "at": {"x": 820, "y": 300}, "width": 200, "height": 80, "shape": "rect", "radius": 40, "fill": "@accent"}, "parent"?: "group-id", "index"?: 0}`
  (`index`: position in the list, 0 = bottom; default top)
- `{"op": "update_element", "id": "title", "patch": {"font_size": 80, "align": "left"}}`
- `{"op": "remove_element", "id": "badge"}`
- `{"op": "move_element", "id": "badge", "index": 0}` (painting order)
- `{"op": "move_element_to", "id": "badge", "parent": "group-id" | null, "index"?: n}`
- `{"op": "rename_element", "from": "text-2", "to": "title-beds"}`

Element kinds and their fields:
- device: `model` (catalog_devices), `screenshot` (key), `width` (height
  follows), `orientation`, `color`, `frame`, `fit`, `shadow`, `chrome`, `url`,
  `fold` (open|folded, foldables such as `iphone-duo`)
- text: `text` (translation key), `style`, `box: {top, left, right, bottom}`
  (margins to the page) or `at` + `width` (+ `height`) for a free box,
  `align`, `valign`, `fit: {shrink_to, max_lines}`, `font_size`, `color`,
  `stroke`, `glow`, `shadow`, `hyphenate`, `langs: {de: {…patch…}}`
- image: `asset`, `width`, `height`, `fit`, `shadow`
- shape: `shape` (rect|ellipse|line), `width`, `height`, `radius`, `fill`, `stroke`, `shadow`
- background: `fill` (a per-page background; canvas backgrounds are canvas `backgrounds`)
- group: `children: [elements]`, `scale` (0.1..10, default 1: scales all children like a
  transform around the group pivot – positions, sizes, fonts, strokes, radii, effects; `at`
  is an offset, `pivot` the origin of rotation and scale)
- common: `page`, `at`, `rotation` (degrees, clockwise), `pivot`, `anchor_y`,
  `opacity`, `z`, `clip` (none|page), `visible`, `visible_in: {targets, langs}`, `lock_page`, `locked`,
  `allow: [finding codes]` (design findings meant on purpose, see validate)

## Variants and language overrides
`update_element` always changes the base element (all targets and
languages). Per format or language use these ops (or element_update with
`scope`, which merges into the existing override):
- `{"op": "add_variant", "canvas": "phone", "id": "tablet-landscape", "variant": {"content_scale": 0.75}}`
- `{"op": "remove_variant", "canvas": "phone", "id": "tablet-landscape"}`
- `{"op": "rename_variant", "canvas": "phone", "from": "a", "to": "b"}`
- `{"op": "set_variant_content_scale", "canvas": "phone", "variant": "tablet-landscape", "content_scale": 0.8 | null}`
- `{"op": "set_variant_override", "canvas": "phone", "variant": "tablet-landscape", "element": "phone-plan", "patch": {"width": 520} | null}`
  (replaces the element's whole override; prefer element_update {scope: "variant"} to change single fields;
  a whole card/ticket group smaller in one override: `"element": "card", "patch": {"scale": 0.85}`)
- `{"op": "reset_override_field", "canvas": "phone", "variant": "tablet-landscape", "element": "phone-plan", "field": "width"}`
- `{"op": "set_lang_override", "element": "title", "lang": "de", "patch": {"box": {"top": 170, "left": 40, "right": 40}} | null}`
  (only at, width, height, anchor_y, style, box, align, valign, fit, color, font_size, stroke, glow, shadow, hyphenate)
- `{"op": "reset_lang_override_field", "element": "title", "lang": "de", "field": "box"}`

Several languages in one step:
```json
{"ops": [
  {"op": "set_lang_override", "element": "title", "lang": "de", "patch": {"font_size": 64}},
  {"op": "set_lang_override", "element": "title", "lang": "fr", "patch": {"font_size": 66, "box": {"top": 160, "left": 50, "right": 50}}},
  {"op": "reset_lang_override_field", "element": "sub", "lang": "en", "field": "at"}
]}
```

## Canvases
- `{"op": "add_canvas", "canvas": {"id": "tablet", "page": {"aspect": "2064:2752", "count": 3, "ids"?: ["a", "b", "c"]}, "backgrounds"?: [fill, …]}, "index"?: n}`
- `{"op": "update_canvas", "id": "phone", "patch": {"aspect"?: "1320:2868", "count"?: 5, "ids"?: [...], "backgrounds"?: [...]}}`
- `{"op": "remove_canvas", "id": "tablet"}`, `{"op": "rename_canvas", "from": "a", "to": "b"}`

## Targets
- `{"op": "add_target", "target": {"id": "ipad", "kind": "pages", "canvas": "phone", "size": [2064, 2752], "group"?: 2, "variant"?: "tablet-landscape", "screenshots"?: "ipad-13", "gallery"?: {"preset": "appstore-ipad"}, "langs"?: ["en"], "store"?: {…}}}`
  (easier: the target_add tool with a catalog preset)
- `{"op": "update_target", "id": "ipad", "patch": {"group": 2, "variant": null, …}}`
- `{"op": "remove_target", "id": "ipad"}`, `{"op": "rename_target", "from": "a", "to": "b"}`

## Translations and languages
- `{"op": "set_string", "lang": "de", "key": "hero.title", "value": "Hallo **Welt**" | null}`
- `{"op": "rename_string_key", "from": "text-2", "to": "beds.title"}`
- `{"op": "add_language", "lang": "fr"}`, `{"op": "remove_language", "lang": "fr"}`
- `{"op": "set_default_language", "lang": "en"}`
- `{"op": "set_languages", "languages": {"default": "en", "available": ["en", "de"], "fallback"?: {"de-AT": "de"}}}`

## Project and styles
- `{"op": "set_name", "name": "Fernwood"}`
- `{"op": "set_styles", "styles": {…complete styles object…}}` (prefer styles_update)
- `{"op": "rename_style", "kind": "color" | "text" | "span" | "shadow", "from": "accent", "to": "highlight"}`
- `{"op": "set_import", "import": {…screenshot import mappings…}}`
- `{"op": "set_checks", "checks": {"device_distance"?: 16, "min_contrast"?: 4.5, "min_contrast_large"?: 3, "off"?: ["page_blends_with_store"]}}`
  (design-check thresholds and codes switched off for the whole project; replaces `checks:`)
