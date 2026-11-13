---
title: GitHub Actions
linkTitle: GitHub Actions
description: "Store-Screenshots bei jedem Pull Request mit Annotationen prüfen, die Bilder als Build-Artefakte aufbewahren und aus einem Release-Workflow hochladen."
weight: 10
icon: github
---

Zwei Workflows decken die meisten Projekte ab: Einer prüft und exportiert die
Screenshots bei jedem Pull Request, der andere lädt sie beim Release in die
Stores hoch. Wie du das Kommandozeilenwerkzeug installierst und was es für CI
allgemein bietet: [Automatisierung](../../automation/).

## Prüfen und exportieren bei Pull Requests {#checking-and-exporting-on-pull-requests}

```yaml
name: Store screenshots
on:
  pull_request:
    paths: ["store-screens/**"]
jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - name: Install vitrine
        run: |
          v=0.1.0
          base=https://github.com/vitrinestudio-app/vitrine/releases/download/v$v
          curl -sSfLO "$base/vitrine-cli_linux_x86_64.tar.gz"
          curl -sSfL "$base/SHA256SUMS.txt" | grep ' vitrine-cli_linux_x86_64.tar.gz$' | sha256sum -c -
          tar xzf vitrine-cli_linux_x86_64.tar.gz
          echo "$PWD/vitrine-cli_$v" >> "$GITHUB_PATH"
      - run: vitrine --project store-screens validate --format github --strict
      - run: vitrine --project store-screens export --out export
      - uses: actions/upload-artifact@v4
        with:
          name: store-screenshots
          path: export
```

- `--format github` macht aus jedem Befund eine Annotation im Pull Request,
  an der Zeile in `project.yaml`, die das Element oder das Ausgabeziel
  definiert. GitHub zeigt pro Schritt höchstens 10 Fehler- und 10
  Warnungs-Annotationen. Fehler kommen zuerst; der vollständige Bericht steht
  immer im Log.
- `--strict` lässt den Job auch bei Warnungen fehlschlagen (Exit-Code 1).
- Auf einem arm64-Runner (`ubuntu-24.04-arm`) nimmst du
  `vitrine-cli_linux_aarch64.tar.gz`, auf einem macOS-Runner
  `vitrine-cli_macos_universal.tar.gz` – siehe
  [die Liste der Archive](../../automation/#installing-the-cli-in-ci).

## Die geplante Action {#the-planned-action}

{{< callout status >}}
Die Action ist ein **Entwurf** und noch nicht veröffentlicht. Bis dahin
nimmst du den Installationsschritt oben.
{{< /callout >}}

[`vitrinestudio-app/vitrine-github-action`](https://github.com/vitrinestudio-app/vitrine-github-action)
übernimmt die Installation (Prüfsumme, Cache; sie wählt bei Bedarf den
arm64- oder den statischen Build) und die Prüf- und Exportschritte in einem
`uses:`:

```yaml
      - uses: vitrinestudio-app/vitrine-github-action@v1
        with:
          version: 0.1.0
          project: store-screens
          validate: true
          export: true
```

## Hochladen aus einem Release-Workflow {#uploading-from-a-release-workflow}

Leg den App-Store-Connect-Schlüssel als Repository- oder Environment-Secrets
ab und gib sie `vitrine upload` als Umgebungsvariablen mit (alle Variablen:
[Hochladen aus der CI](../../automation/#uploading-from-ci)). Ein *Environment* mit
Pflicht-Reviewern sorgt dafür, dass ein Mensch jeden Lauf freigibt, bevor der
Job startet:

```yaml
name: Upload store screenshots
on: workflow_dispatch
jobs:
  upload:
    runs-on: ubuntu-latest
    environment: app-store          # Pflicht-Reviewer geben jeden Lauf frei
    steps:
      - uses: actions/checkout@v5
      - name: Install vitrine
        run: |
          v=0.1.0
          base=https://github.com/vitrinestudio-app/vitrine/releases/download/v$v
          curl -sSfLO "$base/vitrine-cli_linux_x86_64.tar.gz"
          curl -sSfL "$base/SHA256SUMS.txt" | grep ' vitrine-cli_linux_x86_64.tar.gz$' | sha256sum -c -
          tar xzf vitrine-cli_linux_x86_64.tar.gz
          echo "$PWD/vitrine-cli_$v" >> "$GITHUB_PATH"
      - name: Upload
        env:
          VITRINE_ASC_KEY_ID: ${{ secrets.ASC_KEY_ID }}
          VITRINE_ASC_ISSUER_ID: ${{ secrets.ASC_ISSUER_ID }}
          VITRINE_ASC_KEY: ${{ secrets.ASC_KEY }}
        run: |
          vitrine --project store-screens upload plan --store apple --remote --format json > plan.json
          vitrine --project store-screens upload execute --confirm "$(jq -r .confirm_token plan.json)" --yes
      - uses: actions/upload-artifact@v4
        if: always()
        with:
          name: upload-plan
          path: plan.json
```

Ist der Plan blockiert, endet der Plan-Schritt mit Exit-Code 1, und es wird
nichts hochgeladen. Für Google Play setzt du `VITRINE_PLAY_SERVICE_ACCOUNT`
und nimmst `--store google`.

## Geräterahmen {#device-frames}

Der Runner hat keine eigenen Apple-Bezels. Speichere die, die dein Projekt
verwendet, im Repository (nur private Repositories), oder nimm die
eingebauten Rahmen – siehe [Geräterahmen in der CI](../../automation/#device-frames-in-ci).
