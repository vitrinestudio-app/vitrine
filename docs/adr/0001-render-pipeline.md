# 0001 – Render pipeline (Phase 0 spike)

- Status: accepted
- Date: 2026-11-09

## Context

`docs/KONZEPT.md` §6 proposes a pure-Rust render pipeline (tiny-skia,
cosmic-text or parley, fontdb, image + fast_image_resize, png, own blur) and
requires that the canvas model — home pages, store gallery gaps, page
grouping, vertical anchors (§2, §3.2–3.5) — is proven before the data model
is built on top of it. The spike renders a hard-coded scene (no YAML):

- 4 pages, design aspect 1320:2868 (page = 1000 × 2172.7 lu);
- a canvas-wide linear gradient (pages **and** gaps) and a diagonal band +
  accent line anchored to page 1 that run across all pages;
- a phone with Dynamic Island on the right edge of page 0 (`x = 1000`,
  `anchor_y: bottom`, `rotation: -8°`, soft shadow), a second phone on page 2,
  a landscape tablet on page 3, each with a generated test screenshot;
- a two-line headline per page (en/de) with `[accent]…[/]` and `**bold**`
  spans, auto-fit and `clip: page`.

Code: `crates/render/src/{geometry,paint,blur,frames,raster,text,spike}`,
driver `cargo run -p arrangia-render --release --example spike`
(`--engine cosmic|parley`, `--runs N`, `--regen-assets`). Images go to
`docs/spike/`, generated screenshots to `assets/spike/`. (Phase 1 removed
the spike scene, composer and driver; the pipeline that replaced them is
described in ADR 0006. `docs/spike/` and `assets/spike/` are kept.)

## Decision

| Concern | Decision |
|---|---|
| Raster | **tiny-skia 0.12** (paths, gradients, patterns, masks, AA). Confirmed. |
| Text layout | **parley 0.12** (fontique + harfrust shaping + ICU4X line breaking). cosmic-text is dropped after the spike (see comparison). |
| Font handling | Only bundled/project fonts. parley: `Collection::new(CollectionOptions { system_fonts: false, .. })` + `register_fonts(Blob)`. Bundled: Inter 4.1 Regular/SemiBold and Inter Display Bold/Black (OFL, `assets/fonts/`, from the official rsms/inter v4.1 release; zip sha256 `9883fdd4…6b11e`). `fontdb` is **not** needed with parley (fontique replaces it). |
| Glyph rasterisation | Layout in **lu**, unhinted. Glyph outlines are read with **skrifa** (`outline_glyphs().draw(DrawSettings::unhinted(..))`) into one `tiny_skia::Path` per colour, positioned in lu, and filled like any vector shape with the element/view transform. No glyph atlas, no swash. Rotation/scale stay crisp; preview and export break lines identically because layout never sees pixels. |
| Shadows | Own 3-pass box blur (“boxes for Gauss”) on an alpha mask of the device silhouette. For large sigma the mask is rendered at 1/k resolution (k = ⌊σ/3⌋, max 8) and bilinearly upsampled while compositing directly into the target (own source-over loop; a transformed `draw_pixmap` was 20 ms+ slower per shadow). Two layers: ambient (σ 48 lu, dy 46 lu, 42 %) + contact (σ 10 lu, dy 10 lu, 30 %). |
| Resampling | Screenshots are decoded once (`image`), then resampled with **fast_image_resize Lanczos3** (`fit_into_destination` = cover) to the exact pixel size of the device screen and cached per (key, w, h). tiny-skia then maps them with a **bilinear** pattern that only handles rotation and sub-pixel scale (bicubic cost +30 ms/page for no visible gain). |
| PNG | `png` 0.18: 24-bit RGB (alpha flattened onto white — pages are opaque anyway), `sRGB` chunk (perceptual), `Compression::High`, adaptive filtering. **No oxipng**: it hard-depends on `libdeflater` (C). |
| Frames | Own parametric vector frames (`frames.rs`), styles `stylized` and `detailed`; no vendor artwork. Housing uses “continuous” squircle-like corners. |

### cosmic-text 0.19 vs parley 0.12

Both engines feed the same `RawLayout` → outline renderer, so rasterised
quality is the same by construction. Image: `docs/spike/text-engines_comparison.png`.

