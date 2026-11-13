---
title: Styles & fonts
description: "Project colours, text styles, span styles and shadows; bundled, project and installed fonts, storing fonts in the project, supported scripts (Arabic, Hebrew, Indic, Thai, CJK), outlines and glows."
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
*Hyphenate automatically when needed*. In the inspector you pick a style for
a text element (*Style*) and can override single values (colour, font size,
effects) for just that element.

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

A highlight can become a pill or a marker: **padding** (one value or
`{x, y}`, in lu) grows the box around the words, **radius** rounds its
corners – a radius of at least half the box height gives a pill. The padding
never moves the text or changes line breaks, so leave a space next to the
pill. Words that wrap get one box per line. In the app: Project settings →
Styles → span style → *Highlight box*.

```yaml
styles:
  spans:
    tag: {color: "#FFFFFF", weight: 800, background: "@accent",
          padding: {x: 14, y: 2}, radius: 40}
```

## Shadows

**Shadow presets** are built in (`soft`, `hard`, `none`) or your own: one or
more layers, each with colour, blur and offset. Devices, images, shapes and
texts use presets by name. Shadows of transparent images follow their shape
(like CSS `drop-shadow`).

## Outline, glow, shadow on texts

Texts can have an **Outline** (width in lu, *Outside* or *Centred*), a
**Glow** (radius, opacity, spread) and a **Shadow**. Set them on the text style,
on the element or on a span style. The text style provides the base values,
an effect set on the element replaces the style's, and a span style applies to
its marked words ahead of both – the most specific wins (span before element
before text style). To switch off an inherited effect, set outline width 0, glow opacity 0
or shadow *none*. Effect sizes are in lu and don't shrink with auto-fit.

## Bundled fonts

These families are built in – always available in the app, the CLI and for AI
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

More bundled families cover other scripts and are used automatically –
see [Supported scripts](#supported-scripts).

`vitrine fonts` lists the families of a project (`--installed`: also every
font installed on the computer).

## Fonts installed on your computer {#system-fonts}

The font pickers list three groups and have a search field:

1. **Vitrine Studio** – the bundled families,
2. **This project** – the fonts in the project's `fonts/` folder,
3. **Installed on this computer** – every font family installed on your Mac
   or PC.

Pick any of them. A family name is looked up in this order: **bundled →
project → installed**, so a bundled family always wins over an installed one
with the same name. Installed fonts work everywhere on your computer: in the
preview, in exports, in the CLI (`vitrine export`) and for AI assistants (MCP).

They are switched on by **App settings → Use installed fonts** (on by
default). Switch it off to use bundled and project fonts only – then the
output is the same on every machine. The CLI has `--no-system-fonts`, and
`VITRINE_SYSTEM_FONTS=off` switches them off for any process (e.g. on a build
server). Installed a font while the app is running? **Project settings →
Fonts → Rescan installed fonts**.

{{< callout note >}}
Another computer – a build server (CI) or a colleague – may not have the same
fonts. There, texts in a missing family fall back to Inter. Validation points
this out (`system_font_used`, info). Store the fonts in the project (next
section) or use bundled fonts for projects that are exported elsewhere.
{{< /callout >}}

## Store used system fonts in the project {#store-system-fonts}

**Project settings → Fonts → Store used system fonts in the project** copies
the font files of the installed families the project uses into
`fonts/system/`. A build server then renders exactly what your computer
renders. Only the files of the weights your styles use are copied; the list
shows each file with its size and the licence the font states (vendor, OFL
or not). **Update stored fonts** refreshes them, **Remove from project**
deletes the folder. On the command line:

```sh
vitrine fonts embed --dry-run    # what would be stored, with licences
vitrine fonts embed              # store / update
vitrine fonts unembed            # remove fonts/system/
```

{{< callout warn >}}
**Check the licence before storing.** Fonts under the SIL Open Font License
may be shared. The system fonts of macOS and Windows (San Francisco, New York,
Helvetica Neue, PingFang, Hiragino, Segoe UI …) and most commercial fonts may
only be used on your computer and **must not be redistributed**. Keep a
project with such fonts in a **private** repository and never publish it with
`fonts/system/` – no public repository, no template, no gallery upload, no
shared archive. For a public repository add `fonts/system/` to `.gitignore`.
Templates never contain this folder.
{{< /callout >}}

Validation reminds you of stored fonts (`system_fonts_stored`, info).

## Your own fonts

Put `.ttf` or `.otf` files into the project's **`fonts/`** folder (or use
**Assets → Import…** with *Font (fonts/)*). They are picked up automatically
and appear in the font pickers with their family name. Because they travel
with the project, the export looks the same on every computer.

