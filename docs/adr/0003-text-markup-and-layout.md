# 0003 – Text markup and layout

- Status: accepted
- Date: 2026-11-09

## Context

`docs/KONZEPT.md` §3.5 defines minimal inline markup for translated texts,
§3.6 text and span styles, §5.1 auto-fit per language and §6.1 the
pipeline split: `core::layout` shapes, wraps and auto-fits; `render` only
rasterises. ADR 0001 chose parley for layout and skrifa outlines drawn with
tiny-skia. The Phase 0 spike kept this code in `arrangia-render`, so layout
reports and validation would have needed the rasteriser. Hyphenation was
decided with the user: soft hyphens now, a warning for emergency breaks,
automatic pattern hyphenation later (Phase 3, opt-in).

## Decision

### Crate split

- `arrangia_core::markup` – the parser.
- `arrangia_core::layout` – `FontLibrary`, `TextLayouter`, `TextLayout`,
  `LayoutReport`. Depends on parley + skrifa, no pixels.
- `arrangia_render::text` – `TextLayout` glyph runs → skrifa outlines →
  `tiny_skia::Path`s, drawn with the caller's transform (rotation,
  lu → px). cosmic-text is removed from the workspace.

### Markup grammar

| Source | Meaning |
|---|---|
| `**…**` | bold = CSS `bolder` of the current weight (≤ 349 → 400, ≤ 549 → 700, else 900) |
| `_…_` | italic. `_` opens only if not preceded by a letter/digit and not followed by whitespace, closes mirror-wise (CommonMark flanking), so `snake_case` stays literal |
| `[name]…[/]` | span style `styles.spans.name`; names are `[A-Za-z0-9_-]+`; may nest, innermost wins; `[/]` closes the innermost |
| `\n`, real newline, `\r\n` | hard line break |
| `\-`, U+00AD | soft hyphen |
| `\*` `\_` `\[` `\]` `\\` | literal character; any other `\x` stays as written |

`[` not followed by a valid name and `]` is literal without diagnostic.
Bold and italic pairs are matched independently (1st with 2nd marker …),
so they may overlap. **Unbalanced markers** (`**`, `_`, `[name]`, `[/]`)
are rendered literally and reported (`TextDiagnostic::Markup`); the
parser never fails or panics (proptest). Unknown span names are reported
(`UnknownSpanStyle`) and render unstyled.

### Soft hyphen and emergency break

- A soft hyphen is invisible (harfrust maps it to an empty zero-advance
  glyph) and is a break opportunity (UAX #14 class BA).
- If a line ends at a soft hyphen, a hyphen glyph (`-`, else U+2010) of
  the line's last font is drawn at the line end. The line must fit
  *including* the hyphen: if it does not, that line is re-broken with
  `width − hyphen advance` (parley `BreakLines` with per-line max
  advance; repeated until stable, bounded).
- Without opportunities a word that is wider than the box is broken
  anywhere (parley `OverflowWrap::Anywhere`), but **only** as the last
  resort at `shrink_to` (see below). Each such break is reported as
  `EmergencyBreak { word, line }` (0-based line); `core::validate` (1.8)
  turns it into a warning. It is not an overflow by itself.
- No automatic pattern hyphenation (Phase 3, opt-in, only used when
  auto-fit at `shrink_to` is not enough).

### Auto-fit

Input: style size `S`, `fit.shrink_to` `m` (default `S`), `fit.max_lines`,
box width and optional height. A layout *fits* if it has ≤ `max_lines`
lines, is not taller than the box and no line is wider than the box
(normal wrapping, no emergency breaks).

1. Lay out at `S`; if it fits, done.
2. Lay out at `m`; if it does not fit, lay out at `m` with emergency
   breaking → result, `overflow` set if lines/height/width still exceed
   (`OverflowLines` / `OverflowHeight` / `OverflowWidth`).
3. Otherwise binary search the largest `k` with `m + k · 0.25 lu` fitting
   (`k = n + 1` stands for `S`, known not to fit). The result is always a
   layout that was actually checked, so non-monotonic cases cannot return
   a non-fitting size. ≈ 7 layouts for 104 → 56 lu; 4 German headlines
   take ≈ 0.75 ms (M1 Pro, release).

Line height is relative to the chosen size; tracking (em) is scaled too.

### Alignment and geometry

All values in lu relative to the box's top-left. Horizontal alignment is
done by core (not parley) so the soft-hyphen glyph counts: per line
`x = f · (box width − visible width)`, `f` = 0 / 0.5 / 1, clamped at 0
(overflowing lines start at the left edge, like CSS). Vertical
alignment applies only with a box height, `y = f · (box height − text
height)`, also clamped at 0. Highlights cover the span's advance from
ascent to descent and are merged across adjacent runs.

### Fonts

- `FontLibrary::bundled()` embeds `assets/fonts/` (Inter 400/600, Inter
  Display 700/900) with `include_bytes!` – no path lookup, identical on
  every platform. System fonts are never loaded (`system_fonts: false`).
- `load_dir(project/fonts)` adds `.ttf/.otf/.ttc/.otc` in file-name
  order; bad files are reported, the rest is loaded. Metadata (family,
  weight, style) is read by fontique, the same code that matches faces
  in layout.
- Matching is CSS-like (fontique); `FontLibrary::match_face` mirrors it
  for tools/validation. Each family gets the chain
  `family → set_fallbacks(family) → Inter`. Unknown families are reported
  (`UnknownFont`), characters without glyph as `MissingGlyphs`. Real CJK
  fallback fonts are a later task; the list is the hook.
- **Italic: synthesised oblique**, no Inter Italic bundled. fontique
  requests a 14° skew when no italic face exists; render shears the
  outlines. This keeps the bundle at four faces (≈ 1.6 MB) and italic is
  rare in store headlines; projects that need a true italic put it into
  `fonts/` and it is matched automatically. Synthetic bold (weight more
  than 200 above the best face) is drawn as an extra outline stroke of
  `size / 32`.

### Layout report

`LayoutReport` (serde): box, content rect, chosen font size, line count,
displayed lines (soft hyphens resolved), overflow flag, diagnostics.
Values are rounded to 0.01 lu for stable snapshots;
`translated(dx, dy)` moves it to page coordinates. Callers key it by
element, language and target (MCP `layout_report`, validation).

## Consequences

- Validation and MCP can lay out text without linking the rasteriser.
- Spike headlines lose the spike's line balancing (`text-wrap: balance`);
  it may come back as a style option later.
- Deferred to Phase 3 or later: pattern hyphenation (opt-in), line
  balancing, underline/strike-through markup, RTL/complex scripts
  (parley `complex-scripts` is off), CJK fallback fonts, text shadows,
  per-language font overrides.
- `docs/spike/text-layout.png` (spike example) shows auto-fit, soft
  hyphen, emergency break, overflow, italic and highlights.
