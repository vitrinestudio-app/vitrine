---
title: Stile & Schriften
description: "Projektfarben, Textstile, Span-Stile und Schatten; mitgelieferte, Projekt- und installierte Schriften, Schriften im Projekt speichern, unterstützte Schriftsysteme (Arabisch, Hebräisch, indische Schriften, Thai, CJK), Konturen und Leuchten."
weight: 70
icon: fonts
---

Stile halten deine Bilder einheitlich: Ändere den Überschrift-Stil einmal, und
jede Überschrift auf jeder Leinwand, in jeder Sprache folgt. Sie stehen unter
**Projekteinstellungen → Stile**.

## Farben {#colours}

Benenne deine Farben einmal (`accent`, `ink`, `paper` …) und nutze sie überall
als `@accent` – in Textstilen, Füllungen, Konturen. Änderst du die Farbe,
ändert sich jede Verwendung. Einfache Farben sind `#RRGGBB` oder `#RRGGBBAA`
(mit Transparenz).

## Textstile {#text-styles}

Ein Textstil legt *Schrift*, *Gewicht*, *Kursiv*, *Größe* (in lu),
*Zeilenhöhe*, *Laufweite*, *Farbe*, *Hinterlegung* (ein Hintergrund hinter dem
Text), *Schreibweise* (*wie geschrieben*, *GROSSBUCHSTABEN*,
*kleinbuchstaben*), *Schatten*, Kontur und Leuchten fest, und ob *Bei Bedarf
automatisch trennen* gilt. Im Inspektor wählst du für ein Textelement einen
Stil (*Stil*) und kannst dort einzelne Werte (Farbe, Schriftgröße, Effekte)
nur für dieses Element überschreiben.

Größen sind in lu: Auf einer 1000 lu breiten Seite ist eine 90-lu-Überschrift
9 % der Seitenbreite – auf jeder Gerätegröße.

## Span-Stile: ein paar Wörter einfärben {#span-styles-colour-a-few-words}

Span-Stile ändern einen Teil eines Textes. Lege einen an (z. B. `accent` mit
der Farbe `@accent`) und markiere dann Wörter in einer Übersetzung:

```yaml
companions.title: Know your [accent]plant partners[/]
```

