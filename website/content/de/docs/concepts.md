---
title: So funktioniert Vitrine Studio
linkTitle: So funktioniert's
description: "Projekte, Leinwände und Seiten, Lücken, Heimatseiten, Ziele × Sprachen × Varianten, die Einheit lu – die wenigen Ideen, auf denen alles aufbaut."
weight: 10
icon: formats
---

Wenn du Store-Screenshots schon mit Figma oder mit fastlane frameit gebaut
hast, kennst du das: ein Artboard pro Screenshot, pro Gerät, pro Sprache – und
jede Änderung heißt, alle anzufassen. Vitrine Studio dreht das um: Du
gestaltest **einmal**, und die App rendert jedes Bild, das du brauchst.

```text
Projekt ─┬─ Leinwand "phone"  ── Seiten 1 … 6, Hintergründe, Geräte, Texte
         └─ Leinwand "mac"    ── …

Ziel × Sprache × Variante  →  fertige Bilder
(iPhone 6,9″, Google Play Phone, iPad 13″ …) × (en, de, fr …)
```

## Ein Projekt ist ein Ordner {#a-project-is-a-folder}

Ein Projekt ist ein ganz normaler Ordner: `project.yaml` (das Layout), eine
Textdatei pro Sprache in `i18n/`, deine App-Screenshots in `screenshots/`,
dazu optional Bilder, Schriften und Frames. Kein Konto, keine Cloud, nichts
Verstecktes – du kannst es in Git verwalten, Diffs ansehen und reviewen. Jede
Änderung in der App wird **sofort gespeichert**; einen Speichern-Befehl gibt es
nicht. Ändern sich die Dateien auf der Platte (Texteditor, `git checkout`,
ein KI-Assistent), lädt das offene Projekt live neu.

Details: [Projektdateiformat](../file-format/).

## Leinwand und Seiten {#canvas-and-pages}

Eine **Leinwand** ist eine Reihe von Seiten, die du als **ein Bild**
gestaltest. Jede Seite wird zu einem Store-Screenshot. Weil es ein Bild ist,

- kann ein Hintergrund durchgehend über alle Seiten laufen (die Hintergründe
  der Leinwand sind Ebenen über der ganzen Leinwand; für eine Farbe oder ein
  Foto pro Seite legst du auf jede Seite ein *Hintergrund*-Element);
- kann ein Gerät über der Grenze zwischen zwei Seiten liegen und existiert
  trotzdem nur einmal – im Store sieht es aus wie ein Smartphone über zwei
  Screenshots;
- siehst du beim Arbeiten das ganze Set so, wie es im Store erscheint.

Ein Projekt kann mehrere Leinwände haben – zum Beispiel *phone*, *tablet* und
*mac* –, jede mit ihrem eigenen Seitenformat.

{{< shot src="images/canvas-phone-en.jpg" alt="Eine Leinwand mit sechs Smartphone-Seiten einer Musik-App: ein durchgehender Synthwave-Himmel mit Sonne, Neon-Gitterboden und Palmen läuft über alle Seiten; schraffierte Streifen markieren die Lücken zwischen den Seiten." caption="Die Phone-Leinwand der Vorlage *Synthwave Sunset*: sechs Seiten, ein durchgehender Hintergrund. Die schraffierten Streifen sind die Store-Lücken." >}}

## Lücken wie im Store {#gaps-like-in-the-store}

App Store und Google Play zeigen Screenshots nebeneinander mit einer kleinen
Lücke dazwischen. Die Leinwand hat **dieselben Lücken** (im Editor
schraffiert), damit ein Gerät oder eine Linie, die über eine Seitengrenze
geht, im Store richtig weiterläuft. Was in einer Lücke liegt, gehört einfach
zu keinem exportierten Bild.

Die Lückengrößen kommen aus dem **Galerie-Profil** des Ziels. Die Werte für
die Store-Websites sind gemessen; die für die nativen Store-Apps sind noch
Schätzungen.

## Heimatseite {#home-page}

Jedes Element (Gerät, Text, Bild, Form) gehört zu einer **Heimatseite**. Seine
Position bezieht sich auf diese Seite, und es darf in die Nachbarseiten
hineinragen. Texte werden standardmäßig auf ihre Heimatseite beschnitten,
damit eine Überschrift nie in den nächsten Screenshot rutscht.

## Eine Einheit: lu {#one-unit-lu}

Eine Seite ist immer **1000 lu** (logische Einheiten, „logical units“) breit; ihre Höhe folgt aus
dem Seitenverhältnis (eine iPhone-Seite ist etwa 2173 lu hoch). Alle
Positionen, Größen, Schriftgrößen und Ränder sind in lu. Deshalb passt ein
Layout für jede Exportgröße: Die Pixelgröße kommt erst vom Ziel.

