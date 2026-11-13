---
title: Nutzung mit fastlane
linkTitle: Nutzung mit fastlane
description: "Vitrine Studio in einer fastlane-Lane: snapshot und screengrab nehmen auf, Vitrine Studio gestaltet die Screenshots, deliver und supply laden hoch – Schritt für Schritt."
weight: 30
aliases: ["/de/docs/fastlane/"]
icon: arrow
---

Ich habe selbst jahrelang fastlane und frameit für die Store-Screenshots
meiner Apps verwendet und bin Felix Krause und allen, die fastlane
weiterentwickeln, sehr dankbar für ihre Arbeit. Vitrine Studio ist deshalb
so gebaut, dass es sich **in fastlane einfügt**: snapshot und screengrab
nehmen weiter auf, deliver und supply laden weiter hoch – Vitrine Studio
übernimmt in der Lane die Gestaltung der Screenshots, also die Aufgabe von
frameit. Auf Wunsch kann es die Screenshots auch selbst hochladen.

## Was wird zu was? {#what-maps-to-what}

| Mit frameit | Mit Vitrine Studio |
|---|---|
| ein gerahmtes Bild pro Screenshot | eine Leinwand für das ganze Set; Seiten können Hintergründe und Geräte teilen |
| `Framefile.json` (Hintergrund, Schrift, Abstände) | Leinwand-Hintergründe, Textstile, Gerätepositionen – visuell gestaltet |
| `title.strings`, `keyword.strings` | die Texte in `i18n/<lang>.yaml` (ein Schlüssel pro Überschrift) |
| die Geräterahmen von frameit | eingebaute Rahmen oder Apples Bezels, einmal importiert ([Geräte & Rahmen](../../devices-frames/)) |
| `*_framed.png` neben den Aufnahmen | Bilder in `export/` oder direkt in die Ordner von fastlane geschrieben |

## Schritt für Schritt {#step-by-step}

1. **Lege ein Projekt an** (App oder `vitrine new`) neben deiner App, z. B. in
   `store-screens/`.

2. **Gib den Aufnahmen einen eigenen Ordner.** deliver lädt jedes Bild in
   `fastlane/screenshots/<locale>/` hoch, supply jedes Bild in
   `fastlane/metadata/android/<locale>/images/<type>/` – und genau dort
   speichern snapshot und screengrab ihre Aufnahmen standardmäßig. Sobald
   Vitrine Studio die fertigen Bilder dorthin schreibt, bekäme der Store
   beides. Lass snapshot und screengrab deshalb unter `fastlane/captures/`
   schreiben, in ihrer gewohnten Struktur:

   ```ruby
   # fastlane/Snapfile
   output_directory("./fastlane/captures/screenshots")

   # fastlane/Screengrabfile
   output_directory("fastlane/captures/metadata/android")
   ```

   (Oder gib `output_directory:` in der Lane an `capture_screenshots` /
   `capture_android_screenshots` mit.) Vorhandene Aufnahmen kannst du einfach
   dorthin verschieben.

3. **Importiere deine Aufnahmen** aus diesem Ordner:

   ```sh
   vitrine import screenshots fastlane/captures --dry-run   # Plan prüfen
   vitrine import screenshots fastlane/captures --save-mapping
   ```

   `screenshots/` wird als snapshot-Ausgabe gelesen,
   `metadata/android/<locale>/images/<type>Screenshots/` als
   screengrab-Ausgabe (Phone, 7″- und 10″-Tablets, Wear, TV). Aus `de-DE`
   wird `de`, aus `iPhone 15 Pro Max-01Home.png` der Schlüssel `home`, und
   frameits eigene Dateien werden übersprungen. `--save-mapping` merkt sich
   die Zuordnung, damit der nächste Import (nach dem nächsten snapshot-Lauf)
   ohne Rückfragen läuft. In der App: ☰ → *Screenshots importieren…*.

