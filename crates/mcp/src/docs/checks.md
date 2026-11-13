# Checks: every validate finding

`validate` resolves every target × language and reports findings
`{level, code, target, lang?, page?, element?, message}`. Levels: **error**
(blocks export / upload), **warn**, **info**. Columns below: **lvl**,
**allow** (✓ = `allow: [code]` on an element or `checks.off` can switch it
off; "off" = only `checks.off`, the finding names no element), what is
measured and the fix.

Two kinds of measurement:
- **layout**: element boxes from resolve (rotated bounding boxes, text ink
  boxes from the text layout). Fast, no pixels.
- **pixels** ("design checks", ADR 0029): small partial renders. An element
  drawn alone, without shadow / glow / outline / highlights, is its mask
  (alpha ≥ 50 %); transparent parts of PNG/SVG do not count. Skipped with
  `validate {pixel_checks: false}` (CLI `--no-pixel-checks`).

Which language: texts (`low_contrast`, `text_near_device`, all text
layout findings) and `seam_overlap` pairs that involve a text are checked
in **every** language. Everything else about devices, pages and corners
(`device_small`, `page_not_covered`, `seam_split`, `screen_*`,
`device_cut`, `element_in_corner`, `page_blends_with_store`, non-text
`seam_overlap`, store rules, `variant_suggested`) runs only on the **first
language of each target** – the first of the target's `langs` (default
`languages.available`) that passes the `langs` filter. `validate {langs:
["de"]}` or `render_preview {lang: "de"}` therefore runs them on `de`.
A text in a corner or over a screen is only checked in that first
language.

`render_preview` returns only the errors and warnings of its target ×
language; most design findings are **info** – call `validate` to see them.

## Design checks (all allowable)

| code | lvl | measured | threshold (`checks:` key) |
|---|---|---|---|
| `low_contrast` | warn | pixels | 4.5:1 normal (`min_contrast`), 3:1 large (`min_contrast_large`) |
| `text_near_device` | warn / info | pixels | 16 lu (`device_distance`) |
| `screen_header_covered` | info | pixels | ≥ 25 % of the top bar (fixed) |
| `screen_header_cut` | info | geometry | cut inside the top bar (fixed) |
| `screen_covered` | info | pixels | ≥ 15 % of the screen below the bar (`screen_cover`) |
| `device_cut` | info | pixels | > 8 lu cut off left / right (fixed) |
| `seam_overlap` | info | pixels | ≥ 576 lu² (≈ 24 × 24, fixed) |
| `seam_split` | info | layout | twin halves at a seam (fixed) |
| `element_in_corner` | info | pixels | ≥ 3 % of the corner zone, min 20 lu² (fixed) |
| `device_small` | info | layout | largest device < 50 % of its page (fixed) |
| `page_not_covered` | info | layout | strip ≤ 15 % (fixed) |
| `page_blends_with_store` | info, allow: off | pixels | ≥ 60 % of edge pixels < 1.3:1 (fixed) |

### low_contrast (warn, text)
- Glyph colour = the text's exact fill (spans included), mixed with what
  is behind for `opacity` < 1. Backdrop = a ring of max(3 lu, 0.15 × font
  size) around the glyphs in the composite of everything drawn up to and
  including the text – **the text's own shadow, glow, outline and span
  highlight count as backdrop**. Only pixels that end up in an export (not
  gaps) and inside the text's clip.
- Per 1-em cell the 30 % quantile of the per-pixel WCAG ratio; the worst
  cell is reported with its page-relative position, the ratio, the
  direction ("lighten the text or darken what is behind it") and the
  source: "mostly the text's own shadow, glow, outline or highlight"
  (≥ half of the failing ring pixels change when the text is drawn bare)
  or "mostly `<id>` there" (topmost element below the text at those
  pixels) or "mostly the canvas background there".
- Large text = laid-out font size ≥ 80 lu, or ≥ 62 lu with a style weight
  ≥ 700 (span bold does not count). Texts with `opacity` < 0.5 are skipped
  (ghost decoration).
