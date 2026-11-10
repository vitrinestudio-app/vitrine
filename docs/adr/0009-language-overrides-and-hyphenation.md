# 0009 – Per-language overrides, font fallbacks and automatic hyphenation

- Status: accepted
- Date: 2026-11-10

## Context

Phase 3 (tasks 3.1, 3.2) covers what KONZEPT §5.1 promises beyond
translations: a text element may need other values in one language
(smaller size for Finnish, a wider box for German), scripts the
headline font lacks need fallback fonts, and long compounds should be
hyphenated automatically instead of being broken anywhere. ADR 0003
deferred pattern hyphenation (opt-in, only when auto-fit at `shrink_to`
is not enough – decided with the user) and per-language font
overrides. Task 3.6 adds Phase 2 follow-ups that touch the same model
(page lock, style renames).

## Decision

### Language overrides (`langs`)

```yaml
- id: title
  kind: text
  text: hero.title
  style: headline
  box: {top: 140, left: 80, right: 80}
  fit: {shrink_to: 52, max_lines: 3}
  hyphenate: true
  langs:
    de: {style: sub, box: {top: 120, left: 60, right: 60}, font_size: 64}
    fi: {font_size: 60, hyphenate: false}
```

- `langs` exists on **text elements only** (images have language
  variants of their assets; devices their screenshots). It is a map
  language → `ElementPatch` (same syntax as variant overrides; since
  2026-11-10 without `null`, see the amendment), written after all other
  fields, sorted by language.
- Allowed fields (`model::LANG_OVERRIDE_FIELDS`): `at`, `width`,
  `height`, `anchor_y`, `style`, `box`, `align`, `valign`, `fit`,
  `color`, `font_size`, `stroke`, `glow`, `shadow`, `hyphenate`. Typography,
  box and fit are the real needs; the size and position of a free box
  (`at`, `width`, `height`) are included because a longer language often
  needs a wider or moved free box, which a margin box gets via `box`.
  Not allowed: `page`, `lock_page`, `rotation`, `pivot`, `z`, `clip`,
  `opacity`, `visible`, `visible_in` (layout decisions shared by all
  languages; `visible_in.langs` already restricts visibility) and the
  text key (that is what translations are for). Loading rejects other
  fields and empty overrides; an override must give a valid element.
- **Resolution order:** base → variant override → language overrides.
  The overrides of the language's prefixes apply first, the most
  specific last: for `de-AT` the `de` override, then the `de-AT` one
  (same prefix rule as `visible_in.langs`; explicit
  `languages.fallback` entries are not followed – an override belongs to
  a language, not to the text it falls back to).
- **Ops:** `SetLangOverride {element, lang, patch | null}` (setting
  needs an available language; removing works for any) and
  `ResetLangOverrideField {element, lang, field}`; both return their
  inverse. References inside overrides (styles, colours, shadows) are
  checked like the element's own and keep styles from being removed.
  `RemoveLanguage` takes the overrides of exactly that language along;
  its inverse `AddLanguage` carries them (`lang_overrides`) and restores
  them. Removing a whole element removes its overrides with it.
- **Validation:** an override for a language outside
  `languages.available` is a `lang_not_available` warning; unknown
  styles/colours inside an override surface through resolve for the
  languages that use it; an override that does not apply is an
  `invalid_override` warning.

### Edit scope

`EditContext.scope` (`all` default, `lang`): in `lang` scope every field
write to a **text** element goes into its override for the view
language (merged into the existing one), others are edited as usual.
This covers inspector patches and canvas gestures alike, because the
position solver works on the resolved scene: a drag moves the box of
this language only. The home page stays (shared), rotation/pivot/page
writes are refused (`Unsupported`). `reset_lang_override(id, field)`
resets the field in the most specific override that sets it;
`lang_overridden_fields` lists the fields per view language. The GUI
passes `scope` with the view context (inspector toggle "Only for
<lang>") and marks language-overridden fields in teal (variant
overrides stay purple), each with its own reset.

### Font fallbacks

```yaml
styles:
  fonts:
    Inter: {fallback: [Noto Sans JP, Noto Sans SC]}
```

- `styles.fonts.<family>.fallback`: families tried in order for
  characters `family` lacks, before the default family Inter. Family
  names are free text (spaces allowed, no control characters, not
  itself).
- The project's map travels with each resolved text
  (`ResolvedText::font_fallbacks`) into `TextInput::fallbacks`; layout
  builds the chain `family → project fallbacks → library fallbacks
  (FontLibrary::set_fallbacks) → Inter → system fallbacks`. Passing it
  per text instead of mutating the shared `FontLibrary` keeps layout a
  pure function of project + fonts, so ops, undo and concurrent
  renderers need no font-library bookkeeping.
- Fallback families must be installed in `fonts/`; a missing one is
  reported **once** (`unknown_font` without target/language). Characters
  that no font in the chain has stay `missing_glyphs` per language.
- **System fonts** (app setting `system_font_fallback`, default off):
  `FontLibrary::add_system_fallbacks(text)` scans the platform font
  folders with fontique (`load_fonts_from_paths`, no system-font
  feature, no C dependency), and greedily adds the families covering
  most of the characters no loaded font has (≤ 8; hidden `.` families
  and `LastResort` skipped) as last-resort families. The GUI session
  loads them when the project opens or the setting is switched on, and
  again when an edit adds uncovered characters. Tests, CLI, MCP and the
  golden images never use system fonts.

