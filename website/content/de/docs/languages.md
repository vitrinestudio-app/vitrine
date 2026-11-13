---
title: Sprachen & Übersetzung
description: "Sprachen hinzufügen, Fallbacks verstehen, in der Tabelle oder mit Übersetzern (CSV, XLIFF) übersetzen, lange Texte einpassen, Markup und Sprach-Overrides nutzen."
weight: 80
icon: languages
---

## Sprachen hinzufügen {#adding-languages}

**Projekteinstellungen → Sprachen** (oder **Sprache hinzufügen …** am Ende der
Sprachauswahl): Tippe einen Code wie `fr`, `pt-BR` oder `fr-CA` ein und klicke
**Hinzufügen**. Codes sind BCP-47-Sprach-Tags; regionale Codes sind in
Ordnung.

- **Als Standard** macht eine Sprache zur Standardsprache – die Quelle für
  Übersetzer und der letzte Fallback jeder anderen Sprache. Wechselst du sie,
  zeigt der Dialog, welche Texte und Screenshots ihre Quelle verlieren würden.
- Die Reihenfolge der Liste ist die Reihenfolge in Auswahllisten und Exporten
  (**Nach oben**, **Nach unten**).
- **Sprache entfernen** löscht die Sprache mit ihren Texten (nach einer
  Rückfrage).

## Fallbacks {#fallbacks}

Alles, was eine Sprache nicht hat, kommt aus einer anderen – eine
unvollständige Übersetzung wird also trotzdem gerendert:

1. Eine **regionale Sprache** fällt automatisch auf ihre Grundsprache zurück:
   `fr-CA → fr`, `de-AT → de`, `pt-BR → pt`. Übersetze nur die Schlüssel, die
   abweichen – das `fr-CA` des Beispielprojekts hat gerade mal zwei
   Québec-Texte.
2. Optional der eigene **Fallback** einer Sprache (z. B. `nl → de`), den du in
   der Sprachliste einstellst.
3. Zum Schluss die **Standardsprache**.

Die Prüfung sagt dir, woher Texte kommen: Ein Text aus der **Standardsprache**
ist eine Warnung („nicht übersetzt“, und `vitrine validate --strict` schlägt
fehl), einer aus einem regionalen oder eingestellten Fallback nur ein Hinweis.
Gibt es einen Text in keiner Sprache der Kette, ist das ein Fehler
(`text_missing`).
Screenshots aus irgendeinem Fallback sind ein Hinweis – ein unvollständiger
Screenshot-Set pro Sprache ist in Ordnung.

**Assets → Screenshots** zeigt pro Sprache, welche Bilder geliehen sind:
*Aus en (Fallback)*, *Aus de (Standardsprache)*; fahre mit der Maus über eine
Sprache, um ihre ganze Fallback-Kette zu sehen.

## In der App übersetzen {#translating-in-the-app}

- **Auf der Leinwand:** Stell die **Sprache** um und doppelklicke einen Text.
  Fehlt ein Text, wird der Fallback mit dem Hinweis *Noch kein Text in dieser
  Sprache* gezeigt; was du tippst, wird zur Übersetzung.
- **Tab Übersetzungen:** eine Zeile pro Schlüssel, eine Spalte pro Sprache.
  Klicke eine Zelle an (oder <kbd>Enter</kbd>), um sie zu bearbeiten;
  <kbd>Enter</kbd> übernimmt, <kbd>Umschalt</kbd>+<kbd>Enter</kbd> bricht die
  Zeile um, <kbd>Esc</kbd> bricht ab. **Schlüssel und Texte suchen**, **Nur
  fehlende / Probleme**, **Übersetzungsschlüssel hinzufügen**, und pro
  Schlüssel **Schlüssel umbenennen**, **Schlüssel löschen** und *Verwendet von*
  (welche Elemente ihn zeigen). Überlaufende Texte sind in ihrer Zelle
  markiert.
- Ansicht **Alle Sprachen:** alle Sprachen nebeneinander, um zu lange Texte zu
  entdecken.

