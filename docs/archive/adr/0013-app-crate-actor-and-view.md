# 0013 – Shared application crate, actors and view state

- Status: accepted
- Date: 2026-11-10

## Context

Phase 4 lets Claude edit projects over MCP. The user decided that the GUI
hosts the MCP server, so Claude's edits run through the GUI's own session:
visible live, undoable, and labelled (`docs/plan/phase-4.md`). The session
and command layer of ADR 0008 lived in `vitrine-gui`, next to Tauri. MCP
(and the headless `vitrine mcp --project`) need exactly the same
behaviour – ops, edit intents, validation, revision checks, atomic saves –
without depending on Tauri.

## Decision

### Crate split

New crate `crates/app` (`vitrine-app`, `publish = false`, workspace
lints, no Tauri dependency – `cargo tree -p vitrine-app | grep -i tauri`
is empty). It holds everything of ADR 0008 that is not tied to a
transport: `commands` (functions, `dispatch` table, args/results,
`CommandError`), `session`, `preview`, `settings` (YAML + recent projects
in SQLite, `VITRINE_APP_DIR`), `watch`, `events` (`EventSink`),
`dialogs` (`Dialogs` trait, `NoDialogs`, `FixedDialogs`), `testutil`, plus
the new `actor` and `view`. The unit tests moved with the code.

`vitrine-gui` keeps the Tauri binary (`main.rs`: generated wrappers,
Tauri `EventSink`/`Dialogs`), the dev bridge (`bridge.rs`,
`bin/vitrine-devbridge.rs`, its tests) and `ui/`. Its library re-exports
the `vitrine-app` modules, so `vitrine_gui::commands` etc. still work.
The move itself changed no behaviour. The CLI keeps its direct
core/render paths.

### Actor

`Actor` (`{"kind":"user"}` | `{"kind":"claude","client":…}`, default user)
says who makes a change. Commands keep their signature
`fn(&AppState, Args)`; the actor is per thread: a front end runs commands
inside `Actor::scope(actor, …)` or calls `commands::dispatch_as(app,
actor, name, args)`, and `Session::apply_ops`/`undo`/`redo` take
`Actor::current()`. `Session::apply_ops_with(ops, ApplyOptions { label,
actor })` (and `undo_as`/`redo_as`) take it explicitly. Steps by Claude
get the history label prefix `Claude: ` (e.g. `Claude: move title`)
(since 2026-11-10 `Actor::Assistant { client, name }` with the client's
display name as prefix, e.g. `Claude Code: move title`; ADR 0015
amendment). A
thread-local was chosen over an argument on every command because ~25
commands apply ops; the Tauri and bridge workers run each command on one
thread, so the scope cannot leak.

### Change notification

Every applied, undone or redone step emits `project-edited`
(`ProjectEdited`: `action` apply/undo/redo, `actor`, `label`,
`revision`, `changes` – incl. `changes.elements`, the touched element
ids). `project-changed` stays reserved for reloads (external edit,
conflict, assets), so the UI does not refresh twice for its own edits.
The GUI can use `project-edited` to highlight Claude's changes and to
fill an activity list (4.3b).

### View state

`ViewState { canvas, target, lang, variant, selection }` lives in the
session (reset when a project is opened). `variant` follows the view
context of the edit commands (missing = the target's own, `null` = base,
a name = that variant). The GUI pushes it with `view_set` 250 ms after its
context or selection last changed (`ui/js/main.js`); `view_get` reads it
(for the MCP tools `view_get` / `selection_get`). The session does not
validate it against the project.

## Consequences

- MCP tools are thin adapters over `vitrine_app::commands` against the
  live `AppState` of the GUI or a headless one; no second code path.
- `vitrine-app` builds without Tauri (fast, also on CI runners without
  web view libraries).
- Edits made outside an `Actor::scope` count as the user's; the MCP
  server must wrap its calls.
