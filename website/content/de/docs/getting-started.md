---
title: Erste Schritte
description: "Ein Projekt aus einer Vorlage anlegen, dich im Fenster zurechtfinden, Screenshots und Überschriften einfügen und dein erstes Set exportieren."
weight: 30
icon: book
---

Dieser Rundgang dauert etwa zehn Minuten. Du brauchst Vitrine Studio
([Installieren](../install/)) und ein paar Screenshots deiner App – drei
iPhone-Screenshots im Hochformat sind ideal.

## 1. Ein Projekt anlegen {#1-create-a-project}

Starte Vitrine Studio. Der Startbildschirm bietet **Neues Projekt**,
**Projekt öffnen…** und deine **zuletzt geöffneten** Projekte (du kannst auch
einen Projektordner auf das Fenster ziehen).

1. Klick auf **Neues Projekt**.
2. **Name:** der Name deiner App, z. B. `Fernwood`.
3. **Speicherort:** der übergeordnete Ordner – das Projekt wird darin ein
   neuer Ordner.
4. **Vorlage:** lass **Phone** stehen – drei Seiten mit je einer Überschrift
   und einem Smartphone, gerendert für iPhone 6,9″, Google Play und einen
   Blog-Header. (*Minimal* ist eine einzelne iPhone-Seite; deine eigenen
   Vorlagen erscheinen hier auch.)
5. **Sprachen:** durch Kommas getrennt, die erste ist die Standardsprache,
   z. B. `en, de`.
6. **Erstellen.**

{{< placeholder "Der Startbildschirm mit dem Dialog Neues Projekt: Name, Speicherort, Vorlagen-Karten und Sprachen." >}}

Beim ersten Öffnen eines Projekts erscheint die Übersicht *So ist Vitrine
Studio aufgebaut* auf einer Seite. Ein Blick lohnt sich; im Hilfe-Menü findest
du sie jederzeit wieder.

## 2. Ein Rundgang durchs Fenster {#2-a-tour-of-the-window}

{{< shot src="images/editor-dark.jpg" alt="Der Editor von Vitrine Studio im dunklen Modus: Seitenleiste links, Leinwand in der Mitte, Inspektor rechts." caption="Der Editor (eine ältere Version – der linke Tab heißt jetzt *Leinwände*)." >}}

- **Obere Leiste:** das ☰-Menü (öffnet sich auch über App-Symbol und
  Projektnamen); die Auswahlen **Leinwand**, **Ziel**,
  **Sprache** und **Variante** bestimmen, was du siehst und bearbeitest; der
  Button neben der Ziel-Auswahl öffnet *Leinwände & Ziele*;
  **Hinzufügen** setzt ein neues Element auf die sichtbare Seite; rechts die
  Assistenten-Anzeige, **Export**, das Prüf-Abzeichen (**OK** oder die Zahl
  der Probleme) und Rückgängig/Wiederholen.
- **Linke Seitenleiste:** die Tabs **Leinwände**, **Ebenen**, **Assets** und
  **Übersetzungen**.
- **Mitte:** die Ansicht. Der Umschalter in der unteren Leiste wählt
  **Leinwand** (die ganze Seitenreihe mit den schraffierten Lücken),
  **Seiten** (jedes Ausgabebild), **Store-Galerie** (wie der Store sie zeigt)
  oder **Alle Sprachen** (alle Sprachen nebeneinander).
- **Rechts:** der **Inspektor** – die Einstellungen des ausgewählten Elements,
  oder die Einstellungen der Leinwand (Seitenformat, Seitenzahl,
  Hintergründe, Varianten), wenn nichts ausgewählt ist.
- **Untere Leiste:** der Ansichts-Umschalter, die Auswahl, der **?**-Button
  (Hilfe zu Tastatur & Maus) und die Zoom-Steuerung (−, %, +, *An Fenster
  anpassen*).

## 3. Screenshots einfügen {#3-add-your-screenshots}

Jedes Smartphone zeigt den Screenshot mit seinem **Schlüssel** – in der
Phone-Vorlage `welcome`, `features` und `start`. Am schnellsten geht es so:
**Zieh eine Screenshot-Datei auf ein Smartphone** auf der Leinwand. Sie wird
zum Screenshot dieses Smartphones für die aktuelle Sprache.

Andere Wege:

- Im Tab **Assets** importiert **Importieren …** einzelne Dateien; **Ordner
  importieren…** (auch im ☰-Menü: **Screenshots importieren…**) holt einen
  ganzen Ordner – auch fastlane-Ausgaben. Siehe [Screenshots](../screenshots/).
- Oder kopiere die Dateien selbst in den Projektordner:
  `screenshots/en/welcome.png`, `screenshots/en/features.png`, … Die App
  übernimmt sie sofort.

Sprachen ohne eigene Screenshots nutzen die der Standardsprache – du kannst
also erst einmal nur mit Englisch oder deiner eigenen Sprache anfangen.

## 4. Überschriften schreiben {#4-write-your-headlines}