## Ziele {#targets}

Ein **Ziel** (Ausgabeziel) ist eine Ausgabe: „App Store iPhone 6,9″“, „Google
Play Phone“, „iPad 13″ quer“, „Mac App Store“, die Google-Play-Feature-Grafik,
ein Blog-Header … Es legt fest, welche Leinwand es zeigt, die Pixelgröße, den
Store-Slot, die Geräte-Frames und das Galerie-Profil. Die meisten Ziele kommen
aus [Presets](../targets/) mit den exakten Store-Größen.

Ein Ziel kann die Leinwand anpassen, ohne sie zu duplizieren:

- **Anderes Seitenverhältnis.** Eine Leinwand für das iPhone (etwa 1:2,17)
  wird auch für Google Play (1:2) gerendert. Elemente halten ihren Abstand zum
  oberen Rand, zum unteren Rand oder zur Mitte der Seite – je nach ihrem
  **Anker**.
- **Gruppieren.** `Seiten pro Bild: 2` setzt zwei Hochformat-Seiten
  nebeneinander in ein Querformat-Bild – ein iPad-Querformat-Set aus der
  Phone-Leinwand.
- **Geräte-Tausch.** Das Play-Ziel ersetzt die iPhone-Frames durch
  Android-Frames.
- **Banner.** Ein Bild aus einem Ausschnitt der Leinwand, z. B. die
  Feature-Grafik.

## Varianten {#variants}

Manchmal braucht ein Format eine leicht andere Anordnung – im
iPad-Querformat-Set sitzt das Smartphone etwas tiefer, das Band ist weniger
steil. Eine **Variante** speichert genau diese Unterschiede (Position, Größe,
Drehung, Heimatseite, Sichtbarkeit …) als **Overrides**. Texte, Screenshots
und Stile bleiben gemeinsam. Ein Ziel zeigt eine Variante; alles, was nicht
überschrieben ist, kommt aus dem Basis-Layout.

## Sprachen {#languages}

Jedes Ziel wird in jeder Projektsprache gerendert. Texte liegen in einer Datei
pro Sprache, Screenshots in einem Ordner pro Sprache. Was fehlt, kommt aus
einer **Ersatzsprache** – `fr-CA` greift auf `fr` zurück, am Ende alles auf
die Standardsprache –, sodass auch eine halbe Übersetzung oder ein
unvollständiges Screenshot-Set gerendert wird. Ein **Sprach-Override** passt
ein Textelement nur für eine Sprache an (eine breitere Box für die lange
deutsche Überschrift). Siehe [Sprachen & Übersetzung](../languages/).

## Ziele × Sprachen × Varianten = deine Bilder {#targets--languages--variants--your-images}

Zusammengenommen: Du bearbeitest eine Leinwand; jedes Ziel rendert sie in
seiner Größe mit seiner Variante, in jeder Sprache. Ein Projekt mit 3 Zielen
und 4 Sprachen liefert dir 12 komplette Bildsätze – und wenn du eine
Überschrift änderst, ändern sich alle.

{{< shot src="images/play-gallery-en.jpg" alt="Dieselben sechs Seiten als Google-Play-Satz: Android-Smartphones statt iPhones auf dem Synthwave-Hintergrund." caption="Dieselbe Leinwand als Google-Play-Ziel: Der Geräte-Tausch zeichnet Android-Smartphones, die Variante `play` passt die kürzeren Seiten an." >}}

## Ein Renderer für alles {#one-renderer-for-everything}

Die Vorschau im Editor, die exportierten Dateien, die Galerie-Vorschauen, der
Store-Upload und die Vorschauen für einen KI-Assistenten kommen alle aus
demselben Renderer. Was du im Editor siehst, landet genau so im Store.

## Drei Arten zu arbeiten {#three-ways-to-work}

- **Die Desktop-App** – der visuelle Editor. Fang hier an.
- **Das Kommandozeilen-Tool `vitrine`** – prüfen, rendern, exportieren,
  übersetzen, importieren und hochladen aus Skripten und CI. Siehe
  [CLI-Referenz](../cli/).
- **KI-Assistenten über MCP** – Claude und andere Assistenten können im
  geöffneten Projekt live gestalten, übersetzen, prüfen und exportieren. Siehe
  [KI-Assistenten](../ai-mcp/).

Alle drei arbeiten auf demselben Projektordner und lassen sich beliebig
kombinieren.
