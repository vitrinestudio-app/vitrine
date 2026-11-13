---
title: Screenshots
description: "Wo deine App-Screenshots hingehören, wie Schlüssel und Screenshot-Varianten funktionieren, ganze Ordner importieren (auch fastlane), Platzhalter und Farbprofile."
weight: 90
icon: image
---

## Schlüssel: ein Name pro Bildschirm {#keys-one-name-per-screen}

Jedes Geräte-Element zeigt den Screenshot mit seinem **Schlüssel** – einem
kurzen Namen wie `home`, `planner` oder `calendar` (Inspektor →
*Screenshot*). Der Schlüssel ist der Dateiname ohne Endung. Zu einem Schlüssel
kann es eine Datei pro Sprache und pro Geräteklasse geben, und Vitrine Studio
wählt für jedes Ziel die richtige.

## Die Ordnerstruktur {#the-folder-layout}

```text
screenshots/
  en/
    ios-phone/home.png        ← iPhone-Ziele, Englisch
    ios-phone/planner.png
    android-phone/home.png    ← Google-Play-Smartphone-Ziele
    ios-tablet/home.png       ← iPad-Ziele
    phone-sync.png            ← ohne Variantenordner: gilt für jedes Ziel
  de/
    ios-phone/home.png        ← Deutsch; was fehlt, kommt aus en
```

`screenshots/<Sprache>/<Screenshot-Variante>/<Schlüssel>.png` – PNG oder JPEG.

- Der Ordner der **Screenshot-Variante** kommt aus dem Ziel (*Screenshots* im
  Ziel-Formular): `ios-phone`, `android-phone`, `ios-tablet`, `mac`, `watch`,
  `wear`, `web` … So zeigt das iPhone-Ziel iPhone-Aufnahmen und das Play-Ziel
  Android-Aufnahmen desselben Bildschirms, unter demselben Schlüssel.
- **Eigene Variantenordner** gehen auch: Trag im Feld *Screenshots* des Ziels
  einen beliebigen Namen ein (etwa `landscape` für Querformat-Aufnahmen) und
  leg die Dateien in `screenshots/<Sprache>/landscape/`. Die Prüfung meldet
  nur eine Variante, die weder im Katalog steht noch als Ordner existiert
  (meist ein Tippfehler).
- Eine Datei **direkt im Sprachordner** (`screenshots/en/home.png`) gilt für
  jede Variante – der einfachste Einstieg, und gut genug, wenn deine iPhone-
  und Android-Aufnahmen gleich aussehen.
- **Suchreihenfolge** pro Schlüssel: der Variantenordner der Sprache, der
  Sprachordner, dann dasselbe für jede Fallback-Sprache (`fr-CA → fr → en`).

## Screenshots in der App hinzufügen {#adding-screenshots-in-the-app}

- **Zieh eine Datei auf ein Gerät** auf der Leinwand: Sie wird der Screenshot
  dieses Geräts für die aktuelle Sprache (um Schlüssel und Ordner kümmert sich
  die App).
- **Assets → Importieren …** (oder Dateien auf den Tab Assets oder irgendwo auf
  die Leinwand ziehen): wähle *Art* (*Screenshot*, *Bild (assets/)*, *Schrift
  (fonts/)*), *Sprache*, *Variante* und *Schlüssel*. Die Variante wird nach
  Möglichkeit an der Pixelgröße erkannt.
- **Screenshots importieren…** (☰-Menü, oder **Ordner importieren…** im Tab
  Assets): ein ganzer Ordner auf einmal – siehe unten.

**Assets → Screenshots** listet jeden Schlüssel pro Sprache und Variante,
markiert *fehlende* und *unbenutzte* und zeigt, welche aus einer
Fallback-Sprache geliehen sind. Ein Klick auf ein Vorschaubild
(Screenshots und Bilder) öffnet die **Vorschau**: das Bild groß mit
Dateiname, Pixelgröße, Dateigröße, Format, Farbprofil (mit Warnung, wenn es
ignoriert wird) und den Elementen, die es verwenden; <kbd>←</kbd>/<kbd>→</kbd>
blättern, <kbd>Esc</kbd> oder ein Klick daneben schließt sie.

## Einen Ordner importieren (auch fastlane) {#importing-a-folder-and-fastlane}

Der Import versteht drei Strukturen und erkennt sie automatisch:

- **Vitrine-Studio-Ordner** (`<Sprache>/<Variante>/<Schlüssel>.png`);
- **fastlane** – snapshot-Ausgabe (`de-DE/iPhone 17 Pro Max-01_home.png`) und
  screengrab-Ausgabe (`metadata/android/<Locale>/images/phoneScreenshots/`);
  du kannst den ganzen Ordner `fastlane/` wählen;
- **Lose Dateien** in einem Ordner.

Was er tut:

- Locales werden zu Projektsprachen (`de-DE → de`; Store-Codes wie `iw-IL →
  he` werden verstanden).
