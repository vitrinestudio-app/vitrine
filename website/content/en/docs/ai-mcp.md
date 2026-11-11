---
title: AI assistants (MCP)
description: "Let Claude or any MCP client design, translate, preview and export – live in your editor."
weight: 70
icon: sparkle
---

Vitrine Studio is an [MCP](https://modelcontextprotocol.io) server. The
desktop app hosts it, so an assistant works on the project you have open: its
changes appear live, briefly outlined, with a small note naming the client –
and every change is one undo step labelled with the client's name, e.g.
"Claude Desktop: Move title".

## Claude Desktop

1. Start Vitrine Studio.
2. App settings → **AI assistants (MCP)** → **Add to Claude Desktop…** – it
   shows the entry and the config file, asks before writing and keeps a backup.
3. Restart Claude Desktop and ask it to work on your screenshots.

Claude Desktop then starts `vitrine mcp`, which connects to the running app –
and starts it when it is closed (setting *Start on demand*).

## Claude Code and other clients

```sh
claude mcp add vitrine-studio -- /path/to/vitrine mcp
```

Any MCP client: a stdio server named `vitrine-studio` with the command
`vitrine` and the argument `mcp` (the exact path is shown in the settings):

```json
{
  "mcpServers": {
    "vitrine-studio": { "command": "/path/to/vitrine", "args": ["mcp"] }
  }
}
```

Clients that speak Streamable HTTP can use `http://127.0.0.1:<port>/mcp` with
the bearer token shown in the settings. Without the app (CI, scripts):
`vitrine mcp --headless --project my-app`.

## What an assistant can do

- create projects and canvases, add devices, texts and targets, change styles;
- translate (`strings_missing`, `strings_set`) and check the result;
- **see** the result: `render_preview` returns the rendered PNG of an image,
  the canvas or the store gallery;
- validate, export, import screenshots, save and insert templates;
- plan store uploads – executing one needs your click in the app.

Tools default to what you look at: without a target or language they use the
app's current view, and `selection_get` lets you say "make *this* bigger".

## Safety

- The endpoint listens on **127.0.0.1** only, with a token that changes on
  every start.
- No network from the MCP tools; files are written only inside the project and
  to paths you name.
- Store keys can't be read or imported over MCP.

## Prompts to try

- "Create a Vitrine Studio project in ~/Screens/Fernwood (en, de) for a garden
  planner, three iPhone pages with headline and phone each, show me the gallery."
- "Translate everything missing into French, then check page 2 in French for
  overflows."
- "Select the title in the app and say: make this 20 % smaller."
- "Export iPhone and iPad for all languages."
