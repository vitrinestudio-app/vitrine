---
title: Export
description: "Jedes Ziel × jede Sprache als PNG oder JPEG exportieren, in Vitrine Studios oder fastlanes Ordnerstruktur, mit index.html-Übersicht – inkrementell, aus der App oder der Kommandozeile."
weight: 100
icon: export
---

## Aus der App exportieren {#exporting-from-the-app}

**Export** in der oberen Leiste (oder <kbd>⌘</kbd><kbd>E</kbd>, oder ☰ →
*Exportieren …*):

1. Wähle im Raster **Ziele × Sprachen**, was exportiert wird – *Alle* schaltet
   eine Zeile oder Spalte um. Zellen, die ein Ziel nicht exportiert, sind mit
   *Für dieses Ziel nicht exportiert* markiert.
2. **Ausgabeordner:** leer = `export/` im Projektordner; **Wählen …** für einen
   anderen.
3. **Struktur:** *Standard* oder *fastlane* (siehe unten).
4. **Optionen:** *Alles neu rendern* (den Cache ignorieren),
   *Galerie-Vorschauen*, *Übersichtsseite (index.html)*.
5. **Exportieren.** Fortschritt pro Durchgang; **Abbrechen** stoppt.

Die Zusammenfassung zeigt, wie viele Dateien *Geschrieben* und *Unverändert*
sind, die gefundenen Hinweise und **Im Finder zeigen** / **Im Explorer
zeigen**. **Erneut exportieren** wiederholt dieselbe Auswahl; war schon alles
aktuell, sagt es das (mit einem Knopf *Alles neu rendern*). Aus dem Dialog
führt **Hochladen…** weiter zum [Store-Upload](../store-upload/).

## Was du bekommst {#what-you-get}

```text
export/
  index.html
  ios-iphone-69/
    en/01_plan.png
    en/02_beds.png
    …
    en/_gallery.png
    de/…
  ios-ipad-13-landscape/en/01_plan+beds.png     ← zwei Seiten pro Bild
  play-feature-graphic/en/banner.png
  web-slides/en/01_plots.jpg
```

- Ein Ordner pro Ziel und Sprache; die Dateien sind in Seitenreihenfolge
  nummeriert und nach den **Seiten-IDs** benannt (*Leinwand-Einstellungen →
  Seiten-IDs*). Gruppierte Seiten verbinden ihre IDs mit `+`; Banner heißen
  `banner.png`.
- `_gallery.png` ist eine Vorschau der Store-Galerie (nicht zum Hochladen;
  abschaltbar über *Galerie-Vorschauen*).
- PNG oder JPEG, wie im Ziel eingestellt (*Format*, *Qualität*). Die Bilder
  sind immer **deckend** (ohne Transparenz) und **sRGB** (PNG mit sRGB-Tag) –
  so wie die Stores es erwarten.

## Die Übersichtsseite {#the-overview-page}

`index.html` im Exportordner listet jedes exportierte Bild nach Sprache und
Ziel in Seitenreihenfolge, mit Dateiname, Pixelgröße und Dateigröße. Klicke
ein Bild an, um es zu vergrößern, <kbd>←</kbd>/<kbd>→</kbd> blättert,
<kbd>Esc</kbd> schließt. Die Seite ist eigenständig – sie öffnet sich direkt
aus dem Dateisystem, braucht kein Internet und hat einen hellen und dunklen
Look – praktisch, um sie zur Abstimmung herumzuschicken. Sie listet alle
aktuellen Dateien im Ordner, auch aus früheren Exporten anderer Ziele.

{{< shot src="images/export-overview.jpg" alt="Die Übersichtsseite index.html eines Exports mit einem vergrößerten Bild." caption="Die Übersicht `index.html` eines Exports, ein Bild vergrößert." >}}

## Inkrementelle Exporte {#incremental-exports}

Vitrine Studio merkt sich, was es geschrieben hat
(`export/.vitrine-export.json`). Der nächste Export rendert nur Bilder, deren
Eingaben sich geändert haben – ein geänderter Text, Screenshot, Stil oder eine
Einstellung – und überspringt den Rest. *Alles neu rendern* / `--force` rendert
alle.

