# 0015 – GUI-hosted MCP, stdio proxy and Claude Desktop integration

- Status: accepted
- Date: 2026-11-10

## Context

The user's requirement (2026-11-10): "I start the application, the MCP is
available, Claude connects and works. No terminal, no manually started
server. I see changes live and comment in Claude Desktop." ADR 0014 built
the MCP server on `arrangia-app` with two seams for this:
`arrangia_mcp::http::service` and `stdio::discover_gui`. Claude Desktop
only starts stdio servers from its config file; Claude Code can also talk
Streamable HTTP directly.

## Decision

### In-app endpoint (`arrangia_mcp::host`, feature `http`)

`McpHost::install(app, relay, options)` runs in the GUI (Tauri `setup`)
and in the dev bridge (`--mcp`). It serves rmcp's Streamable HTTP tower
service for `ArrangiaMcp::new(app, relay, Host::Gui)` – the GUI's live
`AppState`, so every MCP session shares the session the user sees – with
hyper 1 (`http1`, `hyper-util` `TokioIo`) on its own two-thread tokio
runtime (independent of Tauri's runtime; the dev bridge has none). The
app's event sink is a `ProgressRelay` wrapping the Tauri sink (export
progress → MCP progress notifications).

- Address: `127.0.0.1`, the preferred port from the setting `mcp_port`
  when free, else a random free port. Path `/mcp`; `/health` answers
  `{name, version, pid}`.
- Every request needs a loopback `Host` with our port (DNS rebinding;
  rmcp checks the host name again), no `Origin` or a loopback `http://`
  one (`null` and every web origin are refused), and `Authorization:
  Bearer <token>`: 32 random bytes (`getrandom`) base64url, new on every
  start, compared in constant time. Failures: 403 (host/origin), 401
  (token, with `WWW-Authenticate: Bearer`).
- Discovery file `<app data dir>/mcp.json` (`{url, port, token, pid,
  version, started_at}`), written atomically with mode `0600` on Unix,
  removed on stop and on `RunEvent::Exit` (only if it still holds our
  token: a newer GUI may have replaced it). A crash leaves it behind;
  readers ignore it when `/health` does not answer and delete it when the
  pid is gone (`kill -0` / `tasklist`). The newest GUI wins when two run.
- The host registers itself as the app's `McpControl`; the command
  `mcp_set_enabled` saves the setting `mcp_enabled` (default on) and
  starts/stops the endpoint live (stop ends all sessions, frees the port,
  removes the file, clears the token).

### Status and live aids (`arrangia-app` stays MCP-free)

`AppState` holds an `McpStatus` (`hosted, enabled, running, url, port,
error, discovery_file, clients[], last_activity`); `update_mcp_status`
emits `mcp-status` (never with the token; `mcp_status` the command adds
the token, the Claude Code commands and the CLI path). The server reports
into it only when `Host::Gui`:

- `initialize` (hand-written `ServerHandler::initialize`) opens a session
  for `clientInfo.name`; every session gets its own server clone
  (`ArrangiaMcp::for_session`) holding a guard whose `Drop` (session
  closed by `DELETE` or expired) closes it again;
- every tool call records client, tool and time. A client counts as
  connected while it has a session or was active in the last 5 minutes
  (stateless protocol versions have no sessions).

`project_open`/`project_new` emit `project-opened` `{dir, name, actor}`,
`project_close` `project-closed`; the UI follows them when the actor is
Claude (start screen → editor). `project-edited` with actor Claude makes
the UI refresh like after its own edit (`editor.remoteEdit`; full reload
when targets, languages or strips were added), outline the touched
elements on the canvas for 1.5 s (`--claude` colour, SVG overlay; opacity
animated per frame because the CSP forbids inline styles), stack a toast
"Claude: <label>" (click selects the elements) and add the step to the
activity list behind the top-bar indicator (off / ready / connected with
the client's name; undo button for the latest step). History labels
already read "Claude: …" (ADR 0013), so the undo tooltip shows them.

GUI-hosted tool calls run with the GUI's `ViewState` in a thread-local
(`params::with_gui_view`): omitted `target` defaults to the target the
user looks at (when it shows the strip in question), omitted `lang` to the
GUI's language (edit context, `render_preview`, `layout_report`,
`element_add`). Headless keeps the first target / default language.

