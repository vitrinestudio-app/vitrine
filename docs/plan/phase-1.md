# Phase 1 – Core + CLI: task list

Scope: KONZEPT.md §12, Phase 1. Done when the example project in
`tests/fixtures/` (phone portrait, iPad landscape via grouping, Mac, web app in
browser and laptop frame, blog banner, two languages) exports headless into all
targets and the golden tests are green.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## 1.1 Data model + YAML IO (`core`)
- [x] Types: `Project`, `Languages` (default, available, fallback), `Styles`
      (colors, text styles, span styles, shadow presets), `Canvas` (page aspect,
      count, backgrounds, elements, variants), `Element` (common props: id, kind,
      page, at, size, rotation, pivot, opacity, z, anchor_y, clip, visible_in) with
      kinds background / device / text / image / shape / group, `Variant`
      (content_scale, overrides), `Target` (pages / banner; size, gallery,
      group, variant, device_map, screenshots, region, fit, gaps, store slot),
      `GalleryProfile`
- [x] Color references `@name`, lu units, sign conventions from ADR 0001
- [x] YAML load/save with `serde-saphyr`; stable field order; `format: vitrine/1`;
      revision number; atomic save (temp file + rename)
- [x] i18n files `i18n/<lang>.yaml` (key → text)
- [x] Round-trip tests (load → save = identical), insta snapshots

## 1.2 Operations API (`core`)
- [x] `Op` enum (add/update/remove element, set string, add/update canvas,
      add/update/remove target, set variant override, set styles, …), patch types
- [x] `Project::apply(op) -> Result<Inverse>`; validation before apply
- [x] Transactions (`apply_ops`) = one undo step; in-memory history
- [x] proptest: apply + inverse = original state

## 1.3 Catalogs (`core/data/*.yaml`)
- [x] `targets.yaml`: Apple iPhone/iPad/Mac sizes and display types, Google Play
      phone/7"/10"/Chromebook/feature graphic rules, banner presets — verified
      against current Apple/Google docs, source + check date in file
- [x] `galleries.yaml`: gap widths + corner radii, clearly marked
      "not yet measured" (placeholders); apps.apple.com / play.google.com
      values measured from the web CSS
- [x] `devices.yaml`: iphone-home, iphone-notch, iphone-island,
      android-punchhole, ipad-home, ipad-modern, android-tablet, macbook,
      generic-laptop, browser, imac, generic-monitor, none — outer size, display
      rect, radii, cutouts, colour variants, orientations, screenshot aspect
- [x] Catalog loader embedded via `include_str!`, tests
- [x] Optional page ids in the canvas page spec (`ids: [hero, …]`, default
      `page-01`) for export naming
- [x] Ops check device models, colour variants, gallery presets and store
      slot values against the catalog
- [ ] Measure native store app galleries (iOS App Store, Mac App Store,
      Play Store app) on real devices and replace the placeholders
      (→ Follow-ups; still open 2026-11-13 – only the web CSS values are
      measured; dark store backgrounds added 2026-11-12 as unmeasured
      estimates, ADR 0029)

## 1.4 Resolve (`core::resolve`)
- [x] target × language → resolved scene: variant + overrides, content_scale,
      page height from target aspect, anchors, grouping (columns), gaps,
      device_map, style tokens, `visible_in`
- [x] i18n fallback chain (`de-AT` → `de` → default), screenshot lookup chain
      (§5.2)
- [x] Geometry from the spike moves into `core` (page→canvas mapping,
      `core::geometry`; the render crate switches to it in 1.6) — ADR 0004
- [x] proptest for anchor/coordinate mapping

## 1.5 Text layout (`core::layout`)
- [x] Inline markup parser: `**bold**`, `_italic_`, `[span]…[/]`, `\n`,
      soft hyphen (`\-` and U+00AD) — break only there when needed, render a
      visible hyphen