- Gerätenamen werden zu Screenshot-Varianten – über deine Zuordnung oder die
  Pixelgröße (*Gerät → Screenshot-Variante*, *Locale → Sprache*).
  screengrab-Ordner werden `android-phone`, `android-tablet` (7″/10″) und
  `wear` zugeordnet.
- Namen werden zu Schlüsseln: `01_home.png` → `home`,
  `iPhone 15 Pro Max-01Home.png` → `home`.
- frameit-Reste und Store-Grafiken werden **mit Begründung übersprungen**:
  `*_framed.png`, `Framefile.json`, `title.strings`, `keyword.strings`,
  frameit-Hintergründe, deliver-Metadaten, Plays Feature-Grafik und Icons.

Der Dialog zeigt jede Datei mit erkannter *Sprache*, *Variante* und
*Schlüssel* (*sicher*, *wahrscheinlich*, *unsicher*), was passieren wird
(*neu*, *ersetzen*, *unverändert*) und welche Screenshots danach noch *fehlen*.
Zeilen, die eine Entscheidung brauchen, sind markiert. **Zuordnung im Projekt
speichern** legt die Geräte-, Locale- und Schlüssel-Zuordnungen in
`project.yaml` ab, damit der nächste Import (z. B. nach einem neuen
snapshot-Lauf) ohne Rückfragen genauso zuordnet.

{{< shot src="images/import-screenshots.jpg" alt="Der Dialog Screenshots importieren mit einem fastlane-Snapshot-Ordner: 25 neue Dateien, gruppiert nach Sprache und Screenshot-Variante, jede mit Sprache, Variante und Schlüssel." caption="**Screenshots importieren** mit einem fastlane-Snapshot-Ordner: erkannte Sprachen, Varianten und Schlüssel." >}}

Auf der Kommandozeile:

```sh
vitrine import screenshots fastlane/ --dry-run      # den Plan zeigen
vitrine import screenshots fastlane/ --save-mapping
vitrine import screenshots shots/ --device-map "iPhone 17 Pro Max=ios-phone" --lang-map pt-PT=pt --key-map 01_home=home
```

Bilder, die Vitrine Studio selbst exportiert hat (etwa in denselben
`fastlane/`-Ordner), werden erkannt und übersprungen. Wie Vitrine Studio in
eine fastlane-Lane passt, mit den Aufnahmen in einem eigenen Ordner:
[Nutzung mit fastlane](../automation/fastlane/).

## Fehlende Screenshots und Platzhalter {#missing-screenshots-and-placeholders}

Ein Schlüssel ohne Datei in irgendeiner Sprache der Kette zeigt im Editor
einen **Platzhalter**, damit du gestalten kannst, bevor du Aufnahmen hast. Die
Prüfung meldet ihn als Fehler (`screenshot_missing`), und der Store-Upload
blockiert jedes Screenshot-Set, das noch einen enthält – so landet kein Platzhalter
versehentlich im Store. (Ein Export schreibt das Bild trotzdem mit Platzhalter,
was für Reviews praktisch ist.) Während du gestaltest, behandelt der
**Vorschau-Modus** in der Prüfliste (oder `vitrine validate --preview`) sie als
Warnungen.

## Größe und Seitenverhältnis {#size-and-aspect}

- Weicht das Seitenverhältnis eines Screenshots mehr als ein wenig vom Display
  des Geräts ab, gibt es eine Warnung (`screenshot_aspect`, „falsches
  Gerät?“) – z. B. eine iPhone-Aufnahme in einem Android-Rahmen.
  *Einpassung* (*Füllen*, *Einpassen*, *Strecken*) legt fest, wie er das
  Display füllt.
- Ein Screenshot, der um mehr als 15 % vergrößert wird, bekommt eine Warnung
  (`screenshot_upscale`) – er kann unscharf wirken. Nimm Aufnahmen vom größten
  Gerät einer Klasse.

## Farbprofile (Display P3) {#colour-profiles-display-p3}

iPhone- und Mac-Screenshots sind meist in **Display P3**. Vitrine Studio
arbeitet in sRGB und wandelt Bilder mit Farbprofil beim Laden nach sRGB um
(Display P3, Adobe RGB, Graustufenprofile …; PNG und JPEG), im Editor, im
Export, beim Upload und in den Vorschauen für Assistenten gleich. Bilder ohne
Profil gelten als sRGB. Sehr gesättigte P3-Farben werden auf die nächste
sRGB-Farbe begrenzt – wie bei jedem sRGB-Export.

Lässt sich ein Profil nicht verwenden (CMYK-JPEG, HDR-PNG, unlesbares Profil),
wird das Bild so gezeichnet, als wäre es sRGB, und die Prüfung vermerkt
`color_profile_unsupported` – speichere es als sRGB- oder Display-P3-PNG neu.
