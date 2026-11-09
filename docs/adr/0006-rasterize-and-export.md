# 0006 – Rasterizing resolved scenes and export

- Status: accepted
- Date: 2026-11-09

## Context

Phase 1 tasks 1.6 and 1.7 turn a `ResolvedScene` (ADR 0004) into pixels
and files. GUI previews, MCP previews and export must share one code path
(KONZEPT §6.1); export must be parallel and incremental (§6.3). ADR 0001
fixed the libraries (tiny-skia, parley via `core::layout`, own blur,
fast_image_resize, png).

## Decision

### API (`arrangia_render::scene`)

- `RenderContext::new(project_root)` holds the asset cache, the font
  library (bundled + `fonts/`) and a pool of `TextLayouter`s. It is
  `Send + Sync`: one per project, shared by all threads.
- `prepare(&scene) -> PreparedScene`: lays out all texts once with
  `core::layout::layout_text` (same placement as `core::validate`) and
  collects text diagnostics. Any number of images of a prepared scene can
  then be rasterized in parallel.
- `rasterize(_prepared)(scene, image, RenderOptions { scale, quality })`:
  one output image (page, page group or banner) through its
  `OutputImage::viewport`. `Scale::Target | Factor | MaxEdge | Width` for
  previews; `Quality::Preview` resamples with a bilinear convolution,
  `Export` with Lanczos3.
- `render_canvas(prepared, CanvasOptions)`: the whole canvas with an
  optional overlay (veiled, hatched gaps; dashed page boundaries; labels).
  `render_view(prepared, viewport, quality)` renders any canvas region.
- Results carry the pixmap, the viewport used and asset issues; text issues
  sit on `PreparedScene`. Rendering is deterministic (no clocks, no
  randomness, caches only change speed).

### Painting

- Canvas backgrounds fill the canvas rect (pages and gaps) united with the
  visible rect (banner `contain` margins); gradient/image geometry is the
  canvas rect.
- Elements are drawn in list order (already z-sorted) with
  `view · translate(center + pivot) · rotate · translate(−pivot)`.
  Elements outside the viewport are culled.
- Opacity and clipping use an offscreen layer covering only the element's
  pixel bounds (including shadows), composited with the opacity and a clip
  mask. A mask is only built when the element actually crosses its clip
  rect. Page images show exactly their source rect, so gap content is
  omitted without special cases.
- Shadows: blurred silhouettes (`blur.rs`), offsets in canvas space (they
  do not rotate with the element). Silhouettes: device outline, shape path
  (stroke outline for unfilled shapes), glyph outlines, image rect.
- Images and image fills are resampled once to the exact pixel size of
  their destination (cover crops by focus, contain letterboxes) and then
  mapped with a bilinear pattern that only handles rotation and sub-pixel
  scale. SVG assets are rendered with resvg at that size (no text, no
  embedded raster images: `default-features = false`). Image fill `tile`
  repeats at the natural size (1 px or SVG unit = 1 lu).
- Devices: `device_geometry` / `draw_device` (device-local model units,
  centred on the outer box; the caller scales to the element width). The
  renderer resamples the screenshot to the screen's on-screen pixel size
  (`fit`), draws the shadow, then the frame. Landscape devices with a
  portrait geometry are the frame rotated 90° counter-clockwise; the
  screenshot is not rotated. Until the catalog-driven frames land,
  `scene::device` adapts the spike frames.

### Caching

- `AssetCache`: decoded assets keyed by path and re-read when the mtime
  changes; resampled pixmaps keyed by (path, mtime, w, h, mode, filter)
  as `Arc<Pixmap>` behind a `Mutex`, cleared when exceeding 768 MB.
  Asset paths must be relative and must not contain `..`.

### Export (`arrangia_render::export`)

- Target × language pairs (target `langs`, else all available languages)
  are resolved and prepared in parallel; then every file (page images,
  banner, gallery preview) is rendered, encoded and written in parallel.
- Names: `<target>/<lang>/<nn>_<page-id>.<ext>`, grouped images
  `<nn>_<id1>+<id2>.<ext>`, `banner.<ext>`, `_gallery.png`.
- PNG: 24-bit RGB, sRGB chunk, flattened onto white, `Compression::Balanced`
  (measured on a 1320×2868 page: Fast 18 ms / 469 kB, Balanced 68 ms /
  362 kB, High 175 ms / 353 kB). JPEG: target `quality` (default 90).
- Gallery preview: images re-rasterized at ≤ 1200 px height and placed at
  their canvas x (so content crossing a gap lines up), with the profile's
  gap, (elliptical) corner radius and background (default `#F2F2F7`).
- Incremental: per file a hash of the serialized scene, catalog data of
  its devices, mtimes of referenced assets and project fonts, format,
  `RENDER_REVISION` (bump when output changes); stored in
  `export/.arrangia-export.json`. Unchanged files are skipped.
- `ExportReport`: files (size, bytes, render/encode ms, skipped), issues
  (resolve diagnostics, text layout and asset issues, deduplicated per
  target × language), errors, timings.

## Measurements

Apple M1 Pro, release, sample project (`cargo run -p arrangia-render
--release --example sample -- <dir>`):

| | |
|---|---|
| Preview 400 px wide, warm cache | 0.8–3.5 ms per image |
| Preview 400 px, first render (decode + resample screenshots) | 5–30 ms |
| Export resolution 1320×2868 (phone + band + texts) | ≈ 78 ms |
| Full export, 4 targets × 4 languages, 66 files incl. galleries | ≈ 1.0 s wall |
| Same export again (incremental, nothing changed) | ≈ 2–4 ms |

## Consequences

- The GUI renders previews with `RenderOptions::preview(w)` from a
  `PreparedScene` and re-prepares only when texts change.
- The coordinator replaces `scene::device` internals with the catalog
  frames (`crate::frames`) without touching the painter.
- Open: image shadows from alpha; text inside SVG assets; colour
  management (P3 → sRGB); `aspect: free` devices follow the screenshot
  (resolve, ADR 0005).
