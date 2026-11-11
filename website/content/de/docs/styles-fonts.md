---
title: Stile & Schriften
description: "Projektfarben, Textstile, Span-Stile und Schatten; die elf mitgelieferten Schriftfamilien, eigene Schriften, Fallbacks für andere Schriftsysteme, Konturen und Leuchten."
weight: 70
icon: fonts
---

Stile halten einen Satz einheitlich: Ändere den Überschrift-Stil einmal, und
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
automatisch trennen* gilt. Textelemente wählen im Inspektor einen Stil
(*Stil*) und können einzelne Werte (Farbe, Schriftgröße, Effekte) für sich
überschreiben.

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

## Schatten {#shadows}

**Schatten-Presets** sind eingebaut (`soft`, `hard`, `none`) oder deine
eigenen: eine oder mehrere Ebenen mit Farbe, Unschärfe und Versatz. Geräte,
Bilder, Formen und Texte verwenden Presets über ihren Namen. Schatten
transparenter Bilder folgen deren Form (wie CSS `drop-shadow`).

## Kontur, Leuchten, Schatten bei Texten {#outline-glow-shadow-on-texts}

Texte können eine **Kontur** (Breite in lu, *Außen* oder *Mittig*), ein
**Leuchten** (Radius, Deckkraft, Ausdehnung) und einen **Schatten** haben.
Setze sie am Textstil, an einem Span-Stil oder am Element – das Genaueste
gewinnt (Span > Element > Stil). Um einen geerbten Effekt abzuschalten, setze
die Konturbreite auf 0, die Leuchtdeckkraft auf 0 oder den Schatten auf
*none*. Effektgrößen sind in lu und schrumpfen beim Auto-Fit nicht mit.

## Mitgelieferte Schriften {#bundled-fonts}

Elf Familien sind eingebaut – immer verfügbar in der App, in der CLI und für
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

`vitrine fonts` listet die Familien eines Projekts.

## Eigene Schriften {#your-own-fonts}

Lege `.ttf`- oder `.otf`-Dateien in den Ordner **`fonts/`** des Projekts (oder
nutze **Assets → Importieren …** mit *Schrift (fonts/)*). Sie werden
automatisch erkannt und erscheinen mit ihrem Familiennamen in den
Schriftauswahlen. Weil sie mit dem Projekt reisen, sieht der Export auf jedem
Rechner gleich aus – Vitrine Studio nutzt nie Schriften deines Systems, außer
du erlaubst es.

## Andere Schriftsysteme: Fallback-Schriften {#other-scripts-fallback-fonts}

Zeichen, die eine Familie nicht hat (Japanisch, Arabisch, Hebräisch, Thai,
Kyrillisch in Bebas Neue …), kommen aus ihren **Fallbacks**, dann aus Inter.
Es ist keine Schrift für CJK, Arabisch, Hebräisch oder Thai mitgeliefert, also
für diese Sprachen:

1. Lege eine Schrift, die sie abdeckt, in `fonts/`, z. B. *Noto Sans JP*.
2. **Projekteinstellungen → Schriften** → die Familie, die du nutzt →
   **Fallback-Familie hinzufügen** → Noto Sans JP.

Die Prüfung meldet Zeichen, die keine Schrift zeichnen kann
(`missing_glyphs`).

**Systemschriften** als letzter Ausweg: **App-Einstellungen →
Systemschriften → Systemschriften als letzten Ausweg verwenden**. Dann füllen
auf deinem Rechner installierte Schriften die Lücken – Vorschau und Export
hängen dann aber von diesem Rechner ab. *Projekteinstellungen → Schriften →
Systemschriften* zeigt, ob welche nötig waren.

{{< callout note >}}
Rechts-nach-links-Layouts (Arabisch, Hebräisch) werden noch nicht gespiegelt.
{{< /callout >}}