- [x] parley layout, auto-fit (`shrink_to`, `max_lines`), box from page margins
      (box size is the caller's input; margins → width happen in resolve 1.4)
- [x] Bundled fonts + project `fonts/`; no system fonts in tests
- [x] Layout report: boxes, chosen font size, overflow, emergency word breaks
      (ADR 0003)

## 1.6 Render (`render`)
- [x] Rasterize resolved scene (backgrounds linear/radial/image, shapes, images
      PNG/JPEG/SVG via resvg, devices, text with clip to page) — ADR 0006
- [x] Parametric frames for all catalog models, styles `stylized` and
      `detailed`, colour variants, portrait/landscape; `chrome: browser` on
      laptop/desktop; browser frame with URL, tabs, light/dark
      (`render::frames`, contact sheets `docs/spike/frames_catalog_*.png`)
- [x] Screenshot fit cover/contain/stretch, placeholder when missing
- [x] Remove cosmic-text; spike example reduced or removed (done in 1.5)

## 1.7 Export (`render::export`)
- [x] Page export (gap content omitted), grouped export, banner (region,
      cover/contain, gaps none/as_store), gallery preview
- [x] PNG 24-bit no alpha + JPEG; naming `export/<target>/<lang>/<nn>_<page-id>.png`
      (grouped: `<nn>_<id1>+<id2>.png`)
- [x] Parallel with rayon; incremental via per-file hashes
      (`export/.vitrine-export.json`)
- [~] Image shadows from the image alpha (currently the image rect);
      SVG text needs a font database for usvg (text in SVG assets is
      currently not rendered) (→ Follow-ups) – shadows follow the alpha
      since 2026-11-10 (with text outline + glow); SVG text still open

## 1.8 Validation (`core::validate`)
- [x] Text in gap, text overflow per language/page, emergency word break
      without hyphen, missing translations (fallback used), screenshot fallback
      info, missing screenshot, screenshot aspect mismatch > 2 %, upscale
      warning, aspect deviation → suggest variant, unknown references
      (ADR 0005)
- [x] Store rules per target (size, count, format, slot) from the catalog
- [x] `AssetLookup::dimensions` (image headers); free-aspect devices
      (browser, `none`) follow the screenshot aspect (ADR 0004)
- [x] Apple presets verified against App Store Connect (2026-11-09)

## 1.9 CLI
- [x] `vitrine new <dir>` (from a template), `render` (preview PNG of page /
      canvas / gallery), `export [--target] [--lang]`, `validate` (exit code)
- [x] Exit codes: 0 ok (warnings only fail `validate --strict`), 1 project
      problems (validation errors, export failures), 2 could not run (usage,
      project not loadable, IO); `--project` defaults to the current folder
      (no upward search); `--page`/`--image` are 1-based like export names

## 1.10 Example project + golden tests
- [x] `tests/fixtures/example/`: phone portrait (6 pages, continuous
      gradient + diagonal band through the gaps, a rotated phone across
      pages 3/4, callouts), iPhone 6.9″ + 6.3″, Google Play (`device_map`
      + `android` variant), iPad landscape via `group: 2` +
      `tablet-landscape` variant + `device_map`, Mac (16:10: notebook,
      all-in-one, phone), web app in a browser frame and in a notebook with
      `chrome: browser`, blog header + Open Graph banners, en + de (spans,
      bold, soft hyphens, screenshot fallbacks); validates with 0 errors /
      0 warnings
- [x] Screenshot generator `crates/render/examples/screens.rs`
      (deterministic fake UI, committed PNGs ≈ 5 MB)
- [x] Validation: with `chrome: browser` on laptops/desktops the expected
      screenshot aspect is the area below the bars (found by the fixture)
- [x] Golden tests (`crates/render/tests/golden.rs`): all output images
      of all targets (en) + de for iPhone and blog header at 25 %, gallery
      previews at 320 px; per-channel tolerance 8/255, max 0.5 % differing
      pixels; `VITRINE_UPDATE_GOLDEN=1` to update; `*.actual.png` /
      `*.diff.png` on failure; fixture validation test; CLI test
      (`validate --strict` + export of the web targets)
- [x] Windows: YAML round-trip tests independent of checkout line
      endings (with `.gitattributes` forcing LF)

## 1.11 Docs
- [x] ADRs for model decisions: already covered – home page coordinates
      and ids by ADR 0002 (*Units, coordinates, references*) and ADR 0004
      (*Frame geometry*, *Anchors and boxes*), anchors / variants /
      `content_scale` / `device_map` by ADR 0004, markup and soft hyphen by
      ADR 0003; ADR 0005 amended for browser chrome
- [x] CLAUDE.md (commands, architecture, golden tests), README (status,
      quick start, example images in `docs/example/`, open questions)

## Follow-ups (after Phase 1)
- Measure the native store app galleries (gap, corner radius) on real
  devices and replace the `not yet measured` placeholders in
  `galleries.yaml` (1.3).
- ~~Image shadows from the image alpha instead of the image rect (1.7)~~
  – done 2026-11-10 (like CSS drop-shadow).
- Text inside SVG assets (usvg needs a font database; 1.7) – open.
- Golden tests run on macOS locally; the first CI run on Linux/Windows
  shows whether 8/255 and 0.5 % absorb the platform differences
  (SIMD resampling, libm); widen only with a measured reason. – CI runs
  the goldens on macOS, Linux and Windows with the original tolerances.
- Banner `focus` for `fit: cover` (ADR 0004 open question); the example's
  blog header uses a `rect` region instead. – open (image fills have a
  `focus`, banners not).
- ~~Per-language element overrides (KONZEPT §5.1) are not modelled yet.~~
  – done in Phase 3 (3.1, `langs`, ADR 0009).

## Decisions taken with the user
- Text engine: parley (ADR 0001).
- Hyphenation: soft hyphens in Phase 1 (1.5) + warning on emergency breaks
  (1.8); automatic pattern-based hyphenation as opt-in in Phase 3, only used when
  auto-fit at `shrink_to` is not enough.
