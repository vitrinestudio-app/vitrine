# Phase 3 – Languages & import: task list

Scope: KONZEPT.md §4.2, §5 and §12, Phase 3. Done when 3 languages × 3 targets
export correctly with fallbacks, overflows are reported, and Apple product
bezels are imported and used.

Already done in Phases 1/2: i18n files, fallback chains, auto-fit per
language, screenshot variants + lookup chain, translations table, all-languages
grid, live reload of screenshots.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## 3.1 Per-language text overrides + font fallback (`core`, GUI)
- [x] Text elements: optional overrides per language (`langs: {fi: {font_size: 60, box: …, style: …}}`, §5.1), resolved after variant overrides; ops/patch, YAML, validation
- [x] Inspector: "only for <lang>" toggle that writes language overrides, marked like variant overrides
- [x] Per-family fallback list in project styles (`fonts: {Inter: {fallback: [Noto Sans JP, …]}}`); missing glyphs reported per language; optional app setting to use system fonts as a last resort (off in tests/CI)

## 3.2 Automatic hyphenation (opt-in)
- [x] Pattern-based hyphenation per language (pure Rust, e.g. `hyphenation` crate or own Liang implementation), only languages whose pattern licence allows redistribution
- [x] Opt-in per text style / element (`hyphenate: true`), used only when auto-fit at `shrink_to` does not fit (decided with the user); soft hyphens keep priority; minimum word/fragment lengths
- [x] Inspector + style editor toggle; validation info when hyphenation was used

## 3.3 Screenshot import
- [x] Import rules: Arrangia folder convention, fastlane snapshot output (`<locale>/<Device Name>-<name>.png`), configurable device-name → variant and locale → language maps (stored in project.yaml) — `core::import`, `import.fastlane` in project.yaml, ADR 0010
- [x] Dry-run report (what goes where, unknown files, overwrites), then apply
- [x] CLI `arrangia import screenshots <dir> [--format auto|arrangia|fastlane|flat] [--dry-run] [--save-mapping]`; GUI import dialog (folder pick, preview table, apply)

## 3.4 Translation exchange
- [x] Export/import CSV (key, per-language columns, notes/context, max length hints) and XLIFF 1.2 + 2.0 (source = default language, target per language; markup mapped to inline tags `<pc>/<ph>` resp. `<g>/<x/>` by default, as text optionally; ADR 0011)
- [x] Import report: changed/new/unknown keys, markup diagnostics, overflow check per language after import
- [x] CLI `arrangia strings export|import`; GUI buttons in the translations panel

## 3.5 Apple Design Resources import
- [x] Importer: DMG (macOS `hdiutil attach -readonly -nobrowse`), zip or extracted folder; recognise device/colour/orientation from file names via `crates/core/data/frame-import/apple.yaml` (data, not code)
- [x] Screen rect + corner mask detection (transparent interior via flood fill from the centre); manual assignment for unknown files
- [x] App-wide frame store (app data dir, never in project or repo); project stores only `frame: imported:apple` (+ model/colour); missing package → parametric frame + validation info
- [x] Renderer: draw imported PNG frames (screenshot under the frame through the detected mask), correct scaling per element width
- [x] GUI: app settings → frame packages (import, list, remove, unmatched files); inspector frame source picker
- [x] Tests with a synthetic bezel package (generated PNGs + names) (ADR 0012)
- [~] Real package verified by the user: iPhone 18 package dry run (2026-11-10) → iPhone rules verified; all bezel sizes kept (Pro for 6.3", Pro Max for 6.9"), automatic choice per target + `bezel:` pin, DMG licence-agreement guidance, licence notice; iPad / Mac rules still unverified; real import + render pending

## 3.6 Follow-ups from Phase 2
- [x] Project settings: styles list must not collapse after each edit
- [x] Persist the home-page lock (model field) instead of UI-only state
- [x] New string key: optionally create in all languages
- [x] Renaming a used colour/style updates element references

## 3.7 Verification + docs
- [x] Example project gets a third language: `fr` (all texts, French phone screenshots for 3 of 5 screens → en fallback, no-break spaces) plus `fr-CA` (2 Québec texts, rest via prefix fallback → info); German language override + automatic hyphenation on `title-companions` ("Mischkultur-partner"); golden tests: fr for the iPhone 6.9" and iPad sets (+1.1 MB)
- [~] "Done when" scenario: 3 languages × 3 targets exported with fallbacks, overflow reported (`crates/cli/tests/scenario.rs`) – done; Apple bezels imported and used with the user's real package – open (coordinator/user)
- [x] ADRs (0009 overrides + hyphenation, 0010 screenshot import, 0011 translation exchange, 0012 frame packages); CLAUDE.md/README updated
