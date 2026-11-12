---
title: CI/CD
linkTitle: CI/CD
description: "Store-Screenshots in GitHub Actions, GitLab CI, Jenkins, Bitrise, Codemagic, Xcode Cloud und anderen prüfen, exportieren und hochladen – mit dem vitrine-Kommandozeilenwerkzeug."
weight: 107
icon: check
---

Das Kommandozeilenwerkzeug `vitrine` läuft in jeder CI ohne Oberfläche. Du
kannst damit ein Projekt bei jedem Pull Request prüfen, die Bilder als
Build-Artefakte exportieren oder sie aus einem Release-Job in die Stores
hochladen.

{{< callout status >}}
Die CLI-Archive und das Container-Image kommen mit dem ersten Release. Die
fertigen Integrationen (GitHub Action, GitLab-Komponenten, fastlane-Plugin)
sind **Entwürfe** und noch nicht veröffentlicht; alles hier, was nur das
Kommandozeilenwerkzeug oder das Image nutzt, funktioniert ab dem ersten
Release.
{{< /callout >}}

## Das Kommandozeilenwerkzeug in der CI installieren {#installing-the-cli-in-ci}

Jedes Release enthält pro Plattform ein eigenständiges CLI-Archiv und
dazu `SHA256SUMS.txt`:

| Runner | Datei |
|---|---|
| Linux x86_64 (glibc 2.35+, z. B. Ubuntu 22.04 oder Debian 12 und neuer) | `vitrine-cli_linux_x86_64.tar.gz` |
| Linux aarch64 / arm64 (glibc 2.35+, z. B. `ubuntu-24.04-arm`) | `vitrine-cli_linux_aarch64.tar.gz` |
| jedes Linux, statischer Build (Alpine, ältere glibc, minimale Images) | `vitrine-cli_linux_x86_64-musl.tar.gz`, `vitrine-cli_linux_aarch64-musl.tar.gz` |
| macOS (Apple Silicon und Intel) | `vitrine-cli_macos_universal.tar.gz` |
| Windows x64 | `vitrine-cli_windows_x64.zip` |

```sh
v=0.2.0
base=https://github.com/vitrinestudio-app/vitrine/releases/download/v$v
curl -sSfLO "$base/vitrine-cli_linux_x86_64.tar.gz"
curl -sSfL "$base/SHA256SUMS.txt" | grep ' vitrine-cli_linux_x86_64.tar.gz$' | sha256sum -c -
tar xzf vitrine-cli_linux_x86_64.tar.gz
export PATH="$PWD/vitrine-cli_$v:$PATH"
vitrine --version
```

Das Programm braucht keine GUI-Bibliotheken, keine Systemschriften und
keinen Schlüsselbund. Schriften, Silbentrennmuster, Gerätedaten und die
eingebauten Rahmen sind enthalten. Die `-musl`-Builds sind vollständig
statisch (gar keine Bibliotheken).

### Container-Image {#container-image}

Für containerbasierte CI (GitLab, Jenkins, CircleCI, Buildkite, Azure, …)
veröffentlicht jedes Release außerdem das Image
**`ghcr.io/vitrinestudio-app/vitrine-cli`** (linux/amd64 und linux/arm64):

| Tag | |
|---|---|
| `0.2.0` | genau dieses Release (empfohlen für reproduzierbare Builds) |
| `0.2` | das neueste `0.2.x` |
| `latest` | das neueste Release |

Es ist ein Alpine mit dem statischen `vitrine`, `git`, `jq`,
CA-Zertifikaten und einer POSIX-`sh` (kein `bash`), läuft als
Nicht-Root-Benutzer `vitrine` (UID 10001) in `/work` und hat keinen
Entrypoint, damit CI-Systeme ihre Shell darin starten können. Die Lizenzen
liegen in `/usr/share/doc/vitrine/licenses/`.

```sh
docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/work" \
  ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0 vitrine --project store-screens validate
```

## Was das Werkzeug für CI bietet {#what-the-cli-offers-ci}