### Automatic hyphenation

- **Library: own Liang implementation** (`core::layout::hyphen`, ~150
  lines) over the hyph-utf8 plain-text patterns, embedded with
  `include_str!` and compiled into hash maps on first use per language.
  The `hyphenation` crate was considered: it bundles all ~70 hyph-utf8
  languages with `embed_all` (licences we would have to review one by
  one, and several MB), otherwise loads dictionaries from files at run
  time, and adds `fst`/`bincode` dependencies. Our own code lets us
  bundle exactly the languages whose licence we checked.
- **Bundled sets** (licences in `assets/hyphenation/LICENSES.md`):
  `en-us` (permissive notice), `en-gb` (MIT), `de-1996` (MIT,
  Trennmustermannschaft), `fr` (MIT), `es` (MIT), `it` (MIT of
  LPPL/MIT), `nl` (MIT), `pt` (BSD-3), `da` (MIT of LPPL/MIT), `nb`
  (permissive notice; also used for `no`), `fi` ("may be freely
  distributed"). `sv` is LPPL 1.2 only and is **not** bundled until the
  project licence is decided. Mapping: `en` → `en-us`, `en-GB`/`-AU`/
  `-NZ`/`-IE`/`-ZA`/`-IN` → `en-gb`, `de-*` → `de-1996`, `pt-*` → `pt`,
  otherwise the primary subtag. Size: ≈ 0.7 MB of pattern text in the
  binary.
- **Opt-in:** `hyphenate: true|false` on text styles, text elements and
  language overrides (element ← style, default off).
- **When:** only in the auto-fit fallback. The fit loop is: style size →
  binary search down to `shrink_to` → if it does not fit at `shrink_to`
  and hyphenation is on (and patterns exist for the text's language),
  soft hyphens are inserted at the pattern positions of every word with
  ≥ 6 characters (`MIN_WORD_CHARS`), at least 2 characters before and 3
  after a break (`MIN_LEFT`, `MIN_RIGHT`), and the text is laid out again
  at `shrink_to` → if that still does not fit, the emergency break of ADR
  0003 runs on the hyphenated text (overflow as before). Larger sizes are
  not retried with hyphens: shrinking always wins.
- Words that already contain a soft hyphen (`\-`) are not touched, so
  manual breaks keep priority. Inserted breaks behave exactly like soft
  hyphens (visible hyphen glyph, room reserved at the line end).
- The language is the one the text came from (`text_lang`, so a German
  fallback text is hyphenated as German).
- **Report:** `TextDiagnostic::Hyphenated { words }` (words actually
  broken at an inserted hyphen) → validation `hyphenated` (info);
  `hyphenation_unavailable` (info) when an overflowing text has
  `hyphenate` on but its language has no patterns. The overlay lists the
  words (`OverlayText::hyphenated`) for the inspector.

### Phase 2 follow-ups

- `lock_page: true` (all element kinds, written after `page`): edit
  intents keep the home page when dragging (`MoveOptions::keep_page`
  defaults from it). It replaces the UI-only lock of Phase 2 and may be
  set in variant overrides.
- `RenameStyle {kind: color|text|span|shadow, from, to}` renames a style
  token and every reference in one undoable op: styles, elements,
  variant overrides, language overrides, strip backgrounds, and for span
  styles the `[from]` tags in all translations (escaped `\[from]` stays).
  A name already in use is refused; a built-in shadow name may be taken
  over when the project does not define it yet.

## Consequences

- One element can look different per language without a variant per
  language; the YAML stays readable (`langs` block at the end).
- Language overrides do not compose with variants per language (an
  override applies in every variant); a variant × language matrix was
  not needed so far.
- Hyphenation quality is that of the TeX patterns (good for German
  compounds, conservative elsewhere); exceptions files are used where
  hyph-utf8 ships them.
- Adding a hyphenation language means a licence check and one line in
  `hyphen.rs`.
- With system fonts on, a project may render differently on another
  machine; validation still reports characters no project font covers.

## Amendment 2026-11-10: overrides hold no `null`

A `null` in an override meant "reset this field to the kind's default in
this language/variant" – never what an override is wanted for, and the
MCP tools wrote it when asked to *reset an override* (`langs.en: {at:
null}` moved an English title to (0, 0)). Now:

- Overrides (variant and language) only set values. "Back to the base" is
  the absence of the field.
- Edits in a variant or language scope treat a `null` field as "remove it
  from the override" (`Editor::patch`; an override left empty disappears).
- `SetVariantOverride`, `SetLangOverride`, `AddElement` (`langs`,
  `overrides`), `AddStrip`, `AddLanguage` drop `null` fields from override
  patches (`ElementPatch::drop_nulls`).
- Loading drops `null` fields inside overrides of a hand-written file
  (`Project::drop_override_nulls`, recorded in
  `Project::dropped_override_nulls`); `validate` reports each as info
  `override_null` until the next save writes the file without them.
- Lost on purpose: switching a field *off* only for one variant/language
  by `null` (e.g. no shadow on iPad). Use an explicit value instead
  (`shadow: none`, `stroke.width: 0`, `glow.opacity: 0`, `visible: false`).
