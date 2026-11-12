---
title: CLI reference
description: "Every vitrine command and option: new, validate, render, export, strings, import, frames, fonts, template, gallery, upload, mcp, install-cli."
weight: 140
icon: terminal
---

The `vitrine` command line tool works on the same project folders as the app –
for scripts, CI and quick checks. It ships with the app; see
[Install](../install/#the-command-line-tool) to make it available in your
terminal.

Each section below has the command's complete `--help` text (click to open);
it is generated from the tool itself (`scripts/docs/cli-help.sh`).

## Global options and exit codes

| Option | Meaning |
|---|---|
| `--project <DIR>` | the project folder (the one with `project.yaml`); default: the current folder |
| `-q`, `--quiet` | print only results and errors |
| `-v`, `--verbose` | print more details (every exported file, image sizes) |
| `-V`, `--version` | print the version |

Exit codes: **0** ok · **1** the project has problems (validation errors,
failed exports, blocked upload plan) · **2** the command could not run (wrong
arguments, project not found). Output is coloured on a terminal unless
`NO_COLOR` is set.

{{< cli-help "vitrine" >}}

## `new` – create a project

```sh
vitrine new my-app                                   # Phone template, English
vitrine new my-app --template minimal --lang en --lang de --lang fr
vitrine new my-app --template ./bold.vitrine-template --name "My App"
```

The folder must not exist or be empty. The first `--lang` is the default
language.

{{< cli-help "vitrine new" >}}

## `validate` – check a project

```sh
vitrine validate                          # every target × language
vitrine validate --target play-phone --lang de
vitrine validate --preview                # missing screenshots are warnings
vitrine validate --strict                 # fail on warnings too (CI)
vitrine validate --format json
vitrine validate --format github          # GitHub Actions annotations
vitrine validate --format junit -o validate.xml   # report to a file, text to the log
```

Findings are listed per target and language as `error`, `warn` or `info` with a
code (see [Troubleshooting](../troubleshooting/#validation-findings)). The CI
formats `github`, `gitlab` (Code Quality) and `junit` are described in
[CI/CD](../ci/).

{{< cli-help "vitrine validate" >}}

## `render` – a single preview PNG

```sh
vitrine render --target ios-iphone-69 --page 3 --width 600 -o page3.png
vitrine render --canvas --lang de          # the whole canvas with hatched gaps
vitrine render --gallery --max-edge 800    # the store gallery, 800 px high
```

Prints the path of the written file. With `--gallery`, `--max-edge` is the
image **height**.

{{< cli-help "vitrine render" >}}

## `export` – write the images

```sh
vitrine export
vitrine export --target ios-iphone-69 --target play-phone --lang en
vitrine export --out dist/ --force
vitrine export --layout fastlane --out fastlane
```

See [Export](../export/).

{{< cli-help "vitrine export" >}}

## `strings` – translation exchange

```sh
vitrine strings export --format csv --delimiter semicolon --bom -o texts.csv
vitrine strings export --format xliff20 --missing-only --context -o xliff/
vitrine strings import xliff/*.xlf --dry-run
vitrine strings import texts.csv --create-keys --empty remove
```

See [Languages & translation](../languages/#working-with-translators-csv-and-xliff).

{{< cli-help "vitrine strings export" >}}
{{< cli-help "vitrine strings import" >}}

## `import screenshots`

```sh
vitrine import screenshots fastlane/ --dry-run
vitrine import screenshots shots/ --format flat --lang-map en-US=en --device-map "Pixel 9=android-phone"
```

See [Screenshots](../screenshots/#importing-a-folder-and-fastlane).

{{< cli-help "vitrine import screenshots" >}}

## `frames` – device frames

```sh
vitrine frames import /Volumes/Bezel-iPhone-18     # Apple bezels (folder, .zip, mounted .dmg)
vitrine frames import my-pack.zip --to project      # own frame pack
vitrine frames list                                 # installed Apple frames
vitrine frames list --packs                         # frame packs
vitrine frames remove --model iphone-island --color silver
vitrine frames remove --pack my-pack --from project
vitrine frames embed                                # store the used Apple bezels in the project (CI)
vitrine frames unembed                              # remove them again
```

See [Devices & frames](../devices-frames/) and, for `embed`,
[Apple bezels in CI](../devices-frames/#apple-bezels-in-ci) (private
repositories only).

{{< cli-help "vitrine frames import" >}}
{{< cli-help "vitrine frames list" >}}
{{< cli-help "vitrine frames remove" >}}
{{< cli-help "vitrine frames embed" >}}
{{< cli-help "vitrine frames unembed" >}}

## `fonts`

Lists the font families text styles can use: the bundled ones and those in the
project's `fonts/` folder.

{{< cli-help "vitrine fonts" >}}

## `template` – templates

```sh
vitrine template list
vitrine template save --name "Bold gradient" --tag dark -o bold.vitrine-template
vitrine template insert bold-gradient --dry-run
vitrine template import bold.vitrine-template --replace
vitrine template export bold-gradient -o bold.vitrine-template
vitrine template remove bold-gradient
vitrine template new my-app --template bold-gradient   # same as vitrine new
```

See [Templates & gallery](../templates/).

{{< cli-help "vitrine template save" >}}
{{< cli-help "vitrine template insert" >}}
{{< cli-help "vitrine template list" >}}
{{< cli-help "vitrine template import" >}}
{{< cli-help "vitrine template export" >}}
{{< cli-help "vitrine template remove" >}}

## `gallery` – the template gallery

```sh
vitrine gallery list --tag dark
vitrine gallery download bold-gradient -o bold.vitrine-template
vitrine gallery login
vitrine gallery upload bold.vitrine-template
vitrine gallery logout
```

{{< cli-help "vitrine gallery list" >}}
{{< cli-help "vitrine gallery download" >}}
{{< cli-help "vitrine gallery login" >}}
{{< cli-help "vitrine gallery upload" >}}
{{< cli-help "vitrine gallery logout" >}}

## `upload` – store upload

```sh
vitrine upload plan --remote
vitrine upload execute --confirm <token> --yes
vitrine upload log --last --events
vitrine upload refdata                      # App Store asset types, read-only
```

See [Store upload](../store-upload/#on-the-command-line). In CI the keys come
from environment variables instead of the keychain – see
[CI/CD](../ci/#uploading-from-ci).

{{< cli-help "vitrine upload plan" >}}
{{< cli-help "vitrine upload execute" >}}
{{< cli-help "vitrine upload log" >}}
{{< cli-help "vitrine upload refdata" >}}

## `mcp` – MCP server over stdio

```sh
vitrine mcp                                 # connect to the running app (start it if needed)
vitrine mcp --headless --project my-app     # own session, no app
```

In this mode nothing but MCP messages is printed to stdout. See
[AI assistants](../ai-mcp/).

{{< cli-help "vitrine mcp" >}}

## `install-cli`

Makes `vitrine` available in every terminal (macOS: link in `/usr/local/bin`,
Windows: user PATH). The app's *Install command line tool…* does the same.

{{< cli-help "vitrine install-cli" >}}

## Environment variables

| Variable | Meaning |
|---|---|
| `VITRINE_APP_DIR` | use this folder for the app data (settings, store accounts, frames, templates) instead of the platform folder |
| `NO_COLOR` | no colours in the output |
| `VITRINE_KEYRING=mock` | keep store keys in memory only (tests) – never touches the system keychain |
| `VITRINE_ASC_KEY_ID`, `VITRINE_ASC_ISSUER_ID`, `VITRINE_ASC_KEY` / `VITRINE_ASC_KEY_PATH` | `upload` only: App Store Connect key from the environment (CI), instead of the project's account – see [CI/CD](../ci/#uploading-from-ci) |
| `VITRINE_PLAY_SERVICE_ACCOUNT` / `VITRINE_PLAY_SERVICE_ACCOUNT_PATH` | `upload` only: Google Play service account from the environment (CI) |
