# 0014 – MCP server design

- Status: accepted (GUI hosting, proxy and discovery: ADR 0015)
- Date: 2026-11-10

## Context

Phase 4 lets Claude create, design, translate and export projects over MCP
(KONZEPT §8). The user decided that the GUI hosts the server so Claude's
edits run through the session the user sees (ADR 0013,
`docs/plan/phase-4.md`); `arrangia mcp` must also work without a GUI
(CI, power users). The official Rust SDK `rmcp` is in the workspace.

## Decision

### SDK

`rmcp = "=3.9.0"` (exact pin; the API moves quickly), features `server`,
`macros`, `schemars`, `transport-io`; `arrangia-mcp`'s feature `http`
adds `transport-streamable-http-server` for the GUI. Tool arguments are
structs deriving `schemars::JsonSchema` (schemars 1.x, as rmcp) with
`deny_unknown_fields`; doc comments become the schema descriptions, serde
defaults are documented there. Each area has its own `#[tool_router(router
= …_tools)]` impl block on `ArrangiaMcp`; `ArrangiaMcp::new` adds them up.
`ServerHandler` is implemented by hand (not `#[tool_handler]`) to add
resources and to turn argument errors into our structured tool errors.

### One handler, two hosts

`ArrangiaMcp { app: Arc<AppState>, progress: Option<Arc<ProgressRelay>>,
host: Host, tool_router }` owns no project state. Headless
(`stdio::headless_server`) builds its own `AppState` (file watcher on,
`NoDialogs`, events into a `ProgressRelay` over `NullSink`); the GUI
(4.3b) passes its live `AppState` with `Host::Gui` and a `ProgressRelay`
wrapping its Tauri sink, and serves `http::service(server, config)`
(rmcp `StreamableHttpService`, loopback `Host` only; the GUI adds the
bearer token and binds `127.0.0.1`). Every MCP session gets a clone of the
server, so all share the one session.

`arrangia mcp [--project DIR] [--app-dir DIR] [--headless]` (CLI) calls
`stdio::run`: it first asks `discover_gui` for a running GUI (4.3b: the
discovery file in the app data folder; returns `None` today) and would
then proxy stdio to it; otherwise it serves headless over stdio. The
project opens at start when `--project` (default `.`) holds a
`project.yaml`.

### Tool calls

Every tool runs `ArrangiaMcp::run`: client name from `clientInfo`
(`RequestContext::client_info`, per request in the 2026-07-28 protocol,
from `initialize` before) → `spawn_blocking` → `Actor::scope(Actor::Claude
{client}, …)` → `sync_with_disk` → the tool body, which calls
`arrangia_app::commands` by name (`dispatch`) and never touches the model.
So history labels read `Claude: …` and `project-edited` events name
Claude (ADR 0013).

`sync_with_disk` calls `Session::reload` (cheap, catches external edits
before the debounced watcher does) and compares the revision on disk with
the session's: a `project.yaml` that changed but does not load is error
`load` ("does not load: …; nothing was changed") instead of stale reads or
a later conflict. A save that still races an external writer returns the
session's `conflict` error (the session reloads; the hint says to read
again and retry).

### Results

`ToolOutput { summary, data, images }` → `CallToolResult` with
`structuredContent` (always an object) and one text block "summary +
compact JSON" (for clients that ignore `structuredContent`); PNGs follow
as image blocks. Outputs are compact: summaries with ids, edit results as
`{applied, revision, step (undo label), created, changed}`; full YAML only
through resources. `CommandError` → tool error (`isError: true`) with
`structuredContent {error: {code, message, details}, hint}` and the text
`error (code): message` + hint. Wrong arguments (rmcp's "failed to
deserialize parameters") become error `invalid_args` the same way, never
a protocol error the client might hide.

### Tool design

- Edit context: `target` (default: the first target showing the
  element's strip), `scope` `base` (default) | `variant` | `lang`,
  `variant`, `lang`. MCP edits default to the base elements; the GUI's
  default (the target's own variant) would silently write variant
  overrides when the first target is the iPad set.
- Gestures use the core edit intents (`edit_move`, `edit_resize`, …).
  `element_move` takes the new centre relative to the top-left of a page
  (default: the home page) and converts it to canvas lu with the overlay
  page rects of the target; `dx/dy` move several elements.
- `element_add` got `id`, `key` and `fields` (YAML fields merged over the
  defaults) in `arrangia-app` so one call builds a finished element in one
  undo step (the GUI does not send them).
- Batch tools build ops and use `apply_ops`: `strings_set` (only changed
  values, one step), `strip_add`, `string_key_rename`.
- `styles_update` merges text/span/shadow entries field by field.
- `render_preview` (view image | canvas | gallery, `max_edge` default
  1200) returns the PNG plus size, image count and the errors/warnings of
  that target × language; `layout_report` comes from the overlay data
  (page-relative boxes, font sizes, lines, overflow, hyphenation,
  emergency breaks, `in_gap`, text overlaps).
- `export` forwards `export-progress` events as MCP progress
  notifications when the request has a progress token; queued
  notifications are sent before the result.
- Not exposed: frame import/removal (the app-wide frame store is read-only
  for MCP), dialogs, settings, upload (Phase 5).

### Resources and instructions

`arrangia://project/project.yaml`, `arrangia://project/i18n/{lang}.yaml`
(template; language tags are validated, so no path traversal),
`arrangia://catalog/devices`, `arrangia://catalog/targets`,
`arrangia://docs/ops` (hand-written op reference, `crates/mcp/src/ops.md`).
The server `instructions` (`crates/mcp/src/instructions.md`, ≈ 40 lines)
explain the concepts, markup, units, rotation sign and the workflow
"render_preview after every visual change and look at it".

### Safety

No network. Writes only through app commands: inside the project folder,
plus paths the caller passes explicitly (`export.out_dir`,
`strings_export.path`); imports read only paths the caller names;
`project_create` refuses non-empty folders.

## Consequences

- One code path for GUI, CLI-less MCP and future HTTP hosting; tests drive
  the real binary over stdio (`crates/cli/tests/mcp.rs`).
- The stdio proxy, discovery file, HTTP endpoint with token and the
  settings UI are Phase 4.3b; `discover_gui` and `http::service` are the
  seams.
- rmcp upgrades need a look at the macro and transport APIs (exact pin).
