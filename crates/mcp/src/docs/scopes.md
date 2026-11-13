# Scopes: base, variant and language overrides

An element has **base** fields. Two kinds of overrides change some of them
for part of the outputs:

- **Variant override** (`canvases[].variants.<variant>.overrides.<id>`): for the
  targets that use the variant (e.g. `tablet-landscape` for iPad with
  `group: 2`). Any element field except `id`, `kind`, `children`. For objects
  built as a group (cards, tickets, polaroids) one override of the group's
  `scale` (and `at`) resizes all its children – no override per child.
- **Language override** (`langs.<lang>` on a text element): for one language
  and its sub-languages (`de` also applies to `de-AT`). Only at, width,
  height, anchor_y, style, box, align, valign, fit, color, font_size, stroke,
  glow, shadow, hyphenate.

Resolution order: base → variant override → language overrides (`de`, then
`de-AT`). An override only **sets values**; a field it does not set shows the
base value. Overrides never contain `null` (a hand-written `null` is ignored,
reported as info `override_null` and dropped on the next save).

## Setting
`element_update {id, scope, …, patch}`:
- `scope: "base"` (default): the base element (all targets and languages).
- `scope: "variant"` + `variant` (default: the target's variant; `target`
  picks the target): merged into that variant override.
- `scope: "lang"` + `lang`: merged into the language override of exactly
  that language.

Several scopes in ONE undo step: `element_update {updates: [{id, scope,
lang?, variant?, patch}, …]}`; ids may come from several canvases.

## Resetting (back to the base value)
- In scope `variant`/`lang`, a field set to `null` in `patch` is **removed
  from the override** (it does not store `null`). The override disappears
  when nothing is left. Example: `{id: "title", scope: "lang", lang: "en",
  patch: {at: null}}` removes `at` from `langs.en`.
- `element_reset_override {id | ids, scope: "variant" | "lang", variant?,
  lang?, fields?}` removes the listed fields, or the whole override when
  `fields` is omitted.
- In scope `base`, `null` resets the field to the kind's default (see
  docs_get {topic: "elements"}) – usually not what you want for `at`.

## With apply_ops
`update_element` always writes the base element. Overrides:
`set_variant_override {canvas, variant, element, patch | null}` (replaces the
whole override: fields left out are lost, listed in the result as
`dropped_override_fields`; to change single fields use element_update
{scope: "variant"}, which merges), `reset_override_field {canvas, variant, element, field}`,
`set_lang_override {element, lang, patch | null}` (replaces the whole
override), `reset_lang_override_field {element, lang, field}`.

## Reading
`element_get {id, target?, lang?, variant?}` shows `base`, the variant
`override`, the merged `lang_override` and the `effective` element.
