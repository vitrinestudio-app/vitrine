# 0005 – Validation

- Status: accepted
- Date: 2026-11-09

## Context

KONZEPT §3.3, §3.4, §4.3, §5 and §6.4 name many things the app must warn
about: text in store gaps, text overflow per language, missing
translations and screenshots, wrong screenshot aspect, store size rules,
large aspect deviations. ADR 0002 keeps loading lenient (dangling
references load) and leaves reporting to `core::validate`; ADR 0004 makes
resolve collect infos/warnings as diagnostics; ADR 0003 makes layout
report overflow and emergency breaks. CLI (`validate`, exit code), MCP and
the GUI need one report with stable codes. Task 1.8.

## Decision

### API

- `core::validate::validate(project, &ValidateOptions) -> ValidationReport`.
  `ValidateOptions`: catalog, `AssetLookup` (existence + dimensions),
  `FontLibrary` (bundled + project fonts), optional target and language
  filters, `ValidateMode` (`export` | `preview`) and `Thresholds`.
- `ValidationReport { findings: Vec<Finding> }`, `Finding { level, code,
  target, lang, page, element, message }` (serde both ways; `None` fields
  omitted). `Severity` = `info` | `warn` | `error`; `error` blocks export
  (CLI exits non-zero). `FindingCode` serializes as stable snake case
  (`FindingCode::as_str`, `FindingCode::ALL`); new codes may be added,
  existing ones are not renamed.
- Order: targets in project order, languages in target order, findings in
  the order they arise (resolve diagnostics, then elements in painting
  order). Exact duplicates are dropped.
- `lang` is set only for language-dependent findings (texts, screenshot
  lookup). Reference problems, store rules, geometry and screenshot
  aspect/upscale (which depend on the file, named in the message) carry
  no language, so they appear once per target, not once per language.
- `page` is the element's home page (0-based).

### How it runs

For every target (filtered) and each of its languages (`langs`, default
all available; filtered): `resolve` → map its diagnostics → lay out every
text with `layout::layout_text` (the same helper the renderer uses) → per
element checks. Store rules and the anchor check run once per target on
the first resolved frame. A target with an unknown strip is reported and
skipped.

### Levels (decisions)

- Reference problems keep resolve's level, except where the output is
  visibly broken: unknown device model (element missing), unknown
  `@colour` (magenta), home page out of range → **error**.
- Text missing in the language and all its fallbacks → **error** (nothing
  is rendered); message says whether the key exists anywhere.
- Text fallback: to the **default language → warn** (not translated); to a
  prefix (`de-AT` → `de`) or a configured `languages.fallback` → **info**
  (deliberate).
- Text overflow (lines, height, width) → **error**, once per element and
  language. Emergency break, unbalanced markup, unknown span style/font,
  missing glyphs → warn.
- Text in a gap: the laid-out content rect (rotated with the element,
  intersected with its clip rect) touching a gap of the target → warn.
  Text cut off by `clip: page` → warn `text_clipped`. Devices, shapes and
  images in gaps are intended and not reported.
- Screenshot missing → **error** in `export` mode, warn in `preview` (the
  preview shows a placeholder). Fallback screenshot → info
  ("`fr` uses the default `en` screenshot for `planner`").
- Screenshot aspect vs device display (fixed-aspect models only) beyond
  `Thresholds::screenshot_aspect` (2 %, `catalog::ASPECT_TOLERANCE`) →
  warn ("wrong device?"). Free-aspect models adapt (ADR 0004).
- Upscaling: display size in export px (element size × display fraction ×
  viewport scale) vs screenshot px; factor per `fit` (`cover`/`stretch`:
  max of both axes, `contain`: min) above `Thresholds::max_upscale`
  (**1.15**, `DEFAULT_MAX_UPSCALE`) → warn.
- Store rules (catalog `rules`, found via presets with the target's store
  slot): format not accepted → error; size → error (`exact`: one of the
  presets' sizes; `range`: min/max side, long ≤ `max_ratio` × short,
  aspects); more images than `count.max` → error, fewer than `count.min`
  → warn (Google's minimums are per listing / for featuring). Unknown slot
  value → warn (no rules checked); a slot known only from unverified
  presets (`APP_IPHONE_DUO`) → info. Alpha: exports never have an alpha
  channel, so `alpha: false` holds by construction.
- Anchors: a pages target without variant whose page (column) height
  deviates from the strip's by more than `Thresholds::variant_aspect`
  (**15 %**, `DEFAULT_VARIANT_ASPECT`) → info suggesting a variant.
- An element whose (rotated) bounds touch no page → warn.
- Catalog-unknown `screenshots` variant key on a target → info (custom
  folders are allowed).

### Assets

`AssetLookup::dimensions` (ADR 0004) provides screenshot sizes; with
`None` the aspect and upscale checks are skipped. `FsAssets` reads only
the image header (`imagesize`).

## Consequences

- Front ends only format the report; they never re-implement checks.
- Validation lays out every text of every target × language; for large
  projects the GUI may run it in the background or filter by target.
- Store rule changes are data edits in `targets.yaml`; validation follows.

## Open questions

- Per-store "required set present" checks (e.g. an iPhone 6.3″ target
  exists when any iPhone target exists) are not done yet.
- Text overflow of `auto_height` margin boxes past the page bottom is
  reported as `text_clipped`, not `text_overflow`.
- Image elements (`kind: image`) are not checked for upscaling yet.
