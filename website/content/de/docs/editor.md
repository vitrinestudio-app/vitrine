---
title: Der Editor
description: "Ansichten, Auswählen und Anordnen, die Elementarten, Inspektor und Ebenen, das Kontextmenü, Kopieren und Einsetzen, Sperren, Varianten und Rückgängig."
weight: 40
icon: canvas
---

## Was du siehst: die vier Auswahlen {#what-you-see-the-four-pickers}

Die Auswahlen in der oberen Leiste bestimmen, **was** die Ansicht in der Mitte
zeigt und **wohin deine Änderungen gehen**:

| Auswahl | Bedeutung |
|---|---|
| **Leinwand** | welche Leinwand du gestaltest (z. B. `phone`, `mac`) |
| **Ziel** | welche Ausgabe du ansiehst – Größe, Frames, Lücken (z. B. `ios-iphone-69  (1320×2868)`) |
| **Sprache** | wessen Texte und Screenshots angezeigt und bearbeitet werden |
| **Variante** | welches Layout: die eigene Variante des Ziels, das **Basis**-Layout oder eine andere Variante |

Jede Auswahl endet mit einem „+“-Eintrag: **Neue Leinwand …**, **Ziel
hinzufügen …**, **Sprache hinzufügen …**, **Neue Variante …**.

