---
title: Automatisierung
linkTitle: Automatisierung
description: "Vitrine Studio ohne die App: Store-Screenshots aus einer CI-Pipeline, einer fastlane-Lane oder eigenen Skripten prüfen, exportieren und hochladen – mit dem vitrine-Kommandozeilenwerkzeug."
weight: 107
icon: repeat
aliases: ["/de/docs/ci/"]
# Alte /de/docs/ci/#…-Links, deren Abschnitt auf eine Unterseite gezogen ist (layouts/alias.html).
aliasFragments:
  github-actions: "github-actions/"
  gitlab-ci: "gitlab-ci/"
  fastlane-bitrise-codemagic-circleci-local: "fastlane/#fastlane-in-ci"
  other-systems: "other-systems/"
---

Das Kommandozeilenwerkzeug `vitrine` läuft ohne Oberfläche überall dort, wo
deine Builds laufen: in jeder CI, in einer fastlane-Lane oder in eigenen
Skripten. Du kannst damit ein Projekt bei jedem Pull Request prüfen, die
Bilder als Build-Artefakte exportieren oder sie aus einem Release-Job in die
Stores hochladen.

{{< callout status >}}
Die CLI-Archive und das Container-Image kommen mit dem ersten Release. Die
fertigen Integrationen (GitHub Action, GitLab-Komponenten, fastlane-Plugin)
sind **Entwürfe** und noch nicht veröffentlicht; alles in diesem Abschnitt,
was nur das Kommandozeilenwerkzeug oder das Image nutzt, funktioniert ab dem
ersten Release.
{{< /callout >}}

Diese Seite beschreibt, was alle Setups gemeinsam haben. Schritt für Schritt
für dein System:

{{< section-cards >}}

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
v=0.1.0
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
| `0.1.0` | genau dieses Release (empfohlen für reproduzierbare Builds) |
| `0.1` | das neueste `0.1.x` |
| `latest` | das neueste Release |

Es ist ein Alpine mit dem statischen `vitrine`, `git`, `jq`,
CA-Zertifikaten und einer POSIX-`sh` (kein `bash`), läuft als
Nicht-Root-Benutzer `vitrine` (UID 10001) in `/work` und hat keinen
Entrypoint, damit CI-Systeme ihre Shell darin starten können. Die Lizenzen
liegen in `/usr/share/doc/vitrine/licenses/`.

```sh
docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/work" \
  ghcr.io/vitrinestudio-app/vitrine-cli:0.1.0 vitrine --project store-screens validate
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
- Alle oder keine: Sind die Variablen eines Stores nur zum Teil gesetzt,
  bricht der Befehl mit Exit-Code 2 ab. So greift ein falsch konfigurierter Job nie auf ein anderes Konto zurück.
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
den Job: GitHub-*Environments* mit Pflicht-Reviewern
([Beispiel](github-actions/#uploading-from-a-release-workflow)) oder
manuelle GitLab-Jobs auf geschützten Branches
([Beispiel](gitlab-ci/#uploading-from-a-manual-job)).

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
