---
title: Download
eyebrow: "Download"
description: "Vitrine Studio 1.0 für macOS, Windows und Linux – Installer, Kommandozeilen-Tool und Quellcode."
lead: "**Vitrine Studio 1.0** – kostenlos für macOS, Windows und Linux. Alle Dateien, Prüfsummen und die Kommandozeilen-Archive liegen auf [GitHub Releases](https://github.com/vitrinestudio-app/vitrine/releases/latest)."
lead_pending: "**Vitrine Studio 1.0.0** erscheint gerade. Die Downloads gehen hier mit dem Release online – bis dahin lässt sich die App aus dem Quellcode bauen."
---

{{< os-cards soon="Mit dem Release 1.0.0" get="Herunterladen" macos="Universelles .dmg (Apple Silicon und Intel), von Apple signiert und notarisiert." windows="Setup (.exe) für dein Benutzerkonto. Noch nicht code-signiert – SmartScreen fragt einmal nach: Weitere Informationen → Trotzdem ausführen." linux="AppImage, .deb und .rpm (x86_64). Unsigniert; mit SHA256SUMS.txt prüfen." >}}

## Aus dem Quellcode bauen

Du brauchst [rustup](https://rustup.rs) – die festgelegte Rust-Version wird
automatisch installiert – und für die Desktop-App die
[Tauri-2-Voraussetzungen](https://v2.tauri.app/start/prerequisites/) deines
Systems.

```sh
git clone https://github.com/vitrinestudio-app/vitrine.git
cd vitrine
cargo run -p vitrine-gui --release                       # die Desktop-App
cargo install --path crates/cli                           # das Kommandozeilen-Tool `vitrine`
vitrine export --project tests/fixtures/example           # am Beispielprojekt ausprobieren
```

## Was du bekommst

- **Die Desktop-App** (`vitrine-gui`) – der Editor mit eingebautem MCP-Server für KI-Assistenten.
- **Das Kommandozeilen-Tool** (`vitrine`) – prüfen, rendern, exportieren, übersetzen, importieren und hochladen aus Skripten und CI.

Beide arbeiten mit denselben Projektordnern. Releases erscheinen auf
[GitHub](https://github.com/vitrinestudio-app/vitrine/releases) mit Prüfsummen
und dem zugehörigen Quellcode. Installieren, der Befehl `vitrine` und Updates:
[Installieren & aktualisieren](/de/docs/install/).

## Lizenz

Vitrine Studio ist freie Software unter der GNU General Public License v3 oder
neuer. Der Name „Vitrine Studio“ und das Logo fallen nicht unter die GPL:
unveränderte Kopien dürfen sie tragen, Forks verwenden einen eigenen Namen und
ein eigenes Icon.
