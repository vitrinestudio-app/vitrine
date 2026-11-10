# Project status & decision log

Living hand-off document so work can continue after a context reset. Keep it
current: update after each phase/merge and whenever the user decides something.

## Current state (2026-11-10)
- Phase 0 (spike), Phase 1 (core + CLI), Phase 2 (GUI editor) done and reviewed by the user.
- Phase 3 (languages & import) in progress – see `docs/plan/phase-3.md`.
  - Done: 3.1 language overrides + font fallback, 3.2 hyphenation, 3.3 screenshot import, 3.4 CSV/XLIFF, 3.5 Apple bezel import (base), 3.6 Phase-2 follow-ups.
  - Done: 3.5 follow-up (all bezel sizes kept, auto selection target → screenshot → aspect → largest, `bezel:` pin, SLA guidance, licence notice).
  - Open: user re-checks with the real `Bezel-iPhone-18` frames after the 2026-11-10 colour fixes (project default Apple, colour swatches measured from the real bezels, picking each colour, switching back to parametric) → then Phase 3 complete.
  - 3.7 done except the real-package bezel check: example has fr (+ fr-CA sparse), German override + hyphenation, fr goldens (iPhone 6.9", iPad), scenario test `crates/cli/tests/scenario.rs`, README/CLAUDE.md/NOTES updated.
- Phase 4 (MCP) started – see `docs/plan/phase-4.md`.
  - Done: 4.1 `arrangia-app` extracted (shared session/commands layer, no Tauri; ADR 0013) + actor labels, `project-edited` event, `view_set`/`view_get`.
  - Done: 4.2–4.4 headless MCP server (`arrangia-mcp`, rmcp =3.9.0, ADR 0014): ~50 tools, resources, instructions, `arrangia mcp [--project] [--app-dir] [--headless]`; stdio integration tests (`crates/cli/tests/mcp.rs`). `element_add` in `arrangia-app` gained `id`/`key`/`fields`.
  - Done: 4.3b GUI hosting (ADR 0015): in-app Streamable HTTP endpoint (`arrangia_mcp::host`, 127.0.0.1, bearer token, Host/Origin checks, discovery file `<app data>/mcp.json`, on/off setting), `arrangia mcp` stdio proxy (GUI autostart, reconnect, headless fallback), app settings "AI assistants (MCP)" (generic: switch, status, port, autostart, other MCP clients; then Add to Claude Desktop with confirm + backup, Claude Code commands), live aids (top-bar indicator + activity list, canvas glow, toasts with the client's display name, projects opened by an assistant open in the GUI, tools default to the GUI's target/language); dev bridge `--mcp`; tests `crates/cli/tests/mcp_proxy.rs`.
  - 4.5 in progress: first real session (Claude Desktop, 2026-11-10, "ging super") done; its findings fixed and merged: null in variant/lang scope resets the field (new tool `element_reset_override`), `element_update` batch `updates: [...]` in one undo step, ids across strips, `docs_get(topic)` (Claude Desktop lets only the user attach resources; resources work through the proxy), `assets_import` mapping list + dry run. Behaviour change: switching a field off per variant/lang needs an explicit value (`shadow: none`, `visible: false`), not null.
    - Next: re-run the scenario with the fixes.
  - Still to confirm: Claude Desktop's real `clientInfo.name` (assumed `claude-ai` → "Claude Desktop"); clean-exit removal of `mcp.json` on ⌘Q in the real app; CLI path for bundled builds (Phase 6 TODO, today `arrangia` next to the GUI binary or `ARRANGIA_CLI`); oddity: `element_add` text without `key` creates key `text`.
- Phase 5 (store upload) – planning started 2026-11-10, see `docs/plan/phase-5.md`.
  - Done: 5.1 credentials & config (ADR 0016): app-wide store accounts in `settings.yaml` (secrets in the system keychain, service `app.arrangia`, user `<id>/<kind>`; `ARRANGIA_KEYRING=mock` for tests/dev bridge), `store:` mapping in `project.yaml` (op `set_store`), locale defaults `crates/core/data/store-locales.yaml` (Apple 50 / Google 86 codes, checked 2026-11-10), store validation findings, commands `store_account_*`, `project_store_get/patch`, MCP `store_config_get` (read-only), GUI app settings → Stores + project settings → Stores tab (not yet checked visually).
  - Done: 5.3/5.4 store clients merged (ADR 0017): `arrangia_publish::apple` (ES256 JWT, editable version → localizations → screenshot sets, reserve/upload parts/commit MD5/poll/reorder) and `arrangia_publish::google` (service-account OAuth, edits, `deleteall`, upload, commit, abort = delete edit); retry/backoff/Retry-After, cancel, observer + progress hooks; wiremock tests `crates/publish/tests/`. Resume + upload log belong to 5.5.
  - Done: "Test connection" wired (`arrangia_app::store_test::StoreClientTester`, GUI + dev bridge; CLI/headless MCP keep `not_implemented`): read-only – Apple `GET /v1/apps?limit=1` → "key accepted, N apps visible (first: …)", Google token fetch only → client e-mail; errors with `details.reason` (`key_rejected`, `no_permission`, `offline`, …). Not yet tried with real keys.
  - Done: 5.2 plan + 5.5 executor/CLI/GUI/MCP (ADR 0018): `arrangia_app::upload` (plan with incremental render into `.cache/upload/images`, SHA-256/MD5, compare with `.cache/uploads.sqlite` and optionally the store, blocking problems, confirm token 30 min/once/fingerprint; executor Apple per set via `AppleUploadFlow` (set-based), Google one edit; progress/cancel/resume/rate-limit pauses; log), CLI `arrangia upload plan|execute|log`, GUI "Upload to stores…" dialog + MCP confirmation dialog (not checked visually yet), MCP `upload_plan/execute/status/log`. Tests: `crates/app/tests/upload.rs` (fake ASC + Play on wiremock), `crates/cli/tests/upload.rs`, `crates/cli/tests/mcp.rs`.
    - Defaults implemented for the open questions: (a) **B for now** – set-based flow behind the `AppleUploadFlow` trait (asset library = second implementation, nothing else changes); (b) the recommended MCP policy (GUI click, 5 min timeout, headless off unless `mcp_allow_headless_upload`), isolated in `crates/app/src/upload/policy.rs`.
  - Next: user looks at the Upload dialog (dev bridge / app); 5.6 real run with test apps (c)/(d); then decide (a)/(b) for good.
- Next phases per KONZEPT §12: 6 polish/website, 7 3D.

## Open questions for the user
(asked 2026-11-10, Phase 5; move the answer to "Decisions taken with the user")
- (a) [default B implemented 2026-11-10, switch is cheap] **Apple App Asset Library.** Apple deprecated the screenshot-set API (App Screenshot Sets / App Screenshots) in ASC API 4.5.1; the successor is the App Asset Library (assets + placements + ordering requests, display types from `appAssetLibraryRefData`, needed for `IPHONE_DUO`; no removal date published; ADR 0017 "Deprecation"). Option **A (recommended)**: build the asset-library flow now as the default, with the set-based flow as fallback. Option **B**: ship with the set-based flow (already implemented) and add the asset library later.
- (b) [recommended default implemented 2026-11-10] **MCP uploads.** When the GUI hosts MCP, `upload_execute` additionally needs a confirmation click in the GUI; headless MCP (`arrangia mcp --headless`) uploads are disabled by default (opt-in setting/flag). OK?
- (c) **Test apps for the real run:** which App Store Connect app (bundle id / Apple ID) and which Play package name?
- (d) **Keys:** does the user already have an App Store Connect API key (role App Manager; issuer id + key id + `.p8`) and a Google Play service account (JSON key, "Manage store presence" for the app)?

## Session practicalities
- Build for the user's manual tests: `cargo build -p arrangia-cli -p arrangia-gui` (Claude Desktop config points to `target/debug/arrangia mcp`).
- The user runs the real GUI themselves; agents must never touch the real app data dir, the real Claude Desktop config or the user's running app. Use `ARRANGIA_APP_DIR`, `ARRANGIA_CLAUDE_DESKTOP_CONFIG`, dev bridge `--app-dir`.
- The user's real app data dir contains the imported Apple iPhone 18 bezels (16 frames).
- Subagents sometimes leave a background process; stop finished agents with TaskStop and check `ps aux | grep arrangia`.

## How we work
- Coordinator session plans and reviews (incl. looking at rendered images); subagents implement (worktrees for parallel work, merged with `--no-ff`).
- Every commit: fmt, clippy `-D warnings`, `cargo test --all`, `cargo build -p arrangia-gui`.
- CI on GitHub (Linux/macOS/Windows) must stay green; LF line endings enforced by `.gitattributes`.
- GUI verification via the dev bridge in Chrome (Claude-in-Chrome).
- User-facing facts go to `docs/guide/NOTES.md` (English, notes only, no prose yet).

## Decisions taken with the user
- 2026-11-09 Text engine parley (Phase 0 ADR 0001) – "du entscheidest".
- 2026-11-09 Hyphenation: soft hyphens + warning on emergency break in Phase 1; automatic pattern hyphenation opt-in in Phase 3, only when auto-fit at `shrink_to` is not enough.
- 2026-11-09 Phase 1 → Phase 2 → Phase 3 approved after reviews.
- 2026-11-10 Image shadows must follow alpha; texts get outline + glow (done).
- 2026-11-10 Inspector hint "(Breite/Deckkraft 0 schaltet ab)" removed (too wide).
- 2026-11-10 Bezel import: keep all sizes (6.3" Pro needed for the required ASC set), auto-select by size, manual pin, clear hint for DMGs with licence agreement, show Apple licence notice.
- 2026-11-10 Documentation: user guide in **English**, start collecting notes now (not prose); document a lot (also to survive context compression).
- 2026-11-10 MCP: the GUI hosts the MCP server; no terminal needed; Claude Desktop starts a stdio proxy that connects to the running GUI; changes visible live, undoable, commentable in Claude.
- 2026-11-10 Real MCP session feedback (Claude Desktop → proxy → GUI) → fixes: language/variant override reset (`null` in scope variant/lang removes the field; overrides never store `null`, hand-written ones dropped + info `override_null`; tool `element_reset_override`), several scoped patches in one undo step (`element_update.updates`), multi-element tools accept ids of several strips, `docs_get {topic}` because Claude Desktop does not let the model read resources (they work through the proxy), `assets_import` mapping list (`files`, `dry_run`) and kind alias `image`.
- 2026-11-10 User test of the real iPhone 18 package → fixes: project-wide default "Apple (imported)" in project settings; vendor colours (Burgundy…) accepted with imported frames (was an op error); one colour row with the installed colours as real swatches (measured at import); colour follows a frame-source switch (closest colour, same undo step); uninstalled colour → default installed colour + info; short bezel/frame labels in the inspector; dialogs collapsed in WebKit (app settings) fixed (`.dlg-body` flex basis auto).
- 2026-11-10 Neutral MCP wording: generic MCP parts (settings section "AI assistants (MCP)", indicator, activity list, toasts, history labels, server instructions) are client-neutral, labels use the client's display name (`Actor::Assistant`); Claude-specific setup (Add to Claude Desktop, `claude mcp` commands) stays named (ADR 0015 amendment).
- License of the project: still open (no LICENSE file, `publish = false`). Swedish hyphenation patterns (LPPL 1.2) wait for this decision.

## Verified external facts
- App Store Connect (logged-in web app, 2026-11-09): display types ↔ sizes, see `crates/core/data/targets.yaml` and `docs/guide/NOTES.md`.
- Apple bezel package `Bezel-iPhone-18.dmg` (user test 2026-11-10): file names `iPhone 18 Pro[ Max] - <Colour> - <Portrait|Landscape>.png`, + PSDs + License.rtf, DMG has an SLA.

## Backlog for later versions (noted, not scheduled)
- 2026-11-10 (user): more devices – **iPhone Duo** (two screens; ASC display type `IPHONE_DUO` behind a feature flag, sizes not public yet; required from April 2027 for iOS 27.1 SDK apps) and **Apple Watch** (ASC types `APP_WATCH_ULTRA` 410×502/422×514, `SERIES_10` 416×496, `SERIES_7` 396×484, `SERIES_4` 368×448, `SERIES_3` 312×390; Play Wear OS 1:1 ≥ 384×384, no frames allowed). Needs: catalog entries + new frame shapes (dual screen / watch with crown & band) in both styles, target presets, Apple bezel import rules for Watch/Duo packages. How-to: `docs/dev/adding-devices.md`.

## Known open items (cross-phase)
- Native store app gallery gaps not measured (only web CSS).
- Drag & drop of files in the real Tauri app not tested with a real drop.
- No native menu bar; no font import UI.
- See "Follow-ups" sections in `docs/plan/phase-*.md`.
