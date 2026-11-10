# Project status & decision log

Living hand-off document so work can continue after a context reset. Keep it
current: update after each phase/merge and whenever the user decides something.

## Current state (2026-11-10)
- Phase 0 (spike), Phase 1 (core + CLI), Phase 2 (GUI editor) done and reviewed by the user.
- Phase 3 (languages & import) in progress – see `docs/plan/phase-3.md`.
  - Done: 3.1 language overrides + font fallback, 3.2 hyphenation, 3.3 screenshot import, 3.4 CSV/XLIFF, 3.5 Apple bezel import (base), 3.6 Phase-2 follow-ups.
  - In progress: 3.5 follow-up (keep all bezel sizes, auto selection, pinning, SLA/licence hints) after the user's real-package test.
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
- License of the project: still open (no LICENSE file, `publish = false`). Swedish hyphenation patterns (LPPL 1.2) wait for this decision.

## Verified external facts
- App Store Connect (logged-in web app, 2026-11-09): display types ↔ sizes, see `crates/core/data/targets.yaml` and `docs/guide/NOTES.md`.
- Apple bezel package `Bezel-iPhone-18.dmg` (user test 2026-11-10): file names `iPhone 18 Pro[ Max] - <Colour> - <Portrait|Landscape>.png`, + PSDs + License.rtf, DMG has an SLA.

## Known open items (cross-phase)
- Native store app gallery gaps not measured (only web CSS).
- Drag & drop of files in the real Tauri app not tested with a real drop.
- No native menu bar; no font import UI.
- See "Follow-ups" sections in `docs/plan/phase-*.md`.
