---
title: AI assistants (MCP)
description: "Connect Claude Desktop, Claude Code or any MCP client: the assistant designs, translates, checks and exports in the project you have open – live, with undo."
weight: 130
icon: sparkle
---

Vitrine Studio has a built-in [MCP](https://modelcontextprotocol.io) server.
An assistant such as Claude works on the **project you have open**: you see
every change live on the canvas, and every change is one undo step labelled
with the assistant's name – "Claude Desktop: Move title".

## Claude Desktop

1. Start Vitrine Studio.
2. **App settings → AI assistants (MCP) → Add to Claude Desktop…** The dialog
   shows the configuration file and the entry it adds, and its status (*Not
   set up yet*, *Already set up*, *Set up with a different path – will be
   updated*). **Write configuration** makes a backup copy first and leaves your
   other entries alone.
3. Restart Claude Desktop.

Claude Desktop then starts `vitrine mcp`, which connects to the running app –
or starts it, if it's closed (*Start on demand*).

The entry, if you prefer to add it by hand (the exact path is shown in the
settings):

```json
{
  "mcpServers": {
    "vitrine-studio": {
      "command": "/Applications/Vitrine Studio.app/Contents/MacOS/vitrine",
      "args": ["mcp"]
    }
  }
}
```

Config file: macOS `~/Library/Application Support/Claude/claude_desktop_config.json`,
Windows `%APPDATA%\Claude\claude_desktop_config.json`, Linux
`~/.config/Claude/claude_desktop_config.json`.

{{< shot src="images/mcp-settings.jpg" alt="App settings, section AI assistants (MCP): connection status, fixed port, start on demand, the commands for other MCP clients, Add to Claude Desktop and the Claude Code command." caption="App settings → **AI assistants (MCP)**." >}}

## Claude Code

Copy the command from **App settings → AI assistants (MCP) → Claude Code →
Connect through the vitrine command** (recommended), e.g.:

```sh
claude mcp add vitrine-studio -- "/Applications/Vitrine Studio.app/Contents/MacOS/vitrine" mcp
```

(With the [command line tool installed](../install/#the-command-line-tool):
`claude mcp add vitrine-studio -- vitrine mcp`.) *Direct HTTP* is offered too,
but its port and token change with every start of the app.

## Other MCP clients

Any MCP client works. Preferred: a stdio server named `vitrine-studio` with
the command `vitrine` and the argument `mcp` (**Other MCP clients → Command
(stdio)** shows the full path). Clients that speak Streamable HTTP can use the
URL `http://127.0.0.1:<port>/mcp` with the header `Authorization: Bearer
<token>` from the settings – valid until the app restarts; set a **Fixed port**
to keep the port.

## How it connects

- The app offers the MCP endpoint on **this computer only** (127.0.0.1), with
  a secret token that changes on every start. Switch it off with **Allow AI
  assistants to connect via MCP**.
- `vitrine mcp` (what Claude Desktop starts) forwards to the running app; if
  none runs and *Start on demand* is on, it starts the app (and waits up to 20
  seconds).
- Without the app – CI, a server, *Start on demand* off – `vitrine mcp
  --headless --project <dir>` works on the folder directly, without the live
  view.

## What you see in the app

- The **assistant indicator** in the top bar: grey = off, *MCP* = ready, with
  the client's name when connected. Click it for **Assistant activity** – each
  step with time; click to select the changed elements, undo the latest.
- Changed elements are outlined on the canvas for a moment, with a small note
  "Claude Desktop: …" (click to select them).
- A project the assistant opens or creates opens in the app.

## What an assistant can do

- **Create and structure:** projects (also from templates), canvases, targets,
  banners, languages, variants.
- **Design:** add, move, resize, rotate, group and restyle elements; edit
  styles; per-variant and per-language overrides.
- **Translate:** find missing texts, write translations, exchange CSV/XLIFF.
- **See the result:** `render_preview` returns the rendered image – one
  exported image, the whole canvas with gaps, or the store gallery – plus the
  problems of that target and language. `layout_report` shows how each text
  was fitted. The gallery can be shown on the store's dark page
  (`background: dark`), sent as JPEG for big sets, or saved into the project
  (`out_file`, e.g. for a README).
- **Check and produce:** validate, export, import screenshots and fonts. An
  export that takes longer than about 45 seconds keeps running in the
  background; the assistant asks `export_status` until it is done.
- **Templates:** list, save, insert.
- **Uploads:** plan, and execute with your confirmation (below).

Tools default to **what you're looking at**: without a target or language they
use the app's current view, and `selection_get` reads your selection – so
"make *this* 20 % smaller" works.

## Prompts to try

- "Create a Vitrine Studio project in ~/Screens/Fernwood (en, de) for a garden
  planner, three iPhone pages with headline and phone each, show me the
  gallery."
- "Translate everything missing into French, then check page 2 in French for
  overflows."
- "I've selected the title – make it 20 % smaller in German only."
- "Add an iPad 13″ landscape target with two pages per image and fix anything
  that overlaps."
- "Export iPhone and iPad for all languages."

## Uploads

An assistant can always make an upload plan (`upload_plan`) – it's a dry run.
To **execute** it, Vitrine Studio shows you a dialog "*Claude Desktop wants to
upload to the stores*" with the plan; only your click on **Confirm upload**
starts it. Declined or unanswered for 5 minutes → nothing happens.

Headless (`vitrine mcp --headless`), assistant uploads are off. They can only
be enabled by setting `mcp_allow_headless_upload: true` in the app's
`settings.yaml` – think twice.

## Privacy and safety

- The MCP tools make **no network requests** of their own – only the store
  upload tools talk to the stores. Files are written only inside the project folder and to paths
  the assistant names explicitly (export folder, translation files).
- **Store keys** can't be read, imported or changed over MCP; the store mapping
  is read-only for assistants.
- Installed device frames are read-only.
- What the assistant sees of your project goes to its provider like any other
  conversation content – the usual rules of your AI provider apply.
