---
title: CLI-Referenz
description: "Alle vitrine-Befehle und Optionen: new, validate, render, export, strings, import, frames, fonts, template, gallery, upload, mcp, install-cli."
weight: 140
icon: terminal
---

Das Kommandozeilen-Tool `vitrine` arbeitet mit denselben Projektordnern wie
die App – für Skripte, CI und schnelle Prüfungen. Es ist in der App enthalten;
unter [Installation](../install/#the-command-line-tool) steht, wie du es in
deinem Terminal verfügbar machst.

Jeder Abschnitt unten enthält den vollständigen `--help`-Text des Befehls
(zum Aufklappen, auf Englisch); er wird aus dem Tool selbst erzeugt
(`scripts/docs/cli-help.sh`).

## Globale Optionen und Exit-Codes {#global-options-and-exit-codes}

| Option | Bedeutung |
|---|---|
| `--project <DIR>` | der Projektordner (der mit `project.yaml`); Vorgabe: der aktuelle Ordner |
| `-q`, `--quiet` | nur Ergebnisse und Fehler ausgeben |
| `-v`, `--verbose` | mehr Details ausgeben (jede exportierte Datei, Bildgrößen) |
| `-V`, `--version` | die Version ausgeben |

Exit-Codes: **0** ok · **1** das Projekt hat Probleme (Prüffehler,
fehlgeschlagene Exporte, blockierter Upload-Plan) · **2** der Befehl konnte
nicht laufen (falsche Argumente, Projekt nicht gefunden). Im Terminal ist die
Ausgabe farbig, außer `NO_COLOR` ist gesetzt.

{{< cli-help "vitrine" >}}

## `new` – ein Projekt anlegen {#new--create-a-project}

```sh
vitrine new my-app                                   # Vorlage Phone, Englisch
vitrine new my-app --template minimal --lang en --lang de --lang fr
vitrine new my-app --template ./bold.vitrine-template --name "My App"
```

Der Ordner darf nicht existieren oder muss leer sein. Das erste `--lang` ist
die Standardsprache.

{{< cli-help "vitrine new" >}}

## `validate` – ein Projekt prüfen {#validate--check-a-project}

```sh
vitrine validate                          # jedes Ziel × jede Sprache
vitrine validate --target play-phone --lang de
vitrine validate --preview                # fehlende Screenshots sind Warnungen
vitrine validate --strict                 # auch bei Warnungen fehlschlagen (CI)
vitrine validate --format json
```

Befunde werden pro Ziel und Sprache als `error`, `warn` oder `info` mit einem
Code aufgelistet (siehe [Fehlerbehebung](../troubleshooting/#validation-findings)).

{{< cli-help "vitrine validate" >}}

## `render` – ein einzelnes Vorschau-PNG {#render--a-single-preview-png}

```sh
vitrine render --target ios-iphone-69 --page 3 --width 600 -o page3.png
vitrine render --canvas --lang de          # die ganze Leinwand mit schraffierten Lücken
vitrine render --gallery --max-edge 800    # die Store-Galerie, 800 px hoch
```

Gibt den Pfad der geschriebenen Datei aus. Bei `--gallery` ist `--max-edge`
die **Höhe** des Bildes.

{{< cli-help "vitrine render" >}}

## `export` – die Bilder schreiben {#export--write-the-images}

```sh
vitrine export
vitrine export --target ios-iphone-69 --target play-phone --lang en
vitrine export --out dist/ --force
vitrine export --layout fastlane --out fastlane
```

Siehe [Export](../export/).

{{< cli-help "vitrine export" >}}

## `strings` – Übersetzungen austauschen {#strings--translation-exchange}

```sh
vitrine strings export --format csv --delimiter semicolon --bom -o texts.csv
vitrine strings export --format xliff20 --missing-only --context -o xliff/
vitrine strings import xliff/*.xlf --dry-run
vitrine strings import texts.csv --create-keys --empty remove
```

Siehe [Sprachen & Übersetzung](../languages/#working-with-translators-csv-and-xliff).

{{< cli-help "vitrine strings export" >}}
{{< cli-help "vitrine strings import" >}}

## `import screenshots` {#import-screenshots}

```sh
vitrine import screenshots fastlane/ --dry-run
vitrine import screenshots shots/ --format flat --lang-map en-US=en --device-map "Pixel 9=android-phone"
```

Siehe [Screenshots](../screenshots/#importing-a-folder-and-fastlane).

{{< cli-help "vitrine import screenshots" >}}

## `frames` – Geräterahmen {#frames--device-frames}

```sh
vitrine frames import /Volumes/Bezel-iPhone-18     # Apple-Bezels (Ordner, .zip, eingehängtes .dmg)
vitrine frames import my-pack.zip --to project      # eigenes Frame-Paket
vitrine frames list                                 # installierte Apple-Rahmen
vitrine frames list --packs                         # Frame-Pakete
vitrine frames remove --model iphone-island --color silver
vitrine frames remove --pack my-pack --from project
```

Siehe [Geräte & Rahmen](../devices-frames/).

{{< cli-help "vitrine frames import" >}}
{{< cli-help "vitrine frames list" >}}
{{< cli-help "vitrine frames remove" >}}

## `fonts` {#fonts}

Listet die Schriftfamilien, die Textstile verwenden können: die
mitgelieferten und die aus dem Ordner `fonts/` des Projekts.

{{< cli-help "vitrine fonts" >}}

## `template` – Vorlagen {#template--templates}

```sh
vitrine template list
vitrine template save --name "Bold gradient" --tag dark -o bold.vitrine-template
vitrine template insert bold-gradient --dry-run
vitrine template import bold.vitrine-template --replace
vitrine template export bold-gradient -o bold.vitrine-template
vitrine template remove bold-gradient
vitrine template new my-app --template bold-gradient   # wie vitrine new
```

Siehe [Vorlagen & Galerie](../templates/).

{{< cli-help "vitrine template save" >}}
{{< cli-help "vitrine template insert" >}}
{{< cli-help "vitrine template list" >}}
{{< cli-help "vitrine template import" >}}
{{< cli-help "vitrine template export" >}}
{{< cli-help "vitrine template remove" >}}

## `gallery` – die Vorlagen-Galerie {#gallery--the-template-gallery}

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

## `upload` – Store-Upload {#upload--store-upload}

```sh
vitrine upload plan --remote
vitrine upload execute --confirm <token> --yes
vitrine upload log --last --events
vitrine upload refdata                      # App-Store-Asset-Typen, nur lesend
```

Siehe [Store-Upload](../store-upload/#on-the-command-line).

{{< cli-help "vitrine upload plan" >}}
{{< cli-help "vitrine upload execute" >}}
{{< cli-help "vitrine upload log" >}}
{{< cli-help "vitrine upload refdata" >}}

## `mcp` – MCP-Server über stdio {#mcp--mcp-server-over-stdio}

```sh
vitrine mcp                                 # mit der laufenden App verbinden (startet sie bei Bedarf)
vitrine mcp --headless --project my-app     # eigene Sitzung, ohne App
```

In diesem Modus gibt der Befehl auf stdout nichts außer MCP-Nachrichten aus.
Siehe [KI-Assistenten](../ai-mcp/).

{{< cli-help "vitrine mcp" >}}

## `install-cli` {#install-cli}

Macht `vitrine` in jedem Terminal verfügbar (macOS: Verknüpfung in
`/usr/local/bin`, Windows: Benutzer-PATH). *Kommandozeilen-Tool
installieren…* in der App macht dasselbe.

{{< cli-help "vitrine install-cli" >}}

## Umgebungsvariablen {#environment-variables}

| Variable | Bedeutung |
|---|---|
| `VITRINE_APP_DIR` | diesen Ordner für die App-Daten (Einstellungen, Store-Konten, Rahmen, Vorlagen) statt des Plattform-Ordners verwenden |
| `NO_COLOR` | keine Farben in der Ausgabe |
| `VITRINE_KEYRING=mock` | Store-Schlüssel nur im Arbeitsspeicher halten (Tests) – der Schlüsselbund des Systems bleibt unberührt |
