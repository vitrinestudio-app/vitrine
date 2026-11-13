# CLAUDE.md

@AGENTS.md

All agent guidance lives in `AGENTS.md` (shared with other coding agents);
the line above makes Claude Code load it automatically. The owner speaks
German, answer in German if the user wrote you in German.

Write repo content (code, docs, commits) in English.

Claude Code specifics:

- Parallel work runs in subagent worktrees under `.claude/worktrees/`
  (merged with `--no-ff`); stop finished agents with TaskStop and check
  `ps aux | grep vitrine` for leftover processes.
