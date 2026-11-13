# Vitrine Studio agent skill – store screenshots

An agent skill that teaches an AI assistant to make **App Store and Google Play screenshots for
your own app** with [Vitrine Studio](https://www.vitrinestudio.app/) through its MCP server:
finding the benefits, writing the copy, laying out the pages (including panoramas across
pages), languages, store rules, checks, export and – only on your confirmation – the store
upload.

Repository name: **TBD** (working name `vitrinestudio-app/vitrine-agent-skill`; not decided
yet). Until then it lives in the Vitrine Studio repository under `integrations/skill/`.

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

Generic: copy the folder `skills/vitrine-store-screenshots/` into your agent's skills
directory (keep the folder name – it must match the skill's `name`), or upload it as a zip
where your client takes skill uploads. Restart or reload the agent if it reads skills at
start-up. Then ask for something like "make App Store screenshots for my app in Vitrine".

Per-client notes – examples, paths and menus change, **verify** them in your client's docs:

- **Claude Code** *(verify)*: personal `~/.claude/skills/vitrine-store-screenshots/`, or
  per project `.claude/skills/vitrine-store-screenshots/`. Plugin marketplaces are another
  way to distribute skills; this folder does not ship a plugin manifest.
- **Claude Desktop / claude.ai** *(verify)*: zip the folder `vitrine-store-screenshots`
  (the zip contains that folder) and upload it under Settings → Capabilities → Skills.
- **OpenAI Codex CLI** *(verify)*: reported to read Agent Skills from a skills folder such as
  `~/.codex/skills/` or `.agents/skills/`.
- **Gemini CLI** *(verify)*: reported to read skills from `.gemini/skills/` (or
  `~/.gemini/skills/`).
- **Cursor** *(verify)*: reported to read skills from `.cursor/skills/` (project) or
  `~/.cursor/skills/` (global) since version 2.4.
- Cross-agent installers (for example the `skills` CLI from Vercel Labs) can copy a skill into
  the right folder for several agents *(verify)*.

Client-specific packaging (a plugin manifest, an `.mcp.json`) is deliberately not part of the
main structure; if it is added later, it goes into a separate subfolder.

## Updating

The skill names Vitrine Studio tools and fields. When the app's MCP server changes, its own
reference wins: the skill tells the assistant to read `docs_get` first. If `apply_ops` lacks
a `file` parameter, the app is older than the skill – update Vitrine Studio.

## Licence

MIT (`LICENSE`). Vitrine Studio itself is GPL-3.0-or-later; this skill only talks to it over
MCP. Sources and their licences: `skills/vitrine-store-screenshots/CREDITS.md`.