**Doppelklick auf eine Überschrift** auf der Leinwand, dann tippen.
<kbd>Enter</kbd> übernimmt, <kbd>Umschalt</kbd>+<kbd>Enter</kbd> macht einen
Zeilenumbruch, <kbd>Esc</kbd> bricht ab. Markiere ein Wort und nutze die
Werkzeugleiste für **Fett**, *Kursiv* oder einen Span-Stil (die
Akzentfarbe).

Du bearbeitest den Text der **aktuellen Sprache**. Stell die Auswahl
**Sprache** auf `de` und doppelklick noch einmal, um den deutschen Text zu
schreiben – oder öffne den Tab **Übersetzungen**, um alle Sprachen jedes
Textes in einer Tabelle zu sehen.

Texte verkleinern sich automatisch, bis sie in ihre Box passen. Ist eine
Übersetzung selbst bei der kleinsten Größe zu lang, wird das Prüf-Abzeichen
rot und der Text markiert – siehe
[Lange Texte einpassen](../languages/#fitting-long-texts).

## 5. Anordnen {#5-arrange}

- Zieh Elemente, um sie zu verschieben – sie rasten an Seitenrändern, Mitten
  und anderen Elementen ein. Mit den Anfassern skalierst und drehst du.
- Ein Gerät kann über zwei Seiten liegen; zieh es über die Grenze.
- Ziehen auf einer leeren Fläche verschiebt die Ansicht. Zum Zoomen
  scrollst du mit gedrückter <kbd>⌘</kbd>-Taste (macOS) bzw.
  <kbd>Strg</kbd>-Taste (Windows, Linux) oder ziehst zwei Finger auf dem
  Trackpad auseinander; <kbd>⌘</kbd><kbd>0</kbd> passt die Ansicht ans
  Fenster an.
- Vertan? <kbd>⌘</kbd><kbd>Z</kbd>. Jede Änderung ist ein Rückgängig-Schritt
  und sofort gespeichert.

Unter Windows und Linux steht <kbd>Strg</kbd> überall dort, wo hier
<kbd>⌘</kbd> steht. Alles Weitere steht auf der [Editor-Seite](../editor/);
der **?**-Button unten rechts zeigt alle Tastenkürzel.

## 6. Alle Formate prüfen {#6-check-all-formats}

Stell die Auswahl **Ziel** auf **play-phone**: dasselbe Layout als
Google-Play-Bild (1:2 statt etwa 1:2,17) mit Android-Frame. Schalte die
Ansicht auf **Store-Galerie**, um das Set so zu sehen, wie der Store es zeigt,
oder auf **Alle Sprachen**, um die Übersetzungen zu vergleichen.

Ein Klick auf das Prüf-Abzeichen listet, was fehlt oder nicht passt – *Auf der
Leinwand zeigen* springt zum Problem.

## 7. Exportieren {#7-export}

Drück <kbd>⌘</kbd><kbd>E</kbd> (oder **Export** in der oberen Leiste). Der
Dialog zeigt ein Raster aus **Zielen × Sprachen** – alles ist angehakt. Klick
auf **Exportieren**.

{{< placeholder "Der Export-Dialog: Raster aus Zielen × Sprachen, Ausgabeordner, Struktur und Optionen." >}}

Die Bilder landen im Ordner `export/` des Projekts:

```text
export/
  index.html                  ← Übersicht aller Bilder
  ios-iphone-69/en/01_welcome.png
  ios-iphone-69/en/02_features.png
  ios-iphone-69/en/03_start.png
  ios-iphone-69/en/_gallery.png   ← Vorschau der Store-Galerie
  ios-iphone-69/de/…
  play-phone/en/…
  blog-header/en/banner.jpg
```

**Im Finder zeigen** / **Im Explorer zeigen** öffnet den Ordner. Exporte sind
inkrementell: Beim nächsten Mal werden nur geänderte Bilder gerendert.
Details: [Export](../export/).

## Dasselbe auf der Kommandozeile {#the-same-on-the-command-line}

```sh
vitrine new my-app --lang en --lang de         # Phone-Vorlage, en + de
cp ~/Desktop/shots/welcome.png my-app/screenshots/en/
cp ~/Desktop/shots/features.png my-app/screenshots/en/
cp ~/Desktop/shots/start.png my-app/screenshots/en/
vitrine validate --project my-app              # was fehlt oder nicht passt
vitrine export --project my-app                # → my-app/export/
```

Die Texte bearbeitest du direkt in `my-app/i18n/en.yaml` und `de.yaml` –
oder du öffnest das Projekt (den Ordner `my-app`) in der Desktop-App und
schreibst sie dort wie oben beschrieben. Beides geht auch gemischt: Die App
lädt Änderungen an den Dateien sofort neu. Siehe [CLI-Referenz](../cli/).

## Wie geht's weiter? {#next-steps}

- Weitere Formate – iPad, Mac, die Play-Feature-Grafik: [Ziele & Store-Formate](../targets/)
- Mit Übersetzern oder Claude übersetzen: [Sprachen & Übersetzung](../languages/)
- Echte Apple-Geräte-Frames: [Geräte & Frames](../devices-frames/)
- Direkt in die Stores hochladen: [Store-Upload](../store-upload/)
