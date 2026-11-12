# Vitrine Studio – Erkenntnisse aus 50 Galerievorlagen über MCP

*Stand: 12.11.2026 · Quelle: Bau von 50 Galerievorlagen ausschließlich über den Vitrine-MCP-Server (Ordner `Vorlagen/`, Rohprotokoll in `_plan/mcp-befunde.md`)*

---

## ⚠️ Wichtig: Wer hier entscheidet

Dieses Dokument wurde von Claude (Chat-Assistent) und seinen Sub-Agenten **aus Anwendersicht** geschrieben. Wir haben Vitrine nur über die MCP-Schnittstelle benutzt und den Quellcode **nicht** gesehen.

**Die Code-Expertin ist die Coding-LLM, nicht der Autor dieses Dokuments.** Deshalb gilt:

- Die Coding-LLM hat die **Hoheit** über alle technischen Entscheidungen.
- Jede Anforderung unten ist ein **Vorschlag**. Bitte zuerst gegen den bestehenden Code prüfen:
  - Gibt es die Funktion schon (vielleicht unter anderem Namen, undokumentiert oder anders zu bedienen)?
  - Ist das beobachtete Verhalten ein Bug, Absicht oder ein Bedienfehler unsererseits?
  - Passt der Vorschlag zur Architektur, oder gibt es einen besseren Weg zum selben Ziel?
- Danach selbst entscheiden – oder **Thorsten konsultieren**, wenn es um Produktrichtung, größere Umbauten oder Zielkonflikte geht.
- Feld- und Funktionsnamen in den Vorschlägen (`allow_clip`, `text_near_element` …) sind **Platzhalter**, keine Vorgaben.

---

## 1. Kontext

- 50 Vorlagen, je 5–6 Seiten, 2–3 Canvases (Phone, Tablet hoch, Tablet quer), 4–6 Targets (App Store + Google Play), Sprachen EN + DE.
- Gebaut nacheinander von Sub-Agenten (immer nur ein offenes Projekt), jeweils mit `project_create` → großen `apply_ops`-Batches (bis ~120 Ops) → `render_preview` / `layout_report` / `validate` → `export`.
- Typische Inhalte: gekippte Geräte, mehrere Geräte pro Seite, Gerätemix, Motive über Seitennähte, Panoramen, UI-Karten, die aus dem Screen „herausbrechen“, rahmenlose Phones (`model: none`) im iPad-Set.
- Play-Targets bekamen fast immer eigene Varianten (`device_map` + Overrides), weil Seitenhöhen und Geräteproportionen anders sind.

## 2. Was gut funktioniert hat

Das soll bleiben – bitte bei Änderungen nicht verschlechtern:

- **Stabilität:** `project_create`, `apply_ops` (auch große Batches), `validate` und `render_preview` haben fast immer sofort und fehlerfrei geantwortet.
- **`apply_ops` als Batch** ist das Herzstück – ein Generator-Skript erzeugt ein Projekt in ~8–15 Schreibaufrufen.
- **Kontinuierliches Canvas** (Seiten à 1000 lu, Elemente über Nähte) ist das stärkste Gestaltungsmerkmal von Vitrine.
- **Varianten + `device_map`** sind mächtig genug, um App Store und Play aus einem Projekt zu bedienen.
- **Auto-Fit für Texte:** Deutsche Headlines liefen fast immer ohne Sprach-Overrides.
- **Spans** (`[name]…[/]`) für Akzentwörter/Schriftwechsel.
- **`visible_in.langs` / `visible_in.targets`** als Notlösung für sprach- oder storeabhängige Elemente.
- SVG mit `pattern` + `mask` wird korrekt gerendert.
- `shadow` als Element-Override auf Text funktioniert (war aber nicht dokumentiert, siehe Abschnitt 5).

---

## 3. Anforderungen nach Priorität

Jede Anforderung hat: **Problem → Beobachtung → heutiger Workaround → Vorschlag → Woran man sieht, dass es gelöst ist.**