### `arrangia mcp`: stdio proxy (`arrangia_mcp::proxy`, feature `proxy`)

Without `--headless`: discovery file + health check → proxy. None found
and setting `mcp_autostart_gui` (default on) → start the GUI (`ARRANGIA_GUI`
override; macOS app bundle around the CLI → `open -a <bundle> --args
<project>`; else `arrangia-gui` next to the CLI binary), in its own process
group with null stdio and `ARRANGIA_APP_DIR` = `--app-dir`, wait up to 20 s
for its endpoint, then proxy. Otherwise headless as before, with a line on
stderr.

The proxy is a transparent JSON-RPC forwarder, not an rmcp client+server
pair: rebuilding every request type through typed handlers would lose
fidelity (unknown methods, `_meta`, progress tokens) and would replace the
client's `clientInfo` with the proxy's. Each stdin line is POSTed as is
(`Accept: application/json, text/event-stream`, bearer token); a JSON body
or every SSE `data` event (progress notifications, then the result) goes
to stdout as one line. `initialize` is sent alone first (its
`Mcp-Session-Id` is kept); its `protocolVersion` – or a request's `_meta`
protocol version – becomes `MCP-Protocol-Version`, and `Mcp-Method` /
`Mcp-Name` (SEP-2243) are always set. Other messages run concurrently
(reqwest, `no_proxy`). A refused connection or an unknown session (GUI
restarted) → read the discovery file again, replay `initialize` +
`notifications/initialized`, retry once. HTTP errors on requests become
JSON-RPC errors (`-32603`), so clients never hang. stdin EOF: wait for
in-flight requests (≤ 60 s), `DELETE` the session. Server-initiated
messages outside requests (the optional `GET` stream) are not forwarded;
Arrangia sends none.

### Claude Desktop / Claude Code (`arrangia_app::claude_desktop`)

"Add to Claude Desktop" writes `mcpServers.arrangia = {command: "<abs
path of arrangia>", args: ["mcp"]}` into
`~/Library/Application Support/Claude/claude_desktop_config.json` (macOS),
`%APPDATA%\Claude\claude_desktop_config.json` (Windows),
`~/.config/Claude/claude_desktop_config.json` (Linux). `claude_desktop_plan`
returns path, state (`missing`/`same`/`different`/`invalid`), current and
new entry and the whole new file; the UI shows it and asks;
`claude_desktop_install` refuses without `confirm: true`, never touches an
invalid file, is a no-op when the entry is identical, and otherwise copies
the file to `claude_desktop_config.json.bak-<UTC YYYYMMDD-HHMMSS>` and
writes atomically. Key order is preserved (an order-keeping JSON value;
the workspace's `serde_json` sorts keys) with 2-space indentation. Tests
and the dev bridge point it elsewhere (`ARRANGIA_CLAUDE_DESKTOP_CONFIG`,
`AppState::with_claude_desktop_config`; the dev bridge always uses
`<app-dir>/claude_desktop_config.json`).

The CLI path is `ARRANGIA_CLI` or `arrangia` next to the running
executable (dev: `target/<profile>/arrangia` next to `arrangia-gui`).
TODO (Phase 6): verify for bundled builds (CLI inside the app bundle).
Claude Code gets two copyable commands: `claude mcp add arrangia --
<cli> mcp` (recommended; always works) and `claude mcp add --transport
http arrangia http://127.0.0.1:<port>/mcp --header "Authorization: Bearer
<token>"` (valid until the next start; the port can be fixed in the
settings, the token cannot).

## Consequences

- The normal path needs no terminal: start Arrangia, "Add to Claude
  Desktop", restart Claude Desktop. Claude's edits appear live, labelled,
  undoable.
- The token protects against other local users' and web pages' requests;
  any process of the same user can read `mcp.json` (same trust level as
  the project files).
- New dependencies: hyper/hyper-util/http-body-util/bytes/tower-service/
  tokio-util/getrandom (feature `http`, all already in the lock file via
  rmcp/reqwest), reqwest in `arrangia-mcp` (feature `proxy`).
- Tests: `crates/cli/tests/mcp_proxy.rs` hosts the endpoint in-process
  and drives the real `arrangia mcp` binary (proxy, GUI restart,
  autostart with a fake GUI script, headless fallback, token/host/origin
  checks, on/off); unit tests for the config merge, discovery file and
  status.
