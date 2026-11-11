# Vitrine Studio overview

Vitrine Studio designs App Store / Play Store screenshots: one layout, rendered
automatically for every target (store size) and language.

## Concepts
- **Project** = a folder: `project.yaml` (strips, elements, styles, targets),
  `i18n/<lang>.yaml` (translations), `screenshots/<lang>/[<variant>/]<key>.png`,
  `assets/` (images), `fonts/`. Every tool change is saved at once and is
  one undo step (history_undo).
- **Strip** = a canvas of N equally sized pages side by side. Backgrounds run
  across all pages. A page is **1000 lu** wide; its height follows the strip
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
- **Variants**: per-strip overrides for one format (iPad landscape …).
  **Language overrides** (`langs.<lang>`) adjust a text for one language.
  See docs_get {topic: "scopes"}.
- **Texts** reference translation keys; strings live per language
  (strings_get / strings_set); missing ones fall back (fr-CA → fr → default).
- **Devices** show the screenshot `screenshots/<lang>/[<variant>/]<key>.png`
  of their `screenshot` key; a missing one is drawn as a placeholder.

## Workflow
1. project_open {dir} or project_create {dir, name, template, langs}.
2. project_summary → strips, pages, element ids, targets, languages.
3. Change: element_add / element_update / element_move / element_resize /
   element_rotate, strings_set, styles_update, strip_update, target_add,
   assets_import / screenshots_import, apply_ops for raw atomic changes.
4. After every visual change: render_preview and look at the image; also
   other targets and languages (view: gallery shows a whole target).
5. layout_report for exact boxes, font sizes, overflows; validate (0 errors
   before export); export.

## Units and geometry
- x grows to the right, y downward; rotation in degrees, positive = clockwise.
- Colours `#RRGGBB[AA]` or `@name` (styles.colors).
- Errors list the allowed fields and values; read them and retry.
