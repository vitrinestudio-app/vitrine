---
title: Styles & fonts
description: "Project colours, text styles, span styles and shadows; the eleven bundled font families, your own fonts, fallbacks for other scripts, outlines and glows."
weight: 70
icon: fonts
---

Styles keep a set consistent: change the headline style once and every
headline on every canvas, in every language, follows. They live in **Project
settings → Styles**.

## Colours

Name your colours once (`accent`, `ink`, `paper` …) and use them everywhere as
`@accent` – in text styles, fills, outlines. Changing the colour changes every
use. Plain colours are `#RRGGBB` or `#RRGGBBAA` (with transparency).

## Text styles

A text style sets *Font*, *Weight*, *Italic*, *Size* (in lu), *Line height*,
*Tracking*, *Colour*, *Highlight* (a background behind the text), *Case* (*as
written*, *UPPERCASE*, *lowercase*), *Shadow*, outline and glow, and whether to
*Hyphenate automatically when needed*. Text elements pick a style in the
inspector (*Style*) and can override single values (colour, font size,
effects) for themselves.

Sizes are in lu: on a 1000 lu wide page, a 90 lu headline is 9 % of the page
width – on every device size.

## Span styles: colour a few words

Span styles change part of a text. Define one (e.g. `accent` with the colour
`@accent`), then mark words in a translation:

```yaml
companions.title: Know your [accent]plant partners[/]
```

In the inline text editor, select the words and use **Span style…**. Spans can
set colour, weight, italic, font, highlight and effects. `**bold**` and
`_italic_` work without a span style. See the
[markup reference](../languages/#text-markup).

## Shadows

**Shadow presets** are built in (`soft`, `hard`, `none`) or your own: one or
more layers, each with colour, blur and offset. Devices, images, shapes and
texts use presets by name. Shadows of transparent images follow their shape
(like CSS `drop-shadow`).

## Outline, glow, shadow on texts

Texts can have an **Outline** (width in lu, *Outside* or *Centred*), a
**Glow** (radius, opacity, spread) and a **Shadow**. Set them on the text style,
on a span style or on the element – the most specific wins (span > element >
style). To switch off an inherited effect, set outline width 0, glow opacity 0
or shadow *none*. Effect sizes are in lu and don't shrink with auto-fit.

## Bundled fonts

Eleven families are built in – always available in the app, the CLI and for AI
assistants, no files needed, all under the SIL Open Font License:

| Family | Category | Weights | Italic |
|---|---|---|---|
| Inter | sans serif | 400, 600 | slanted |
| Inter Display | sans serif | 700, 900 | slanted |
| Montserrat | sans serif | 100–900 | ✓ |
| Nunito | sans serif | 200–1000 | ✓ |
| Oswald | sans serif | 200–700 | slanted |
| Playfair Display | serif | 400–900 | ✓ |
| Lora | serif | 400–700 | ✓ |
| Bebas Neue | display (all caps) | 400 | slanted |
| Caveat | handwriting | 400–700 | slanted |
| Dancing Script | handwriting | 400–700 | slanted |
| JetBrains Mono | monospace | 100–800 | ✓ |

- Variable fonts take any weight in their range; a weight outside looks like
  the range end (Lora 900 = Lora 700). Inter and Inter Display come in the
  listed weights only; other weights use the nearest one.
- "Slanted" = no real italic; italic text is slanted synthetically.
- Handwriting fonts are best for short callouts.
- All cover Latin and Latin Extended. **Cyrillic:** Montserrat, Playfair
  Display, Lora, Nunito, Oswald, Caveat, JetBrains Mono. **Greek:** Inter and
  JetBrains Mono. **Vietnamese:** all but Bebas Neue and Caveat.

`vitrine fonts` lists the families of a project.

## Your own fonts

Put `.ttf` or `.otf` files into the project's **`fonts/`** folder (or use
**Assets → Import…** with *Font (fonts/)*). They are picked up automatically
and appear in the font pickers with their family name. Because they travel with
the project, the export looks the same on every computer – Vitrine Studio never
uses fonts from your system unless you allow it.

## Other scripts: fallback fonts

Characters a family doesn't have (Japanese, Arabic, Hebrew, Thai, Cyrillic in
Bebas Neue …) come from its **fallbacks**, then from Inter. No CJK, Arabic,
Hebrew or Thai font is bundled, so for those languages:

1. Put a font that covers them into `fonts/`, e.g. *Noto Sans JP*.
2. **Project settings → Fonts** → the family you use → **Add fallback family**
   → Noto Sans JP.

Validation reports characters no font can draw (`missing_glyphs`).

**System fonts** as a last resort: **App settings → System fonts → Use system
fonts as last resort**. Then fonts installed on your computer fill the gaps –
but preview and export depend on that computer. *Project settings → Fonts →
System fonts* shows whether any were needed.

{{< callout note >}}
Right-to-left layouts (Arabic, Hebrew) are not mirrored yet.
{{< /callout >}}
