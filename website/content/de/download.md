---
title: Download
eyebrow: "Download"
description: "Vitrine Studio für macOS, Windows und Linux – Installer folgen bald; bis dahin aus dem Quellcode bauen."
lead: "Vitrine Studio ist im **Pre-Alpha**-Stadium. Signierte Installer für macOS, Windows und Linux sind in Vorbereitung – bis dahin lässt sich die App aus dem Quellcode bauen."
---

{{< os-cards soon="Bald verfügbar" get="Herunterladen" macos="Geplant: signiertes und notarisiertes .dmg." windows="Geplant: signierter Installer." linux="Geplant: AppImage und .deb." >}}

## Aus dem Quellcode bauen

Du brauchst [rustup](https://rustup.rs) – die festgelegte Rust-Version wird
automatisch installiert – und für die Desktop-App die
[Tauri-2-Voraussetzungen](https://v2.tauri.app/start/prerequisites/) deines
Systems. Node.js oder npm sind nicht nötig.

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
[GitHub](https://github.com/vitrinestudio-app/vitrine) mit Prüfsummen und dem
zugehörigen Quellcode.

## Lizenz

Vitrine Studio ist freie Software unter der GNU General Public License v3 oder
neuer. Der Name „Vitrine Studio“ und das Logo fallen nicht unter die GPL:
unveränderte Kopien dürfen sie tragen, Forks verwenden einen eigenen Namen und
ein eigenes Icon.
