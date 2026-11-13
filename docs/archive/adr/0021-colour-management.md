# 0021 – Colour management: imported images → sRGB

- Status: accepted
- Date: 2026-11-10

## Context

KONZEPT §6.2 lists "Farbmanagement: `qcms` (später) – Display-P3-Screenshots
→ sRGB"; Phase 6.1 schedules it. Screenshots from iPhones and Macs are
usually Display P3 (PNG `iCCP`, newer files also `cICP`; JPEG APP2 ICC).
Until now every decoder ignored the profile and drew the P3 values as if
they were sRGB: saturated colours came out dull or shifted. Apple's
product bezels (imported by the user, ADR 0012) are tagged too.

The renderer works in sRGB (tiny-skia, model colours `#RRGGBB`, text,
parametric frames), and exports are sRGB PNG/JPEG.

## Decision

**Convert every decoded raster image to sRGB once, at decode, with the
pure-Rust CMS `moxcms`.**

- `vitrine_core::colorspace` (no rendering, works on decoded buffers):
  - `sniff(reader)` reads only the header: PNG chunks up to the first
    `IDAT` (`cICP`, `iCCP` zlib-decompressed, `sRGB`), JPEG markers up to
    `SOS` (APP2 `ICC_PROFILE` segments joined in sequence order, SOF
    component count → CMYK).
  - `ToSrgb::new(tag)`: precedence `cICP` → `iCCP`/ICC → `sRGB` chunk →
    untagged (= sRGB), as in PNG 3rd edition. Profiles that map a 9³ probe
    grid to itself within ±1 (any flavour of sRGB) are the identity, so
    sRGB-tagged files stay bit-identical to untagged ones (golden images
    unchanged; all fixture screenshots carry an `sRGB` chunk).
  - Conversion: relative colorimetric, out-of-gamut colours clip
    (matrix-shaper profiles have no perceptual tables anyway). Straight
    alpha is kept, colour is converted before premultiplication. 16-bit
    images are converted at 16 bit and then rounded to 8; gray images
    with a gray profile go GrayAlpha → RGBA.
- One decode path: `vitrine_render::raster::SourceImage::decode` is used
  by the asset cache (screenshots, image assets, imported bezels → GUI
  preview, export, upload, MCP render) and by the thumbnail commands.
  `core::frame_import::decode_png` converts too, so measured bezel
  swatches match the drawn bezel.
- Fallbacks (pixels used as sRGB, `ToSrgb::note`, validation info
  `color_profile_unsupported` per screenshot file, via
  `AssetLookup::color_note` → `colorspace::inspect_file`, cached by path +
  mtime + length):
  - unreadable/corrupt ICC or `iCCP`, incomplete JPEG APP2 sequence;
  - ICC colour spaces other than RGB/gray (CMYK, Lab …);
  - CMYK/YCCK JPEGs (the `image` decoder converts them naively; no CMYK
    colour management);
  - `cICP` with HDR transfer (PQ 16, HLG 18 – no tone mapping), non-RGB
    matrix or narrow range, unknown primaries/transfer. An `iCCP` next to
    such a `cICP` is used instead.
- `gAMA`/`cHRM` are **ignored** (treated as sRGB): current Apple/Google
  tools write `iCCP`/`sRGB`/`cICP`; gAMA-only files are almost always
  meant as sRGB (gAMA 1/2.2), and browsers differ in honouring them.
- Model colours (`#RRGGBB`) are sRGB. Exports stay sRGB: PNG keeps its
  `sRGB` chunk; JPEG stays **untagged** (untagged = sRGB by convention for
  browsers, stores and viewers; an embedded sRGB profile would only add
  bytes).

### CMS comparison (crates.io, checked 2026-11-10)

| Crate | Version / last release | Pure Rust | Licence | Notes |
|---|---|---|---|---|
| `moxcms` | 0.9.2 (2026-11-07); 0.8.1 already in our lock file via `image` 0.25.10 | yes | BSD-3-Clause OR Apache-2.0 | active, ~126 M downloads; ICC v2/v4, matrix-shaper + LUT profiles, 8/16-bit and f32, gray, CMYK, CICP, SIMD paths, can write profiles (tests generate their P3 profile) |
| `qcms` | 0.3.0 (2024-01-09) | yes | MIT | Firefox's CMS; 8-bit RGB/gray only, no CICP, few crates.io releases |
| `lcms2` | 6.2.0 (2026-08-26) | no (C `lcms2-sys`) | MIT | reference quality, but a C dependency and cross-compilation burden |

`moxcms` wins: no new crate at all (we pin `0.8.1`, the version `image`
already pulls in – moving to 0.9 would compile two copies; upgrade together
with `image`), pure Rust, 16-bit and CICP support. KONZEPT's `qcms`
suggestion predates moxcms and lacks 16-bit/CICP.

Sources: <https://crates.io/crates/moxcms>, <https://crates.io/crates/qcms>,
<https://crates.io/crates/lcms2>, PNG 3rd edition colour chunk precedence
<https://www.w3.org/TR/png-3/#color-chunk-precendence>, ITU-T H.273 code
points <https://www.itu.int/rec/T-REC-H.273>.

### Display P3 output – not now

Apple accepts P3 screenshots, and a P3 export would keep the extra gamut
of P3 screenshots. It would mean rendering in P3 (or wide-gamut float)
space: convert model colours, gradients, parametric frames and SVGs to
P3, blend in that space, tag exports with a P3 profile, keep sRGB for
Google Play and the web targets – a second colour pipeline through the
whole renderer and all golden images. The visible gain is small (store
pages are viewed downscaled, and most UI colours lie inside sRGB). Revisit
after 1.0 as a per-target option (`color_space: display-p3`) if users ask.

## Measurements

`cargo run -p vitrine-render --release --example color_bench` (MacBook,
Apple silicon, 2026-11-10, other builds running – best of 15, interleaved),
1320×2868 RGBA PNG:

| Input | `image` decode | `SourceImage::decode` | Conversion |
|---|---|---|---|
| 8 bit untagged | 38 ms | 38 ms | ±0 (sniff only) |
| 8 bit Display P3 | 38 ms | 44 ms | ≈ 6.5 ms (isolated `ToSrgb::rgba8`: 6.5 ms incl. transform setup; `ToSrgb::new` 0.2 ms) |
| 16 bit untagged | 67 ms | 67 ms | ±0 |
| 16 bit Display P3 | 67 ms | 79 ms | ≈ 12 ms |

Each image is decoded once per change (asset cache keyed by path +
mtime), so the overhead is paid once per screenshot, not per render.
Validation reads only headers and builds the transform once per file
(cached).

## Consequences

- P3 screenshots and Apple bezels now look as in Preview/Photos; sRGB and
  untagged files are unchanged (golden images unchanged).
- Bezel swatches stored before this change were measured from unconverted
  values; they differ slightly (re-import or delete `swatch` to re-measure).
- Wide-gamut colours outside sRGB clip; an HDR (PQ/HLG) screenshot without
  an ICC fallback is drawn unconverted, with an info finding.
- Tests generate P3/gray profiles with moxcms (no vendor ICC files in the
  repo) and check clipping (P3 red → sRGB red), neutral gray, and a mid
  colour against the P3 → sRGB matrix reference.