## Supported scripts

Text layout shapes every script with HarfBuzz rules (in Rust): Arabic letters
join, Indic consonants form conjuncts and vowel signs move into place,
right-to-left text runs right to left, Thai is broken into lines at word
boundaries (it has no spaces), Chinese and Japanese break between characters
with the usual rules (no line starts with `。` or `、`).

| Script | Languages (examples) | Font |
|---|---|---|
| Latin, Cyrillic, Greek | most European languages | bundled families (see the table above) |
| Arabic | Arabic, Persian, Urdu | **Noto Sans Arabic** (bundled, automatic) |
| Hebrew | Hebrew | **Noto Sans Hebrew** (bundled, automatic) |
| Devanagari | Hindi, Marathi, Nepali | **Noto Sans Devanagari** (bundled, automatic) |
| Bengali | Bengali, Assamese | **Noto Sans Bengali** (bundled, automatic) |
| Tamil | Tamil | **Noto Sans Tamil** (bundled, automatic) |
| Thai | Thai | **Noto Sans Thai** (bundled, automatic) |
| Chinese, Japanese, Korean | zh-Hans, zh-Hant, ja, ko | an **installed** font, or a font in `fonts/` |

The bundled script fonts step in for characters the style's font doesn't
have – your headline font stays for Latin text and digits. You can also pick
them directly (category *script font*).

**Chinese, Japanese and Korean** fonts are 10–20 MB each, so none is bundled.
With *Use installed fonts* on, each language gets an installed font made for
it – Japanese texts Hiragino Sans or Yu Gothic, Simplified Chinese PingFang SC
or Microsoft YaHei, Traditional Chinese PingFang TC or Microsoft JhengHei,
Korean Apple SD Gothic Neo or Malgun Gothic (macOS and Windows ship them; on
Linux install Noto Sans CJK). Han characters look different in Chinese and
Japanese, so the language decides. For identical exports on every machine,
put a font like *Noto Sans JP* into `fonts/` (OFL, free to share) or store the
used system fonts in the project.

{{< callout note >}}
Right-to-left layouts are not mirrored: alignment is physical (*left* means
left in every language). For Arabic and Hebrew set the alignment to *right*
with a [language override](../languages/).
{{< /callout >}}

## Other scripts: fallback fonts

Characters a family doesn't have come, in this order, from its
**fallbacks**, Inter, the bundled script fonts and – with *Use installed
fonts* – an installed font. To choose the fallback yourself (e.g. a Japanese
font that matches your headline font):

1. Put the font into `fonts/` (or install it), e.g. *Noto Sans JP*.
2. **Project settings → Fonts** → the family you use → **Add fallback family**
   → Noto Sans JP.

Validation reports characters no font can draw as a warning
(`missing_glyphs`) and
characters drawn with an installed fallback font (`system_font_fallback`,
info).

## Fonts in templates

Templates contain only fonts under the SIL Open Font License. When a project
font may not be shared, *Save as template* stops – or, with **Leave out fonts
that may not be shared** (`vitrine template save --skip-unlicensed-fonts`),
saves the template without it. The template then names the family as
*required, not included*; installed system fonts are always listed that way.
A project made from the template uses the font where it is installed and
shows Inter elsewhere – the app tells you which fonts are missing.