| | cosmic-text 0.19.0 | parley 0.12.0 |
|---|---|---|
| Shaping | harfrust 0.5 | harfrust 0.12 |
| Output / glyph positions | identical sizes, line counts and visually identical lines in all cases tested | (same) |
| Line breaking | UAX#14 via unicode-linebreak; `Wrap::Word` lets a too-long word overflow, `Wrap::WordOrGlyph` breaks inside it | ICU4X segmenter; `OverflowWrap::Normal` overflows, `OverflowWrap::Anywhere` breaks inside; plus CSS `word-break`, `line-break`, `text-wrap-mode` |
| Long German compounds | “Mischkulturempfehlungen” at 104 lu: overflows (1351/840 lu) with word wrap; emergency mode breaks “Mischkulturemp/fehlungen” without hyphen | identical |
| Inline spans (colour + weight) | `set_rich_text` with per-span `Attrs` | `RangedBuilder::push(prop, range)`; brush is a generic type (we use `[u8;4]`) — maps 1:1 to our markup |
| Overflowing line alignment | starts at the left edge | starts at the start edge (`align_when_overflowing: false`), configurable |
| Layout + auto-fit, 4 German headlines | ≈ 0.6–1.2 ms | ≈ 0.5–0.95 ms |
| API effort | Buffer/editor oriented (scroll, cursor, `shape_until_scroll`); font ids via `fontdb::ID` | paragraph oriented; fonts identified by `Blob` id + index |
| System fonts off | build own `fontdb::Database` | `CollectionOptions { system_fonts: false }` |
| Font stack | fontdb 0.23, skrifa 0.40 (older than the ecosystem) | fontique, skrifa 0.44 — same skrifa we use for outlines |
| Maintenance (crates.io, 2026-11-09) | last release 0.19.0 on 2026-04-22 | 0.12.0 on 2026-10-09; ~monthly releases (Linebender) |
| Dependency footprint (`--no-default-features --features std`) | ~30 crates, pure Rust; contains `unsafe` (mmap'd shared face data) | ~45 crates (ICU4X data), pure Rust |
| C dependencies | none | none |

**Pick: parley.** Same quality and speed; more active maintenance; styled
ranges and CSS-like breaking knobs match our markup and future options
(`word_break`, `text-wrap`); font collection with system fonts switched off;
shares skrifa with our outline renderer. Cost: larger dependency tree and API
churn between minor versions (pin and update deliberately). cosmic-text stays
in `arrangia-render` only for the spike comparison and is removed in Phase 1.

## Coordinate system, home pages, gaps, grouping (as implemented)

`crates/render/src/geometry.rs`, unit-tested.

- Page width `W = 1000 lu`; design page height `H0 = W · 2868/1320 = 2172.7 lu`.
- Target with `w × h` px and grouping `k` (pages per export image):
  page (column) height `H = k · W · h / w` (1320×2868 → 2172.7; 1080×2160 → 2000;
  2752×2064 with k = 2 → 1500).
- Page origin on the canvas, gap `g` lu (from the gallery profile):
  `page_x(i) = i · W + ⌊i / k⌋ · g`, `page_y = 0`.
  Columns of one export image are adjacent; gaps only between images.
- Export image `j` covers `[page_x(j·k), page_x(j·k) + k·W] × [0, H]`;
  pixel scale `s = w / (k·W)`. Gap rectangles `[page_x(j·k)+k·W, +g]` are
  covered by no export image → **content in a gap is omitted** from page
  exports (tested with a bar placed in the gap).
- Element on home page `p` at `(x, y)`: canvas `(page_x(p) + x, anchor_y(y))`
  with `ΔH = H − H0`:
  - `top`: `y`; `bottom`: `y + ΔH`; `center`: `y + ΔH/2` (offset from the
    centre is preserved); `stretch`: `y · H/H0` (and height scales).
- Element transform: `view · translate(canvas pos) · rotate(rotation)`; for
  device variants additionally `· scale(content_scale)` (devices only).
- Text with `clip: page` is clipped to its home page rect; margins are
  relative to the page = the column when grouped. A mask is only built when
  the glyph bounds actually leave the page.
- **Canvas-wide background:** one gradient over the whole canvas rectangle
  `[0, canvas_width] × [0, H]`, i.e. over pages *and* gaps (`stretch`
  vertically). Its geometry therefore depends slightly on the gap value,
  which is acceptable for smooth gradients.
- **Shapes crossing pages** (the band) are normal elements with a home page:
  their position is relative to that page and they run straight through the
  physical gaps. In the gallery preview (pages placed at `page_x · s`) edges
  line up exactly; in a gapless banner (`gaps: none`, g = 0) the band stays a
  straight line anchored to its home page, pages further away shift by the
  removed gaps relative to it.
- Banner: `Viewport::cover(canvas_rect, 2400, 1000, focus)` with `g = 0`;
  `focus = (0.5, 0)` keeps the headlines (vertical excess is cropped from the
  bottom).
- Gallery preview: export images (downscaled) placed at `page_x(j·k) · s`,
  corner radius 40 lu (placeholder), hairline frame, on #F2F2F7.

### Sign conventions

y-down; rotation in degrees, **positive = clockwise** (CSS / tiny-skia
convention). The hero phone's `-8°` tilts it counter-clockwise (top leans
left). CSS gradient angles: 0° = to top, 90° = to right.

## Measured timings

Apple M1 Pro, release build, macOS; median of 15–25 runs per page; text
already laid out (layout is per scene/language, not per render). The two
engines' render times differ only by noise because rendering does not
involve the engine. Numbers vary ±20 % between runs on this machine.

