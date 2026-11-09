# Phase 1 – Core + CLI: task list

Scope: KONZEPT.md §12, Phase 1. Done when the example project in
`tests/fixtures/` (phone portrait, iPad landscape via grouping, Mac, web app in
browser and laptop frame, blog banner, two languages) exports headless into all
targets and the golden tests are green.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## 1.1 Data model + YAML IO (`core`)
- [x] Types: `Project`, `Languages` (default, available, fallback), `Styles`
      (colors, text styles, span styles, shadow presets), `Strip` (page aspect,
      count, backgrounds, elements, variants), `Element` (common props: id, kind,
      page, at, size, rotation, pivot, opacity, z, anchor_y, clip, visible_in) with
      kinds background / device / text / image / shape / group, `Variant`
      (content_scale, overrides), `Target` (pages / banner; size, gallery,
      group, variant, device_map, screenshots, region, fit, gaps, store slot),
      `GalleryProfile`
- [x] Color references `@name`, lu units, sign conventions from ADR 0001
- [x] YAML load/save with `serde-saphyr`; stable field order; `format: arrangia/1`;
      revision number; atomic save (temp file + rename)
- [x] i18n files `i18n/<lang>.yaml` (key → text)
- [x] Round-trip tests (load → save = identical), insta snapshots

## 1.2 Operations API (`core`)
- [x] `Op` enum (add/update/remove element, set string, add/update strip,
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
- [x] Optional page ids in the strip page spec (`ids: [hero, …]`, default
      `page-01`) for export naming
- [x] Ops check device models, colour variants, gallery presets and store
      slot values against the catalog
- [ ] Measure native store app galleries (iOS App Store, Mac App Store,
      Play Store app) on real devices and replace the placeholders

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
- [ ] Inline markup parser: `**bold**`, `_italic_`, `[span]…[/]`, `\n`,
      soft hyphen (`\-` and U+00AD) — break only there when needed, render a
      visible hyphen
- [ ] parley layout, auto-fit (`shrink_to`, `max_lines`), box from page margins
- [ ] Bundled fonts + project `fonts/`; no system fonts in tests
- [ ] Layout report: boxes, chosen font size, overflow, emergency word breaks

## 1.6 Render (`render`)
- [ ] Rasterize resolved scene (backgrounds linear/radial/image, shapes, images
      PNG/JPEG/SVG via resvg, devices, text with clip to page)
- [ ] Parametric frames for all catalog models, styles `stylized` and
      `detailed`, colour variants, portrait/landscape; `chrome: browser` on
      laptop/desktop; browser frame with URL, tabs, light/dark
- [ ] Screenshot fit cover/contain/stretch, placeholder when missing
- [ ] Remove cosmic-text; spike example reduced or removed

## 1.7 Export (`render::export`)
- [ ] Page export (gap content omitted), grouped export, banner (region,
      cover/contain, gaps none/as_store), gallery preview
- [ ] PNG 24-bit no alpha + JPEG; naming `export/<target>/<lang>/<nn>_<page-id>.png`
- [ ] Parallel with rayon

## 1.8 Validation (`core::validate`)
- [ ] Text in gap, text overflow per language/page, emergency word break
      without hyphen, missing translations (fallback used), screenshot fallback
      info, missing screenshot, screenshot aspect mismatch > 2 %, upscale
      warning, aspect deviation → suggest variant, unknown references

## 1.9 CLI
- [ ] `arrangia new <dir>` (from a template), `render` (preview PNG of page /
      canvas / gallery), `export [--target] [--lang]`, `validate` (exit code)

## 1.10 Example project + golden tests
- [ ] `tests/fixtures/example/`: phone portrait, iPad landscape via
      `group: 2` + variant + device_map, Mac (16:10), web app in browser frame
      and in laptop frame with `chrome: browser`, blog banner, en + de
- [ ] Golden tests with per-pixel tolerance and max share of differing
      pixels; `ARRANGIA_UPDATE_GOLDEN=1` to update; `*.actual.png`/`*.diff.png`
      on failure

## 1.11 Docs
- [ ] ADRs for model decisions (home page coordinates, anchors/variants,
      markup/soft hyphen), update CLAUDE.md / README

## Decisions taken with the user
- Text engine: parley (ADR 0001).
- Hyphenation: soft hyphens in Phase 1 (1.5) + warning on emergency breaks
  (1.8); automatic pattern-based hyphenation as opt-in in Phase 3, only used when
  auto-fit at `shrink_to` is not enough.