{{< shot src="images/translations.jpg" alt="Der Tab Übersetzungen: Schlüssel × Sprachen en, de und fr; fehlende Texte zeigen ihren Fallback grau und kursiv, ein überlaufender französischer Titel ist markiert." caption="Der Tab **Übersetzungen**: Deutsch fällt bei vier Schlüsseln auf Englisch zurück, Französisch bei den meisten; der lange französische Titel läuft über." >}}

Ein Textelement zeigt den Text seines **Schlüssels** (z. B. `plan.title`).
Mehrere Elemente können sich einen Schlüssel teilen. Änderst du den Schlüssel
im Inspektor auf einen neuen Namen, wird der aktuelle Text dorthin übernommen.

{{< shot src="images/all-languages.jpg" alt="Die Ansicht Alle Sprachen: die sechs iPhone-Seiten auf Englisch, Deutsch und Französisch, je eine Zeile; der überlaufende französische Titel auf Seite 2 ist rot umrandet." caption="Die Ansicht **Alle Sprachen**: eine Zeile pro Sprache, Probleme pro Zeile markiert." >}}

## Mit Übersetzern arbeiten: CSV und XLIFF {#working-with-translators-csv-and-xliff}

Im Tab **Übersetzungen** schreibt **Exportieren…** Dateien für Übersetzer,
**Importieren…** liest die übersetzten Dateien wieder ein.

Optionen beim **Export**: *Format* (*CSV*, *XLIFF 2.0*, *XLIFF 1.2*), die
Ziel-*Sprachen* (die Standardsprache ist die Quelle), *Nur fehlende
Übersetzungen*, *Kontext für Übersetzer* (welche Elemente und Seiten einen
Text nutzen, die Boxbreite und eine ungefähre Maximallänge), bei CSV das
Trennzeichen (*Semikolon ( ; ) – Excel* für Gebietsschemas mit Dezimalkomma)
und *UTF-8-Kennung für Excel*; bei XLIFF die *Formatierung* als geschützte
Tags (Vorgabe, sicher in CAT-Tools) oder als einfacher Text. XLIFF schreibt
eine Datei pro Sprache.

Der **Import** liest CSV (Excel-Varianten werden erkannt) und XLIFF und zeigt
eine Vorschau, bevor sich etwas ändert: neue, geänderte und entfernte
Übersetzungen (*Vorher* / *Nachher*), unbekannte Schlüssel (übersprungen, außer
mit *Unbekannte Schlüssel anlegen*), Formatierungsprobleme und das **Layout
nach dem Import** – welche Texte überlaufen würden. **Übernehmen** macht daraus
einen Rückgängig-Schritt. *Leere Zellen*: *nichts ändern* oder *Übersetzung
entfernen*.

Auf der Kommandozeile:

```sh
vitrine strings export --format xliff20 -o translations/      # eine Datei pro Sprache
vitrine strings export --format csv --lang fr --missing-only --context -o fr.csv
vitrine strings import translations/*.xlf --dry-run           # nur Bericht
vitrine strings import translations/*.xlf                     # übernehmen
```

Eine dünn besetzte regionale Sprache (`fr-CA`) bekommt mit `--missing-only`
alle Schlüssel, die ihr fehlen. Übersetzen mit Claude geht auch – siehe
[KI-Assistenten](../ai-mcp/).

## Lange Texte einpassen {#fitting-long-texts}

Jeder Text hat eine Box – Ränder zu den Seitenkanten oder eine freie Box.
**Auto-Fit** verkleinert die Schrift von ihrer Größe bis **Verkleinern bis**
(lu) und bis zu **Höchstens Zeilen**, bis der Text passt. Der Inspektor zeigt
das Ergebnis unter *Gesetzt* (Schriftgröße, Zeilen, *Passt*).

Passt ein Text selbst in der Mindestgröße nicht, ist das ein **Überlauf**: ein
Fehler (`text_overflow`) nur für diese Sprache und dieses Ziel – die anderen
Sprachen werden trotzdem exportiert. Abhilfe:

