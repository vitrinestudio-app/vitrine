---
title: CLI reference
description: "The `vitrine` command: new, validate, render, export, strings, import, frames, fonts, template, gallery, upload, mcp."
weight: 80
icon: terminal
---

Every command works on a project folder: `--project <DIR>` (default: the
current folder). Exit codes: **0** ok, **1** the project has problems
(validation errors, export failures), **2** the command could not run.
Colours on a terminal unless `NO_COLOR` is set.

## Projects

```sh
vitrine new my-app --lang en --lang de          # --template phone|minimal|<name|file>
vitrine validate                                # --target, --lang, --preview, --strict, --format json
vitrine render --canvas                         # --page N | --image N | --canvas | --gallery
vitrine render --target play-phone --page 2 --width 400 -o page.png   # --width | --max-edge | --scale
vitrine export                                  # --target, --lang, --out, --force, --no-gallery, --no-html
vitrine export --layout fastlane --out fastlane
```

`validate --strict` fails on warnings; `--preview` treats missing screenshots
as warnings. `render` prints the path of the PNG it wrote.

## Texts and screenshots

```sh
vitrine strings export --format csv|xliff12|xliff20 [--lang L] [-o DIR] [--missing-only] [--context]
vitrine strings import FILE… [--dry-run] [--create-keys] [--empty keep|remove]
vitrine import screenshots DIR [--format auto|vitrine|fastlane|flat] [--device-map A=B] [--lang-map A=B] [--key-map A=B] [--dry-run] [--save-mapping]
```

## Frames, fonts and templates

```sh
vitrine frames import PATH [--dry-run]          # Apple bezels: folder, zip or mounted DMG
vitrine frames import PACK [--to app|project] [--id ID]   # own frame packs
vitrine frames list [--packs]
vitrine frames remove [--model] [--color] [--orientation] [--bezel] [--pack ID] [--all]
vitrine fonts [--format json]                   # bundled font families
vitrine template list | save | import | export | insert | remove | new
vitrine gallery list | download | login | logout | upload
```

## Store upload

```sh
vitrine upload plan [--store apple|google] [--target ID]… [--lang L]… [--remote] [--force] [--format json]
vitrine upload execute --confirm TOKEN [--yes] [--changes-not-sent-for-review]
vitrine upload log [--last] [--events]
vitrine upload refdata                          # App Store asset types (read-only)
```

`upload` reads the store accounts of the app (`--app-dir` or `VITRINE_APP_DIR`
selects another app data folder).

## MCP

```sh
vitrine mcp                                     # stdio: proxy to the running app (starts it if needed)
vitrine mcp --headless --project my-app         # own session, no app
```

Never prints anything else to stdout in this mode.