| | per page |
|---|---|
| Render 1320×2868 (export resolution), warm screenshot cache | 70–100 ms (median ≈ 85 ms; page 3 with two devices ≈ 95 ms) |
| Render 400×869 (preview), warm cache | **12–15 ms** (target < 50 ms ✔) |
| Render 400×869, cold (includes Lanczos3 resampling of the screenshots) | 22–35 ms |
| PNG encode 1320×2868 (`Compression::High`) | ≈ 200 ms |
| Text layout + auto-fit, 4 headlines (de) | cosmic-text ≈ 0.6–1.2 ms, parley ≈ 0.5–0.95 ms |

One-time costs: font setup cosmic-text 0.4 ms / parley 0.2 ms (fonts are
`include_bytes!`), decoding the three screenshot PNGs ≈ 35–50 ms.
Breakdown of a 1320×2868 page before optimisation: background gradient
≈ 18 ms, band ≈ 5 ms, device ≈ 70–100 ms (of which shadows 46 ms → ≈ 5 ms
after the custom compositing; housing gradient 7 ms → ring-only fill;
screenshot pattern 13 ms), text < 1 ms.

Consequences for Phase 1: export time is dominated by PNG encoding
(parallelise with rayon, consider `Compression::Default`); the full-canvas
gradient could be cached per target.

## Known limits and open points

- **Gap values are placeholders, not yet measured**: 40 lu (App Store iPhone)
  and 60 lu (iPad grouped set); gallery corner radius 40 lu likewise.
- No hyphenation: long German compounds rely on auto-fit; the emergency
  break splits words without a hyphen. Consider a hyphenation crate later.
- Auto-fit is binary search on size (0.25 lu) + optional line balancing
  (`text-wrap: balance`-like); balance does nothing when the fitted size
  already fills the box.
- Italic (`_…_`) not implemented (no italic font bundled).
- RTL, complex scripts (parley's `complex-scripts` feature is off) and
  emoji/fallback fonts untested.
- Colour management (Display-P3 screenshots → sRGB, `qcms`) later; all
  pixels are treated as sRGB, blending is in gamma space (tiny-skia default).
- The spike's tablet variant keeps phones (no `device_map` yet); it uses
  `content_scale: 0.75` plus two overrides (phone y positions) to avoid
  headline collisions in the 1500 lu columns — exactly the variant
  mechanism of §3.4.
- Validation warnings (text in gap, upscaled screenshots) not part of the spike.
- Deviations from §6.2: parley instead of cosmic-text; fontique instead of
  fontdb; no oxipng (C dependency); bilinear instead of bicubic for the
  pre-resampled screenshot pattern.

## Images in `docs/spike/`

Full-resolution page exports are kept for a representative subset; the
rest is shown in the gallery previews (downscaled, ≈ 420 px per page).
Total ≈ 16 MB. All PNGs are 8-bit RGB without alpha, with sRGB chunk.

- `pages/1320x2868_gap40/{en,de}_01–04.png` – page exports, placeholder gap 40 lu (content in the gap omitted; phone part on page 2 shifted by the gap).
- `pages/1320x2868_nogap/en_01–04.png` – same without gaps, for comparison.
- `pages/1080x2160_gap40/de_01–04.png` – 1:2 target, page height 2000 lu: headlines `top`, devices `bottom`, background `stretch`.
- `pages/2752x2064_group2-gap60/{en,de}_01–02.png` – `group: 2`, columns 1000 × 1500 lu, devices × 0.75.
- `gallery_1320x2868_gap40_{en,de}.png` – store gallery preview with gaps and rounded corners; phone, band and accent line run through the gaps.
- `gallery_1320x2868_nogap_en.png` – gallery of the gapless export (shows the misalignment the gaps avoid).
- `gallery_1080x2160_gap40_{en,de}.png` – gallery of the 1:2 target.
- `gallery_2752x2064_group2-gap60_{en,de}.png` – gallery of the grouped landscape set.
- `gallery_1320x2868_gap40-stylized_en.png` – whole set with `stylized` frames.
- `banner_{en,de}_2400x1000.png` – banner over the whole canvas, `gaps: none`, `fit: cover`.
- `canvas-debug_en_gap40.png`, `canvas-debug_en_group2-gap60.png` – canvas view with page boundaries, hatched gaps, page origins.
- `frames_styles.png` – phone (stylized porcelain/midnight, detailed silver/graphite) and tablet frames.
- `frames_closeup.png` – island, corner radius and screen mask close-up, stylized vs detailed.
- `text-engines_comparison.png` – the same headlines through cosmic-text and parley (auto-fit, overflow, emergency break).

## Consequences

- Phase 1 builds `core::layout` on parley and `render` on tiny-skia with the
  outline-path text renderer; `geometry.rs` is the reference for
  `core::resolve` (anchors, grouping, gaps).
- cosmic-text (and its fontdb) is removed from the workspace in Phase 1.
- Golden tests can rely on bundled fonts and the deterministic,
  resolution-independent text layout.
