# CLAUDE.md

Guidance for Claude (and other agents) working in this repository.

## Project

Vitrine Studio is an open-source (GPL-3.0-or-later) desktop app, written in Rust, for creating
App Store and Play Store screenshots with device frames, texts, translations,
panorama sets, store upload and an MCP interface. A layout is designed once and
rendered automatically for every language, device class and store. Projects are
plain folders of YAML files and images: no account, no cloud, Git-friendly. GUI
(Tauri 2), CLI and MCP server are three front ends on the same core.

The binding concept is **`docs/KONZEPT.md`** (German). Read it before changing
the model, renderer or architecture. The user communicates in German; code,
comments, commit messages and repo docs are in English.

Status: pre-alpha (version 0.1.0 = `[workspace.package] version`, the
single source; releases: `docs/dev/releasing.md`). Phases 0–4 (spike, core + CLI, GUI editor, languages &
import, MCP) are done. Phase 5 (store upload, `docs/plan/phase-5.md`) is
built – App Store Connect screenshot sets + App Asset Library (iPhone Duo,
header/search creatives), Google Play, plan → confirm → execute – and the
owner is testing real runs. Phase 6 (polish → 1.0, `docs/plan/phase-6.md`)
is in progress: colour management, fastlane layout + `index.html` export
overview and the licence (GPL-3.0-or-later + CLA) are done; next are
installer/signing, templates + gallery (`api.vitrinestudio.app`), own frame
packages, the website and the docs. Current state and decisions:
`docs/plan/STATUS.md`. See the roadmap in KONZEPT.md §12.

## Core ideas

