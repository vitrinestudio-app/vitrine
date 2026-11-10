# Phase 4 – MCP server: task list

Scope: KONZEPT.md §8 and §12, Phase 4. Done when Claude can create a project
from scratch, design it, translate it and export it – purely via MCP.

Status markers: `[ ]` open · `[~]` in progress · `[x]` done

## Architecture (decided)
- **Shared application layer.** The transport-agnostic session/command layer
  currently in `crates/gui` (session, preview cache, commands, settings,
  watcher, events) moves into a new crate `arrangia-app` (`crates/app`, no
  Tauri dependency). `arrangia-gui` (Tauri + dev bridge) and `arrangia-mcp`
  both use it, so GUI and MCP share one behaviour: same ops, edit intents,
  validation, revision checks, atomic saves.
- **GUI hosts the MCP server (user requirement 2026-11-10).** No terminal
  needed: when the GUI runs, it serves MCP for the currently open project, and
  Claude's edits run through the GUI's own session – the user sees every change
  live, can undo it, and comments in Claude Desktop/Code.
  - In-app server: MCP Streamable HTTP on `127.0.0.1` (random port), bearer
    token, Origin/Host checks (DNS-rebinding protection). Can be switched
    off in app settings. Usable directly by Claude Code
    (`claude mcp add --transport http …`).
  - `arrangia mcp` (stdio) is a thin proxy that Claude Desktop starts by itself
    from its config: it discovers the running GUI (discovery file in the app
    data dir with port + token) and forwards everything to it. Without a
    running GUI it can start the GUI (setting) or work headless on
    `--project <dir>` (CI, power users).
  - GUI: app settings → "Claude / MCP": status (connected clients), a
    one-click "Add to Claude Desktop" (writes the `mcpServers` entry into
    Claude Desktop's config after confirmation, with backup) and the
    `claude mcp add` command for Claude Code to copy.
  - Live collaboration aids: changes by Claude are labelled in the undo
    history ("Claude: move title-plan") and briefly highlighted on the canvas;
    an activity list; `selection_get` tool so the user can select elements in
    the GUI and say "make this bigger"; `view_get` (current strip/target/
    language/variant) so Claude works on what the user is looking at.
- **Tool results:** structured JSON (`structuredContent` + short text);
  `render_preview` returns an image content block (PNG) so Claude sees the
  result.
- **Safety:** no network access in Phase 4 tools; upload tools come in Phase 5
  (`upload_plan`/`upload_execute` with confirm token). Asset import only from
  paths the user names; nothing deletes outside the project folder.

## 4.1 Extract `arrangia-app`
- [x] Move session/commands/preview/settings/watch/events/dialog traits from
      `crates/gui` into `crates/app`; gui keeps Tauri wrappers, bridge, UI
      (ADR 0013)
- [x] No behaviour change; all tests green; CLI keeps its own direct paths or
      uses app where it simplifies (CLI unchanged)
- [x] Prepared for MCP: `Actor` (history label "Claude: …"), `project-edited`
      event (action, actor, label, changed ids), `ViewState` with
      `view_set` (GUI pushes, debounced) / `view_get`

## 4.2 MCP server skeleton
- [x] `crates/mcp` with rmcp: tool router usable over Streamable HTTP (hosted
      by the GUI, against its live session) and stdio (proxy to the GUI, or
      headless with its own session); server info/instructions (short guide to
      Arrangia concepts for the model: canvas, home page, lu, gaps, anchors,
      variants, targets, markup), `arrangia mcp` CLI subcommand (ADR 0014;
      headless now, `discover_gui` + feature `http` (`http::service`) are the
      seams for 4.3b)
- [x] Project handling: `project_open`, `project_create` (templates),
      `project_summary` (compact: strips, pages, elements, languages, targets,
      revision)
- [x] Resources: `project.yaml`, `i18n/<lang>.yaml`, device catalog, target
      presets (read-only)

## 4.3 Tools (KONZEPT §8.2 + what Phases 1–3 added)
- [x] Catalog: `catalog_devices`, `catalog_targets`, `catalog_fonts`,
      `catalog_galleries`
- [x] Strips/targets: `strip_add`, `strip_update`, `target_add` (from preset),
      `target_update`, `target_remove`
- [x] Elements: `element_add`, `element_update` (patch; base/variant/language
      scope), `element_remove`, `element_list`, `element_move`/`resize`/
      `rotate` via core edit intents, `apply_ops` (atomic, one undo step)
- [x] Strings: `strings_get`, `strings_set` (batch), `strings_missing`
      (for "translate everything missing into fr"), key rename
- [x] Assets: `assets_list`, `screenshots_import` (folder/fastlane plan +
      apply), `frames_list` (imported bezels, read-only)
- [x] Output: `render_preview` (page/image, canvas, store gallery; target,
      language, max edge) → PNG image content; `layout_report` (boxes, font
      sizes, overflows, hyphenation as text/JSON); `validate`; `export`
- [x] Exchange: `strings_export`/`strings_import` (CSV/XLIFF) optional
- [x] Undo/redo tools (`history_undo`, `history_redo`) for the MCP session

## 4.3b GUI hosting + Claude Desktop integration (ADR 0015)
- [x] In-app HTTP MCP endpoint bound to the GUI session; token, origin checks,
      on/off setting; discovery file (`arrangia_mcp::host`, also in the dev
      bridge with `--mcp`)
- [x] `arrangia mcp` stdio proxy: discover GUI → forward; optional GUI auto-start;
      headless fallback (`arrangia_mcp::proxy`, reconnects after a GUI restart)
- [x] App settings "Claude / MCP": status, add to Claude Desktop (confirm +
      backup), Claude Code command (stdio + HTTP), fixed port, autostart
- [x] Live aids: "Claude:" history labels, canvas highlight of changed
      elements, toasts, activity list (top-bar indicator), `selection_get`,
      `view_get`; GUI-hosted tools default to the GUI's target/language;
      projects opened by Claude open in the GUI
- Follow-ups: bundled-build CLI path (Phase 6); Claude Desktop's real
  `clientInfo.name` to confirm in 4.5 (assumed `claude-ai`)

## 4.4 Tests
- [x] Integration test: start `arrangia mcp` as a subprocess, speak MCP over
      stdio (initialize, list tools, call tools), create a project, add device
      + text, set strings in two languages, render a preview (valid PNG),
      validate, export – all in a temp dir
- [x] Concurrency test: external edit between calls → reload, conflict
      handling (`crates/cli/tests/mcp.rs`, `crates/mcp/src/server.rs` tests)

## 4.5 Real-world check with Claude
- [ ] Register the server in Claude Code and Claude Desktop through the app
      settings ("Add to Claude Desktop", copied `claude mcp add` command) and
      check the live view with a real client
- [ ] Scenario run by a Claude session: "Create screenshots for app X in en/de,
      iPhone + iPad, translate, export" – judge the rendered result; fix tool
      ergonomics (descriptions, defaults, error messages) based on what the
      model struggles with
- [~] Docs: ADR (app crate + MCP design), CLAUDE.md, README, `docs/guide/NOTES.md`
      (setup snippets, tool list, example prompts) – done for the server
      and GUI hosting (ADR 0013, 0014, 0015); real-world findings follow
