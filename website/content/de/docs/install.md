---
title: Installieren & aktualisieren
description: "Die App unter macOS, Windows oder Linux installieren, den Befehl vitrine im Terminal verfügbar machen, aktuell halten – oder aus dem Quellcode bauen."
weight: 20
icon: download
---

{{< callout status pending >}}
Version 1.0.0 erscheint gerade. Die Download-Buttons gehen mit dem Release
online – bis dahin kannst du [aus dem Quellcode bauen](#build-from-source).
Die Schritte unten beschreiben die Release-Versionen.
{{< /callout >}}

## Download {#download}

Releases erscheinen auf
[GitHub Releases](https://github.com/vitrinestudio-app/vitrine/releases)
(verlinkt auf der [Download-Seite](/de/download/)):

| Plattform | App | Nur Kommandozeilen-Tool |
|---|---|---|
| macOS 11 oder neuer (Apple Silicon und Intel) | `VitrineStudio_macos_universal.dmg` | `vitrine-cli_macos_universal.tar.gz` |
| Windows 10/11 (x64) | `VitrineStudio_windows_x64-setup.exe` | `vitrine-cli_windows_x64.zip` |
| Linux (x86_64) | `VitrineStudio_linux_x86_64.AppImage`, `…_amd64.deb`, `…_x86_64.rpm` | `vitrine-cli_linux_x86_64.tar.gz` |
| Linux (aarch64 / arm64) | – | `vitrine-cli_linux_aarch64.tar.gz` |
| Linux, statischer Build (jede Distribution, Alpine) | – | `vitrine-cli_linux_x86_64-musl.tar.gz`, `vitrine-cli_linux_aarch64-musl.tar.gz` |

Die Archive „nur Kommandozeilen-Tool“ sind für Build-Server und CI gedacht,
wo du die App nicht brauchst. Die normalen Linux-Builds brauchen glibc 2.35
oder neuer (Ubuntu 22.04, Debian 12 und neuer); die statischen
`-musl`-Builds haben keine Abhängigkeiten und laufen auf jedem Linux, auch
auf Alpine. Für CI gibt es außerdem ein Container-Image mit dem
Kommandozeilen-Tool (amd64 und arm64):
`ghcr.io/vitrinestudio-app/vitrine-cli` – siehe [Automatisierung](../automation/).

Jedes Release enthält außerdem `SHA256SUMS.txt`, mit dem du deinen Download
prüfen kannst:

```sh
sha256sum -c SHA256SUMS.txt --ignore-missing
```

## Installieren {#install}

**macOS:** Öffne die `.dmg` und zieh *Vitrine Studio* in *Programme*.

**Windows:** Starte das Setup. Es installiert nur für dein Benutzerkonto
(keine Administratorrechte nötig) und bietet an, den Befehl `vitrine` zu
deinem PATH hinzuzufügen.

**Linux:** Mach das AppImage ausführbar (`chmod +x`) und starte es, oder
installiere `.deb` / `.rpm` mit deinem Paketmanager.

### Warnungen „unbekannter Entwickler“ {#unknown-developer-warnings}

In den Versionshinweisen steht, welche Versionen signiert sind. Eine (noch)
nicht signierte Version zeigt beim ersten Start eine Warnung:

- **macOS:** Rechtsklick auf die App → *Öffnen*, oder *Systemeinstellungen →
  Datenschutz & Sicherheit → Dennoch öffnen*.
- **Windows SmartScreen:** *Weitere Informationen* → *Trotzdem ausführen*.

**Zu Windows:** Die Windows-Version ist vorerst nicht code-signiert, deshalb
zeigt jede Version diese Warnung. Windows meldet „Der Computer wurde durch
Windows geschützt“ mit *Herausgeber: Unbekannter Herausgeber* – das heißt
nur, dass das Setup kein Zertifikat trägt, nicht dass etwas damit nicht
stimmt. Wenn du magst, prüfst du den Download vorher mit `SHA256SUMS.txt`
(`Get-FileHash` in der PowerShell zeigt die SHA-256-Prüfsumme) und wählst
dann *Weitere Informationen* → *Trotzdem ausführen*. Updates, die du in der
App installierst, werden gegen die eigene Update-Signatur von Vitrine Studio
geprüft.

Beim ersten Start einer neu signierten Version fragt macOS eventuell einmal,
ob Vitrine Studio seine eigenen Schlüsselbund-Einträge weiter verwenden darf –
erlaube es (ein Eintrag ist an die Signatur der App gebunden, die sich
geändert hat). Gemeint sind nur die Einträge, die Vitrine Studio selbst
angelegt hat: die Store-Schlüssel, die du in der App eingetragen hast, und
gegebenenfalls deine Anmeldung bei der Vorlagen-Galerie (Dienst
`app.vitrinestudio`). Andere Einträge in deinem Schlüsselbund liest
die App nicht.

## Das Kommandozeilen-Tool {#the-command-line-tool}

Das Kommandozeilen-Tool `vitrine` steckt schon in der App – du musst nichts
extra laden:

| Installation | Wo `vitrine` liegt |
|---|---|
| macOS-App | `/Applications/Vitrine Studio.app/Contents/MacOS/vitrine` |
| Windows-Setup | `vitrine.exe` im Installationsordner |
| Linux deb/rpm | `/usr/bin/vitrine` (schon im PATH) |

Damit du es in jedem Terminal einfach als `vitrine` aufrufen kannst:
**App-Einstellungen → Kommandozeilen-Tool → Kommandozeilen-Tool
installieren…**

- **macOS** legt die Verknüpfung `/usr/local/bin/vitrine` an (und fragt nach
  deinem Administrator-Passwort, wenn der Ordner nicht beschreibbar ist).
- **Windows** fügt den Installationsordner zu deinem Benutzer-PATH hinzu –
  öffne danach ein neues Terminal.
- **deb/rpm:** nichts zu tun.
- **AppImage:** aus dem Image heraus nicht möglich; lade stattdessen das
  CLI-Archiv.

Ein anderes Programm namens `vitrine` wird nie ersetzt. Im Terminal geht das
Gleiche mit dem vollen Pfad: `vitrine install-cli` (`--status` zeigt den
Stand, `--remove` macht es rückgängig, `--yes` spart die Rückfrage).

## Updates {#updates}

Die App sucht beim Start **einmal am Tag** nach einer neuen Version
(*App-Einstellungen → Updates → Einmal am Tag nach Updates suchen*,
standardmäßig an) und immer dann, wenn du **Nach Updates suchen…** wählst
(☰-Menü, macOS-App-Menü oder App-Einstellungen). Sie fragt nur GitHub, ob es
eine neuere Version gibt – keine persönlichen Daten, keine Statistik.

Gibt es ein Update, wählst du **Später**, **Versionshinweise** oder
**Installieren und neu starten**. Ohne deinen Klick wird nichts installiert;
der Download wird zuerst gegen seine Signatur geprüft, dann startet die App
neu (dein Projekt ist ohnehin gespeichert – jede Änderung wird es).

- macOS-App, Windows-Setup und AppImage aktualisieren sich selbst.
- deb/rpm-Installationen zeigen nur den Hinweis – installiere das neue Paket
  von der Release-Seite.
- Selbst gebaute Versionen suchen nie nach Updates.

## Wo Vitrine Studio seine Daten ablegt {#where-vitrine-studio-keeps-its-data}

Projekte liegen, wo du sie hinlegst. Die eigenen Daten der App –
Einstellungen, zuletzt geöffnete Projekte, importierte Geräte-Frames, deine
Vorlagen – liegen hier:

| System | Ordner |
|---|---|
| macOS | `~/Library/Application Support/app.vitrinestudio/` |
| Windows | `%APPDATA%\vitrinestudio\` (`config\` und `data\`) |
| Linux | `~/.config/vitrinestudio/` (Einstellungen) und `~/.local/share/vitrinestudio/` (Daten) |

Store-Schlüssel liegen nicht in diesen Ordnern, sondern im Schlüsselbund des
Systems (siehe [Store-Upload](../store-upload/)). Um einen eigenen
App-Datenordner zu nutzen (für Tests oder CI), setze `VITRINE_APP_DIR=<dir>`
oder gib `--app-dir` an, wo ein Befehl das anbietet.

## Aus dem Quellcode bauen {#build-from-source}

Du brauchst [rustup](https://rustup.rs) (die festgelegte Rust-Toolchain
installiert sich selbst) und für die Desktop-App die
[Tauri-2-Voraussetzungen](https://v2.tauri.app/start/prerequisites/) deines
Systems.

```sh
git clone https://github.com/vitrinestudio-app/vitrine.git
cd vitrine
cargo run -p vitrine-gui --release              # die Desktop-App
cargo install --path crates/cli                  # das Kommandozeilen-Tool `vitrine`
vitrine export --project tests/fixtures/example  # am Beispielprojekt ausprobieren
```

`tests/fixtures/example` ist *Fernwood*, ein ausgedachter Gartenplaner mit
sechs Smartphone-Seiten sowie iPad-, Mac-, Web- und Banner-Zielen auf
Englisch, Deutsch, Französisch und kanadischem Französisch – ein guter
Spielplatz.

## Lizenz {#licence}

Vitrine Studio ist freie Software unter der GNU GPL v3 oder neuer. Die App
bringt ihre Lizenztexte und Hinweise zu Drittkomponenten mit (macOS:
`Vitrine Studio.app/Contents/Resources/licenses/`); der Quellcode eines
Releases ist sein Git-Tag. Name und Logo fallen nicht unter die GPL.
