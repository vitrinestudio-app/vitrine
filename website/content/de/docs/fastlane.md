---
title: Nutzung mit fastlane
linkTitle: Nutzung mit fastlane
description: "Vitrine Studio in einer fastlane-Lane: snapshot und screengrab nehmen auf, Vitrine Studio gestaltet die Screenshots, deliver und supply laden hoch – Schritt für Schritt."
weight: 105
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
| die Geräterahmen von frameit | eingebaute Rahmen oder Apples Bezels, einmal importiert ([Geräte & Rahmen](../devices-frames/)) |
| `*_framed.png` neben den Aufnahmen | Bilder in `export/` oder direkt in die Ordner von fastlane geschrieben |

## Schritt für Schritt {#step-by-step}

1. **Lege ein Projekt an** (App oder `vitrine new`) neben deiner App, z. B. in
   `store-screens/`.

2. **Importiere deine Aufnahmen.** Wähle den ganzen Ordner `fastlane/`:

   ```sh
   vitrine import screenshots fastlane/ --dry-run   # Plan prüfen
   vitrine import screenshots fastlane/ --save-mapping
   ```

   `screenshots/` wird als snapshot-Ausgabe gelesen,
   `metadata/android/<locale>/images/<type>Screenshots/` als
   screengrab-Ausgabe (Phone, 7″- und 10″-Tablets, Wear, TV). Aus `de-DE`
   wird `de`, aus `iPhone 15 Pro Max-01Home.png` der Schlüssel `home`, und
   frameits eigene Dateien werden übersprungen. `--save-mapping` merkt sich
   die Zuordnung, damit der nächste Import (nach dem nächsten snapshot-Lauf)
   ohne Rückfragen läuft. In der App: ☰ → *Screenshots importieren…*.

3. **Einmal gestalten.** Lege die Leinwand an und übernimm die Überschriften
   aus deinen `title.strings` in die Texte (ein Schlüssel pro Screen, eine
   Datei pro Sprache – oder importiere sie als CSV, siehe
   [Übersetzer](../languages/#working-with-translators-csv-and-xliff)).

4. **Exportiere die fastlane-Struktur** in deinen fastlane-Ordner:

   ```sh
   vitrine export --layout fastlane --out fastlane
   ```

   App-Store-Ziele landen in `screenshots/<asc-locale>/<nn>_<target>.png`,
   Google-Play-Ziele in `metadata/android/<play-locale>/images/<type>/`, die
   Feature-Grafik in `images/featureGraphic.png`.

5. **Nimm `frameit` aus der Lane** – die fertigen Bilder kommen jetzt von
   Vitrine Studio – und lösche alte `*_framed.png`-Dateien: Findet deliver
   welche, lädt es nur diese hoch. `upload_to_app_store` und
   `upload_to_play_store` laden die Ordner dann wie gewohnt hoch.

Eine Lane könnte so aussehen:

```ruby
lane :screenshots do
  capture_screenshots                     # snapshot, wie bisher
  sh("vitrine", "import", "screenshots", "../fastlane", "--project", "../store-screens")
  sh("vitrine", "export", "--layout", "fastlane", "--out", "../fastlane", "--project", "../store-screens")
  upload_to_app_store(skip_binary_upload: true, skip_metadata: true)
end
```

## Was der Export prüft {#what-the-export-checks}

- deliver erkennt das Screenshot-Set an der Pixelgröße. Zwei Ziele derselben
  Größenklasse im selben Locale-Ordner sind deshalb ein **Konflikt** (das
  erste gewinnt, das andere wird gemeldet). Dasselbe gilt für zwei Ziele mit
  demselben Play-Bildtyp.
- Mehr als 10 (Apple) oder 8 (Google Play) Bilder in einem Set → Warnung.
- Bilder für das iPad 12,9″ (2. Generation) bekommen die deliver-Kennung
  `_app_ipad_pro_129` im Namen.
- Mit Hinweis ausgelassen: Ziele ohne Store-Slot (Web, Blog), Kopfzeilen- und
  Suchergebnis-Creatives des App Store (sie gehören nicht zu den
  Screenshot-Sets im fastlane-Ordnerlayout),
  Größen, die deliver ablehnt, Sprachen ohne Store-Sprache.

## Screenshots mit Vitrine Studio hochladen {#upload-screenshots-with-vitrine-studio}

deliver und supply übertragen weiterhin App-Binary und Metadaten. Die
Screenshots kann auf Wunsch Vitrine Studio übernehmen: Es lädt
[direkt hoch](../store-upload/) – zu App Store Connect und Google Play, mit
einem Plan, den du bestätigst, bevor etwas gesendet wird, und inkrementell
(unveränderte Sets werden übersprungen). In der Lane lässt du die Screenshots
dann bei deliver und supply aus:

```ruby
upload_to_app_store(skip_screenshots: true)          # Binary und Metadaten wie bisher
upload_to_play_store(skip_upload_screenshots: true)
```

Wie der Upload aus der CI heraus läuft: [Hochladen aus der CI](../ci/#uploading-from-ci).