### P1 – Qualitätsprüfung: Fehler, die heute nur im Bild auffallen

Der mit Abstand häufigste Befund. In fast jeder Vorlage gab es Fehler, die weder `layout_report` noch `validate` gemeldet haben. Die Agenten mussten jede Galerie-Vorschau Seite für Seite ansehen. Für Endnutzer heißt das: Vitrine lässt kaputte Layouts ohne Hinweis durch.

#### P1.1 Text zu nah an / auf einem Gerät
- **Beobachtung:** Headlines (besonders deutsche, längere) lagen wenige lu neben dem Geräterahmen oder liefen in ihn hinein, vor allem bei gekippten Geräten und bei Play-Varianten (andere Geräteproportionen). Ursache: Es wird offenbar nur Box-gegen-Box geprüft; Text-Boxen sind oft breiter als die Glyphen, gekippte Geräte haben schräge Kanten.
- **Workaround:** Im exportierten Bild prüfen, Gerät oder Textspalte per Override verschieben.
- **Vorschlag:** Info/Warnung `text_near_element` – Abstand zwischen **tatsächlichen Glyphen** (nicht Text-Box) und der **gedrehten Rahmenkontur** eines Geräts unter Schwellwert X lu (konfigurierbar, Default z. B. 24 lu). Pro Target und Sprache.
- **Gelöst, wenn:** Ein deutscher Headline-Text, der ein gekipptes Gerät berührt, in `layout_report` mit Target + Sprache + Abstand auftaucht.

#### P1.2 Bildschirm-Box eines Geräts in Seitenkoordinaten
- **Beobachtung:** Häufige Fehler, die man nur prüfen kann, wenn man weiß, wo der *Screen* (nicht der Rahmen) liegt:
  - UI-Karten, die über dem Screen schweben, verdecken den App-Kopf oder halbe Listenzeilen.
  - Oben angeschnittene Geräte zeigen noch eine halbe Statusleiste („9:41“) oder schneiden den App-Titel an.
  - Requisiten (Lampen, Pfeile) liegen hinter/auf dem Gerät.
- **Workaround:** Agenten haben Rahmenverhältnisse selbst ausgemessen und Screen-Positionen per Skript nachgerechnet – fehleranfällig, besonders bei `device_map` auf Android.
- **Vorschlag:**
  1. `layout_report` (oder eigener Aufruf) liefert je Gerät: Rahmen-Polygon und Screen-Polygon in Seitenkoordinaten, inkl. Drehung, pro Target (also nach `device_map`).
  2. Darauf aufbauend Infos wie `element_over_screen` (mit Anteil), `screen_top_clipped` (Statusleiste/Titelbereich angeschnitten).
- **Gelöst, wenn:** Ein Generator die Screen-Box abfragen kann und Vitrine eine Karte meldet, die die oberen x % des Screens verdeckt.

#### P1.3 Kontrast Text gegen Hintergrund (auch Bild/SVG)
- **Beobachtung:** Texte auf Verläufen, Fotos, Panoramen, Mustern hatten oft zu wenig Kontrast (bis 2,4 : 1), besonders an Seitenrändern, bei feinen Mustern und auf Play-Varianten mit anderem Bildausschnitt. Wurde nie gemeldet.
- **Workaround:** Eigene Messung: Export-PNG, Ring um Glyphen bzw. Hintergrund ohne Text, Perzentil der Luminanz.
- **Vorschlag:** `validate`-Warnung `low_contrast` – gerenderten Hintergrund unter jeder Textzeile messen (z. B. 5./95. Perzentil), gegen WCAG 4,5 : 1 (große Schrift 3 : 1), pro Target und Sprache.
- **Gelöst, wenn:** Weißer Text über einem hellen Fleck im Panorama gemeldet wird, inkl. gemessenem Verhältnis.