1. **Canvas, not individual images.** The canvas is the top level.
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
LICENSE                 GNU GPL v3 (verbatim); CLA.md, CONTRIBUTING.md (ADR 0022)
crates/
  core/     vitrine-core     model, YAML IO, Operations API, resolve, layout,
                              validation, catalogs (data in crates/core/data/*.yaml)
  render/   vitrine-render   rasterization, parametric frames, text, export encoding
  publish/  vitrine-publish  App Store Connect, Google Play, keychain
  app/      vitrine-app      shared application layer (no Tauri): session, commands,
                              preview cache, settings, watcher, events, view state
  mcp/      vitrine-mcp      MCP server (rmcp 3.9, exact pin) over vitrine-app (ADR 0014);
                              GUI host + stdio proxy (ADR 0015)
  cli/      vitrine-cli      binary `vitrine` (clap)
  api-server/ vitrine-api-server  backend of api.vitrinestudio.app (axum, SQLite,
                              Docker; binary `vitrine-api`; template gallery + GitHub
                              login; ADR 0023, 0025; not in the GUI)
  gui/      vitrine-gui      Tauri 2 app + dev bridge over `app`; frontend in gui/ui/
                              (vanilla ES modules, see gui/ui/README.md)
assets/fonts/           bundled OFL fonts (Inter 4.1 + 9 variable families in
                        <family>/ with OFL.txt; manifest core::layout::bundled,
                        embedded, registered lazily; SOURCES.md)
assets/hyphenation/     hyph-utf8 hyphenation patterns (permissive licences only,
                        see LICENSES.md; embedded by core::layout::hyphen)
tests/fixtures/example/ example project (all target kinds; en, de, fr, fr-CA
                        with deliberate fallbacks) and its golden reference
                        images (golden/)
docs/KONZEPT.md         concept (binding)
docs/adr/               architecture decision records
docs/plan/              task lists per phase
website/                www.vitrinestudio.app: Hugo site (no theme), en + de,
                        landing page + docs; `hugo server` / `hugo --minify`
                        in website/, see website/README.md
scripts/release/        release helpers (licence files via cargo-about, CLI sidecar,
                        collect artifacts, latest.json/checksums/notes; ADR 0026)
deny.toml, about.toml   licence allow-list (cargo deny, CI) / third-party notices
.signpath/              SignPath artifact configuration drafts (Windows signing)
docker/cli/             Dockerfile of the CLI container image for CI (draft)
integrations/           unpublished CI drafts: GitHub Action, GitLab components,
                        fastlane plugin (docs/research/2026-11-12-ci-integrations.md)
.github/workflows/      ci.yml (fmt/clippy/test per OS + licences), release.yml
                        (tag v* → draft release; docs/dev/releasing.md)
docs/example/           preview images of the example project (README)
docs/gui/               screenshots of the editor (README)
```

### Architecture

`core` (no rendering, no network):

- `model` – typed project model (canvases, elements, styles, targets,
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
  only (ADR 0003); `layout::hyphen` – opt-in pattern hyphenation at
  `shrink_to`, project font fallbacks, optional system fonts (GUI
  setting) and per-language text overrides (`langs`, ADR 0009).
- `validate` – findings per target × language (texts, screenshots, store
  rules; ADR 0005).
- `colorspace` – image colour profiles (PNG `cICP`/`iCCP`/`sRGB`, JPEG
  ICC) → sRGB with `moxcms` (ADR 0021); used once at decode by
  `render::raster::SourceImage::decode` (asset cache, thumbnails) and
  `frame_import`; unusable profiles → info `color_profile_unsupported`.

`render` (tiny-skia, ADR 0006): `scene` – rasterize a resolved scene
(`RenderContext`, asset cache, prepared scenes); `frames` – parametric
device frames and browser chrome from the catalog; `text` – glyph outlines
of `core::layout` results; `export` – page/banner files, gallery previews,
incremental manifest (stale-file cleanup), `index.html` overview, fastlane
layout (paths/conflicts in `core::fastlane`, conventions in
`data/fastlane.yaml`); plus `paint`/`blur`/`raster` helpers.

`cli` – `vitrine new|render|export|validate|strings|import|frames|template` on
top of `core` + `render`, plus `vitrine mcp` (stdio server from `vitrine-mcp`).

`core::template` (ADR 0023) – template packages `.vitrine-template` (zip:
`template.yaml` manifest + project subset with one or more canvases, i18n,
generated placeholder screenshots, assets, OFL fonts + licence files,
previews): safe reading (`Limits`, paths, no symlinks, kinds by content,
project validated), `fonts` (OFL check from the name table, licence file
kept or generated), `placeholder`, `extract` (save canvases), `instantiate`
(new project; placeholders not copied), `insert` (merge plan: renames via
ops on a scratch template project, one transaction), `store` (app-wide
`<app data>/templates/`), `builtin` (`phone`, `minimal` from
`crates/core/templates/<id>/`). Previews: `vitrine_render::template`
(parametric frames only). App commands `template_*` (+ image
`template_preview`), `project_new {template}`. Gallery backend:
`crates/api-server` (ADR 0025; reuses core checks + render previews;
`routes/<area>.rs` per API area – `templates`, `admin`, `auth`; GitHub
device + web flow, own hashed session tokens, ownership by GitHub id,
`official` flag; env `VITRINE_API_*`; API `docs/api/API.md`, deploy
`docs/api/DEPLOY.md`). Gallery client: `publish::gallery` (`api_url`:
`--api`, `VITRINE_API_URL` loopback only, else production;
`GalleryError::Unreachable`), app commands `gallery_list|gallery_download`
+ image `gallery_preview` (error codes `unreachable`, `remote`), CLI
`vitrine gallery`; session token in the keychain via
`SecretStore::{set,get,delete}_named` (`gallery/<api host>`).

`core::frame_import` (ADR 0012) – Apple Product Bezels imported by the
user: package (folder/zip/DMG), name rules in
`data/frame-import/apple.yaml`, screen detection, app-wide store
`<app data>/frames/` (`VITRINE_APP_DIR` overrides the app folder, GUI and
CLI), one bezel per vendor model (iPhone 18 Pro and Pro Max side by side);
Own frame packs (ADR 0024, `core::frame_import::pack`): `frame.yaml` +
PNG/SVG in a project's `frames/<id>/` or `<app data>/frames/packs/<id>/`,
compiled (bezel + mask + index, stamp) into `.cache/frames/<id>/` /
`.compiled/`, added to the `FrameIndex` as source `pack:<id>` with its own
folder; SVG via `vitrine_render::frame_packs::ResvgRasterizer`; one entry
point `vitrine_render::frame_packs::frame_index(frames_root, project)`;
`frame: pack:<id>` resolves like an imported frame.
`frame: imported:apple` resolves to it (bezel picked per target page size,
screenshot size, aspect, or pinned with `bezel:`) or falls back to the
parametric frame (info `imported_frame_missing`).

`app` (ADR 0008, split out in Phase 4, ADR 0013) – the transport-agnostic
application layer shared by GUI and MCP (no Tauri dependency): `commands` (plain
functions `fn(&AppState, Args) -> Result<T, CommandError>`, `dispatch` by
name, one module per area: project, edit, inspect, add, texts, strings,
assets, targets, project_settings, render, export, catalog, os, system;
a new command goes into the `command_table!` in
`crates/app/src/commands/mod.rs` *and* the `json_commands!`/
`image_commands!` + `generate_handler!` lists in `crates/gui/src/main.rs`,
which only wraps them as Tauri commands; GUI-only commands without an app
counterpart: `menu_set`, `update_status|check|install` in
`crates/gui/src/updater.rs`), `cli_install` ("Install command line tool":
macOS symlink /usr/local/bin, Windows user PATH, `VITRINE_CLI_LINK_DIR` /
`with_cli_link_dir` redirect it; commands `cli_install_status|cli_install`,
CLI `vitrine install-cli`; ADR 0026), `session` (open project,
apply/undo/redo with atomic save per transaction, revision conflicts, live
reload via `watch`; every step emits `project-edited` with its `actor`),
`actor` (`Actor::User | Assistant {client, name}`: history labels
prefixed with the MCP client's display name, e.g. "Claude Desktop: ",
`client_display_name`; set per thread with `Actor::scope` / `dispatch_as`), `view` (`ViewState`:
canvas/target/lang/variant/selection pushed by the GUI via `view_set`,
read via `view_get`), `preview` (binary preview payload + cache),
`settings` (settings.yaml + recent projects in SQLite), `events`,
`dialogs`, `mcp` (`McpStatus` of the GUI-hosted endpoint, `mcp-status`
event, `McpControl` hook), `claude_desktop` ("Add to Claude Desktop":
order-keeping JSON merge with backup; `VITRINE_CLAUDE_DESKTOP_CONFIG`
redirects it – never write the real config in tests), `stores` (app-wide
store accounts in `settings.yaml`, `ConnectionTester` seam; commands
`store_account_*`, `store_apps_list` (Apple app picker, session cache),
`store_package_check` (Play: temporary edit, deleted), `open_url`
(allow-listed help links), `project_store_get/patch`, `store_config_get`; ADR
0016), `store_test` (`StoreClientTester`: read-only connection test with
the store clients, installed by GUI + dev bridge only; dev bridge
endpoint overrides `VITRINE_ASC_BASE_URL`/`VITRINE_PLAY_TOKEN_URL`,
loopback only). `publish::credentials` validates key files and wraps the keychain
(`keyring`, service `app.vitrinestudio`, user `<id>/<kind>`;
`VITRINE_KEYRING=mock` = in-memory, `VITRINE_KEYRING_SERVICE` renames;
unit tests use `SecretStore::mock()`, never touch the real keychain).
`publish::{apple, google}` (ADR 0017) – App Store Connect (ES256 JWT,
screenshot sets: reserve/upload parts/commit MD5/poll/reorder; App Asset
Library in `apple::library`, ADR 0019) and Google
Play (service-account OAuth, edits, `deleteall`, upload, commit) REST
clients with retry/backoff, cancellation, observer and progress hooks;
no network in tests (wiremock, `crates/publish/tests/`).
Project store mapping: `store:` in `project.yaml` (`core::model::store`,
op `set_store`, locale defaults `data/store-locales.yaml`). `app::upload`
(ADR 0018) – plan → confirm → execute: `plan` (incremental render into
`.cache/upload/images`, SHA-256/MD5, compare with the log or the store
(`remote`), blocking problems, confirm token stored with a fingerprint in
the log), `log` (`<project>/.cache/uploads.sqlite`: plans, runs, sets,
files, redacted events), `remote` (clients from account + keychain,
`AppleUploadFlow` trait, flow selection `store.apple.upload_flow`
auto|asset_library|screenshot_sets via `open_apple`/`route`), `library`
(`AssetLibraryFlow`, ADR 0019: placement groups resolved at runtime from
the ref data with `core/data/asset-library.yaml`, content marker in
`referenceName`; `auto` = sets + Duo/creative via the library), `exec` (run thread: Apple per set,
Google one edit, progress events `upload-progress`/`upload-finished`,
cancel, rate-limit pauses; resume = new plan, done sets are unchanged),
`policy` (MCP rule table: GUI-hosted needs the user's "Confirm upload"
click via `upload-confirm-requested` → `upload_confirm`, headless only with
setting `mcp_allow_headless_upload`). `readiness` (setup checklist with `fix` place, also in every plan).
Commands `upload_readiness|plan|execute|status|
cancel|log|confirm|refdata` (`refdata` = read-only App Store asset types); `UploadConfig` on `AppState::upload()` (endpoints via
`VITRINE_ASC_BASE_URL`… loopback only in CLI/dev bridge; tests set it). `mcp` (ADR 0014,
0015) – `VitrineMcp` over an `Arc<AppState>` (headless: own state; GUI:
the live one, `Host::Gui`: omitted target/lang default to the GUI's view,
sessions/tool calls go into the MCP status). `host` (feature `http`):
`McpHost` serves it on 127.0.0.1 (hyper, own tokio runtime, bearer
token, Host/Origin checks, discovery file `<app data>/mcp.json`);
`discovery`; `proxy` (feature `proxy`, CLI): `vitrine mcp` forwards
stdio JSON-RPC to the GUI (GUI autostart, `VITRINE_GUI` overrides the
binary), else headless. Tools
live in `tools/<area>.rs`, one `#[tool_router(router = <area>_tools)]` impl
block each (combined in `VitrineMcp::new`); arguments are
`schemars::JsonSchema` structs (doc comments = schema descriptions,
`deny_unknown_fields`). Every tool body runs in `VitrineMcp::run`
(`spawn_blocking`, `Actor::Assistant {client, name}` from `clientInfo`,
`sync_with_disk`) and calls
`vitrine_app::commands` by name (`server::cmd` / `image_cmd`), never the
model; results via `output::ToolOutput` (structuredContent + text, PNG
image blocks), errors via `output::error_result`. A new tool: add it to
the matching `tools/*.rs` block and to the tool list in
`docs/guide/NOTES.md`; the server `instructions` are
`crates/mcp/src/instructions.md`, the op reference `crates/mcp/src/ops.md`.
`gui` – Tauri binary (`main.rs`: wrappers, Tauri `EventSink`/
`Dialogs`) and `bridge` (dev HTTP bridge, feature `devbridge`); its library
re-exports the `app` modules (`vitrine_gui::commands` …). UI transport
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
- **Platform-gated code must be lint-clean on every OS** (CI runs clippy on
  macOS, Linux and Windows). Keep `cfg` branches symmetric: an item, enum
  variant, import or helper used only inside a `#[cfg(target_os = "macos")]`
  (or `unix`) branch is gated the same way, otherwise it is dead code
  elsewhere and `-D warnings` fails. Check other targets from the Mac with
  the cross-clippy commands under Commands when touching `cfg` code
  (Linux without the GUI; CI covers the rest).
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
- Licence: **GPL-3.0-or-later** (`LICENSE`, ADR 0022), set once in
  `[workspace.package]` (`license.workspace = true` in every crate);
  `publish = false` until the crates.io names are settled. Contributions need
  the CLA (`CLA.md`, `CONTRIBUTING.md`). No SPDX headers per file. New
  dependencies and bundled data need a GPL-3.0-compatible licence (MIT,
  Apache-2.0, BSD, ISC, Zlib, MPL-2.0, Unicode-3.0, CC0, OFL for fonts) and
  their notice; name, logo and icons (`assets/brand/`) are not GPL.
- **Document as you go.** Every user-facing feature or finding adds short
  bullet notes to `docs/guide/NOTES.md` (English, notes only – the user guide
  is written later). Decisions taken with the user and the current phase state
  go into `docs/plan/STATUS.md`; read it first when resuming work.

## Commands

```sh
cargo build                                  # build everything
cargo test --all                             # all tests
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all
# cross-clippy for Windows from the Mac (one-time: brew install mingw-w64,
# rustup target add x86_64-pc-windows-gnu); verified 2026-11-10
CARGO_TARGET_DIR=target/cross cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets --all-features -- -D warnings
# Linux (one-time: brew install messense/macos-cross-toolchains/x86_64-unknown-linux-gnu,
# rustup target add x86_64-unknown-linux-gnu); excludes the GUI (needs GTK); verified 2026-11-10
CARGO_TARGET_DIR=target/cross cargo clippy --target x86_64-unknown-linux-gnu --workspace --exclude vitrine-gui --all-targets --all-features -- -D warnings
cargo deny check licenses                    # licence allow-list (deny.toml), also a CI job
cargo run -p vitrine-cli -- --help          # CLI (binary `vitrine`)
cargo run -p vitrine-gui                    # start the GUI (start screen)
cargo run -p vitrine-gui -- <project-dir>   # … opening a project
# dev HTTP bridge: the UI + all GUI commands in a normal browser (prints the URL;
# --port N, --copy = work on a temp copy, --app-dir, --pick-folder/--pick-file
# answer the folder/file dialogs; without --project it shows the start screen; ADR 0008)
cargo run -p vitrine-gui --features devbridge --bin vitrine-devbridge -- --project tests/fixtures/example --copy
cargo run --release -p vitrine-gui --features devbridge --bin vitrine-devbridge -- --port 7801 --pick-folder /tmp/x
cargo run -p vitrine-gui --release --example preview_transport   # preview payload measurements
VITRINE_UPDATE_GOLDEN=1 cargo test -p vitrine-render --test golden   # rewrite golden references (review the diff!)

# CLI on the example project (release build: a full export takes ~2 s)
cargo run -p vitrine-cli --release -- --project tests/fixtures/example validate
cargo run -p vitrine-cli --release -- --project tests/fixtures/example export
cargo run -p vitrine-cli --release -- --project tests/fixtures/example render --target ios-iphone-69 --gallery --max-edge 800 -o /path/to/gallery.png

# MCP: the GUI hosts it (ADR 0015); `vitrine mcp` proxies stdio to the running GUI
# (or starts it), `--headless` works on the folder directly
cargo run -p vitrine-cli -- mcp --headless --project tests/fixtures/example --app-dir /tmp/vitrine-mcp
cargo test -p vitrine-cli --test mcp          # drives the binary over stdio (JSON-RPC)
cargo test -p vitrine-cli --test mcp_proxy    # in-process GUI endpoint + `vitrine mcp` proxy
# dev bridge hosting MCP too (discovery file in <app-dir>/data/mcp.json); drive it with
# `vitrine mcp --app-dir <app-dir>` while the browser shows the edits live
cargo run -p vitrine-gui --features devbridge --bin vitrine-devbridge -- --project tests/fixtures/example --copy --mcp --app-dir /tmp/ab

# release bundle locally (unsigned; CI: .github/workflows/release.yml, docs/dev/releasing.md)
scripts/release/third-party-notices.sh crates/gui/bundle/licenses && scripts/release/build-cli-sidecar.sh aarch64-apple-darwin
(cd crates/gui && cargo tauri build --target aarch64-apple-darwin --bundles app --config tauri.bundle.conf.json)

# regenerate the example screenshots (deterministic fake app UI)
cargo run -p vitrine-render --release --example screens -- tests/fixtures/example/screenshots
```

CLI usage (`--project <DIR>`, default the current folder): `new <dir>
[--template <name|id|file>] [--lang …]`, `template save --name N
[--canvas ID]… [--tag] [--id] [--version] [--replace] [-o file] | list |
import <file> [--replace] | export <id> -o <file> | remove <id> | insert
<template> [--canvas ID]… [--styles rename|use-project] [--dry-run] | new`, `validate [--target] [--lang]
[--preview] [--strict] [--format text|json|github|gitlab|junit] [-o FILE]`, `render [--target] [--lang]
[--page N | --image N | --canvas | --gallery] [--width|--max-edge|--scale]
[-o file]`, `export [--target] [--lang] [--out dir] [--layout
standard|fastlane] [--no-gallery] [--no-html] [--force] [--format json]`, `strings export [--format csv|xliff12|xliff20]
[--lang] [-o] [--missing-only] [--context]`, `strings import <file>…
[--dry-run] [--create-keys] [--empty keep|remove]`, `import screenshots
<dir> [--format auto|vitrine|fastlane|flat] [--device-map|--lang-map|--key-map
A=B] [--dry-run] [--save-mapping]`, `frames import <path> [--dry-run]`, `frames
list`, `frames remove [--model] [--color] [--orientation] [--bezel] [--all]`
(synthetic test package: `cargo run -p vitrine-core --example
synthetic_bezels -- [--full-size] <dir|file.zip>`), `mcp [--app-dir DIR]
[--headless]` (MCP over stdio: proxy to the running GUI, GUI autostart,
else headless opening `--project` when it holds a `project.yaml`; never
print to stdout there), `upload plan [--store apple|google] [--target]
[--lang] [--remote] [--force] [--format json]`, `upload execute --confirm
<token> [--yes] [--changes-not-sent-for-review]`, `upload log [--last]
[--events]`, `upload refdata [--account] [--app] [--format json]`
(`upload` takes `--app-dir`; ADR 0018, 0019; CI credentials
`VITRINE_ASC_KEY_ID|_ISSUER_ID|_KEY|_KEY_PATH`,
`VITRINE_PLAY_SERVICE_ACCOUNT[_PATH]`, read by `upload` only, ADR 0016), `gallery login|logout|list
[--query] [--tag] [--format json]|upload <file> [--token-env VAR]|download
<id> [-o FILE]` (`--api URL`; ADR 0025), `install-cli [--remove] [--status]
[--yes]` (`vitrine` on PATH; ADR 0026). Exit codes: 0 ok, 1 project
problems, 2 could not run.

Golden tests (`crates/render/tests/golden.rs`) render every output image
of every target of `tests/fixtures/example` (en, plus `EXTRA_LANGS`: de
for the iPhone set and the blog header, fr for the iPhone and iPad sets)
at 25 % and the gallery previews at 320 px, and
compare them with `tests/fixtures/example/golden/`: a pixel differs above
8/255 in any channel, an image fails above 0.5 % differing pixels
(constants at the top of the file). On failure they write `*.actual.png` /
`*.diff.png` next to the reference (ignored by Git). With
`VITRINE_UPDATE_GOLDEN=1` they rewrite references that are missing or
outside the tolerance and delete stale ones. They run in the normal debug
`cargo test` (dependencies are built with `opt-level = 2`, our crates are
not; ~7 s on 10 cores). Changing the fixture or the renderer visibly means
updating the references in the same commit. `crates/cli/tests/scenario.rs`
is the Phase 3 "done when" check (3 languages × 3 targets exported with
fallbacks, overflow reported).

Cross-platform tests: paths via `Path::join`, never compare paths as
strings; line endings are LF (`.gitattributes`), tests reading checked-in
text normalise `\r\n` anyway.
