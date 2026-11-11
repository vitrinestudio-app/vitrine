---
title: Languages & translation
description: "Fallback chains, per-language overrides, hyphenation, markup and CSV/XLIFF exchange with translators."
weight: 30
icon: languages
---

## Fallbacks

- Regional languages fall back to their base language automatically:
  `fr-CA → fr → default`. Translate only the keys that differ.
- Other chains go into `languages.fallback` (e.g. `fr → en`); the default
  language always comes last.
- A text taken from the default language is a **warning** ("not translated"),
  from a configured fallback an **info**. Screenshots may come from any
  fallback (info) – a partial screenshot set per language is fine.
- The Assets panel shows per language which screenshot comes from where.

## Per-language overrides

A language override changes a text element for one language only: font size,
box, style, fit, effects. `de-AT` inherits the overrides of `de`. Use it when
one translation is much longer than the others.

## Fitting and hyphenation

- Texts shrink to fit down to `shrink_to`; an overflow is an **error** for that
  language and target.
- Automatic hyphenation is opt-in (`hyphenate: true`) and only kicks in at
  `shrink_to`. Bundled patterns: English (US/GB), German, French, Spanish,
  Dutch, Italian, Danish, Portuguese, Norwegian, Finnish.
- A soft hyphen `\-` forces the break point; a no-break space keeps words
  together (also before `:` `;` `?` `!` in French).

## Markup

`**bold**`, `_italic_`, `[style]…[/]` for a named span style, `\n` for a line
break, `\-` for a soft hyphen.

## Working with translators

```sh
vitrine strings export --format xliff20 --lang fr -o translations/   # or csv, xliff12; --missing-only, --context
vitrine strings import translations/fr.xlf --dry-run                 # report: changed, new, unknown keys, overflows
```

CSV (Excel variants detected) and XLIFF 1.2/2.0. Markup becomes protected
inline tags so translation tools don't break it; the CSV context column names
element, page, style and an approximate maximum length. An import is one undo
step and is checked for overflowing text afterwards.

## Fonts and scripts

Eleven font families are bundled (SIL OFL) and always available. Characters a
family lacks come from its fallbacks (Project settings → Fonts), then Inter.
For CJK, Arabic, Hebrew or Thai put a font such as Noto Sans into `fonts/` and
add it as a fallback. Right-to-left layouts are not mirrored yet.