#### P1.4 Überlappungen über Seitengrenzen hinweg
- **Beobachtung:** Ein Element, das von Seite n in Seite n+1 ragt (z. B. 3D-Karte über die Naht), wurde nicht mit Elementen der Nachbarseite abgeglichen. Kollisionen fielen nur in der Galerie auf.
- **Vorschlag:** `overlaps` auch für Elemente mit fremder Heimatseite, die in eine Seite hineinragen.

#### P1.5 Overlap-Prüfung mit echter Pixel-Deckung statt Box
- **Beobachtung:** Bilder mit großen transparenten Flächen (freigestellte Objekte, ganzseitige Linien-SVGs) erzeugen entweder Rauschen (Text über Panorama = „Overlap 1,0“) oder verstecken echte Kollisionen.
- **Vorschlag:** Für Bilder optional Alpha-Maske statt Bounding-Box verwenden; ganzseitige Hintergrundbilder als solche erkennen und aus `overlaps` herausnehmen.

#### P1.6 Weitere sinnvolle Infos
- `device_small`: Gerät belegt < ~60 % der Seitenhöhe (Agenten haben mehrmals zu kleine Geräte gebaut).
- `element_narrower_than_page`: ein Hintergrund deckt die Seite nicht ganz (passierte bei Play-Höhen).
- `page_blends_with_store`: Seitenrand hat < ~1,3 : 1 Abstand zum hellen (#F2F2F7) bzw. dunklen (#1C1C1E) Store-Hintergrund.

### P1 – Nähte und Store-Lücken

Elemente über Seitennähte sind das Alleinstellungsmerkmal – und die häufigste Quelle sichtbarer Fehler.

#### P1.7 Lückenbreite je Target als verlässlicher Wert
- **Beobachtung:** Die Lücke zwischen Store-Screenshots ist laut `catalog_galleries` ein Schätzwert (iPhone ~45 lu, iPad ~40 lu, Play anders). Weicht die echte Lücke ab, versetzt sich alles, was über die Naht läuft: bei Diagonalen um Steigung × Abweichung, bei „Zwillings“-Elementen (je eine Hälfte auf jeder Seite) sichtbar. **Konkreter Fall:** In `paper-calm` sitzt das Häkchen-Kästchen über Naht 1|2 sichtbar versetzt – genau das, was mit Vitrine nicht passieren soll.
- **Workaround:** Motive an der Naht waagerecht und auf gleicher Höhe führen (dann ist die Lücke egal); Diagonalen und Zwillinge vermeiden.
- **Vorschlag:**
  1. Lückenbreite je Target als abfragbarer Wert (in `project_summary` / `catalog_targets`), möglichst gemessen statt geschätzt, mit Angabe der Unsicherheit.
  2. Eine Funktion „Element läuft auf der Folgeseite weiter“ bzw. **Clip auf eine andere Seite**: Vitrine schneidet ein Element selbst an der Naht und setzt den Rest auf der Nachbarseite lückengenau fort – pro Target.
- **Gelöst, wenn:** Ein Kreis exakt über einer Naht in allen Targets ohne Versatz erscheint, ohne dass der Autor Lücken kennt.

#### P1.8 Abgerundete Galerie-Ecken
- **Beobachtung:** Store-Galerien zeigen Screenshots mit abgerundeten Ecken. Eckige Flächen bis an den Seitenrand (z. B. Notizbuch-Blätter in `notebook-grid`) laufen sichtbar in die Rundung und wirken abgeschnitten.
- **Vorschlag:** Eckenradius je Store/Target in `catalog_galleries` ausgeben; optional Info `element_in_corner_radius`; `render_preview gallery` zeigt die Rundung bereits – das als Hinweis dokumentieren.

### P2 – Weniger Overrides für Play-Varianten

#### P2.1 Gruppen / Element-Sets in Varianten skalieren
- **Beobachtung:** Pro Canvas waren für Play typischerweise 35 bis über 200 Einzel-Overrides nötig (jedes Kind einer Karte, Feature-Wand mit 14 Elementen …), alle per Skript erzeugt. Von Hand wäre das nicht wartbar.
- **Vorschlag:** In Varianten `scale` (und `dx/dy`) für Gruppen bzw. Elementlisten; dazu `anchor_x: left|center|right`, damit linksbündige Texte ihre Kante halten. Alternativ „alle Elemente unterhalb y = … um Faktor k stauchen“.
- **Bonus:** `move_element {dx, dy}` für mehrere Elemente auf einmal.

#### P2.2 Rahmenmaße der Geräte im Katalog
- **Beobachtung:** Um Geräte sauber zu platzieren (auf einer Linie stehen, oben angeschnitten, Karte über Screen), mussten Rahmen-Seitenverhältnisse und Screen-Anteile je Modell ausgemessen werden – besonders bei `device_map` iPhone → Android, iPad → Android-Tablet (16:9, anderes Screen-Format).
- **Vorschlag:** `catalog_devices` liefert je Modell und Orientierung: Rahmengröße, Screen-Rechteck relativ zum Rahmen, Eckenradius, verfügbare `color`-Werte. Optional Geräte-Anker „Oberkante Screen“ / „Unterkante Rahmen“, damit gemappte Geräte auf derselben Linie stehen bleiben.

#### P2.3 Sprach-Override für Bild-Assets
- **Beobachtung:** Bilder mit eingebranntem Text (UI-Karten, Textmarker-Unterstreichung mit sprachabhängiger Breite) gibt es je Sprache. Device-Screenshots lösen das über `screenshots/<lang>/`, normale `image`-Elemente nicht.
- **Workaround:** Jedes Bild doppelt anlegen, je mit `visible_in.langs` – Position, Drehung und alle Varianten-Overrides doppelt pflegen.
- **Vorschlag:** `asset` pro Sprache überschreibbar (wie Texte) oder Pfad-Platzhalter `assets/card-{lang}.png` mit Fallback auf die Standardsprache.

### P2 – Export und Vorschau

#### P2.4 Export-Timeouts
- **Beobachtung:** Vollexporte (3 Canvases, 84 Dateien) mit großen Fotos/Panoramen liefen regelmäßig in das 80-s-MCP-Zeitlimit. Die Dateien kamen trotzdem vollständig an, der Aufrufer weiß das aber nicht. Exporte ohne große Bilder: 20–45 s.
- **Vorschlag:** Export asynchron (Job-ID + `export_status`) oder Fortschrittsmeldungen, die das Timeout verlängern; Option „nur Galerie“ (`_gallery.png` ohne Einzelseiten); echtes Inkrementell auf Seitenebene.

#### P2.5 `render_preview` mit Store-Hintergrund
- **Vorschlag:** Option `store_background: light|dark` für die Galerie-Vorschau. Fast jede Vorlage hat diesen Test von Hand nachgebaut (helle Seiten nahe #F2F2F7, dunkle nahe #1C1C1E).

#### P2.6 Vorschau-Größe
- **Beobachtung:** Große Galerien überschritten das Transportlimit.
- **Vorschlag:** JPEG-Option oder automatische Verkleinerung unter das Limit; optional direkt in eine Datei im Projekt schreiben (für README-Vorschauen).

### P3 – Neue Gestaltungsmöglichkeiten

| Wunsch | Warum | Häufigkeit |
|---|---|---|
| **Element-Typ „Pfad/Polyline“** (Spline-Punkte, Strichstärke, Dash, Kontur, Marker/Pfeilspitzen, Text entlang des Pfads, Dekor in Abständen, Farbverlauf entlang des Pfads, z je Abschnitt) | Routen, Ranken, Maßlinien, Sternbilder, Kreidelinien, Bon-Band – alles heute als statische SVG je Variante, im Editor nicht änderbar | sehr häufig |
| **Span-Felder `padding`, `radius`, `scale`/`size`, `rotation`** | Pillen/Highlights hinter Wörtern, Handschrift-Akzent größer als Rest der Headline in **einem** Textelement | häufig |
| **Span-Deko „highlight/underline-image“** (Form/Bild hinter dem Span, Breite automatisch) | Textmarker/Kritzel-Unterstreichung heute als Bild je Sprache mit fester Breite | mehrfach |
| **`backdrop_blur` für Formen** | Echtes Milchglas über Geräten | einmal, aber wichtig für einen ganzen Stil |
| **`allow_clip` / Flag „decorative“ für Texte** | Absichtlich angeschnittene Riesen-Typo erzeugt sonst Dutzende Warnungen (48 bei `big-type`) | einmal |
| **Formen mit automatischer Breite / „Card“-Element mit Padding und Kindern im lokalen Koordinatensystem** | Pillen, Blasen, Bento-Kacheln ohne Generator wartbar | mehrfach |
| **Tab-Stopps/Leader in Texten** | Bon-/Preiszeilen | einmal |
| **`focus` auch für `image` mit `fit: cover`** | Bildausschnitt bei verschiedenen Seitenhöhen steuern | einmal |
| **Schatten-Offset relativ zur Schriftgröße** | Gleicher Schatten wirkt auf Querformat doppelt so schwer | einmal |
| **Weitere Gerätemodelle:** Wear OS (rund/eckig), Farbvarianten (silver/white) dokumentieren | Gerätemix | einmal |
| **`catalog_fonts` mit Unicode-Abdeckung** | Font-Teilmengen ohne ✓, griechische Buchstaben, Pixel-Fonts | mehrfach |

---

### Status P1 (Coding-LLM, 12.11.2026)

Umgesetzt in ADR 0028 (`docs/adr/0028-design-checks.md`). Alle Befunde gelten pro Target × Sprache, mit stabilem Code, Stufe, Element-ID und Seite, in CLI (`validate`, alle Formate), GUI-Befundliste und MCP (`validate`, `render_preview`-Issues). Gerenderte Prüfungen sind standardmäßig an (`--no-pixel-checks` / `pixel_checks: false` schaltet sie ab). Gewollte Fälle: Elementfeld `allow: [code]` (auch an Gruppen, auch in Varianten), projektweit `checks:` in `project.yaml` (Schwellwerte, `off: [codes]`, Op `set_checks`).

- **P1.1 Text am Gerät:** gab es nicht (nur Text-Box gegen Box in `layout_report`). Neu `text_near_device`: echte Glyphen-Maske gegen die gerenderte Rahmenkontur (gekippte Geräte, Apple-Bezels, Laptops exakt). Warnung, wenn Glyphen hinter dem Gerät liegen oder über die Rahmenkante laufen; Hinweis bei < 16 lu Abstand (`checks.device_distance`). Text ganz auf dem Gerät oder auf einer Karte über dem Gerät ist gewollt und wird nicht gemeldet; Riesen-Typo ≥ 240 lu ebenfalls nicht. Statt des Platzhalters `allow_clip` heißt das Opt-out `allow` und nimmt Codes (`allow: [text_clipped]` erledigt auch den `big-type`-Fall).
- **P1.2 Screen-Box:** neu. `layout_report` liefert je Gerät `frame` (gedrehte Ecken) und `screen` (Box vor der Drehung, gedrehtes `quad`, Eckenradius, `header` = Höhe Statusleiste + Titel) in Seiten-lu, nach `device_map`, plus `screen_covered_by` (Anteil an Screen und Kopfzeile). Befunde `screen_header_covered` (Element über ≥ 25 % der Kopfzeile) und `screen_header_cut` (Seitenkante schneidet durch die Kopfzeile). Andere Geräte über einem Screen gelten als gewollter Stapel.
- **P1.3 Kontrast:** neu `low_contrast` (Warnung): Füllfarbe der Glyphen gegen das tatsächlich Gerenderte direkt um die Glyphen (Farbe, Verlauf, Foto, SVG, eigene Kontur/Schatten/Highlight), WCAG-Verhältnis je 1-em-Zelle (30-%-Quantil), schlechteste Stelle mit Seitenposition und Richtung. AA: 4,5 : 1, große Schrift (≥ 80 lu, fett ≥ 62 lu ≈ 24 pt bei ~300 pt Store-Breite) 3 : 1. Texte mit Deckkraft < 0,5 gelten als Deko. Budget: ~0,3 s pro Target × Sprache (Release).
- **P1.4 Seitengrenzen:** `layout_report.overlaps` verglich schon alle Seiten, aber nur Texte und nur Boxen. Neu: Befund `seam_overlap` (Element ragt über die Naht und liegt *über* einem Element der Nachbarseite; Elemente, die darauf liegen – Text auf einer Brückenkachel –, Seitenhintergründe und Gruppenmitglieder zählen nicht) und `seam: true`-Einträge im Report.
- **P1.5 Pixel-Deckung:** `overlaps` misst jetzt gezeichnete Pixel (transparente Bildflächen zählen nicht); Bilder/Formen, die ≥ 90 % einer Seite decken, gelten als Hintergrund und fallen heraus.
- **P1.6 Weitere:** `device_small` (größtes Gerät < 50 % der Seite – 60 % war in Bento-/Mehrgeräte-Layouts zu laut), `page_not_covered` (fast seitenfüllendes Bild/Panel lässt Streifen ≤ 15 %), `page_blends_with_store` (≥ 60 % des Bildrands < 1,3 : 1 gegen hellen bzw. dunklen Store-Hintergrund; Dunkelwerte als *nicht gemessen* in `galleries.yaml`).
- **P1.7 Lückenbreite:** gab es (Galerie-Profil je Target), war aber nicht abfragbar. Jetzt in `layout_report.gallery` (`gap_lu`, `gap_px`, Radius, `measured`) und `seams` (`next_page_offset`). Die gewünschte „Fortsetzung auf der Folgeseite“ existiert bereits: **ein** Element mit `clip: none` über die Naht läuft in jedem Target lückengenau hinter der Lücke weiter. Der `paper-calm`-Versatz kam von zwei Hälften (`seam-a-box`/`seam-b-box`, je `clip: page`) – neu meldet `seam_split` genau diesen Bau. Offen bleibt nur, die echte Lücke der Store-Apps zu messen.
- **P1.8 Galerie-Ecken:** Radius war in `galleries.yaml` und in der Galerie-Vorschau schon vorhanden; jetzt auch im Report und als Befund `element_in_corner` (Element ragt in den abgerundeten Teil; Geräte und eckfüllende Flächen wie Scrims ausgenommen). Radien der Store-Apps sind weiterhin Schätzwerte (`measured: no`).

**Abstimmung an 14 Vorlagen (Kopien):** erste Fassung 60–190 Befunde pro Vorlage, nach Feinschliff 0–40. Größte Lärmquellen und Lösung: UI-Karten über Geräten (Text sitzt auf der Karte → ausgenommen), Antialiasing in der Kontrastmessung (exakte Füllfarbe statt Mischpixel), Hilfslinien/Raster neben Glyphen (Quantil statt Minimum), Scrims in Ecken, Elemente unter Brückenkacheln. Was bleibt, sind überwiegend echte WCAG-Unterschreitungen kleiner Eyebrows auf Verläufen (`aurora-glass`, `bento-grid`, `map-route`), die bewusst angeschnittenen bzw. geteilten Riesenziffern in `big-type` (→ `allow`), die Maßlinien-Beschriftungen in `blueprint` (→ `allow`), angeschnittene Statusleisten (`day-to-night`, `paper-calm`, `still-lake`) und die Zwillings-Häkchen in `paper-calm`.

## 4. Bugs (bitte verifizieren)

1. **`render_preview gallery` bei großem `max_edge`:** Fehler „preview too large (…×0)“ – Höhe 0. Tritt vor allem bei Play-Querformat auf. Mit `max_edge` 1000–1600 geht es.
2. **Mehrere Farben in `backgrounds` des Canvas werden still ignoriert:** Alle Seiten bekamen die erste Farbe, ohne Fehlermeldung. Entweder Bug oder undokumentiertes Verhalten (siehe 5).
3. **Gelegentliche 80-s-Timeouts bei `styles_update`/`styles_get` und erstem `validate`** direkt nach großen Projekten oder nach einem App-Start; nach Neustart der App bzw. beim zweiten Aufruf normal. Vermutlich Asset-Laden/Cache-Aufwärmen.
4. **`render_preview` hängt nach erstem großen `apply_ops`** (mehrere Timeouts in Folge bei einem Projekt mit vielen großen Fotos). Wunsch: Fortschritt oder Cache vorwärmen.

## 5. Dokumentationslücken

Diese Dinge haben die Agenten durch Ausprobieren herausgefunden – kurze Hinweise in `docs_get` hätten viel Zeit gespart:

- Was mehrere Einträge in `backgrounds` bedeuten (Ebenen? Zyklus pro Seite?) – oder Seitenfarbe pro Seite erlauben.
- Wie `background`-Elemente mit anderen Elementen nach z sortiert werden (liegen undurchsichtige Bilder darüber?).
- Was `at`/`width` bei Gruppen bedeuten und wie `at` in einem Variant-Override bei `anchor_y: bottom` wirkt.
- Kachelgröße in lu für `fill: tile`.
- `shadow` ist als Element-Override auf Texten möglich.
- Welche `color`-Werte je Gerätemodell gelten.
- Dass `model: none` (rahmenloser Screen) Ecken selbst rundet und im iPad-Set die Apple-Regel „kein iPhone-Rahmen“ erfüllt.
- Dass `content_scale` auch Panorama-Hintergründe skaliert.
- Dass undurchsichtige Bilder über `background`-Elementen liegen (Panoramen daher transparent anlegen).

## 6. Rahmenbedingungen außerhalb von Vitrine (nur zur Info)

Nicht von Vitrine verursacht, aber für den Ablauf relevant:

- **Freigaben nach Server-Neustarts:** Zweimal blieb ein Sub-Agent an einer Freigabe-Anfrage hängen, die beim Neustart des Claude-Servers verfallen war. Kein Vitrine-Problem.
- **Transportlimit ~10 MB** pro übertragener Datei zwischen Claude-Container und Mac; Galerie-PNGs im Querformat (bis 12 MB) wurden deshalb nur auf dem Mac kopiert.
- **`rm` ist auf dem Mac gesperrt** – Reste liegen geleert in `_unused/`-Ordnern.
- `screenshots_import` / `assets_import` wurden bewusst nicht genutzt (Dateien direkt per Shell kopiert), deren Verhalten ist also ungetestet.

## 7. Empfohlene Reihenfolge (aus Anwendersicht)

1. P1.1–P1.3 (Text am Gerät, Screen-Box, Kontrast) – größter Qualitätsgewinn für alle Nutzer.
2. P1.7 (Lücken/Fortsetzung über Nähte) – schützt das Kernfeature.
3. P2.4 + P2.5 (Export asynchron/„nur Galerie“, Store-Hintergrund in der Vorschau).
4. P2.1–P2.3 (Varianten-Scale, Gerätemaße, Sprach-Assets).
5. P3 nach Bedarf, zuerst „Pfad/Polyline“ und Span-Padding/Radius.

**Nochmals:** Diese Reihenfolge ist eine Empfehlung aus Nutzersicht. Die Coding-LLM prüft gegen den bestehenden Code, entscheidet selbst oder spricht mit Thorsten.

---

*Detailprotokoll pro Vorlage: `Vorlagen/_plan/mcp-befunde.md` (~1.300 Zeilen, chronologisch, mit Zahlen und Workarounds).*