Die Auswahl Variante ändert nur, was du **ansiehst und bearbeitest** – welche
Variante ein Ziel exportiert, legst du im Ziel selbst fest (siehe
[Varianten](#variants)).

## Ansichten {#views}

Umschalten in der unteren Leiste:

- **Leinwand** – die ganze Seitenreihe mit schraffierten Lücken; hier
  bearbeitest du. Ein gestricheltes Rechteck zeigt den Sicherheitsbereich
  eines Ziels, falls es einen hat.
- **Seiten** – jedes Ausgabebild des Ziels so, wie es exportiert wird (zwei
  Seiten nebeneinander bei gruppierten Zielen, der Ausschnitt bei Bannern).
- **Store-Galerie** – die Bilder so, wie der Store sie zeigt, mit seinen
  Lücken und abgerundeten Ecken. Banner haben keine Galerie.
- **Alle Sprachen** – jede Sprache als eigene Zeile, um Übersetzungen zu
  vergleichen; überlaufende Texte sind markiert. Ein Klick auf ein Bild öffnet
  es auf der Leinwand.

**Zoomen und Verschieben** funktioniert in jeder Ansicht gleich: Scrollen
verschiebt (<kbd>Umschalt</kbd> + Scrollen seitwärts),
<kbd>⌘</kbd>/<kbd>Strg</kbd> + Scrollen oder Pinch zoomt um den Mauszeiger,
Ziehen auf leerer Fläche (oder <kbd>Leertaste</kbd> + Ziehen, oder mit der
mittleren Maustaste) verschiebt die Ansicht. <kbd>⌘</kbd><kbd>+</kbd> /
<kbd>⌘</kbd><kbd>−</kbd> zoomen, <kbd>⌘</kbd><kbd>0</kbd> passt ans Fenster
an, <kbd>⌘</kbd><kbd>1</kbd> zeigt die Originalgröße (ein Export-Pixel pro
Bildschirm-Pixel). Nach *An Fenster anpassen* bleibt die Ansicht angepasst,
wenn du das Fenster vergrößerst, bis du selbst zoomst oder verschiebst.
Rollbalken erscheinen, wenn der Inhalt nicht passt.

Unter Windows und Linux liest du in diesem Handbuch <kbd>⌘</kbd> als
<kbd>Strg</kbd> und <kbd>⌥</kbd> als <kbd>Alt</kbd>.

{{< shot src="images/gallery-view.jpg" alt="Die Ansicht Store-Galerie: sechs iPhone-Bilder einer Pop-Art-Dating-App nebeneinander, mit den Lücken und abgerundeten Ecken des Stores." caption="Die Ansicht **Store-Galerie** (Vorlage *Pop-Art Panels*, iPhone 6.9″)." >}}

## Elemente {#elements}

**Hinzufügen** in der oberen Leiste setzt ein neues Element auf die
**sichtbare Seite**:

| Art | Was es ist |
|---|---|
| **Gerät** | ein Smartphone, Tablet, Laptop, Desktop, eine Uhr oder ein Browserfenster mit einem Screenshot – siehe [Geräte & Frames](../devices-frames/) |
| **Text** | eine Überschrift oder Beschriftung; der Text selbst liegt in den Übersetzungen |
| **Bild** | ein Logo, Badge, eine Illustration – PNG, JPEG oder SVG aus dem Ordner `assets/` des Projekts |
| **Form** | Rechteck (mit Eckenradius), Ellipse oder Linie, mit Füllung und Kontur |
| **Seitenhintergrund** | eine Füllung (Farbe, Verlauf oder Bild) für eine Seite, über dem Leinwand-Hintergrund |

**Gruppen** fassen Elemente zusammen, die sich gemeinsam bewegen, drehen und
skalieren: <kbd>⌘</kbd><kbd>G</kbd> gruppiert die Auswahl,
<kbd>⇧</kbd><kbd>⌘</kbd><kbd>G</kbd> hebt die Gruppierung auf. Die
**Skalierung** einer Gruppe im Inspektor verkleinert oder vergrößert alles
darin – Positionen, Größen, Schriftgrößen, Konturen, Eckenradien und Schatten
– um ihren Drehpunkt (die Mitte ihrer Elemente, verschiebbar mit
**Drehpunkt**). Texte werden in der neuen Größe gesetzt und bleiben scharf.
In einer Variante gesetzt, macht eine einzige Änderung eine Karte oder ein
Ticket für ein Format kleiner. Eine skalierte Gruppe lässt sich nicht
auflösen; stell die Skalierung vorher auf 100 % zurück.

Hintergründe, die über **alle** Seiten laufen, gehören zur Leinwand: Heb die
Auswahl auf und bearbeite **Leinwand-Hintergründe** im Inspektor.

Ein **Seitenhintergrund** füllt seine ganze Heimatseite; ein Klick auf die
Seite wählt deshalb das Element davor aus, nicht den Hintergrund. Du wählst
ihn per **Doppelklick auf eine freie Stelle** der Seite, mit Rechtsklick →
*Hintergrund „…“ auswählen*, unter **Ebenen** oder in den Einstellungen der
Leinwand (nichts ausgewählt) unter **Seitenhintergründe** aus – dort gibt es
auch *Seitenhintergrund hinzufügen*. Der Inspektor bearbeitet dann
*Heimatseite*, *Füllung* (Farbe, Verlauf oder Bild), *Deckkraft*, *Ebene
(z)*, *Beschnitt* und die Sichtbarkeit.

## Auswählen {#selecting}

- **Klick** wählt ein Element aus (eine Gruppe als Ganzes).
- **Doppelklick** wählt das Element innerhalb seiner Gruppe aus – bei einem
  Text öffnet er den Text zum Bearbeiten, auf einer freien Stelle einer Seite
  wählt er das Hintergrund-Element der Seite aus.
- <kbd>⌘</kbd> + Klick wählt das Element unter dem Mauszeiger direkt aus,
  auch innerhalb einer Gruppe.
- <kbd>Umschalt</kbd> + Klick fügt zur Auswahl hinzu oder entfernt daraus.
- <kbd>Umschalt</kbd> + Ziehen auf leerer Fläche zieht einen Auswahlrahmen
  (ergänzt die Auswahl).
- <kbd>⌘</kbd><kbd>A</kbd> wählt alles aus (außer gesperrten Elementen),
  <kbd>Esc</kbd> hebt die Auswahl auf.

## Verschieben und anordnen {#moving-and-arranging}

- **Ziehen** verschiebt. Elemente rasten an Seitenrändern, Mitten und anderen
  Elementen ein; halte <kbd>⌥</kbd>, um das Einrasten abzuschalten,
  <kbd>Umschalt</kbd>, um nur waagerecht oder senkrecht zu verschieben.
- **Skalieren** mit den Anfassern; <kbd>Umschalt</kbd> behält das
  Seitenverhältnis (nicht bei Texten).
- **Drehen** mit dem Anfasser über der Auswahl; er rastet bei 0°, ±5°, ±15°,
  ±45° und ±90° ein, mit <kbd>Umschalt</kbd> drehst du frei in
  0,1°-Schritten. Positive Winkel drehen im Uhrzeigersinn.
- **Pfeiltasten** verschieben um 1 lu, mit <kbd>Umschalt</kbd> um 10 lu.
- <kbd>[</kbd> / <kbd>]</kbd> schicken eine Ebene nach hinten / nach vorne,
  <kbd>Umschalt</kbd>+<kbd>[</kbd> / <kbd>Umschalt</kbd>+<kbd>]</kbd> ganz
  nach hinten / ganz nach vorne.
- <kbd>⌘</kbd><kbd>D</kbd> dupliziert, <kbd>⌫</kbd> oder <kbd>Entf</kbd>
  löscht.

Ziehst du ein Element auf eine andere Seite, folgt seine **Heimatseite**. Um
sie festzuhalten (z. B. ein Smartphone über zwei Seiten, das zur ersten
gehört), nutze das Schloss neben **Heimatseite** im Inspektor.

## Das Kontextmenü {#the-context-menu}

{{< shot src="images/context-menu.jpg" float="right" width="330" alt="Das Kontextmenü auf einem ausgewählten Smartphone mit seinen Befehlen zum Bearbeiten und Anordnen." caption="Das Kontextmenü der Leinwand auf einem ausgewählten Smartphone." >}}

Rechtsklick auf ein Element (oder <kbd>Umschalt</kbd>+<kbd>F10</kbd> / die
Menütaste) bietet: **Ausschneiden**, **Kopieren**, **Einsetzen**,
**Duplizieren**, **Löschen**, **Ganz nach vorne**, **Eine Ebene nach vorne**,
**Eine Ebene nach hinten**, **Ganz nach hinten**, **Gruppieren** /
**Gruppierung aufheben**, **Sperren** / **Entsperren** und **Ausblenden**. Es
wirkt auf die ganze Auswahl.

Rechtsklick auf leere Fläche bietet **Auf Seite N einsetzen**, **Alles auf
Seite N auswählen**, **Alles auswählen** und **An Fenster anpassen**.

### Kopieren und einsetzen {#copy-and-paste}

<kbd>⌘</kbd><kbd>C</kbd>, <kbd>⌘</kbd><kbd>X</kbd>, <kbd>⌘</kbd><kbd>V</kbd>
kopieren, schneiden aus und setzen Elemente ein – auch zwischen Leinwänden
desselben Projekts.

- Setzt du dort ein, wo die Originale noch liegen, rücken die Kopien 40 lu
  nach rechts unten (mit jedem Einsetzen weiter).
- **Auf Seite N einsetzen** behält die Position relativ zur Seite – so
  wiederholst du ein Layout auf einer anderen Seite.
- Kopien behalten ihre Varianten-Overrides (für Varianten, die die Leinwand
  hat).
- Die Zwischenablage gehört zum offenen Projekt; es ist nicht die
  Zwischenablage des Systems, und sie wird geleert, wenn du das Projekt
  schließt.

### Sperren und ausblenden {#lock-and-hide}

- **Sperren** macht ein Element auf der Leinwand unanklickbar – Klicks und
  Auswahlrahmen gehen hindurch, sodass du an dem arbeiten kannst, was hinter
  einer großen Form liegt. Im Tab **Ebenen** erscheint ein Schloss-Button,
  der es wieder entsperrt. Sperren wirkt nur im Editor, nie auf die
  gerenderten Bilder.
- **Ausblenden** schaltet das Element ab (*Sichtbar* im Inspektor). Im Tab
  Ebenen blendest du es mit dem Augen-Button wieder ein.

## Der Inspektor {#the-inspector}

{{< shot src="images/inspector-text.jpg" float="right" width="338" alt="Der Inspektor mit einer ausgewählten Überschrift: Layout, Text-Schlüssel und Inhalt, Stil, Schriftgröße, Farbe, Box, Ausrichtung und Auto-Fit." caption="Der Inspektor mit einer ausgewählten Überschrift." >}}

Ist ein Element ausgewählt, zeigt der Inspektor seine Einstellungen. Die
wichtigsten:

- **Anordnung:** *Heimatseite*, *Mitte* (Position der Mitte auf der
  Heimatseite, in lu), *Größe*, *Drehung*, *Drehpunkt*, *Anker*.
- **Anker** (*Oben*, *Unten*, *Mitte*, *Strecken*) bestimmt, was passiert,
  wenn die Seite eines Ziels höher oder niedriger ist als die Seite der
  Leinwand: Das Element hält seinen Abstand zu diesem Rand (*Strecken*
  skaliert mit der Seite). Smartphones unten auf der Seite → *Unten*.
- **Darstellung:** *Deckkraft*, *Ebene (z)*, *Beschnitt* (Texte werden
  standardmäßig auf ihre *Heimatseite* beschnitten), *Sichtbar*, *In Zielen*,
  *In Sprachen* (z. B. ein Badge nur in den App-Store-Zielen).
- **Gerät:** *Modell*, *Rahmen*, *Farbe*, *Ausrichtung*, *Screenshot* (der
  Schlüssel), *Einpassung*, *Schatten* … – siehe
  [Geräte & Frames](../devices-frames/).
- **Text:** *Schlüssel*, *Text*, *Stil*, *Textbox* (*Ränder* zu den
  Seitenkanten oder eine *Freie Box*), *Ausrichtung*, *Vertikal*, *Auto-Fit*
  (*Verkleinern bis*, *Höchstens Zeilen*), *Silbentrennung*, *Farbe*,
  *Schriftgröße*, *Kontur*, *Leuchten*, *Schatten*. Darunter zeigt *Gesetzt*
  die Schriftgröße und Zeilenzahl, die der Text tatsächlich bekommen hat, oder
  *Text passt nicht in die Box*.
- **Bild:** *Asset* (der Augen-Button öffnet die Bildvorschau), *Einpassung*, *Schatten*. **Form:** *Form*, *Radius*,
  *Füllung*, *Kontur*. Füllungen können eine Farbe, ein linearer oder radialer
  Verlauf oder ein Bild sein.

Zahlenfelder: Zieh die Beschriftung seitwärts, um den Wert zu ändern
(<kbd>Umschalt</kbd> für größere Schritte), <kbd>↑</kbd>/<kbd>↓</kbd> gehen
einen Schritt, <kbd>Enter</kbd> übernimmt, <kbd>Esc</kbd> verwirft. Farben
nehmen `#RRGGBB`, `#RRGGBBAA` oder eine Projektfarbe (`@accent`).

Ist **nichts ausgewählt**, zeigt der Inspektor die **Einstellungen der
Leinwand**: *Seitenverhältnis* und *Seitenhöhe*, Anzahl der *Seiten*
(am Ende hinzufügen oder entfernen), *Seiten-IDs* (sie benennen die
exportierten Dateien), *Leinwand-Hintergründe*, *Seitenhintergründe* und
die *Varianten*.

Der Inspektor ist breiter als früher und **in der Breite veränderbar**: Zieh
seinen linken Rand (Doppelklick darauf: Standardbreite). Die Breite bleibt
gespeichert.

## Ebenen {#layers}

Der Tab **Ebenen** listet die Elemente der Leinwand in Zeichenreihenfolge
(oben = vorne). Klick wählt aus, <kbd>Umschalt</kbd> + Klick einen Bereich,
<kbd>⌘</kbd> + Klick schaltet einzelne um, <kbd>↑</kbd>/<kbd>↓</kbd> gehen
durch die Liste, Ziehen ändert die Reihenfolge, Doppelklick /
<kbd>Enter</kbd> / <kbd>F2</kbd> benennt um (die Element-Id). Gruppen lassen
sich auf- und zuklappen; Augen- und Schloss-Buttons zeigen ausgeblendete und
gesperrte Elemente.

## Varianten {#variants}

Eine **Variante** speichert formatabhängige Unterschiede einer Leinwand –
z. B. die Smartphones im iPad-Querformat-Set etwas tiefer – als
**Overrides**, während Texte, Screenshots und Stile gemeinsam bleiben.

- Die Auswahl Variante steht standardmäßig auf der **eigenen Variante des
  Ziels** (`… · Ziel`). Solange eine Variante angezeigt wird, sagt der
  Inspektor *Änderungen werden Overrides von …*: Jedes Verschieben, Skalieren
  oder Ändern landet nur in dieser Variante. Überschriebene Felder sind
  **lila** markiert, mit *Auf Basis zurücksetzen* pro Feld und *Alle
  zurücksetzen*.
- Stell die Auswahl auf **Basis**, um das gemeinsame Layout zu bearbeiten.
- **Neue Variante …** (Auswahl Variante, Einstellungen der Leinwand oder
  *Leinwände & Ziele*): *Name*, *Basiert auf* (das Basis-Layout oder
  eine Kopie einer anderen Variante), *Inhalts-Skalierung* (skaliert Geräte
  und Bilder in dieser Variante) und *Gezeigt von* (Ziele, die sie ab jetzt
  verwenden).

Welche Variante ein Ziel **exportiert**, steht im Ziel (*Leinwände &
Ziele* → Ziel → *Variante*). Siehe
[Ziele & Store-Formate](../targets/).

## Leinwände {#canvases}

Der Tab **Leinwände** listet die Leinwände des Projekts mit ihren Seiten und
Zielen; **Neue Leinwand …** und **Verwalten …** öffnen *Leinwände &
Ziele* (auch im ☰-Menü). Dort kannst du

- eine Leinwand anlegen – *Leer*, *Überschrift + Gerät* (eine Überschrift und
  ein Gerät auf jeder Seite) oder als *Kopie einer Leinwand* – mit einem
  Seitenformat aus einem Store-Preset und im selben Schritt das passende Ziel
  (*Dieses Ziel gleich anlegen*);
- eine Leinwand umbenennen, duplizieren oder entfernen (Entfernen löscht auch
  ihre Ziele);
- Leinwände *Aus Vorlage* einfügen und *Als Vorlage speichern…* – siehe
  [Vorlagen](../templates/);
- Store-Banner mit **Banner aus dieser Leinwand** hinzufügen.

Eine Leinwand wird nur über ihre Ziele angezeigt und exportiert – eine neue
Leinwand braucht mindestens eins.

## Rückgängig und Verlauf {#undo-and-history}

Alles, was du tust, ist ein Rückgängig-Schritt: <kbd>⌘</kbd><kbd>Z</kbd> macht
rückgängig, <kbd>⇧</kbd><kbd>⌘</kbd><kbd>Z</kbd> oder
<kbd>⌘</kbd><kbd>Y</kbd> wiederholt. Der Tooltip des Rückgängig-Buttons nennt
den Schritt. Änderungen werden sofort gespeichert – Speichern gibt es nicht.

Der Verlauf enthält auch die Änderungen eines
[KI-Assistenten](../ai-mcp/), mit dessen Namen beschriftet („Claude Desktop:
Move title“). Änderungen von außen (Texteditor, `git checkout`) laden das
Projekt neu.

## Prüfung {#validation}

{{< shot src="images/validation.jpg" float="right" width="380" alt="Die Prüfliste für das iPhone-Ziel: deutsche und französische Texte, die auf Englisch zurückfallen, und ein französischer Titel, der überläuft." caption="Die Prüfliste mit eingeschaltetem **Vorschau-Modus**: Fallbacks sind Warnungen, der überlaufende französische Titel ist ein Fehler." >}}

Das Abzeichen in der oberen Leiste prüft laufend das aktuelle Ziel und die
aktuelle Sprache: **OK** oder die Zahl der Fehler und Warnungen. Ein Klick
zeigt die Liste – filtern nach Stufe oder Code, zwischen Sprachen wechseln,
**Auf der Leinwand zeigen** springt zum Element. Der **Vorschau-Modus**
behandelt fehlende Screenshots beim Gestalten als Warnungen (der Export prüft
sie trotzdem streng). Die Prüfung schaut auch auf die gerenderten Seiten:
Textkontrast gegen das, was wirklich dahinter liegt, Text am Gerät,
verdeckte oder angeschnittene Bildschirm-Kopfzeilen, Überlappungen über
Seitengrenzen, die abgerundeten Ecken des Stores. Häufige Befunde – und wie
man einen gewollten mit `allow` markiert – erklärt
[Fehlerbehebung](../troubleshooting/).

## Projekteinstellungen und App-Einstellungen {#project-settings-and-app-settings}

| | Projekteinstellungen (☰ → *Projekteinstellungen …*) | App-Einstellungen (☰ → *App-Einstellungen …*, <kbd>⌘</kbd><kbd>,</kbd>) |
|---|---|---|
| Gehört zu | dem Projektordner – reist mit, geteilt über Git | deinem Computer – alle Projekte |
| Enthält | *Allgemein* (Name, Standard-*Frame-Stil*), *Sprachen*, *Stile*, *Schriften*, *Stores* | Sprache der Oberfläche, Vorgaben für neue Projekte, Systemschriften, *Frame-Pakete*, Store-Konten, *KI-Assistenten (MCP)*, *Updates*, *Kommandozeilen-Tool* |

## Hilfe in der App {#help-in-the-app}

{{< shot src="images/app-menu.jpg" float="right" width="380" alt="Das ☰-Menü mit Neues Projekt, Projekt öffnen, Projekt schließen, Projekteinstellungen, Leinwände & Ziele, Export, In die Stores hochladen, Screenshots importieren, Als Vorlage speichern, App-Einstellungen und dem geöffneten Untermenü Hilfe." caption="Das ☰-Menü mit dem Untermenü **Hilfe** (englische Oberfläche)." >}}

Das ☰-Menü öffnet sich über den ☰-Button, das App-Symbol oder den
Projektnamen in der oberen Leiste (mit Fokus auch per <kbd>Enter</kbd>/
<kbd>Leertaste</kbd>; die Pfeiltasten gehen durch die Einträge).

- **Tastatur & Maus …** (☰ → Hilfe, der **?**-Button in der unteren Leiste
  oder <kbd>?</kbd>) – alle Tastenkürzel und Gesten. Auch auf
  [Tastatur & Maus](../shortcuts/).
- **So ist Vitrine Studio aufgebaut …** – die Übersicht auf einer Seite.
- **Apple Product Bezels herunterladen** (oben im Hilfe-Menü) öffnet Apples
  Design-Resources-Seite – siehe [Geräte & Frames](../devices-frames/).