Erzeugt ein Ziel weniger oder andere Bilder als beim letzten Export (du hast
z. B. Seiten gelöscht oder Seiten-IDs umbenannt), entfernt der Export die
Bilder, die **er selbst früher** für diese Ziele und Sprachen
geschrieben hat und nicht mehr erzeugt. Andere Dateien im Ordner löscht er nie.

`export/` und `.cache/` stehen in der `.gitignore` des Projekts – Exporte sind
Build-Ergebnisse.

## fastlane-Struktur {#fastlane-layout}

Wähle *Struktur → fastlane* (oder `--layout fastlane`), um die Ordnerstruktur
zu schreiben, die fastlanes **deliver** und **supply** unverändert hochladen
(Standardordner `export/fastlane/`):

```text
screenshots/en-US/01_ios-iphone-69.png              ← deliver
screenshots/de-DE/…
metadata/android/en-US/images/phoneScreenshots/01_en-US.png   ← supply
metadata/android/en-US/images/featureGraphic.png
```

- Nur Ziele mit Store-Slot; keine Galerie-Vorschauen (deliver würde versuchen,
  sie hochzuladen).
- Die Locales kommen aus den Store-Sprachen des Projekts (`de → de-DE`,
  Hebräisch bei Play `iw-IL`).
- Mit Hinweis ausgelassen: Ziele ohne Store-Slot (Web, Blog), die
  App-Store-Creatives für Header und Suchergebnisse, Größen, die deliver
  ablehnt, Sprachen ohne Store-Locale.
- deliver erkennt das Screenshot-Set an der Pixelgröße, deshalb sind zwei
  Ziele derselben Größenklasse in einem Locale-Ordner ein **Konflikt** (das
  erste gewinnt, das andere wird gemeldet). Dasselbe gilt für zwei Ziele mit
  einem Play-Bildtyp. Mehr als 10 (Apple) bzw. 8 (Play) Bilder in einem Set →
  Warnung.
- Liegen im Ordner noch frameit-Dateien `*_framed.png`, gibt es eine Warnung –
  deliver würde nur diese hochladen.

Schritt für Schritt: [Nutzung mit fastlane](../automation/fastlane/).

## Auf der Kommandozeile exportieren {#exporting-from-the-command-line}

```sh
vitrine export                                        # alles, aktueller Ordner
vitrine export --project my-app --target ios-iphone-69 --lang de
vitrine export --out ~/Desktop/shots --force          # anderer Ordner, alles neu rendern
vitrine export --layout fastlane --out ../fastlane    # deliver-/supply-Ordner
vitrine export --no-gallery --no-html --format json   # für Skripte
vitrine export --gallery-only --target play-phone     # nur _gallery.png, ein schneller Blick
```

Ein kompletter Export des Beispielprojekts (neun Ziele, vier Sprachen) dauert
in einem Release-Build etwa zwei Sekunden. Projekte mit vielen großen Fotos
brauchen länger – die meiste Zeit kostet das Komprimieren der PNGs (etwa 25
Sekunden für 84 Fotoseiten). `--gallery-only` schreibt nur die
Galerie-Vorschauen und lässt die Seitendateien früherer Exporte stehen. Exit-Code 1 bedeutet, dass einige
Dateien nicht exportiert werden konnten; siehe [CLI-Referenz](../cli/).

## Einzelne Vorschauen rendern {#rendering-single-previews}

`vitrine render` schreibt ein PNG für einen schnellen Blick – ein Bild, die
ganze Leinwand oder die Galerie – in beliebiger Größe:

```sh
vitrine render --target play-phone --page 2 --width 600 -o page2.png
vitrine render --canvas --lang de                     # die Leinwand mit schraffierten Lücken
vitrine render --gallery --max-edge 800 -o gallery.png   # Galerie, 800 px hoch
```

Ohne `-o` landet die Datei in `.cache/render/`, und ihr Pfad wird ausgegeben.
