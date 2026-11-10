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
- **MCP server:** `arrangia mcp [--project <dir>]` (CLI subcommand) starts a
  stdio server using `rmcp` (official Rust SDK). Tools return structured JSON
  (`structuredContent` + short text); `render_preview` returns an image content
  block (PNG) so Claude sees the result.
- **Concurrency with the GUI (§8.3):** disk is the source of truth; every op is
  saved atomically with the revision check; the GUI's watcher already reloads
  external edits live. MCP reloads before each tool call if the revision on
  disk changed.
- **Safety:** no network access in Phase 4 tools; upload tools come in Phase 5
  (`upload_plan`/`upload_execute` with confirm token). Asset import only from
  paths the user names; nothing deletes outside the project folder.

## 4.1 Extract `arrangia-app`
- [ ] Move session/commands/preview/settings/watch/events/dialog traits from
      `crates/gui` into `crates/app`; gui keeps Tauri wrappers, bridge, UI
- [ ] No behaviour change; all tests green; CLI keeps its own direct paths or
      uses app where it simplifies

## 4.2 MCP server skeleton
- [ ] `crates/mcp` with rmcp (stdio), server info/instructions (short guide to
      Arrangia concepts for the model: canvas, home page, lu, gaps, anchors,
      variants, targets, markup), `arrangia mcp` CLI subcommand
- [ ] Project handling: `project_open`, `project_create` (templates),
      `project_summary` (compact: strips, pages, elements, languages, targets,
      revision)
- [ ] Resources: `project.yaml`, `i18n/<lang>.yaml`, device catalog, target
      presets (read-only)

## 4.3 Tools (KONZEPT §8.2 + what Phases 1–3 added)
- [ ] Catalog: `catalog_devices`, `catalog_targets`, `catalog_fonts`,
      `catalog_galleries`
- [ ] Strips/targets: `strip_add`, `strip_update`, `target_add` (from preset),
      `target_update`, `target_remove`
- [ ] Elements: `element_add`, `element_update` (patch; base/variant/language
      scope), `element_remove`, `element_list`, `element_move`/`resize`/
      `rotate` via core edit intents, `apply_ops` (atomic, one undo step)
- [ ] Strings: `strings_get`, `strings_set` (batch), `strings_missing`
      (for "translate everything missing into fr"), key rename
- [ ] Assets: `assets_list`, `screenshots_import` (folder/fastlane plan +
      apply), `frames_list` (imported bezels, read-only)
- [ ] Output: `render_preview` (page/image, canvas, store gallery; target,
      language, max edge) → PNG image content; `layout_report` (boxes, font
      sizes, overflows, hyphenation as text/JSON); `validate`; `export`
- [ ] Exchange: `strings_export`/`strings_import` (CSV/XLIFF) optional
- [ ] Undo/redo tools (`history_undo`, `history_redo`) for the MCP session

## 4.4 Tests
- [ ] Integration test: start `arrangia mcp` as a subprocess, speak MCP over
      stdio (initialize, list tools, call tools), create a project, add device
      + text, set strings in two languages, render a preview (valid PNG),
      validate, export – all in a temp dir
- [ ] Concurrency test: external edit between calls → reload, conflict
      handling

## 4.5 Real-world check with Claude
- [ ] Register the server in Claude Code (`claude mcp add arrangia -- arrangia mcp --project …`)
      and in Claude Desktop config; GUI shows the config snippet to copy
      (app settings → "MCP")
- [ ] Scenario run by a Claude session: "Create screenshots for app X in en/de,
      iPhone + iPad, translate, export" – judge the rendered result; fix tool
      ergonomics (descriptions, defaults, error messages) based on what the
      model struggles with
- [ ] Docs: ADR (app crate + MCP design), CLAUDE.md, README, `docs/guide/NOTES.md`
      (setup snippets, tool list, example prompts)
