# 0000 – Record architecture decisions

- Status: accepted
- Date: 2026-11-09

## Context

Vitrine Studio makes a number of foundational decisions (renderer libraries, text
engine, YAML crate, license, gallery gap values …) that later contributors –
human or AI – need to understand without digging through chat logs or commit
history. The concept document (`docs/KONZEPT.md`) describes the target state
but not why individual choices were made or which alternatives were rejected.

## Decision

Architecture decisions are recorded as short Markdown files in `docs/adr/`,
named `NNNN-title-in-kebab-case.md` with a running four-digit number.

Each ADR contains: status (proposed, accepted, superseded by NNNN), date,
context, decision, and consequences. ADRs are not edited after acceptance
except to change the status; a changed decision gets a new ADR that
supersedes the old one.

## Consequences

- Decisions such as the Phase 0 outcome (cosmic-text vs. parley, tiny-skia)
  get an ADR before the corresponding code is merged.
- `AGENTS.md` (the agent guide) points to this directory so agents check existing decisions
  before proposing changes.
