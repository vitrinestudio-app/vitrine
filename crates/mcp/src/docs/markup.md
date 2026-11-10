# Text markup, auto-fit and hyphenation

Translations (strings_set values, `i18n/<lang>.yaml`) may contain inline
markup. Keep the markup when translating.

| Source | Meaning |
|---|---|
| `**bold**` | bold (bolder than the style's weight) |
| `_italic_` | italic; `_` inside a word (`snake_case`) stays literal |
| `[name]…[/]` | named span style from `styles.spans` (colour, weight, highlight …); may nest |
| `\n` or a newline | hard line break |
| `\-` (or U+00AD) | soft hyphen: optional break point, drawn as `-` only when used |
| `\*` `\_` `\[` `\]` `\\` | the literal character |

Unbalanced markers are drawn literally and reported (`invalid_markup`).
Example: `Plan your garden [accent]in minutes[/]` or
`Gemüse\-beet\-planung **leicht** gemacht`.

## Auto-fit
`fit: {shrink_to: 52, max_lines: 3}` on a text element: the font starts at
`font_size` (or the style's size) and shrinks down to `shrink_to` (lu) until
the text fits the box and `max_lines`. If it still does not fit,
`text_overflow` is reported (layout_report shows the chosen size and lines).

## Long words
- Prefer a soft hyphen `\-` in the translation at a good break point.
- `hyphenate: true` (element or text style) enables automatic pattern
  hyphenation, used only when the text does not fit at `shrink_to`.
- A word broken without a hyphen is reported as `emergency_break`.

## Fixing overflows
Shorten the text, add `\-`, widen the box (smaller `left`/`right` margins –
per language with element_update {scope: "lang"}), lower `shrink_to`, or
raise `max_lines`. Check with layout_report and render_preview in that
language.