Im Text-Editor auf der Leinwand markierst du die Wörter und nutzt **Span-Stil
…**. Spans können Farbe, Gewicht, Kursiv, Schrift, Hinterlegung und Effekte
setzen. `**fett**` und `_kursiv_` funktionieren ohne Span-Stil. Siehe die
[Markup-Referenz](../languages/#text-markup).

Aus einer Hinterlegung wird eine Pille oder ein Textmarker: **padding**
(ein Wert oder `{x, y}`, in lu) vergrößert die Fläche um die Wörter,
**radius** rundet ihre Ecken – ab der halben Flächenhöhe entsteht eine
Pille. Der Innenabstand verschiebt den Text nie und ändert keine
Zeilenumbrüche; lass also ein Leerzeichen neben der Pille. Bricht ein Span
um, bekommt jede Zeile ihre eigene Fläche. In der App: Projekteinstellungen →
Stile → Span-Stil → *Hinterlegungsfläche*.

```yaml
styles:
  spans:
    tag: {color: "#FFFFFF", weight: 800, background: "@accent",
          padding: {x: 14, y: 2}, radius: 40}
```

## Schatten {#shadows}

**Schatten-Presets** sind eingebaut (`soft`, `hard`, `none`) oder deine
eigenen: eine oder mehrere Ebenen mit Farbe, Unschärfe und Versatz. Geräte,
Bilder, Formen und Texte verwenden Presets über ihren Namen. Schatten
transparenter Bilder folgen deren Form (wie CSS `drop-shadow`).

## Kontur, Leuchten, Schatten bei Texten {#outline-glow-shadow-on-texts}

Texte können eine **Kontur** (Breite in lu, *Außen* oder *Mittig*), ein
**Leuchten** (Radius, Deckkraft, Ausdehnung) und einen **Schatten** haben.
Setze sie am Textstil, am Element oder an einem Span-Stil. Der Textstil
gibt die Grundwerte vor, ein am Element gesetzter Effekt ersetzt den des
Stils, und ein Span-Stil gilt für seine markierten Wörter vor beidem – das
Genaueste gewinnt (Span vor Element vor Textstil). Um einen geerbten Effekt abzuschalten, setze
die Konturbreite auf 0, die Leuchtdeckkraft auf 0 oder den Schatten auf
*none*. Effektgrößen sind in lu und schrumpfen beim Auto-Fit nicht mit.

## Mitgelieferte Schriften {#bundled-fonts}

Diese Familien sind eingebaut – immer verfügbar in der App, in der CLI und für
KI-Assistenten, ohne Dateien, alle unter der SIL Open Font License:

| Familie | Kategorie | Gewichte | Kursiv |
|---|---|---|---|
| Inter | serifenlos | 400, 600 | schräggestellt |
| Inter Display | serifenlos | 700, 900 | schräggestellt |
| Montserrat | serifenlos | 100–900 | ✓ |
| Nunito | serifenlos | 200–1000 | ✓ |
| Oswald | serifenlos | 200–700 | schräggestellt |
| Playfair Display | Serifenschrift | 400–900 | ✓ |
| Lora | Serifenschrift | 400–700 | ✓ |
| Bebas Neue | Display (nur Großbuchstaben) | 400 | schräggestellt |
| Caveat | Handschrift | 400–700 | schräggestellt |
| Dancing Script | Handschrift | 400–700 | schräggestellt |
| JetBrains Mono | Monospace | 100–800 | ✓ |

- Variable Schriften können jedes Gewicht in ihrem Bereich; ein Gewicht
  außerhalb sieht aus wie das Bereichsende (Lora 900 = Lora 700). Inter und
  Inter Display gibt es nur in den genannten Gewichten; andere Gewichte nutzen
  das nächstliegende.
- „Schräggestellt“ = keine echte Kursive; kursiver Text wird künstlich
  schräggestellt.
- Handschriften eignen sich am besten für kurze Hinweise.
- Alle decken Latein und Latein erweitert ab. **Kyrillisch:** Montserrat,
  Playfair Display, Lora, Nunito, Oswald, Caveat, JetBrains Mono.
  **Griechisch:** Inter und JetBrains Mono. **Vietnamesisch:** alle außer
  Bebas Neue und Caveat.

Weitere mitgelieferte Familien decken andere Schriftsysteme ab und
werden automatisch verwendet – siehe
[Unterstützte Schriftsysteme](#supported-scripts).

`vitrine fonts` listet die Familien eines Projekts (`--installed`: auch alle
auf dem Computer installierten Schriften).

## Auf deinem Computer installierte Schriften {#system-fonts}

Die Schriftauswahlen zeigen drei Gruppen und haben ein Suchfeld:

1. **Vitrine Studio** – die mitgelieferten Familien,
2. **Dieses Projekt** – die Schriften im Ordner `fonts/` des Projekts,
3. **Auf diesem Computer installiert** – alle Schriftfamilien, die auf deinem
   Mac oder PC installiert sind.

Du kannst jede davon wählen. Ein Familienname wird in dieser Reihenfolge
gesucht: **mitgeliefert → Projekt → installiert** – eine mitgelieferte Familie
gewinnt also immer gegen eine installierte gleichen Namens. Installierte
Schriften funktionieren überall auf deinem Computer: in der Vorschau, im
Export, in der CLI (`vitrine export`) und für KI-Assistenten (MCP).

Eingeschaltet werden sie mit **App-Einstellungen → Installierte Schriften
verwenden** (standardmäßig an). Ausgeschaltet gelten nur mitgelieferte und
Projektschriften – dann ist das Ergebnis auf jedem Rechner gleich. Die CLI hat
`--no-system-fonts`, und `VITRINE_SYSTEM_FONTS=off` schaltet sie für jeden
Prozess ab (z. B. auf einem Build-Server). Schrift installiert, während die
App läuft? **Projekteinstellungen → Schriften → Installierte Schriften neu
einlesen**.

{{< callout note >}}
Ein anderer Computer – ein Build-Server (CI) oder ein Kollege – hat vielleicht
nicht dieselben Schriften. Dort fallen Texte in einer fehlenden Familie auf
Inter zurück. Die Prüfung weist darauf hin (`system_font_used`, Info).
Speichere die Schriften im Projekt (nächster Abschnitt) oder nutze
mitgelieferte Schriften für Projekte, die anderswo exportiert werden.
{{< /callout >}}

{{< shot src="images/project-fonts.jpg" alt="Projekteinstellungen, Tab Schriften: Fallback-Familien für Inter und die Projektschrift Bangers, der Status der Systemschriften, das Speichern verwendeter Systemschriften und die mitgelieferten Schriften." caption="Projekteinstellungen → **Schriften**." >}}

## Verwendete Systemschriften im Projekt speichern {#store-system-fonts}

**Projekteinstellungen → Schriften → Verwendete Systemschriften im Projekt
speichern** kopiert die Schriftdateien der installierten Familien, die das
Projekt verwendet, nach `fonts/system/`. Ein Build-Server erzeugt dann genau
das, was dein Computer erzeugt. Kopiert werden nur die Dateien der Gewichte,
die deine Stile verwenden; die Liste zeigt jede Datei mit Größe und der
Lizenz, die die Schrift angibt (Hersteller, OFL oder nicht). **Gespeicherte
Schriften aktualisieren** frischt sie auf, **Aus dem Projekt entfernen**
löscht den Ordner. In der Kommandozeile:

```sh
vitrine fonts embed --dry-run    # was gespeichert würde, mit Lizenzen
vitrine fonts embed              # speichern / aktualisieren
vitrine fonts unembed            # fonts/system/ entfernen
```

{{< callout warn >}}
**Prüfe die Lizenz vor dem Speichern.** Schriften unter der SIL Open Font
License dürfen weitergegeben werden. Die Systemschriften von macOS und Windows
(San Francisco, New York, Helvetica Neue, PingFang, Hiragino, Segoe UI …) und
die meisten kommerziellen Schriften dürfen nur auf deinem Computer verwendet
und **nicht weitergegeben** werden. Halte ein Projekt mit solchen Schriften in
einem **privaten** Repository und veröffentliche es nie mit `fonts/system/` –
kein öffentliches Repository, keine Vorlage, kein Galerie-Upload, kein
weitergegebenes Archiv. Für ein öffentliches Repository trage `fonts/system/`
in `.gitignore` ein. Vorlagen enthalten diesen Ordner nie.
{{< /callout >}}

Die Prüfung erinnert an gespeicherte Schriften (`system_fonts_stored`, Info).

## Eigene Schriften {#your-own-fonts}

Lege `.ttf`- oder `.otf`-Dateien in den Ordner **`fonts/`** des Projekts (oder
nutze **Assets → Importieren …** mit *Schrift (fonts/)*). Sie werden
automatisch erkannt und erscheinen mit ihrem Familiennamen in den
Schriftauswahlen. Weil sie mit dem Projekt reisen, sieht der Export auf jedem
Rechner gleich aus.

## Unterstützte Schriftsysteme {#supported-scripts}

Das Textlayout formt jedes Schriftsystem nach den HarfBuzz-Regeln (in Rust):
Arabische Buchstaben werden verbunden, indische Konsonanten bilden Ligaturen
und Vokalzeichen rücken an ihren Platz, Rechts-nach-links-Text läuft von
rechts nach links, Thai wird an Wortgrenzen umbrochen (es hat keine
Leerzeichen), Chinesisch und Japanisch brechen zwischen Zeichen nach den
üblichen Regeln um (keine Zeile beginnt mit `。` oder `、`).

| Schriftsystem | Sprachen (Beispiele) | Schrift |
|---|---|---|
| Lateinisch, Kyrillisch, Griechisch | die meisten europäischen Sprachen | mitgelieferte Familien (siehe Tabelle oben) |
| Arabisch | Arabisch, Persisch, Urdu | **Noto Sans Arabic** (mitgeliefert, automatisch) |
| Hebräisch | Hebräisch | **Noto Sans Hebrew** (mitgeliefert, automatisch) |
| Devanagari | Hindi, Marathi, Nepali | **Noto Sans Devanagari** (mitgeliefert, automatisch) |
| Bengalisch | Bengalisch, Assamesisch | **Noto Sans Bengali** (mitgeliefert, automatisch) |
| Tamil | Tamil | **Noto Sans Tamil** (mitgeliefert, automatisch) |
| Thai | Thai | **Noto Sans Thai** (mitgeliefert, automatisch) |
| Chinesisch, Japanisch, Koreanisch | zh-Hans, zh-Hant, ja, ko | eine **installierte** Schrift oder eine Schrift in `fonts/` |

Die mitgelieferten Schriften für andere Schriftsysteme springen für Zeichen
ein, die der Schrift des Stils fehlen – deine Überschriftenschrift bleibt für
lateinischen Text und Ziffern. Du kannst sie auch direkt wählen (Kategorie
*Schrift für andere Schriftsysteme*).

Schriften für **Chinesisch, Japanisch und Koreanisch** sind je 10–20 MB groß,
deshalb ist keine mitgeliefert. Mit *Installierte Schriften verwenden*
bekommt jede Sprache eine dafür gemachte installierte Schrift – japanische
Texte Hiragino Sans oder Yu Gothic, vereinfachtes Chinesisch PingFang SC oder
Microsoft YaHei, traditionelles Chinesisch PingFang TC oder Microsoft
JhengHei, Koreanisch Apple SD Gothic Neo oder Malgun Gothic (macOS und Windows
bringen sie mit; unter Linux installiere Noto Sans CJK). Han-Zeichen sehen im
Chinesischen und Japanischen verschieden aus, deshalb entscheidet die Sprache.
Für gleiche Exporte auf jedem Rechner lege eine Schrift wie *Noto Sans JP* in
`fonts/` (OFL, frei weitergebbar) oder speichere die verwendeten
Systemschriften im Projekt.

{{< callout note >}}
Rechts-nach-links-Layouts werden nicht gespiegelt: Die Ausrichtung ist
physisch (*links* heißt in jeder Sprache links). Stelle für Arabisch und
Hebräisch die Ausrichtung mit einer
[Sprachüberschreibung](../languages/) auf *rechts*.
{{< /callout >}}

## Andere Schriftsysteme: Fallback-Schriften {#other-scripts-fallback-fonts}

Zeichen, die eine Familie nicht hat, kommen in dieser Reihenfolge aus ihren
**Fallbacks**, aus Inter, aus den mitgelieferten Schriften für andere
Schriftsysteme und – mit *Installierte Schriften verwenden* – aus einer
installierten Schrift. So wählst du den Fallback selbst (z. B. eine japanische
Schrift, die zu deiner Überschriftenschrift passt):

1. Lege die Schrift in `fonts/` (oder installiere sie), z. B. *Noto Sans JP*.
2. **Projekteinstellungen → Schriften** → die Familie, die du nutzt →
   **Fallback-Familie hinzufügen** → Noto Sans JP.

Die Prüfung meldet Zeichen, die keine Schrift zeichnen kann, als Warnung
(`missing_glyphs`), und Zeichen, die eine installierte Fallback-Schrift
zeichnet (`system_font_fallback`, Info).

## Schriften in Vorlagen {#fonts-in-templates}

Vorlagen enthalten nur Schriften unter der SIL Open Font License. Darf eine
Projektschrift nicht weitergegeben werden, bricht *Als Vorlage speichern* ab –
oder speichert die Vorlage mit **Schriften weglassen, die nicht weitergegeben
werden dürfen** (`vitrine template save --skip-unlicensed-fonts`) ohne sie.
Die Vorlage nennt die Familie dann als *benötigt, nicht enthalten*;
installierte Systemschriften werden immer so eingetragen. Ein Projekt aus der
Vorlage verwendet die Schrift, wo sie installiert ist, und zeigt sonst Inter –
die App sagt dir, welche Schriften fehlen.