- Fix: change text colour / position, add a scrim, `glow`, `stroke` or
  `shadow` in a contrasting colour (a dark glow under light text helps; a
  glow in the text colour makes it worse).

### text_near_device (text)
- Glyph mask vs the device mask (the drawn frame incl. tilt, imported
  bezels, laptops; not the device's shadow). Devices whose box comes
  within `device_distance` + 4 lu of the text are compared.
- **warn**: glyphs behind a device drawn above the text ("partly hidden");
  glyphs touching the frame (< 1 % on it); glyphs crossing a frame edge
  (1–97 % on it). **info**: no overlap but closer than `device_distance`.
  No finding: ≥ 97 % of the glyphs on a device drawn below the text (a
  label on the screen, deliberate).
- **Carriers are exempt**: when the text is drawn above the device, every
  element painted between device and text (not a text, background or page
  backdrop, opacity ≥ 0.5 – a card, bubble, badge) removes its area from
  both masks. A text sitting on a card over the device is fine; only
  glyphs outside the card count.
- Display type with a laid-out size ≥ 240 lu (3 × 80) is skipped.
- `device_distance: 0` reports only touching / crossing / hidden glyphs.

### screen_header_covered / screen_covered (info, on the covering element)
- Top bar ("header") = top 14 % of the screen height, or 9 % when the
  screen is wide (width > 0.8 × height: landscape phones and tablets).
  Portrait iPads (3:4 = 0.75) use 14 %. `layout_report` gives it as
  `screen.header` (lu).
- Elements checked: drawn above the device, not another device, not a
  `background` element, not a page backdrop (image/shape covering ≥ 90 %
  of its page), opacity ≥ 0.3.
- `screen_header_covered`: the element's pixels cover **≥ 25 %** of the
  top bar (below 25 % nothing is reported).
- `screen_covered`: covers ≥ `checks.screen_cover` (default 0.15, range
  0..1; 1 = off) of the **visible** screen below the bar (parts cut off by
  the page edge or in a gap do not count).
- Finding and `allow` belong to the covering element, not the device.
- Fix: move the card below the bar, onto the frame or beside the device;
  make it smaller; `layout_report` → `screen_covered_by[]` gives
  `share_of_screen` / `share_of_header` per element.

### screen_header_cut (info, on the device)
- Fires only when a horizontal cut line lies **inside the top bar**: from
  0.5 % of the screen height below the screen top to the bottom of the bar.
  Cut lines: the top edge of each export image (= page top for page
  targets; for banners the region's top) and the top of the device's clip
  (`clip: page`). The message names which ("the top edge of page 3",
  "of export image 2", "its clip").
- Not reported: the cut lies below the bar (the whole bar is outside – a
  deliberate crop), the bar is completely visible, bottom edges, tilted
  devices (rotation not within ±1°).
- Fix: move the device so the cut lies below the bar or the whole bar
  shows; `allow: [screen_header_cut]` on the device when meant.

### device_cut (info, on the device)
- The device drawn without its clip loses more than 8 lu (`DEVICE_CUT_MIN`,
  fixed; side buttons stay below it) beyond the **left or right** edge of
  its page (`clip: page`) or of the whole canvas (`clip: none`), measured
  on pixels within the visible height – a tilted device's rotated corners
  count exactly. Top / bottom crops are not reported. A device with
  `clip: none` that runs on into the next page is not cut.
- Message: lu lost per side and "move it N lu right/left". Fix: move,
  shrink or tilt less; or `allow: [device_cut]`.

### seam_overlap (info, on the element reaching across)
- An element (not a background or page backdrop) reaching from its home
  page into another page is drawn **over** an element homed there; the
  overlapping pixels are ≥ 576 lu². Not reported: the other element is
  drawn above it (a text or card lying on a bridging tile), both in one
  group, either one allows `seam_overlap`.
- Meant for "is this crossing piece hiding something on the next page?"
  – check the gallery. Wide panoramas *under* everything do not trigger it.

### seam_split (info, layout, on the left half)
- Two elements of the same kind, both clipped (`clip: page`), the first
  centred on the right edge of page n (± 25 % of its width), the second on
  the left edge of page n+1, same height (± 10 %) and size (± 5 %), and
  the store shows a gap there. The gap pulls the halves apart.
- Fix: ONE element with `clip: none` across the border (see overview →
  Panoramas and seams). Never compute gap offsets yourself.

### element_in_corner (info)
- The store rounds each export image's corners (gallery `radius`,
  `layout_report.gallery.radius_lu`; only when ≥ 4 lu). Reported when the
  visible pixels of an element (not a device, background element or page
  backdrop; opacity ≥ 0.3) outside the rounded corner cover ≥ 3 % of the
  cut-off zone (min 20 lu²).
- Ignored: parts hidden under opaque elements (opacity ≥ 0.9) drawn above
  it; elements that fill ≥ 90 % of the corner square (full-bleed pictures,
  scrims – they round like the page). A square motif *inside* a page-sized
  background picture is not seen.
- Fix: keep square shapes clear of the corners, or give them a radius.

### device_small (info, layout, on the device)
- The largest device homed on a page: rotated box height / page height
  (or width / page width, whichever is larger) < 0.5. Fixed threshold.

### page_not_covered (info, layout heuristic, on the covering element)
- An unrotated image or shape with opacity ≥ 0.99 (its **box**, not its
  pixels) covers ≥ 85 % of its home page and three edges, but leaves a
  strip > 2 lu and ≤ 15 % of the page at the fourth (typical after a
  variant changed the page height).
- Quiet when something **painted below** it fills the strip: a `background`
  element, or an opaque unrotated image/shape covering ≥ 90 % of the page.
  The canvas `backgrounds` do **not** count (it reports exactly the case
  where only the canvas colour shows in the strip).
- Fix: `anchor_y: stretch` + height = page height, a variant override, or
  put a `background` element under it.

### page_blends_with_store (info; only `checks.off`)
- Each export image rendered 64 px wide; its outer 2 px ring of pixels
  compared with the gallery's light `background` and `background_dark`
  (App Store #000000, Play #131314, not yet measured). ≥ 60 % of the ring
  below 1.3:1 → one finding per background naming the images. Only targets
  with a gallery gap > 0.

## Text layout (per language)

| code | lvl | allow | what / fix |
|---|---|---|---|
| `text_missing` | error | – | key in no language of the fallback chain; nothing drawn |
| `text_fallback` | warn / info | – | text from another language; warn when it is the default language (untranslated), info for a configured / prefix fallback |
| `text_overflow` | error | – | does not fit box / `max_lines` even at `fit.shrink_to`: shorten, `\-`, bigger box, smaller `shrink_to` |
| `emergency_break` | warn | – | word broken without hyphen: add `\-` or `hyphenate` |
| `hyphenated` | info | ✓ | automatic hyphenation was needed |
| `hyphenation_unavailable` | info | – | `hyphenate` on but no patterns for the language (reported with an overflow) |
| `invalid_markup` | warn | – | unbalanced markup, drawn literally |
| `unknown_span_style` | warn | – | `[name]…[/]` names no span style |
| `unknown_font` | warn | – | font family / fallback not available |
| `missing_glyphs` | warn | – | characters no font of the chain has |
| `text_clipped` | warn | ✓ | ink box (hard outline incl. stroke; soft glow / shadow ignored, 0.5 lu tolerance) leaves its clip (`clip: page`) |
| `text_in_gap` | warn | ✓ | visible ink box reaches into a store gap (cut from exports) |
| `text_outside_safe_area` | warn | ✓ | outside the target's `safe_area` |
| `system_font_used` / `system_font_fallback` | info | – | font only installed on this computer |
| `system_fonts_stored` | info | – | licence reminder for `fonts/system/` |

## Elements, pages, references

| code | lvl | allow | what |
|---|---|---|---|
| `element_outside` | warn | ✓ | rotated box touches no page (only gaps / outside): in no export |
| `variant_suggested` | info | off | target page height differs > 15 % from the canvas page height and the target has no variant |
| `background_hidden` | warn | – | a canvas `backgrounds` layer lies under an opaque one (layers, not one per page) |
| `invalid_clip_to` | warn | – | `clip_to` names no rect/ellipse shape of this canvas; drawn unclipped |
| `clip_to_other_page` | info | ✓ | `clip_to` shape homed on another page than the image (works) |
| `page_out_of_range` | error | – | home page outside the canvas |
| `missing_size` | info / warn | – | device / free text without `width` (info, default used); image / shape without size (warn, 0) |
| `partial_image` | info | – | page count not a multiple of `group` |
| `region_out_of_range` | warn | – | banner region outside the canvas |
| `unknown_color` | error | – | `@name` not in styles.colors (drawn magenta) |
| `unknown_text_style`, `unknown_shadow`, `unknown_gallery`, `no_gallery` | warn | – | unknown names; `gaps: as_store` without gallery |
| `unknown_variant`, `unknown_override_element`, `invalid_override` | warn | – | override problems |
| `override_null` | info | – | `null` inside an override in the file (ignored, dropped on save) |
| `unknown_allow` | warn | – | `allow` / `checks.off` names an unknown or non-allowable code |
| `unknown_target`, `unknown_canvas`, `invalid_lang` | error | – | bad filter / reference |
| `lang_not_available` | warn | – | language not in `languages.available` |

## Devices, frames, screenshots

| code | lvl | allow | what |
|---|---|---|---|
| `unknown_device` | error | – | model id not in the catalog (not drawn) |
| `unknown_mapped_device` | warn | – | `device_map` names an unknown model |
| `unknown_device_color` | warn (info when mapped) | – | colour not available for the model |
| `unsupported_orientation` | warn | – | model has no such orientation |
| `screenshot_missing` | error (warn with `preview: true`) | – | `screenshots/<lang>/[<variant>/]<key>.png` not found; placeholder drawn |
| `screenshot_fallback` | info | – | screenshot of another language |
| `screenshot_aspect` | warn (info up to 5 % with `fit: cover`) | – | screenshot aspect differs > 2 % from the display: wrong device / folder? Up to 5 % under `cover` only a thin edge is cropped (e.g. iPhone captures on the Android frame of a Play target): info naming the device's own folder (`screenshots/<lang>/android-phone/`) |
| `screenshot_upscale` | warn | ✓ | enlarged > 1.15 × to fill the display in px: may look blurry |
| `screenshot_only_empty` | warn | – | `screenshot_only` page without a device |
| `sample_screenshot` | info | – | template sample, replace it (uploads refuse samples) |
| `color_profile_unsupported` | info | – | colour profile not usable, drawn as sRGB |
| `unknown_screenshot_variant` | info | – | target `screenshots: X` is neither a catalog variant nor an existing folder `screenshots/<lang>/X/` (target languages and their fallbacks): a typo. Own folders are fine |
| `imported_frame_missing` | info | – | `frame: imported:…` not installed here; parametric frame drawn |
| `imported_bezel_fallback`, `imported_color_fallback` | info | – | pinned bezel / colour not installed, another used |
| `imported_frame_not_stored`, `imported_frames_stored`, `stored_frames_stale` | info | – | imported frames vs project copies (CI) |

## Store rules and store mapping

| code | lvl | what |
|---|---|---|
| `store_size` | error | size not accepted for the target's store slot |
| `store_count` | error above max / warn below min | images per set |
| `store_format` | error | format not accepted |
| `store_decorations` | info | the store wants bare UI (Wear OS): use `screenshot_only` |
| `unknown_store_slot` | warn | unknown slot; rules not checked |
| `unverified_store_slot` | info | slot not confirmed by the store API |
| `store_locale_unknown` | error | `store.locales` code not a store locale |
| `store_locale_missing`, `store_locale_duplicate`, `store_no_slot`, `store_slot_duplicate` | warn | upload mapping problems |
| `store_locale_unused` | info | locale entry for a language the project lacks |
| `required_store_set_missing` | warn (off) | no target fills a set App Store Connect requires (below) |

None of these is allowable, except `required_store_set_missing` via
`checks.off` (it names no element).

### required_store_set_missing (warn; only `checks.off`)
- Data: Apple presets in `targets.yaml` with `requirement: required` –
  today iPhone 6.3″ (`ios-iphone-63`, `APP_IPHONE_61`), iPad 13″
  (`ios-ipad-13`, `APP_IPAD_PRO_3GEN_129`), Mac (`mac`, `APP_DESKTOP`).
  Watch presets are `conditional` (any one watch size) and never reported.
- A required set counts for its platform once the project targets it: the
  platforms of the presets matching the slots of the project's Apple
  targets. iPad / Mac are asked for only when the project already has an
  iPad / Mac target (whether the app runs on iPad cannot be known
  otherwise). Mapped to the App Store (`store.apple`) without iPhone/iPad
  targets: `store.apple.platform` decides (`ios` → iPhone, `macos` → Mac).
- Filled by any target with the same store slot (any size the slot takes).
- Warn, not error: export and upload of the other sets still work; App
  Store Connect refuses to submit the version without it. Also a
  non-blocking upload readiness step (`fix: targets`).
- Fix: add the target from the named preset (`target_add {preset, canvas}`), or
  `checks.off` when the set comes from elsewhere.

## Switching findings off

Project `checks:` (op `set_checks`, replaces the whole section; validated
on apply):

```yaml
checks:
  device_distance: 16       # lu, 0..1000 (text_near_device)
  min_contrast: 4.5         # 1..21 (low_contrast, normal text)
  min_contrast_large: 3     # 1..21 (large: ≥ 80 lu, bold ≥ 62 lu)
  screen_cover: 0.15        # 0..1 (screen_covered; 1 = never)
  off: [page_blends_with_store]   # allowable codes, project-wide
```

Everything else (top-bar shares, 25 %, 8 lu, 576 lu², 50 %, 85 / 15 %,
60 % / 1.3:1, large-text sizes) is fixed.

Element field `allow: [codes]`:
- Allowable codes: `low_contrast`, `text_near_device`, `screen_header_covered`,
  `screen_header_cut`, `screen_covered`, `device_cut`, `seam_overlap`,
  `seam_split`, `element_in_corner`, `device_small`, `page_not_covered`,
  `page_blends_with_store`, `clip_to_other_page`, `text_clipped`,
  `text_in_gap`, `text_outside_safe_area`, `element_outside`,
  `hyphenated`, `variant_suggested`, `screenshot_upscale`,
  `required_store_set_missing`. Errors and missing content never.
- It silences findings **whose `element` is that element** (for
  `screen_header_covered` / `screen_covered`: the covering element; for
  `seam_overlap` either of the two). Findings without an element
  (`page_blends_with_store`, `variant_suggested`,
  `required_store_set_missing`) only go with `checks.off`.
- On a group it applies to all children (merged with their own lists).
- A common field: a variant override `allow` applies only to that
  variant's targets.
- Unknown / non-allowable entries → warn `unknown_allow`.

## layout_report for the same geometry

`layout_report {target?, lang?, page?}` returns the numbers the checks use:
- `elements[]`: `box` (page lu), `in_gap`, text `font_size` / `lines` /
  `overflow`; for devices `frame` (4 rotated corners), `screen` {x, y, w,
  h before rotation, `radius`, `quad` = rotated corners, `header` = top
  bar height in lu} and `screen_covered_by[]` {id, kind,
  `share_of_screen`, `share_of_header`} (every element drawn above with
  opacity > 0, page backdrops left out – no 0.3 / 25 % / 15 % filter).
- `overlaps[]`: drawn-pixel overlaps of every text with other elements
  (`area_lu2`, `share_of_text`), and of non-text elements across a page
  border (`seam: true`, ≥ 100 lu²).
- `seams[]`: {`after_page`, `x`, `gap_lu`, `next_page_offset` =
  −(1000 + gap)}: a point x on `after_page` is x + `next_page_offset` on
  the next page.
- `gallery`: `gap_lu`, `gap_px`, `radius_lu`, `measured` (`no` =
  estimate).
