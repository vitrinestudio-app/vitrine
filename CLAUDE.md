# CLAUDE.md

Guidance for Claude (and other agents) working in this repository.

## Project

Arrangia is an open-source (freeware) desktop app, written in Rust, for creating
App Store and Play Store screenshots with device frames, texts, translations,
panorama sets, store upload and an MCP interface. A layout is designed once and
rendered automatically for every language, device class and store. Projects are
plain folders of YAML files and images: no account, no cloud, Git-friendly. GUI
(Tauri 2), CLI and MCP server are three front ends on the same core.

The binding concept is **`docs/KONZEPT.md`** (German). Read it before changing
the model, renderer or architecture. The user communicates in German; code,
comments, commit messages and repo docs are in English.

Status: pre-alpha, Phase 0 (rendering spike). See roadmap in KONZEPT.md §12.

## Core ideas

1. **Canvas, not individual images.** The strip (canvas) is the top level.
   Backgrounds run continuously across pages; a device spanning two pages
   exists exactly once. Texts are bound to their page by default (`clip: page`).
2. **Home page.** Every element hangs on a page (`page`); its position is
   relative to that page and it may reach into neighbouring pages.
3. **Gaps like in the store.** Gaps between pages come from the target's
   gallery profile; content inside a gap is omitted in page exports.
4. **One layout, many formats.** Vertical anchors (`top`/`bottom`/`center`/
   `stretch`), page grouping (2 portrait pages → 1 landscape image) and
   variants with overrides. Content is never maintained twice.
5. **Resolution-independent.** A page is 1000 lu wide; all sizes are in lu.
   The export resolution comes only from the target.
6. **One renderer for everything.** GUI preview, export and MCP
   `render_preview` all come from the same Rust pipeline. The GUI does not
   render in the browser; it shows Rust pixmaps plus HTML/SVG overlays.

## Layout

```
Cargo.toml              workspace: shared package metadata, dependencies, lints
rust-toolchain.toml     pinned toolchain (1.99.1)
crates/
  core/     arrangia-core     model, YAML IO, Operations API, resolve, layout,
                              validation, catalogs (data in crates/core/data/*.yaml)
  render/   arrangia-render   rasterization, parametric frames, text, export encoding
  publish/  arrangia-publish  App Store Connect, Google Play, keychain
  mcp/      arrangia-mcp      MCP server (rmcp) over core/render/publish
  cli/      arrangia-cli      binary `arrangia` (clap)
  gui/      arrangia-gui      Tauri 2 app; frontend in gui/ui/ (vanilla ES modules)
assets/fonts/           bundled OFL fonts (planned)
tests/fixtures/         sample projects and golden reference images (planned)
docs/KONZEPT.md         concept (binding)
docs/adr/               architecture decision records
```

## Stack

Rust edition 2024, toolchain pinned · Tauri 2, frontend without framework, npm
or build step; web view gets only `core:default`, all file/system access via
custom Tauri commands · UI i18n via JSON catalogs (`crates/gui/ui/i18n/en.json`,
`de.json`) · tokio, reqwest + rustls (publish only; core stays network-free) ·
YAML via serde + `serde-saphyr` · SQLite (`rusqlite`, bundled) only for caches,
history, upload log, recent projects · secrets only in the system keychain
(`keyring`) · rendering with tiny-skia, text layout with parley, glyph outlines via skrifa (ADR 0001) ·
targets macOS, Windows, Linux.

## Rules

- Before every commit: `cargo fmt --all`,
  `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test --all`
  must all be green.
- Small, thematic commits.
- **All model changes go through the Operations API in `core`**
  (`Op` → `Project::apply` → inverse op for undo). GUI, CLI and MCP never
  modify model fields directly.
- No `unsafe` (workspace lint `unsafe_code = "forbid"`).
- Golden-image tests use only the bundled OFL fonts, never system fonts.
- No vendor graphics (Apple/Google device images etc.) in the repo. Built-in
  frames are drawn parametrically from catalog data in the styles `stylized`
  and `detailed`; Apple Design Resources are imported by the user, app-wide.
- Store-specific values (sizes, display types, rules, gallery gaps) are data
  in `crates/core/data/*.yaml`, never hard-coded, each with a source URL and
  verification date. Gallery gap values not yet measured on real store views
  are marked as `not yet measured`.
- Architecture decisions go into `docs/adr/NNNN-title.md` (see ADR 0000).
- No `license` field and `publish = false` in all manifests until the license
  is decided. Do not add LICENSE files.

## Commands

```sh
cargo build                                  # build everything
cargo test --all                             # all tests
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
cargo run -p arrangia-cli -- --help          # CLI (binary `arrangia`)
cargo run -p arrangia-gui                    # start the GUI
ARRANGIA_UPDATE_GOLDEN=1 cargo test          # rewrite golden reference images (review the diff!)
```

Golden tests (planned) compare renders of `tests/fixtures` against reference
PNGs with a per-pixel tolerance and a maximum share of differing pixels. On
failure they write `*.actual.png` / `*.diff.png` next to the reference (ignored
by Git). With `ARRANGIA_UPDATE_GOLDEN=1` they overwrite the references instead.