4. **Einmal gestalten.** Lege die Leinwand an und übernimm die Überschriften
   aus deinen `title.strings` in die Texte (ein Schlüssel pro Screen, eine
   Datei pro Sprache – oder importiere sie als CSV, siehe
   [Übersetzer](../../languages/#working-with-translators-csv-and-xliff)).

5. **Exportiere die fastlane-Struktur** in deinen fastlane-Ordner:

   ```sh
   vitrine export --layout fastlane --out fastlane
   ```

   App-Store-Ziele landen in `screenshots/<asc-locale>/<nn>_<target>.png`,
   Google-Play-Ziele in `metadata/android/<play-locale>/images/<type>/`, die
   Feature-Grafik in `images/featureGraphic.png`.

6. **Nimm `frameit` aus der Lane** – die fertigen Bilder kommen jetzt von
   Vitrine Studio. `upload_to_app_store` und `upload_to_play_store` laden die
   Ordner dann wie gewohnt hoch. Mit `overwrite_screenshots: true` ersetzt
   deliver die Screenshots in App Store Connect, statt sie zu ergänzen;
   supply ersetzt die Screenshots jedes Bildtyps, den es hochlädt.

Eine Lane könnte so aussehen (`sh` läuft in `fastlane/`, daher die `../`):

```ruby
lane :screenshots do
  capture_screenshots                     # snapshot → fastlane/captures/screenshots
  sh("vitrine", "--project", "../store-screens", "import", "screenshots", "captures")
  sh("vitrine", "--project", "../store-screens", "export", "--layout", "fastlane", "--out", ".")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
end
```

### Wenn die Aufnahmen in den Upload-Ordnern bleiben {#if-the-captures-stay-in-the-upload-folders}

Du musst sie nicht sofort verschieben – es geht nichts kaputt:

- Der Import erkennt Vitrine Studios eigene Bilder und überspringt sie
  („exported by Vitrine Studio“): Jeder Export führt seine Dateien in
  `.vitrine-export.json`, und Dateien, die wie der Export heißen
  (`01_<target>.png`, `01_<locale>.png`), werden ebenfalls übersprungen.
- Der Export warnt vor jedem Bild in den Ordnern, in die er schreibt, das
  nicht von Vitrine Studio stammt – Rohaufnahmen, ältere `*_framed.png` –,
  weil deliver und supply es mit hochladen würden.
  `vitrine export --layout fastlane --out fastlane --remove-foreign` löscht
  genau diese Bilder nach dem Export; snapshot und screengrab nehmen sie
  beim nächsten Lauf neu auf. Andere Dateien werden nie angefasst.

## Was der Export prüft {#what-the-export-checks}

- deliver erkennt das Screenshot-Set an der Pixelgröße. Zwei Ziele derselben
  Größenklasse im selben Locale-Ordner sind deshalb ein **Konflikt** (das
  erste gewinnt, das andere wird gemeldet). Dasselbe gilt für zwei Ziele mit
  demselben Play-Bildtyp.
- Mehr als 10 (Apple) oder 8 (Google Play) Bilder in einem Set → Warnung.
- Bilder in den Upload-Ordnern, die nicht von Vitrine Studio exportiert
  wurden → Warnung (siehe oben). Liegen in einem Locale-Ordner
  `*_framed.png`-Dateien, lädt deliver nur diese hoch – auch darauf weist
  der Export hin.
- Bilder für das iPad 12,9″ (2. Generation) bekommen die deliver-Kennung
  `_app_ipad_pro_129` im Namen.
- Mit Hinweis ausgelassen: Ziele ohne Store-Slot (Web, Blog), Kopfzeilen- und
  Suchergebnis-Creatives des App Store (sie gehören nicht zu den
  Screenshot-Sets im fastlane-Ordnerlayout),
  Größen, die deliver ablehnt, Sprachen ohne Store-Sprache.

## fastlane in der CI {#fastlane-in-ci}

fastlane führt dieselbe Lane auf deinem Mac und in der CI aus – auf Bitrise,
Codemagic, CircleCI, GitHub Actions, GitLab CI, Jenkins und anderen.
Installiere auf dem CI-Rechner zuerst das Kommandozeilenwerkzeug
([Installation in der CI](../../automation/#installing-the-cli-in-ci)); die Lane bleibt
gleich. Diese hier prüft das Projekt vor dem Export und lässt den Build auch
bei Warnungen fehlschlagen:

```ruby
lane :screenshots do
  capture_screenshots
  sh("vitrine", "--project", "../store-screens", "import", "screenshots", "captures")
  sh("vitrine", "--project", "../store-screens", "validate", "--strict")
  sh("vitrine", "--project", "../store-screens", "export", "--layout", "fastlane", "--out", ".")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
end
```

(`--out .` schreibt nach `fastlane/screenshots/` und
`fastlane/metadata/android/`.) Der CI-Rechner hat keine eigenen Apple-Bezels –
siehe [Geräterahmen in der CI](../../automation/#device-frames-in-ci).

## Das fastlane-Plugin (Entwurf) {#the-fastlane-plugin-draft}

{{< callout status >}}
Das Plugin `fastlane-plugin-vitrine` ist ein **Entwurf**: Es ist noch nicht
veröffentlicht und wurde noch nicht in fastlane ausgeführt. Bis dahin
erledigen die `sh`-Aufrufe oben dasselbe.
{{< /callout >}}

Es verpackt das Kommandozeilenwerkzeug in drei Actions:

| Action | Macht |
|---|---|
| `vitrine_import` | importiert die Aufnahmen aus `fastlane/captures` (sonst `fastlane/`; oder `source:`), mit `lang_map`, `device_map`, `key_map`, `save_mapping`, `dry_run`; stoppt die Lane, wenn sich Dateien nicht zuordnen lassen |
| `vitrine_validate` | prüft das Projekt; stoppt die Lane bei Fehlern (mit `strict: true` auch bei Warnungen) |
| `vitrine_export` | exportiert die fastlane-Struktur nach `fastlane/` und verweist deliver und supply darauf; stoppt die Lane, wenn in den Upload-Ordnern Bilder liegen, die nicht von Vitrine Studio stammen (`foreign_images: "warn"` meldet sie nur, `"remove"` löscht sie) |

```ruby
lane :screenshots do
  capture_screenshots                     # Snapfile: output_directory("./fastlane/captures/screenshots")
  vitrine_import(project: "store-screens")
  vitrine_validate(project: "store-screens", strict: true)
  vitrine_export(project: "store-screens")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true, overwrite_screenshots: true)
end
```

Pfade in den Actions beziehen sich auf dein Projektverzeichnis (eine Ebene
über `fastlane/`).

## Screenshots mit Vitrine Studio hochladen {#upload-screenshots-with-vitrine-studio}

deliver und supply übertragen weiterhin App-Binary und Metadaten. Die
Screenshots kann auf Wunsch Vitrine Studio übernehmen: Es lädt
[direkt hoch](../../store-upload/) – zu App Store Connect und Google Play, mit
einem Plan, den du bestätigst, bevor etwas gesendet wird, und inkrementell
(unveränderte Sets werden übersprungen). In der Lane lässt du die Screenshots
dann bei deliver und supply aus:

```ruby
upload_to_app_store(skip_screenshots: true)          # Binary und Metadaten wie bisher
upload_to_play_store(skip_upload_screenshots: true)
```

Wie der Upload aus der CI heraus läuft: [Hochladen aus der CI](../../automation/#uploading-from-ci).
