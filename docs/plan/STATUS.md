# Project status & decision log

Living hand-off document so work can continue after a context reset. Keep it
current: update after each phase/merge and whenever the user decides something.

## Current state (2026-11-10)
- Phase 0 (spike), Phase 1 (core + CLI), Phase 2 (GUI editor) done and reviewed by the user.
- Phase 3 (languages & import) in progress – see `docs/plan/phase-3.md`.
  - Done: 3.1 language overrides + font fallback, 3.2 hyphenation, 3.3 screenshot import, 3.4 CSV/XLIFF, 3.5 Apple bezel import (base), 3.6 Phase-2 follow-ups.
  - Done: 3.5 follow-up (all bezel sizes kept, auto selection target → screenshot → aspect → largest, `bezel:` pin, SLA guidance, licence notice).
  - Open: user re-runs the real `Bezel-iPhone-18` package import (expect 16 frames, Pro for ios-iphone-63, Pro Max for ios-iphone-69) → then Phase 3 complete.
  - 3.7 done except the real-package bezel check: example has fr (+ fr-CA sparse), German override + hyphenation, fr goldens (iPhone 6.9", iPad), scenario test `crates/cli/tests/scenario.rs`, README/CLAUDE.md/NOTES updated.
- Next phases per KONZEPT §12: 4 MCP, 5 store upload, 6 polish/website, 7 3D.

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
