# 0008 – GUI session, commands and preview transport

- Status: accepted (amended by ADR 0013: the library part moved to `vitrine-app`)
- Date: 2026-11-09

## Context

Phase 2 (tasks 2.2/2.3) needs a GUI backend that keeps the project on disk
as the source of truth (KONZEPT §8.3), renders previews in Rust (§6.1,
§9.3) and can be driven without the Tauri window for automated and visual
checks in a normal browser.

## Decision

### Library + thin Tauri wrappers

`vitrine-gui` is a library (`session`, `commands`, `preview`, `settings`,
`watch`, `events`, `dialogs`, `bridge`) plus the Tauri binary. Every
command is a plain function `fn(&AppState, Args) -> Result<T,
CommandError>` with serde `Args`/`T`; `commands::dispatch(name, json)` is
the table used by the bridge, `main.rs` only has macro-generated
`#[tauri::command]` wrappers that run the same functions on a blocking
worker thread. JS calls `invoke(name, {args})`; errors are
`{code, message, details}` (`no_project`, `invalid_args`, `not_found`,
`op_rejected`, `conflict`, `load`, `io`, `render`, `busy`, …). Events go
through an `EventSink` trait (Tauri `emit`, bridge SSE); dialogs through a
`Dialogs` trait (Tauri: `tauri-plugin-dialog` called from Rust only, the
web view keeps `core:default`; bridge: fixed answers).

Planned commands slot in the same way: `edit_*` (gesture intents from
`core::edit`, answering with the `apply_ops` result) and `overlay`
(hit-test/overlay data per target × language). The inspector (task 2.6)
adds `element_get` (base / override / effective values plus the text in
the view language), `element_patch` (fields → base or variant override,
decided in core; optional translations in the same step), `edit_rename`,
`edit_arrange` (layer tree), `canvas_get`, `canvas_patch` (aspect, page
count, page ids, backgrounds) and `canvas_variant` (add / remove / rename
/ content scale). The UI never builds override ops itself.

### Session

- One open project per `AppState` (a map of sessions per window can come
  later). `Session` = `Project` + core `History` + canonical folder +
  `RenderContext` + preview cache.
- `apply_ops`/`undo`/`redo` work on copies of project and history, save the
  copy (`Project::save`: revision check, atomic writes) and commit only on
  success. A revision conflict reloads from disk, emits `project-changed`
  (`reason: conflict`) and returns `conflict`; other save errors change
  nothing. The result reports the new revision and what changed (diff of
  old/new project: canvases, elements, strings, targets, flags), plus the
  history state with undo labels kept beside the core history.
- `notify` + `notify-debouncer-full` (150 ms) watch the folder. Model files
  (`project.yaml`, `i18n/*.yaml`) are reloaded; our own writes are
  recognised because the folder then loads to exactly the in-memory
  project (no event). External changes replace the model, clear the undo
  history and emit `project-changed` (`reason: external`); a file that does
  not parse emits `project-error` and keeps the model. Screenshot/asset
  changes drop the previews using them; font changes rebuild the render
  context (`reason: assets`).

### Preview cache

Key: target, language, view (image N / canvas / gallery), size, quality,
format and a hash of the resolved scene incl. asset mtimes. Ops thus
invalidate exactly the previews whose scene changed (a miss with a new hash
drops the stale entries of the same view); asset events drop entries by
asset path. LRU eviction above 256 MiB of encoded data. `interactive: true`
renders at half size for drags.

### Transport format (measured)

`cargo run -p vitrine-gui --release --example preview_transport`
(Apple Silicon, release, example project, 5 runs averaged):

| image | size | render | raw RGBA | PNG Fast/Up | encode | decode |
|---|---|---|---|---|---|---|
| iPhone page | 184×400 | 4.4 ms | 287 KiB | 74 KiB | 0.3 ms | 0.3 ms |
| iPhone page | 368×800 | 12.3 ms | 1150 KiB | 189 KiB | 0.7 ms | 0.7 ms |
| iPad group | 800×600 | 16.9 ms | 1875 KiB | 272 KiB | 1.0 ms | 1.1 ms |
| Mac | 800×500 | 13.8 ms | 1562 KiB | 263 KiB | 0.8 ms | 0.9 ms |

`Balanced`/adaptive saves another ~40 % but costs 14–17 ms at 800 px
(more than the render). **Default: PNG, `Compression::Fast`, `Up` filter**
– encoding adds < 10 % to the render time, the payload is ~6× smaller than
raw RGBA (matters for the IPC copy and the browser bridge), and decoding
happens off the main thread via `createImageBitmap`. Raw RGBA (straight
alpha) stays available (`format: "rgba"`) for cases where decode latency
matters more than bytes.

Payload: Tauri returns `tauri::ipc::Response` (an `ArrayBuffer` in JS)
with a 40-byte little-endian header – magic `ARPX`, width, height, format
(0 RGBA, 1 PNG) as u32, then canvas origin x/y and px-per-lu scale as f64 –
followed by the data. The bridge sends the bare PNG/RGBA with
`Content-Type` and `X-Width`, `X-Height`, `X-Format`, `X-Origin-X`,
`X-Origin-Y`, `X-Scale` headers (so `curl -o page.png` works).
`ui/js/transport.js` hides the difference.

### Dev HTTP bridge

`vitrine-devbridge` (feature `devbridge`, `required-features`, so neither
built by default nor bundled) on a minimal blocking HTTP/1.1 server over
`std::net`: no dependency, no async runtime; one thread per connection
and one request per connection (`Connection: close`; SSE streams block
their thread). Amended in 2.11: the first version used `tiny_http`,
whose connection pool could leave new connections unserved behind
threads blocked on idle keep-alive sockets (a page reload after a long
export stalled for over a minute on a stylesheet).
`GET /` serves `crates/gui/ui/`, `POST /cmd/<name>` → `dispatch`,
`GET /events` → server-sent events. Binds `127.0.0.1` only, port 0 or
`--port`; app data goes to a temp folder unless `--app-dir` is given;
`--copy` opens a temporary copy of `--project`. The integration tests
enable the feature through a self dev-dependency.

### App data

`settings.yaml` (UI language, default project language, default frame
style, window state) in the platform config folder (`directories`);
recent projects in `app.sqlite` (`rusqlite`, bundled) in the platform data
folder.

### Shared templates

The project templates moved from `crates/cli` to `vitrine_core::templates`
(additive) so `project_new` and `vitrine new` create identical projects.
`render::export` got optional progress/cancel hooks (`ExportHooks`) for the
`export` command's `export-progress` events and `export_cancel`.

## Consequences

- The UI can be developed and checked in Chrome against real commands; the
  Tauri layer has no logic of its own (a unit test checks every library
  command has a wrapper).
- External edits clear the undo history (ops recorded against the old
  model may no longer apply). A per-step persistent history
  (`.cache/history.sqlite`, §10.2) remains open.
- The fixture can be opened directly by the bridge; edits then change it –
  use `--copy` for anything that edits.
