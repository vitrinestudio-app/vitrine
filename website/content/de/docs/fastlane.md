---
title: Umstieg von fastlane frameit
linkTitle: Umstieg von fastlane
description: "snapshot, screengrab, deliver und supply behalten – frameit durch Vitrine Studio ersetzen, Schritt für Schritt."
weight: 105
icon: arrow
---

Vitrine Studio ersetzt **frameit** in einem fastlane-Setup. Aufnehmen
(snapshot, screengrab) und Hochladen (deliver, supply) können genau so
bleiben, wie sie sind – oder du lässt Vitrine Studio auch hochladen.

## Was wird zu was? {#what-maps-to-what}

| frameit | Vitrine Studio |
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

5. **Entferne `frameit` aus deinem Fastfile** und lösche alte
   `*_framed.png`-Dateien – findet deliver welche, lädt es nur diese hoch.
   `upload_to_app_store` und `upload_to_play_store` laden die Ordner dann
   unverändert hoch.

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
  Suchergebnis-Creatives des App Store (deliver kann sie nicht hochladen),
  Größen, die deliver ablehnt, Sprachen ohne Store-Sprache.

## Oder deliver für Screenshots ganz weglassen {#or-skip-deliver-for-screenshots}

Vitrine Studio kann [direkt hochladen](../store-upload/) – zu App Store
Connect und Google Play, mit einem Plan, den du bestätigst, bevor etwas
gesendet wird, inkrementell (unveränderte Sets werden übersprungen) und
inklusive iPhone Duo und der App-Store-Creatives, mit denen deliver nicht
umgehen kann.
