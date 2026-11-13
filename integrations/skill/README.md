# Vitrine Studio agent skill – store screenshots

An agent skill that teaches an AI assistant to make **App Store and Google Play screenshots for
your own app** with [Vitrine Studio](https://www.vitrinestudio.app/) through its MCP server:
finding the benefits, writing the copy, laying out the pages (including panoramas across
pages), languages, store rules, checks, export and – only on your confirmation – the store
upload.

Repository: [`vitrinestudio-app/vitrine-agent-skill`](https://github.com/vitrinestudio-app/vitrine-agent-skill).
Its source lives in the Vitrine Studio repository under `integrations/skill/` and is synced to
that repository (this folder is its root).

## What's inside

```
skills/vitrine-store-screenshots/   the skill (Agent Skills format)
  SKILL.md                          workflow and hard rules (loaded when the skill is used)
  references/                       read on demand: Vitrine technical reference, design
                                    rules, copy, store specs, QA gate
  CREDITS.md                        sources
LICENSE                             MIT
```

The skill uses the open **Agent Skills** format (https://agentskills.io): a folder named like
the skill, with a `SKILL.md` (YAML front matter `name` + `description`, then instructions)
and optional `references/`. Several agent tools read this format; the content is
vendor-neutral and only assumes an MCP client connected to Vitrine Studio.

## Requirements

- **Vitrine Studio** running, with MCP enabled: App settings → **AI assistants (MCP)** →
  "Allow AI assistants to connect via MCP".
- Your AI assistant connected to the `vitrine-studio` MCP server. Generic setup for any MCP
  client: the stdio command `vitrine mcp` (the full path is shown in the app settings; with the
  command line tool installed, plain `vitrine mcp`). It connects to the running app, or starts
  it. The same settings page offers client-specific helpers, for example
  **"Add to Claude Desktop"**, and an HTTP endpoint. Details:
  https://www.vitrinestudio.app/docs/ai-mcp/
- Best results with an assistant that can also read and write files (for ops files in the
  project folder). Without file access the skill falls back to smaller inline edits.

## Install

Also described on the website: https://www.vitrinestudio.app/docs/ai-mcp/#agent-skill

With the cross-agent `skills` CLI (Node.js 22.20+; it knows the skills folders of Claude
Code, Codex, Cursor, Gemini, GitHub Copilot, OpenCode and many more):

```sh
npx skills add vitrinestudio-app/vitrine-agent-skill            # pick agents interactively
npx skills add vitrinestudio-app/vitrine-agent-skill -g -a claude-code -a codex   # global, chosen agents
npx skills add vitrinestudio-app/vitrine-agent-skill@v1.0.0     # pin a release
npx skills update                                               # later: get the newest version
```

Manually: copy the folder `skills/vitrine-store-screenshots/` into your agent's skills
directory (keep the folder name – it must match the skill's `name`), or upload it as a zip
where your client takes skill uploads. Restart or reload the agent if it reads skills at
start-up. Then ask for something like "make App Store screenshots for my app in Vitrine".

Per-client notes (checked against the clients' docs in November 2026; paths and menus
change, so look at your client's docs if something doesn't match):

- **Claude Code**: personal `~/.claude/skills/vitrine-store-screenshots/`, or per project
  `.claude/skills/vitrine-store-screenshots/`. Plugin marketplaces are another way to
  distribute skills; this folder does not ship a plugin manifest.
- **Claude Desktop / claude.ai**: zip the folder `vitrine-store-screenshots` (the zip
  contains that folder) and upload it under Customize → Skills → "+" → Create skill →
  Upload a skill. Skills need code execution ("Create and edit files") switched on in
  Settings → Capabilities; older app versions have the upload there too.
- **OpenAI Codex**: per repository `.agents/skills/`, personal `~/.codex/skills/`
  (`$CODEX_HOME/skills/`).
- **Gemini CLI**: per project `.gemini/skills/` (or `.agents/skills/`), personal
  `~/.gemini/skills/`; `/skills list` shows what it found.
- **Cursor**: per project `.cursor/skills/`; it also reads `.claude/skills/`,
  `.codex/skills/` and their counterparts in your home folder.
- The cross-agent `skills` CLI (above) copies the skill into the right folder for each agent
  you pick.

Client-specific packaging (a plugin manifest, an `.mcp.json`) is deliberately not part of the
main structure; if it is added later, it goes into a separate subfolder.

## Updating

The skill names Vitrine Studio tools and fields. When the app's MCP server changes, its own
reference wins: the skill tells the assistant to read `docs_get` first. If `apply_ops` lacks
a `file` parameter, the app is older than the skill – update Vitrine Studio.

## Licence

MIT (`LICENSE`). Vitrine Studio itself is GPL-3.0-or-later; this skill only talks to it over
MCP. Sources and their licences: `skills/vitrine-store-screenshots/CREDITS.md`.