- die Übersetzung kürzen;
- eine **weiche Trennstelle** `\-` an einer guten Stelle einfügen (sie
  erscheint nur als „-“, wenn die Zeile dort umbricht): `Gemüse\-beet\-planung`;
- **Silbentrennung → Automatisch** einschalten (oder im Textstil) – sie greift
  nur, wenn der Text in der Mindestgröße nicht passt;
- der Sprache mit einem **Sprach-Override** (unten) eine breitere Box oder eine
  andere Mindestgröße geben;
- *Verkleinern bis* senken oder *Höchstens Zeilen* erhöhen.

Ein Wort, das ohne Trennstrich umbrochen wurde, wird als Warnung gemeldet
(`emergency_break`); automatisch getrennte Texte erscheinen als Hinweis
(`hyphenated`), damit du sie prüfen kannst.

**Mitgelieferte Trennmuster:** Englisch (US, GB), Deutsch, Französisch,
Spanisch, Niederländisch, Italienisch, Dänisch, Portugiesisch, Norwegisch
(Bokmål), Finnisch. Schwedisch fehlt (die Lizenz der Muster ist nicht
kompatibel).

## Sprach-Overrides {#language-overrides}

Ein **Sprach-Override** ändert ein Textelement nur für eine Sprache – die lange
deutsche Überschrift bekommt eine breitere Box, Französisch eine kleinere
Schrift. Wähle den Text aus, stell die Sprachauswahl auf die Sprache und hake
im Inspektor **Nur für de** an: Änderungen an diesem Text landen dann im
Override. Überschriebene Felder sind **türkis** markiert; *Sprach-Override
zurücksetzen* entfernt ein Feld, *Alle zurücksetzen* den ganzen Override.

Erlaubt im Override: Position, Größe, Anker, Stil, Box, Ausrichtung, Auto-Fit,
Farbe, Schriftgröße, Kontur, Leuchten, Schatten und Silbentrennung. Geometrie
wie Drehung und Heimatseite bleibt gemeinsam. Overrides für `de` gelten auch
für `de-AT`.

```yaml
- id: title-companions
  kind: text
  text: companions.title
  style: headline
  box: {top: 170, left: 70, right: 70}
  fit: {shrink_to: 64, max_lines: 2}
  langs:
    de: {box: {top: 170, left: 40, right: 40}, fit: {shrink_to: 76, max_lines: 2}}
```

Tipp aus dem Beispiel: Wenn ein langes deutsches Kompositum die Überschrift zu
stark schrumpfen lässt, erhöhe `shrink_to` für Deutsch, damit die
Silbentrennung früher greift („Mischkultur-partner“), und verbreitere die Box,
wenn die Trennung ungünstig fällt.

## Text-Markup {#text-markup}

Übersetzungen dürfen Markup enthalten; behalte es beim Übersetzen bei:

| Schreibweise | Ergebnis |
|---|---|
| `**fett**` | fett |
| `_kursiv_` | kursiv (`_` innerhalb eines Wortes bleibt stehen) |
| `[accent]…[/]` | ein Span-Stil aus dem Projekt (Farbe, Gewicht …) |
| `\n` oder ein echter Zeilenumbruch | Zeilenumbruch |
| `\-` | weiche Trennstelle |
| `\*` `\_` `\[` `\]` `\\` | das Zeichen selbst |

Unausgeglichene Markierungen werden wörtlich gezeichnet und gemeldet
(`invalid_markup`).

**Französische Typografie:** Setze ein geschütztes Leerzeichen (U+00A0) vor
`:` `;` `?` `!` und innerhalb von « Guillemets » sowie zwischen Wörtern, die
auf einer Zeile bleiben müssen. Der Zeilenumbruch berücksichtigt es.

**YAML von Hand bearbeiten:** Eine Übersetzung, die `: ` (Doppelpunkt +
Leerzeichen) enthält oder mit `"`, `[` oder `*` beginnt, muss in
Anführungszeichen stehen. Die App und der Import schreiben korrektes YAML für
dich.
