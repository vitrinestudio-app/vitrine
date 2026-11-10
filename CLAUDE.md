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

Status: pre-alpha. Phase 1 (core + CLI) and Phase 2 (GUI editor) are
complete pending review (`docs/plan/phase-1.md`, `docs/plan/phase-2.md`
with the known GUI gaps under "Follow-ups"); Phase 3 (store upload) is
next. See the roadmap in
KONZEPT.md §12.

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
  gui/      arrangia-gui      Tauri 2 app; frontend in gui/ui/ (vanilla ES modules,
                              see gui/ui/README.md)
assets/fonts/           bundled OFL fonts (Inter 4.1, embedded by core::layout)
tests/fixtures/example/ example project (all target kinds, en + de) and its
                        golden reference images (golden/)
docs/KONZEPT.md         concept (binding)
docs/adr/               architecture decision records
docs/plan/              task lists per phase
docs/example/           preview images of the example project (README)
docs/gui/               screenshots of the editor (README)
```

### Architecture

`core` (no rendering, no network):

- `model` – typed project model (strips, elements, styles, targets,
  variants); `io` – YAML load/save (`serde-saphyr`, canonical form,
  revision + atomic save, ADR 0002); `ops` – Operations API with inverse
  ops and history (the only way to change the model).
- `catalog` – embedded device, target-preset and gallery catalogs
  (`data/*.yaml`); `geometry` – page → canvas mapping, anchors, grouping,
  viewports (ADR 0004).
- `resolve` – project × target × language → `ResolvedScene` (variants,
  `content_scale`, `device_map`, i18n + screenshot fallback chains,
  styles, diagnostics; ADR 0004).
- `markup` – inline markup parser; `layout` – parley text layout with
  auto-fit, soft hyphens and layout reports, bundled + project fonts
  only (ADR 0003).
- `validate` – findings per target × language (texts, screenshots, store
  rules; ADR 0005).

`render` (tiny-skia, ADR 0006): `scene` – rasterize a resolved scene
(`RenderContext`, asset cache, prepared scenes); `frames` – parametric
device frames and browser chrome from the catalog; `text` – glyph outlines
of `core::layout` results; `export` – page/banner files, gallery previews,
incremental manifest; plus `paint`/`blur`/`raster` helpers.

`cli` – `arrangia new|render|export|validate` on top of `core` + `render`
(templates in `core::templates`).

`gui` (Phase 2, ADR 0008) – library + Tauri binary: `commands` (plain
functions `fn(&AppState, Args) -> Result<T, CommandError>`, `dispatch` by
name, one module per area: project, edit, inspect, add, texts, strings,
assets, targets, project_settings, render, export, catalog, os, system;
a new command goes into the `command_table!` in `commands/mod.rs` *and*
the `json_commands!`/`image_commands!` + `generate_handler!` lists in
`main.rs`, which only wraps them as Tauri commands), `session` (open
project, apply/undo/redo with atomic save per transaction, revision
conflicts, live reload via `watch`), `preview` (binary preview payload +
cache), `settings` (settings.yaml + recent projects in SQLite), `events`,
`dialogs`, `bridge` (dev HTTP bridge, feature `devbridge`). UI transport
shim: `ui/js/transport.js` (`invoke`/`listen` over Tauri or the bridge).
Edit gestures go through `overlay` + `edit_*` commands (view context
`{target, lang, variant}`, one undo step each) and `render_preview_edit`
(drag previews from a scratch copy, never saved). New elements come from
`element_add` (defaults built in Rust). Model changes that need several
ops are built in Rust commands, never in JS. Frontend structure and
panel/dialog conventions: **`crates/gui/ui/README.md`** (store, api,
editor controller, views, panels with `mount(el, ctx)`, dialogs via
`dialogs/modal.js`, i18n keys in both catalogs; `?theme=dark|light`,
`?lang=` debug params). The Tauri CSP (`crates/gui/tauri.conf.json`)
forbids inline styles and scripts; the dev bridge has no CSP, so check
the real app after touching resource loading.

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
cargo run -p arrangia-gui                    # start the GUI (start screen)
cargo run -p arrangia-gui -- <project-dir>   # … opening a project
# dev HTTP bridge: the UI + all GUI commands in a normal browser (prints the URL;
# --port N, --copy = work on a temp copy, --app-dir, --pick-folder/--pick-file
# answer the folder/file dialogs; without --project it shows the start screen; ADR 0008)
cargo run -p arrangia-gui --features devbridge --bin arrangia-devbridge -- --project tests/fixtures/example --copy
cargo run --release -p arrangia-gui --features devbridge --bin arrangia-devbridge -- --port 7801 --pick-folder /tmp/x
cargo run -p arrangia-gui --release --example preview_transport   # preview payload measurements
ARRANGIA_UPDATE_GOLDEN=1 cargo test -p arrangia-render --test golden   # rewrite golden references (review the diff!)

# CLI on the example project (release build: a full export takes ~2 s)
cargo run -p arrangia-cli --release -- --project tests/fixtures/example validate
cargo run -p arrangia-cli --release -- --project tests/fixtures/example export
cargo run -p arrangia-cli --release -- --project tests/fixtures/example render --target ios-iphone-69 --gallery --max-edge 800 -o /path/to/gallery.png

# regenerate the example screenshots (deterministic fake app UI)
cargo run -p arrangia-render --release --example screens -- tests/fixtures/example/screenshots
```

CLI usage (`--project <DIR>`, default the current folder): `new <dir>
[--template phone|minimal] [--lang …]`, `validate [--target] [--lang]
[--preview] [--strict] [--format json]`, `render [--target] [--lang]
[--page N | --image N | --canvas | --gallery] [--width|--max-edge|--scale]
[-o file]`, `export [--target] [--lang] [--out dir] [--no-gallery]
[--force] [--format json]`. Exit codes: 0 ok, 1 project problems, 2 could
not run.

Golden tests (`crates/render/tests/golden.rs`) render every output image
of every target of `tests/fixtures/example` (en, plus de for the iPhone
set and the blog header) at 25 % and the gallery previews at 320 px, and
compare them with `tests/fixtures/example/golden/`: a pixel differs above
8/255 in any channel, an image fails above 0.5 % differing pixels
(constants at the top of the file). On failure they write `*.actual.png` /
`*.diff.png` next to the reference (ignored by Git). With
`ARRANGIA_UPDATE_GOLDEN=1` they rewrite references that are missing or
outside the tolerance and delete stale ones. They run in the normal debug
`cargo test` (dependencies are built with `opt-level = 2`, our crates are
not; ~7 s on 10 cores). Changing the fixture or the renderer visibly means
updating the references in the same commit.

Cross-platform tests: paths via `Path::join`, never compare paths as
strings; line endings are LF (`.gitattributes`), tests reading checked-in
text normalise `\r\n` anyway.
