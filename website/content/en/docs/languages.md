---
title: Languages & translation
description: "Add languages, understand fallbacks, translate in the table or with translators (CSV, XLIFF), fit long texts, use markup and per-language overrides."
weight: 80
icon: languages
---

## Adding languages

**Project settings → Languages** (or **Add language…** at the end of the
Language picker): type a code such as `fr`, `pt-BR` or `fr-CA` and **Add**.
Codes are BCP 47 language tags; regional codes are fine.

- **Make default** sets the default language – the source for translators and
  the last fallback of every other language. When you change it, the dialog
  shows which texts and screenshots would lose their source.
- The order of the list is the order in pickers and exports (**Move up**,
  **Move down**).
- **Remove language** deletes the language and its texts (after a question).

## Fallbacks

Everything a language doesn't have comes from another one, so a partial
translation still renders:

1. A **regional language** falls back to its base language automatically:
   `fr-CA → fr`, `de-AT → de`, `pt-BR → pt`. Translate only the keys that
   differ – the example project's `fr-CA` has just two Québec texts.
2. Optionally a language's own **Fallback** (e.g. `nl → de`), set in the
   language list.
3. Finally the **default language**.

Validation tells you where texts come from: a text from the **default
language** is a warning ("not translated", and `vitrine validate --strict`
fails), one from a regional or configured fallback is just an info. A text
that exists in no language of the chain is an error (`text_missing`).
Screenshots from any fallback are an info – a partial screenshot set per
language is fine.

**Assets → Screenshots** shows per language which images are borrowed:
*From en (fallback)*, *From de (default language)*; hover a language for its
full fallback chain.

## Translating in the app

- **On the canvas:** switch the **Language** picker and double-click a text.
  A text that's missing shows the fallback with a note *No text in this
  language yet*; typing creates the translation.
- **Translations tab:** one row per key, one column per language. Click a
  cell (or <kbd>Enter</kbd>) to edit it, <kbd>Enter</kbd> applies,
  <kbd>Shift</kbd>+<kbd>Enter</kbd> breaks the line, <kbd>Esc</kbd> cancels.
  **Search keys and texts**, **Missing / problems only**, **Add translation
  key**, and per key **Rename key**, **Delete key** and *Used by* (which
  elements show it). Overflowing texts are marked in their cell.
- **All languages** view: every language next to each other, to spot texts
  that are too long.

{{< placeholder "The Translations tab: keys × languages, a missing German text shown from the fallback, an overflowing French cell." >}}

A text element shows the text of its **Key** (e.g. `plan.title`). Several
elements can share one key. Changing the key in the inspector to a new name
offers to copy the current text to it.

## Working with translators: CSV and XLIFF

In the **Translations** tab, **Export…** writes files for translators,
**Import…** reads the translated files back.

**Export** options: *Format* (*CSV*, *XLIFF 2.0*, *XLIFF 1.2*), the target
*Languages* (the default language is the source), *Only missing
translations*, *Context for translators* (which elements and pages use a
text, the box width and an approximate maximum length), for CSV the
delimiter (*Semicolon ( ; ) – Excel* for locales with a decimal comma) and
*UTF-8 marker for Excel*; for XLIFF *Formatting* as protected tags (default,
safe in CAT tools) or as plain text. XLIFF writes one file per language.

**Import** reads CSV (Excel variants are recognised) and XLIFF and shows a
preview before anything changes: new, changed and removed translations
(*Before* / *After*), unknown keys (skipped unless *Create unknown keys*),
formatting problems, and the **layout after the import** – which texts
would overflow. **Apply** makes it one undo step. *Empty cells*: *change
nothing* or *remove the translation*.

On the command line:

```sh
vitrine strings export --format xliff20 -o translations/      # one file per language
vitrine strings export --format csv --lang fr --missing-only --context -o fr.csv
vitrine strings import translations/*.xlf --dry-run           # report only
vitrine strings import translations/*.xlf                     # apply
```

A sparse regional language (`fr-CA`) gets all the keys it lacks with
`--missing-only`. Translating with Claude works too – see
[AI assistants](../ai-mcp/).

## Fitting long texts

Every text has a box – margins to the page edges, or a free box. **Auto-fit**
shrinks the font from its size down to **Shrink down to** (lu) and up to
**Maximum lines** until the text fits. The inspector shows the result under
*Laid out* (font size, lines, *Fits*).

If a text doesn't fit even at the minimum, it is an **overflow**: an error
(`text_overflow`) for that language and target only – the other languages
still export. Fix it by:

- shortening the translation;
- adding a **soft hyphen** `\-` at a good break point (it only shows as "-"
  when the line breaks there): `Gemüse\-beet\-planung`;
- switching on **Hyphenation → Automatic** (or in the text style) – used only
  when the text doesn't fit at the minimum size;
- giving the language a wider box or a different minimum with a **language
  override** (below);
- lowering *Shrink down to* or raising *Maximum lines*.

A word broken without a hyphen is reported as a warning
(`emergency_break`); texts that were hyphenated automatically are listed as an
info (`hyphenated`) so you can check them.

**Bundled hyphenation patterns:** English (US, GB), German, French, Spanish,
Dutch, Italian, Danish, Portuguese, Norwegian (Bokmål), Finnish. Swedish is
missing (its patterns' licence is not compatible).

## Language overrides

A **language override** changes a text element for one language only – the
long German headline gets a wider box, French a smaller font. Select the text,
switch the Language picker to the language, and tick **Only for de** in the
inspector: edits of that text then go into the override. Overridden fields are
marked **teal**; *Reset language override* removes one field, *Reset all* the
whole override.

Allowed in an override: position, size, anchor, style, box, alignment, auto-fit,
colour, font size, outline, glow, shadow and hyphenation. Geometry like rotation
and home page stays shared. `de` overrides also apply to `de-AT`.

```yaml
- id: title-companions
  kind: text
  text: companions.title
  style: headline
  box: {top: 170, left: 70, right: 70}
  fit: {shrink_to: 64, max_lines: 2}
  langs:
    de: {box: {top: 170, left: 40, right: 40}, fit: {shrink_to: 76, max_lines: 2}}
```

Tip from the example: when a long German compound makes the headline shrink
too much, raise `shrink_to` for German so hyphenation kicks in earlier
("Mischkultur-partner"), and widen the box if the break lands badly.

## Text markup

Translations may contain markup; keep it when translating:

| Write | Result |
|---|---|
| `**bold**` | bold |
| `_italic_` | italic (`_` inside a word stays literal) |
| `[accent]…[/]` | a span style from the project (colour, weight …) |
| `\n` or a real line break | line break |
| `\-` | soft hyphen |
| `\*` `\_` `\[` `\]` `\\` | the character itself |

Unbalanced markers are drawn literally and reported (`invalid_markup`).

**French typography:** put a no-break space (U+00A0) before `:` `;` `?` `!` and
inside « guillemets », and between words that must stay on one line. Line
breaking respects it.

**Editing the YAML by hand:** a translation containing `: ` (colon + space) or
starting with `"`, `[` or `*` must be quoted. The app and the import write
correct YAML for you.
