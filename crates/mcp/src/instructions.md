Arrangia designs App Store / Play Store screenshots: one layout, rendered for every target (store size) and language.

Concepts
- Project = folder (project.yaml + i18n/<lang>.yaml + screenshots/ + assets/). Every change is saved at once and is one undo step.
- Strip = canvas: N pages side by side; backgrounds run across all pages. A page is 1000 lu wide; its height follows the strip aspect (1320:2868 → 2173 lu). All sizes and positions are in lu, never px.
- Elements: device, text, image, shape, background, group. Each has a home page (`page`, 0-based); `at` is relative to that page's top-left corner and is the element centre (devices, images, shapes, free text boxes). Elements may reach into neighbour pages (a device spanning two pages exists once). Texts usually use margins: `box: {top, left, right}` and are clipped to their page.
- Gaps: between pages the store shows a gap (from the target's gallery profile); content inside a gap is cut from page exports (layout_report flags `in_gap`).
- anchor_y (top | bottom | center | stretch) keeps the distance to that edge when a target's page height differs (titles: top, devices running off the bottom: bottom).
- Targets come from presets (catalog_targets): export size, store slot, gallery gaps, `group` (2 = two portrait pages form one landscape image, e.g. iPad) and an optional variant.
- Variants: per-strip overrides of position/size/rotation/page for one format (e.g. tablet-landscape). Language overrides (`langs.<lang>`) adjust a text element for one language (box, font size, fit …). Edit tools take `scope`: base (default, all targets and languages), variant, lang.
- Texts reference translation keys; strings live per language (strings_get/strings_set). Missing translations fall back (fr-CA → fr → default language).
- Device elements show screenshots/<lang>/[<variant>/]<screenshot key>.png; a missing one is drawn as a placeholder.

Text markup (inside translations)
- `**bold**`, `_italic_`, `[span]text[/]` (named span style from styles.spans, e.g. an accent colour), `\n` hard break, `\-` soft hyphen. Long words: add `\-` rather than shrinking.
- Auto-fit: `fit: {shrink_to: <min font size>, max_lines: N}` shrinks the font until the text fits.

Units and geometry
- lu = layout units; x grows to the right, y downward. Rotation in degrees, positive = clockwise.
- element_move/resize take page-relative lu (x, y of the element centre relative to the top-left of page `page`), or dx/dy deltas.
- Colours `#RRGGBB[AA]` or `@name` (styles.colors). Text styles (styles.text) carry font, weight, size (lu), line height, colour.

Workflow
1. project_open {dir} or project_create {dir, name, template: phone|minimal, langs: ["en", "de"]}.
2. project_summary → strips, pages, element ids, targets, languages. Read view_get/selection_get when the user refers to "this" or "the selected element".
3. Change things: element_add / element_update (patch with YAML field names) / element_move / element_resize / element_rotate, strings_set for texts, styles_update for colours and text styles, strip_update for pages/backgrounds, target_add for more store sizes. apply_ops for complex atomic edits (see resource arrangia://docs/ops).
4. After every visual change call render_preview and LOOK at the image (default: first target, default language). Check other targets/languages too (view: gallery shows all images of a target with store gaps).
5. layout_report for exact boxes, chosen font sizes, overflows, hyphenation and overlaps; fix overflows by shortening text, `\-`, a larger box or a smaller `shrink_to`.
6. validate (errors must be 0 before export; missing screenshots are warnings while editing with preview: true).
7. export {targets?, langs?} writes <project>/export/… and returns the file list.
- Translating: strings_missing {lang} → strings_set {lang, values: {key: text}} (keep the markup) → render_preview with that lang.
- Mistakes: history_undo (history_state shows the labels; your steps start with "Claude:"). The user may edit in the GUI at the same time: re-read project_summary when an id is unknown.

Safety: tools never use the network; they only write inside the project folder and to paths you pass explicitly (export folder, strings_export path).
