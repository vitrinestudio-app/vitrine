---
title: Fehlerbehebung & FAQ
description: "Häufige Prüfbefunde und wie du sie behebst, häufige Fragen und aktuelle Grenzen."
weight: 160
icon: help
---

## Prüfbefunde {#validation-findings}

Das Prüf-Abzeichen (App) und `vitrine validate` (CLI) melden Befunde pro Ziel
und Sprache als **Fehler**, **Warnung** oder **Hinweis**. Bei Fehlern endet
`vitrine validate` mit Exit-Code 1; mit `--strict` auch bei Warnungen. Die
häufigsten:

| Code | Stufe | Bedeutung und Lösung |
|---|---|---|
| `screenshot_missing` | Fehler | kein Screenshot mit diesem Schlüssel in der Sprache oder ihren Fallbacks – ein Platzhalter wird gezeigt. Datei hinzufügen, oder beim Gestalten *Vorschau-Modus* / `--preview` nutzen. |
| `screenshot_fallback` | Hinweis | der Screenshot kommt aus einer Fallback-Sprache – in Ordnung. |
| `screenshot_aspect` | Warnung | das Seitenverhältnis der Aufnahme weicht vom Display des Geräts ab – falsches Gerät oder falscher Ordner der Screenshot-Variante? |
| `screenshot_upscale` | Warnung | die Aufnahme wird mehr als 15 % vergrößert und wirkt evtl. unscharf – nimm eine Aufnahme von einem größeren Gerät. |
| `text_overflow` | Fehler | der Text passt selbst in der Mindestgröße nicht – siehe [Lange Texte einpassen](../languages/#fitting-long-texts). |
| `emergency_break` | Warnung | ein Wort wurde ohne Bindestrich umbrochen – füge einen weichen Trennstrich `\-` ein. |
| `hyphenated` | Hinweis | der Text wurde automatisch getrennt – prüf das Ergebnis. |
| `text_fallback` | Warnung / Hinweis | Text aus der Standardsprache („nicht übersetzt“, Warnung) oder aus einem anderen Fallback (Hinweis). |
| `text_missing` | Fehler | den Schlüssel gibt es in keiner Sprache – es wird nichts gezeichnet. |
| `text_in_gap`, `text_clipped` | Warnung | Text ragt in eine Store-Lücke oder über seine Seite hinaus und wird abgeschnitten. |
| `text_outside_safe_area` | Warnung | Text außerhalb des sicheren Bereichs des Ziels. |
| `low_contrast` | Warnung | der Kontrast des Texts gegen das, was tatsächlich dahinter gezeichnet ist (Farbe, Verlauf, Foto, Muster), liegt unter WCAG AA – 4,5:1, große Schrift (ab 80 lu, fett ab 62 lu) 3:1. Die Meldung nennt Verhältnis und Stelle; Farbe ändern, Scrim, Schatten oder Kontur ergänzen. |
| `text_near_device` | Warnung / Hinweis | Glyphen liegen hinter einem Geräterahmen oder laufen über ihn (Warnung) oder kommen ihm näher als 16 lu (Hinweis). Text ganz auf dem Gerät oder auf einer Karte darüber ist in Ordnung. |
| `screen_header_covered`, `screen_header_cut` | Hinweis | ein Element verdeckt die obere Leiste (Statusleiste, App-Titel) eines Gerätebildschirms, oder der Seitenrand schneidet sie, sodass eine halbe Leiste stehen bleibt. |
| `seam_overlap` | Hinweis | ein Element, das über eine Seitengrenze reicht, liegt über einem Element der Nachbarseite – Galerie prüfen. |
| `seam_split` | Hinweis | eine Form als zwei Hälften an einer Seitengrenze gebaut: die Store-Lücke zieht sie auseinander. Nimm **ein** Element mit *Beschnitt: Keiner* über die Grenze – es läuft in jedem Ziel hinter der Lücke durch. |
| `element_in_corner` | Hinweis | ein Element ragt in eine Bildecke, die der Store abrundet. |
| `device_small` | Hinweis | das größte Gerät einer Seite füllt weniger als die Hälfte. |
| `page_not_covered` | Hinweis | ein Bild oder eine Fläche deckt fast die ganze Seite, lässt aber einen schmalen Streifen frei (typisch, wenn eine Variante die Seitenhöhe ändert). |
| `page_blends_with_store` | Hinweis | die Ränder der Bilder heben sich kaum vom hellen oder dunklen Store-Hintergrund ab. |
| `missing_glyphs` | Warnung | Zeichen, die keine Schrift darstellen kann – füge eine [Fallback-Schrift](../styles-fonts/#other-scripts-fallback-fonts) hinzu. |
| `variant_suggested` | Hinweis | das Seitenverhältnis des Ziels weicht stark von der Leinwand ab – überleg dir eine Variante. |
| `store_size`, `store_count`, `store_format` | Fehler | Größe, Bildanzahl oder Format passt nicht zum Store-Slot (zu wenige Bilder sind eine Warnung). |
| `imported_frame_missing` | Hinweis | Apple-Bezels oder ein Frame-Paket sind hier nicht installiert – der eingebaute Rahmen wird gezeichnet. |
| `imported_frame_not_stored` | Hinweis | ein Ziel zeichnet Apple-Bezels, die nur dieser Rechner hat – CI würde den eingebauten Rahmen zeichnen; [im Projekt speichern](../devices-frames/#apple-bezels-in-ci). |
| `stored_frames_stale` | Hinweis | im Projekt gespeicherte Bezels werden nicht mehr verwendet, oder dieser Rechner hat einen neueren Import – gespeicherte Frames aktualisieren. |
| `imported_frames_stored` | Hinweis | das Projekt speichert Apple-Bezels – halte das Repository privat. |
| `color_profile_unsupported` | Hinweis | das Farbprofil eines Bildes ist nicht nutzbar (CMYK, HDR) – es wird als sRGB gezeichnet. |

In der App springt **Auf der Leinwand zeigen** zum Element.

Die Gestaltungsprüfungen (`low_contrast` bis `page_blends_with_store`)
rendern die Bilder; `vitrine validate --no-pixel-checks` lässt sie weg. Ist
ein Befund gewollt – riesige angeschnittene Schrift, eine Beschriftung, die
auf ein Gerät zeigt –, trag den Code in `allow` des Elements ein
(`allow: [text_clipped, low_contrast]`, auch an einer Gruppe). Schwellwerte
und abgeschaltete Codes für das ganze Projekt stehen in `project.yaml`:

```yaml
checks:
  device_distance: 16      # lu zwischen Glyphen und Geräterahmen
  min_contrast: 4.5
  min_contrast_large: 3
  off: [page_blends_with_store]
```

## Fragen {#questions}

**Wo ist der Speichern-Knopf?**
Es gibt keinen – jede Änderung wird sofort gespeichert, als ein
Rückgängig-Schritt.

**Kann ich das Projekt in einem Texteditor oder mit Git bearbeiten, während die App offen ist?**
Ja. Änderungen auf der Platte werden live übernommen. Hast du im selben
Moment in der App dasselbe geändert, lädt die App die Datei neu und sagt dir,
dass deine letzte Änderung nicht gespeichert wurde.

**Meine deutsche Überschrift ist winzig.**
Das automatische Einpassen hat sie verkleinert. Gib Deutsch einen
[Sprach-Override](../languages/#language-overrides) mit breiterer Box oder
höherer Mindestgröße, füge einen weichen Trennstrich ein oder schalte die
Silbentrennung ein.

**Das Phone sitzt bei Google Play zu hoch.**
Die 1:2-Seite von Google Play ist kürzer als die iPhone-Seite. Stell den
*Anker* des Phones auf *Unten* oder verschieb es in einer Variante für das
Play-Ziel.

**Wie bekomme ich echte Apple-Geräterahmen?**
Lade die Product Bezels bei Apple herunter und importiere sie einmal – siehe
[Geräte & Rahmen](../devices-frames/#apple-product-bezels). Die `.dmg` direkt
zu importieren scheitert: Öffne sie im Finder, akzeptiere die Lizenz und
importiere das eingehängte Volume.

**Ein Kollege sieht andere Rahmen.**
Apple-Bezels und app-weite Frame-Pakete werden pro Computer gespeichert. Ohne
sie wird der eingebaute Rahmen gezeichnet. Frame-Pakete, die du weitergeben
darfst, gehören in den Ordner `frames/` des Projekts; Apples Bezels muss jede
Person selbst importieren.

**Im Exportordner liegen alte Dateien.**
Ein erneuter Export entfernt Bilder, die Vitrine Studio früher geschrieben hat
und nicht mehr erzeugt – aber nie andere Dateien. Fremde Dateien löschst du
selbst.

**Der Upload meldet „keine bearbeitbare App-Store-Version“.**
Lege in App Store Connect eine neue Version an und plane dann erneut. Weitere
Meldungen unter [Store-Upload](../store-upload/#troubleshooting).

**Kann ich Vitrine Studio ohne die App nutzen, z. B. in CI?**
Ja – die [CLI](../cli/) prüft, exportiert, importiert und lädt hoch; nimm auf
Build-Rechnern das eigenständige CLI-Archiv und `VITRINE_APP_DIR` für einen
separaten Einstellungsordner.

**Schickt Vitrine Studio Daten irgendwohin?**
Nur wenn du es darum bittest: Store-Uploads und Verbindungstests gehen an
Apple oder Google, die Galerie-Befehle an `api.vitrinestudio.app`, und die
tägliche Update-Prüfung fragt GitHub, ob es eine neue Version gibt (abschalten
unter *App-Einstellungen → Updates*). Keine Statistik, kein Konto.

**Kann ein KI-Assistent mein Projekt kaputt machen?**
Jede Änderung ist ein Rückgängig-Schritt mit dem Namen des Assistenten, und du
siehst sie live. Store-Uploads brauchen deinen Klick. Siehe
[KI-Assistenten](../ai-mcp/).

## Aktuelle Grenzen {#current-limits}

- Rechts-nach-links-Layouts (Arabisch, Hebräisch) werden noch nicht
  gespiegelt.
- Text in SVG-Bildern wird noch nicht gezeichnet.
- Exporte gibt es nur in sRGB (keine Display-P3-Ausgabe).
- Die Galerie-Lücken der nativen Store-Apps sind Schätzungen; die
  Store-Websites wurden gemessen.
- Keine schwedischen Trennmuster.
- Frame-Pakete in `frames/` werden bei geöffnetem Projekt nicht neu eingelesen
  – öffne das Projekt nach Änderungen neu.
- Die Vorlagen-Galerie lässt sich noch nicht in der App durchsuchen.

## Hilfe bekommen {#getting-help}

Melde Fehler und Ideen auf
[GitHub](https://github.com/vitrinestudio-app/vitrine/issues). Hänge bei
Projektfragen die Ausgabe von `vitrine validate --format json` an – und nie
Store-Schlüssel.