| | |
|---|---|
| **Exit-Codes** | **0** ok · **1** Projektprobleme (Prüffehler, Warnungen mit `--strict`, fehlgeschlagener Export, blockierter Upload-Plan, fehlgeschlagener Upload) · **2** konnte nicht laufen (falsche Argumente, Projekt nicht gefunden, ungültige Zugangsdaten) |
| **Annotationen und Berichte** | `vitrine validate --format github` (GitHub-Annotationen), `--format gitlab` (GitLab Code Quality), `--format junit` (JUnit-XML für GitLab, Jenkins, Azure, CircleCI, Bitrise), `--format json`. Mit `-o DATEI` geht der Bericht in die Datei und der lesbare Bericht ins Log. Befunde zeigen auf die Zeile in `project.yaml`, die das Element oder das Ausgabeziel definiert |
| **Exportliste** | `vitrine export --format json` listet jede Datei (Pfad im Exportordner, Ausgabeziel, Sprache, Größe, unverändert oder geschrieben) – für Artefakt-Uploads |
| **Zugangsdaten** | Umgebungsvariablen statt Schlüsselbund (siehe [Hochladen aus der CI](#uploading-from-ci)) |
| **App-Daten** | `VITRINE_APP_DIR` zeigt auf einen App-Datenordner mit Rahmenpaketen und Vorlagen |

GitHub zeigt pro Schritt höchstens 10 Fehler- und 10 Warnungs-Annotationen.
Fehler kommen zuerst; der vollständige Bericht steht immer im Log.

## GitHub Actions {#github-actions}

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
          v=0.2.0
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

Die geplante Action
[`vitrinestudio-app/vitrine-github-action`](https://github.com/vitrinestudio-app/vitrine-github-action)
übernimmt die Installation (Prüfsumme, Cache; sie wählt bei Bedarf den
arm64- oder den statischen Build) und die Prüf- und Exportschritte in einem
`uses:`:

```yaml
      - uses: vitrinestudio-app/vitrine-github-action@v1
        with:
          version: 0.2.0
          project: store-screens
          validate: true
          export: true
```

## GitLab CI {#gitlab-ci}

```yaml
vitrine-validate:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0
  script:
    - vitrine --project store-screens validate --format gitlab -o gl-code-quality-report.json
  artifacts:
    when: always
    reports:
      codequality: gl-code-quality-report.json

vitrine-export:
  image: ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0
  script:
    - vitrine --project store-screens export --out vitrine-export --no-gallery
  artifacts:
    paths: [vitrine-export/]
```

Code-Quality-Befunde erscheinen im Merge Request. Für den Reiter *Tests* der
Pipeline nimmst du `--format junit -o vitrine-junit.xml` mit `reports:
junit:`. Ohne das Image installierst du das Archiv in `before_script` wie
oben.

Die geplanten CI/CD-Catalog-Komponenten im gitlab.com-Projekt
[`vitrinestudio/vitrine-gitlab-ci`](https://gitlab.com/vitrinestudio/vitrine-gitlab-ci)
fassen diese beiden Jobs zusammen (`validate` mit Code Quality + JUnit,
`export`) und nutzen dasselbe Image:

```yaml
include:
  - component: $CI_SERVER_FQDN/vitrinestudio/vitrine-gitlab-ci/validate@1
    inputs:
      project: store-screens
```

## fastlane (Bitrise, Codemagic, CircleCI, lokal) {#fastlane-bitrise-codemagic-circleci-local}

Die meisten Mobile-Pipelines laufen über fastlane. Exportiere direkt in die
fastlane-Ordner – deliver und supply laden sie unverändert hoch (siehe
[Umstieg von fastlane](../fastlane/)):

```ruby
lane :screenshots do
  sh("vitrine", "--project", "../store-screens", "validate", "--strict")
  sh("vitrine", "--project", "../store-screens", "export", "--layout", "fastlane", "--out", ".")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
end
```

(`sh` läuft in `fastlane/`, daher schreibt `--out .` nach
`fastlane/screenshots/` und `fastlane/metadata/android/`.)

## Andere Systeme {#other-systems}

- **Jenkins:** die Schritte im Container-Image ausführen
  (`agent { docker { image 'ghcr.io/vitrinestudio-app/vitrine-cli:0.2.0' } }`)
  oder das Archiv auf dem Agent installieren. `vitrine validate --format
  junit -o vitrine-junit.xml` mit dem `junit`-Schritt veröffentlichen.
- **Bitrise:** ein *Script*-Schritt, der das Archiv installiert (macOS- oder
  Linux-Stack), danach `vitrine` aufrufen. Für den Testbericht die
  JUnit-Datei mit einer `test-info.json` nach
  `$BITRISE_TEST_RESULT_DIR/vitrine/` schreiben und *Deploy to Bitrise.io*
  hinzufügen. Oder fastlane wie oben.
- **Codemagic:** ein `scripts:`-Eintrag, der das Archiv installiert. Die
  App-Store-Connect-Variablen von Codemagic auf unsere abbilden:
  `VITRINE_ASC_KEY: $APP_STORE_CONNECT_PRIVATE_KEY`,
  `VITRINE_ASC_KEY_ID: $APP_STORE_CONNECT_KEY_IDENTIFIER`,
  `VITRINE_ASC_ISSUER_ID: $APP_STORE_CONNECT_ISSUER_ID`.
- **Xcode Cloud:** das macOS-Archiv in `ci_scripts/ci_post_clone.sh`
  installieren (kein `sudo`; in einen Ordner im Workspace), dann in
  `ci_post_xcodebuild.sh` prüfen oder exportieren.
- **CircleCI, Azure Pipelines, Buildkite:** das Container-Image oder das
  Archiv nutzen. Den JUnit-Bericht über `store_test_results`,
  `PublishTestResults@2` (Format JUnit) oder Buildkite-Annotationen
  veröffentlichen.

## Hochladen aus der CI {#uploading-from-ci}

Auf einem CI-Runner gibt es keinen Schlüsselbund und keine
App-Einstellungen. Deshalb liest `vitrine upload` die Store-Schlüssel auch
aus Umgebungsvariablen. Setze sie aus dem Secret-Speicher deiner CI:

| Store | Variablen |
|---|---|
| App Store Connect | `VITRINE_ASC_KEY_ID`, `VITRINE_ASC_ISSUER_ID` und `VITRINE_ASC_KEY` (der Text der `.p8`-Datei oder dessen Base64) **oder** `VITRINE_ASC_KEY_PATH` (Pfad der `.p8`-Datei) |
| Google Play | `VITRINE_PLAY_SERVICE_ACCOUNT` (das JSON oder dessen Base64) **oder** `VITRINE_PLAY_SERVICE_ACCOUNT_PATH` |

- Sind die Variablen eines Stores gesetzt, werden sie **statt** des Kontos
  aus den Projekteinstellungen verwendet. Das Werkzeug gibt aus, welchen
  Schlüssel es nimmt (Key-ID bzw. E-Mail des Dienstkontos, nie den
  Schlüssel selbst).
- Alle oder keine: Ein halb gefüllter Satz bricht mit Exit-Code 2 ab. So
  greift ein falsch konfigurierter Job nie auf ein anderes Konto zurück.
- Vitrine Studio schreibt die Schlüssel nie auf die Festplatte und nie in
  ein Log. Nur `vitrine upload` liest diese Variablen; die App und die
  Verbindung zu KI-Assistenten (MCP) ignorieren sie.

Ein Upload braucht weiterhin das Bestätigungs-Token des Plans und ohne
Terminal zusätzlich `--yes`. Beide Schritte laufen im selben Job:

```sh
vitrine upload plan --remote --format json > plan.json        # Exit 1 = blockiert
vitrine upload execute --confirm "$(jq -r .confirm_token plan.json)" --yes
```

Bewahre `plan.json` als Artefakt auf – es hält fest, was hochgeladen wurde.
Wenn ein Mensch jeden Upload freigeben soll, baust du das in deiner CI vor
den Job: GitHub-*Environments* mit Pflicht-Reviewern oder manuelle
GitLab-Jobs auf geschützten Branches.

## Geräterahmen in der CI {#device-frames-in-ci}

Apples Product Bezels importierst du auf deinem eigenen Mac, und ihre
Lizenz erlaubt keine Weitergabe – der CI-Rechner hat also keine eigenen. Du
hast zwei Möglichkeiten:

- **Die verwendeten Bezels im Projekt speichern** (Projekteinstellungen →
  Frames, oder `vitrine frames embed`). Sie werden nach `frames/imported/`
  kopiert, und die CI rendert exakt dieselben Bilder wie dein Mac. Nur für
  **private** Repositories – siehe
  [Apple-Bezels in der CI](../devices-frames/#apple-bezels-in-ci).
- Sonst fallen `imported:apple`-Rahmen in der CI auf den eingebauten Rahmen
  zurück, und die Prüfung meldet `imported_frame_missing` (lokal
  `imported_frame_not_stored`). Die eingebauten Rahmen oder ein
  [Rahmenpaket](../devices-frames/) im Projekt liefern ebenfalls überall
  gleiche Ergebnisse.
