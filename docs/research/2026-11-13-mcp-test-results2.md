# MCP-Befunde v2 – nach dem MCP-Umbau (ab 12.11.2026, abends)

Neue Datei für alle Befunde mit dem **neuen** Vitrine-MCP. Die alte `mcp-befunde.md` bleibt als Archiv.
Jeder Subagent hängt hier unten einen Abschnitt `## <slug> (<ID>)` an.

## Was der neue MCP kann (gelesen aus `docs_get`, Stand 12.11. 22:40)

### Prüfungen in `validate` (neu)
- Neue Befund-Codes: `text_near_device`, `low_contrast`, `text_clipped`, `text_in_gap`, `text_outside_safe_area`,
  `element_outside`, `screen_header_covered`, `screen_header_cut`, `seam_overlap`, `seam_split`, `element_in_corner`,
  `device_small`, `page_not_covered`, `page_blends_with_store`, `hyphenated`, `variant_suggested`, `screenshot_upscale`,
  `background_hidden`, `override_null`.
- **Pro Element abschaltbar:** `allow: [code, …]` (z. B. Riesen-Typo `allow: [text_clipped, text_in_gap]`, Pfeile am Gerät `allow: [text_near_device]`).
- **Pro Projekt:** `checks: {device_distance: 16, min_contrast: 4.5, min_contrast_large: 3, off: [...]}` per `apply_ops {op: set_checks}`.
- → Die eigenen Kontrastmessungen und das Absuchen der Bilder nach „Text am Gerät“ / „Karte über App-Kopf“ sollten weitgehend entfallen.
  Trotzdem jede Galerie einmal ansehen (Geschmack, Komposition, Leonardo-Artefakte).

### Vorschau & Export
- `render_preview {view: "gallery"}` zeigt Lücken **und** abgerundete Ecken; `background: "dark"` (oder Farbe) = dunkler Store;
  `format: "jpeg"` für kleine Übertragungen; `out_file` schreibt PNG ins Projekt (README-Vorschau).
- Breite Galerien werden automatisch niedriger gerendert (max. 4096 px) – kein `max_edge`-Basteln mehr nötig.
- `export` läuft im Hintergrund, wenn länger als `wait_seconds` (Default 45) → `export_status {job}` abfragen.
- `export {pages: false}` = nur `_gallery.png` (schneller Gesamtcheck).

### Geräte
- `catalog_devices` liefert `frame` je Modell/Orientierung: Höhe, Screen-Rechteck `[x,y,w,h]`, Eckenradien (für Breite 1000; mit width/1000 multiplizieren) und `colors`.
- `model: none` rundet Ecken (10 % der kurzen Seite), Höhe folgt dem Screenshot.

### Dokumentiert (vorher durch Probieren gefunden)
- Canvas-`backgrounds` = Ebenen über das ganze Canvas; Seitenfarbe = ein `background`-Element pro Seite (niedriges `z`, z. B. -30).
- `background`-Elemente sortieren nach (z, Listenreihenfolge) wie alle anderen.
- `fit: tile`: 1 px / 1 SVG-Einheit = 1 lu.
- Gruppe: `at` = Versatz, `width/height` ungenutzt; Gruppe in Variante verschieben = ein `at`-Override; `rotation` dreht um Gruppenmitte (+`pivot`).
- `anchor_y` wirkt nach Overrides; `content_scale` skaliert nur Geräte.
- `shadow` auch auf Texten (auch in Text-Style und Sprach-Override).
- Text-Elemente: neu `stroke` (Kontur, position outside/center) und `glow`.
- Span-Styles: `color, weight, italic, font, background, stroke, glow, shadow`.
- `styles.fonts.<Familie>.fallback` für fehlende Zeichen (z. B. Noto Sans JP – wichtig für Anime/Manga-Deko).
- Targets: `safe_area`, Banner (`banner_add`), Watch/Wear `screenshot_only`, iPhone Duo.

### Noch NICHT gesehen (Stand docs) – weiter mit Workarounds
- Element-Typ Pfad/Polyline → weiter SVG je Variante.
- Span `padding` / `radius` / `size` → weiter eigene Formen bzw. eigenes Textelement für größeren Akzent.
- Sprach-Override für `asset` → weiter je Sprache ein Bild mit `visible_in.langs`.
- Gruppen-/Element-`scale` in Varianten → weiter Overrides per Skript (aber: Gruppen-`at`-Override spart viel).
- `backdrop_blur`, „läuft auf Folgeseite weiter“ / Clip auf andere Seite, abfragbare Lückenbreite je Target.
- Ob `seam_split` / `seam_overlap` Motive über Nähte zuverlässig prüft → bitte beobachten und hier berichten.

---
<!-- Abschnitte der Subagenten unten anhängen -->

## seed-to-harvest (A01, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort, kein Timeout. 3 Canvases × 6 Seiten, 6 Targets, ~14 Schreibaufrufe (`styles_update`, Strings + Phone 107 Ops,
Tablet 78, Quer 87, danach nur kleine Diff-Aufrufe). `render_preview` immer sofort, `validate` (mit Pixelprüfung, alle 6 Targets × 2 Sprachen) wenige Sekunden,
`export {pages: false}` 12 Galerien in 9,8 s ohne Hintergrund-Job. Keine Freigabe nötig, kein Abbruch. Ergebnis: **0 Fehler, 0 Warnungen**, 2 Infos.

**Neue Prüfungen – echte Fehler gefunden (alle im Bild bestätigt und behoben):**
- `low_contrast` (warn) – Feature-Zeile auf Tablet-S. 6 lag über der Spitze der Tomatenpflanze (2,7 : 1); auf Play-Tablet quer wuchs die Tomate in die Headline (2,9 : 1),
  weil die Variante die Pflanze nach oben skalierte. Genau die Stellen, die ich sonst per Pixelmessung gesucht hätte – mit Ort (x/y lu) in der Meldung, sehr hilfreich.
- `text_near_device` (warn) – Feature-Zeile im Querformat lief 4 % in das iPad.
- `screen_header_covered` (info) – Mitteilungs-Karte deckte 39–48 % der Kopfzeile des iPad-Screens (Querformat iOS + Play). Real, Karte nach unten versetzt.
- `page_blends_with_store` (info, nur play-phone) – März-Grün oben zu nah an Weiß → Verlauf eine Stufe dunkler.
- `device_small` (info, play-tablet S. 2) – zwei rahmenlose Phones nur 50 % der Seitenhöhe → in der Variante × 1,4.
- Schön: `render_preview` liefert die Befunde des Targets × Sprache gleich mit – die meisten Fehler sah ich schon beim ersten Galerie-Blick.

**Falsche Alarme / gewollt:** nur `seam_overlap` (info) für den Erntekorb auf der Naht 5|6 (überdeckt dort Erde/Blüten der Nachbarseite) → `allow: [seam_overlap]`
am Korb, wirkte sofort und auch für Varianten. `unknown_screenshot_variant` für die Querformat-Ordner wie bisher (Info, harmlos). Sonst keine Fehlalarme.

**Nicht gemeldet (nur im Bild gesehen):**
- Breakout-Karte über dem **wichtigsten UI-Inhalt** (iPhone S. 4: Karte lag auf „14 litres“) – `screen_header_covered` prüft nur die Kopfzeile, nicht den restlichen
  Screen. Wunsch: Info „covers N % of the screen content“ ab einer Schwelle oder ein `screen_focus`-Rechteck je Screenshot.
- Szenenlogik: ein hinteres Phone „schwebte“ über der Erde (Unterkante über der Bodenlinie). Nicht MCP-Aufgabe, aber ein Hinweis „Element endet frei über einem Bild,
  das es überdecken sollte“ wäre für Panorama-Böden nützlich. Gelöst über eigenes Skript (Ecken des gedrehten Geräts gegen die Bodenlinie).
- Ein Stern der Abend-SVG lag hinter dem Eyebrow („04 · JUNE“ sah aus wie „04·-JUNE“); Kontrast war ok, darum kein Befund. Sterne-Ruhezone im SVG.
- Der `seam_*`-Check sagt nichts zur Bodenlinie, die per Bild je Seite über die Nähte läuft (zu Recht: die Linie ist auf jeder Seite eigenständig). Die Nahtgleichheit
  sichert hier das Skript (`scene.py`: h(X) an der Seiten-UNTERKANTE, alle Seiten werten dieselbe Funktion bei X = 1000·k aus → passt bei jeder Lücke und jeder Play-Höhe,
  ohne eigene SVGs je Variante – Empfehlung für alle „Boden/Horizont“-Panoramen).

**Neue Funktionen:** `background: "dark"` funktioniert (Play-Tablet-Galerie geprüft); `out_file` nicht gebraucht, `export {pages:false}` + Kopie nach `preview/` reicht;
`catalog_devices.frame` lieferte die Rahmenhöhen für die Pflanz-Rechnung (Gerät steckt in der Erde). `allow` pro Element: gut.

**Was weiterhin fehlt / Stolpersteine:**
- `apply_ops` nur inline: große Canvases = 16–18 KB JSON im Aufruf. Ich habe ein `diff_ops.py` geschrieben (alter vs. neuer Generator-Stand → nur `update_element`/
  geänderte Overrides), danach waren Korrekturen 1–4 KB. Wunsch: `apply_ops {file: …}` (Datei im Projekt) oder `ops_url`.
- Mein eigener Fehler: `render_preview` mit `"format": jpeg` (ohne Anführungszeichen) → Client-JSON-Fehler; erst nach mehreren Versuchen ohne `format` gerendert. Kein MCP-Bug.
- Dateien auf dem Mac: `tar -x` kann bestehende Dateien nicht überschreiben (Löschen gesperrt → „Cannot open: File exists“). Lösung: `tar -xOf <tar> <member> > ziel`.
  Wichtig für alle Subagenten, die Assets nachliefern.
- Weiter offen (wie zuvor): Pfad/Polyline-Element, Sprach-Override für `asset`, Gruppen-`scale` in Varianten, ein „Bodenanker“ (Element folgt einer Linie).

## comic-burst (C01, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 3 Canvases × 6 Seiten, 6 Targets, ~19 Schreibaufrufe (Basis je Canvas 17–19 KB, Play-Varianten 10–11 KB,
danach Diff-Aufrufe 1–6 KB mit `diff_ops.py`). `render_preview` sofort, `validate` (alle Targets × Sprachen, mit Pixelprüfung) wenige Sekunden,
`export {pages: false}` 12 Galerien in 14 s ohne Hintergrund-Job. Keine Freigabe nötig (nach dem Neustart; vorher blockierte ein `curl` auf GitHub –
Schrift dann per `npm pack @fontsource/bangers` + fontTools WOFF→TTF). Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos, kein einziges `allow`**.

**Neue Prüfungen – echte Befunde (alle im Bild bestätigt und behoben):**
- `low_contrast` (warn) auf Burst-Texten (1,0–3,2 : 1): Bangers-Text lief über die schwarze Kontur bzw. den äußeren Zacken des Bursts. Richtig erkannt,
  inklusive Ort. Fix: innerer Stern größer, Textbox 50 % statt 62 % der Burst-Breite.
- `low_contrast` (warn) auf `wall-w4` (1,5 : 1): „GAME NIGHTS“ lief über die Kontur des Mini-Panels. Fix: Schrift 80 statt 86, Box 80 % der Zelle.
- `low_contrast` (warn) auf allen Querformat-Headlines (1,0 : 1): die Headline berührte den **harten Schatten** des Caption-Kastens darüber.
  Sehr gut – das hätte ich im kleinen Galeriebild übersehen. Fix: Headline 10 lu tiefer.
- `low_contrast` (warn, 3,9–4,1 : 1): Rot-Akzent auf Hellcyan im Querformat (56 lu = keine „große Schrift“). Fix: Akzentrot `#C8001E → #A50016`
  und Halbton im Kopf-Panel erst ab 68 % der Diagonale. Auf dem Phone (136 lu) war 3 : 1 ausreichend – die Schwelle nach Größe ist sinnvoll.
- `screen_header_covered` (info): Sprechblase „I’M IN!“ lag auf 35–49 % der Kopfzeile des Event-Phones. Richtig; Blase nach unten links verlegt.
- `element_in_corner` (info): Figuren (Nova, Bix) ragten in die abgerundete Store-Ecke unten links (125 lu). Richtig und hilfreich: die eckige Bildkante
  der Figur schaute aus der runden Panel-Ecke. Fix: Figuren 30–40 lu nach innen + Figur-PNG mit runder Ecke unten links.

**Falsche Alarme:** keine. Erwartete Infos `unknown_screenshot_variant` für die Querformat-Ordner wie bei allen anderen.

**Nicht gemeldet / Grenzen der Prüfungen:**
- `seam_overlap` / `seam_split` meldeten nichts – korrekt, denn die nahtkreuzenden Panels sind `background`-Elemente je Seite (eigene SVG-Ausschnitte).
  Ob die Panel-Kanten an der Naht zusammenpassen, prüft der MCP nicht (ist per Geometrie gesichert: Reihengrenze an jeder Naht waagerecht).
  Wunsch: eine Info „Kante endet an der Naht auf Höhe y₁, auf der Nachbarseite auf y₂“ für Hintergrundbilder (Vergleich der Randspalten).
- `text_near_device` meldete nichts, obwohl Blasen-/Burst-Texte teils 20–40 lu neben dem Gerät stehen und die Blasen-Schwänze das Gerät berühren.
  Ich hatte vorsorglich `allow: [text_near_device]` gesetzt, zum Test wieder entfernt → kein Befund. Offenbar misst die Prüfung nur ab Textbox
  (Blase/Burst als Bild dazwischen?) – in diesem Fall gewollt, aber unklar, ob das Absicht ist.
- Karten/Blasen über wichtigem Screen-Inhalt: erkennt Vitrine weiterhin nicht (nur die Kopfzeile). Im Bild geprüft: Blase S. 2 lag anfangs auf der
  Mika-Kachel des Voice-Screens, Bursts liegen nur auf Tab-Leiste/Composer.
- Figur mit Gesicht hinter einem Gerät (Hero, erster Entwurf: Nova schaute vom Phone verdeckt nach links) – kein MCP-Thema, nur im Bild sichtbar.
- Headline von einem Ausbruch-Splitter berührt (Play-Tablet S. 2, Splitter in der Variante zu groß skaliert): kein `low_contrast`, weil die Spitzen
  hinter den Buchstabenzwischenräumen lagen. Wunsch: Info „Bild/Form überlappt eine Headline-Box“ (layout_report `overlaps` hat es, validate nicht).

**Neue Funktionen:** `render_preview {background: "dark"}` gut. `format: jpeg` habe ich zweimal mit nacktem `jpeg` (ohne Anführungszeichen) aufgerufen –
derselbe Client-JSON-Fehler wie bei seed-to-harvest; ohne `format` geht es. `export {pages: false, out_dir, html: false}` + `cat` nach `preview/` = schnell
und zuverlässig. `catalog_galleries` (Eckradius 125 lu iPhone / 20 iPad / 40 Play) habe ich für die abgerundeten Panel-Ecken genutzt.
Projekt-Schrift (`fonts/Bangers-Regular.ttf` + `OFL.txt`) wurde sofort von `catalog_fonts` erkannt.

**Stolpersteine / Wünsche:**
- Weiter `apply_ops` nur inline (Basis-Canvas 17–19 KB im Aufruf). Wunsch wie gehabt: `apply_ops {file: …}`.
- Variante: keine „Abbildung eines Bereichs“ – die Play-Positionen rechne ich im Generator (Reihe 2 auf die neue Höhe abbilden, Geräte/Figuren eigene Faktoren).
  Ein Varianten-Modus „y relativ zu einer Linie skalieren“ (Anker = Reihengrenze statt Seitenkante) würde viele Overrides sparen.
- Text in Sprechblasen/Bursts: zwei Elemente je Blase (Bild + Text) und eine Blasen-Metadatei (Körpermitte im viewBox). Ein Element-Typ „Sprechblase“
  (Form mit Schwanzrichtung, Text innen, Auto-Größe je Sprache) oder Span-`padding`/`radius` würde das stark vereinfachen.
- Bild-Clip auf eine Form/Panel fehlt (Figuren werden an der Panel-Kante nicht beschnitten) – hier über Platzierung auf der Kontur + runde PNG-Ecke gelöst.

## liquid-chrome (C02, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 2 Canvases × 6 Seiten (phone, tablet-land), 4 Targets. Schreibaufrufe: `styles_update`, Strings,
Phone (23 KB inline), Querformat (24 KB inline), danach ein Diff-Aufruf (6 KB, `diff_ops.py` aus comic-burst übernommen). `render_preview` sofort,
`validate` (alle Targets × Sprachen, mit Pixelprüfung) wenige Sekunden, `export {pages: false, out_dir, html: false}` 8 Galerien in 15,7 s mit
`wait_seconds: 120` ohne Hintergrund-Job. Keine Freigabe nötig, kein Abbruch. Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos, kein `allow`**.
Projekt-Schrift (`fonts/Unbounded-ExtraBold.ttf` aus `npm pack @fontsource/unbounded`, WOFF → TTF) wurde sofort als „Unbounded“ erkannt –
ich hatte vorsorglich nameID 16/17 (Typographic Family „Unbounded“ / „ExtraBold“) ergänzt, weil fontsource-Einzelgewichte in nameID 1
„Unbounded ExtraBold“ heißen (ohne Ergänzung nicht getestet). Tipp für alle: Name-Tabelle prüfen.

**Neue Prüfungen – echte Befunde:**
- `page_blends_with_store` (info): erster Wurf mit fast schwarzen Seitenrändern (#120A2E …) → 3 bzw. 5 Seiten verschmolzen mit dem dunklen Store
  (iOS #000, Play #131314), die Limettenseite mit dem hellen. Richtig und nützlich – Verläufe eine Stufe heller, Limette etwas satter → weg.
  Schön: der Befund nennt die Bildnummern.
- `device_small` (info, play-tablet quer S. 3): iPad nur 50 % der Seite → iPads im Querformat 500 → 530 lu, Play-Faktor 0,8 → 0,86.
- Sonst nichts: Kontrast (weiß/Limette auf Dunkel, Ink/Violett auf Limette) war von Anfang an sauber.

**Nicht gemeldet (im Bild geprüft):**
- **Seite 4: Phone oben angeschnitten** (Statusleiste und Titel des Screens liegen außerhalb der Seite) – kein `screen_header_cut`. Gewollt, aber ich hatte
  die Info erwartet; vermutlich zählt nur ein Anschnitt durch eine Lücke/Karte, nicht durch die Seitenkante? Bitte klären.
- Deko vor dem Gerät über UI: Chrom-Wörter FLIP/MORE und Ring-Vorderteil liegen über Tab-Leiste/Button der Screens, Tropfen-Traube über einem Tab –
  keine Meldung (wie bekannt: nur die Kopfzeile wird geprüft). Im Bild bewusst nur auf Tab-Leisten/CTA gelegt.
- Nahtübergang des Chrom-Flusses: kein `seam_*`-Befund, korrekt, weil die Bänder in die Seiten-Hintergründe eingerechnet sind. Die Nahtgleichheit sichert
  der Generator (Szene in globalen Koordinaten mit der Lücke des jeweiligen Targets). Wunsch wie bei comic-burst: Randspalten-Vergleich für Hintergrundbilder.
- Abgeschnittene Enden der Leonardo-Bänder: nur hinter Geräten versteckt – das prüft Vitrine natürlich nicht; im Generator über Ankerpunkte relativ zur
  (gedrehten) Gerätebox gelöst und in allen 4 Galerien kontrolliert.

**Neue Funktionen:** `render_preview {view: gallery, background: "dark"}` gut zum Prüfen der dunklen Vorlage; breite Querformat-Galerien kommen automatisch
niedriger (541 px) – für Details `view: image` je Seite. `format` habe ich weggelassen (keine Probleme). `catalog_galleries` für die Lückenbreiten
(Play: `basis: height`, 4 % der Seitenhöhe → 71,1 lu Phone / 22,5 lu quer) und `catalog_devices` für die Geräteverhältnisse genutzt.

**Stolpersteine / Wünsche:**
- `apply_ops` weiter nur inline (23–24 KB je Canvas). Wunsch unverändert: `apply_ops {file: …}`.
- Geänderte Bilddateien mit gleichem Namen (per `tar -xOf … > ziel` überschrieben) übernahm Vitrine sofort im nächsten `render_preview` – gut.
- Szenen-Hintergründe je Play-Variante brauchen eigene Dateien (`fill`-Override); ein Platzhalter im Asset-Pfad (z. B. `bg-{variant}-{page}.jpg`) würde
  12 Overrides je Canvas sparen.
- Variante: weiterhin keine Bereichs-Abbildung (y ab Linie skalieren) – im Generator gerechnet.

## fruit-pop (C03, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 3 Canvases × 6 Seiten, 6 Targets. Schreibaufrufe: `styles_update`, Strings + Phone (23 KB inline),
Tablet (23 KB), Querformat (26 KB), danach 5 kleine Diff-Aufrufe (`diff_ops.py` aus comic-burst) und 2 `styles_update`. `render_preview` sofort,
`validate` (alle Targets × Sprachen, mit Pixelprüfung) wenige Sekunden, `export {pages: false, out_dir, html: false}` 12 Galerien in 10 s ohne Hintergrund-Job.
Keine Freigabe nötig, kein Abbruch. Schrift per `npm pack @fontsource/bricolage-grotesque` (WOFF → TTF, nameID 16/17 ergänzt) – sofort als „Bricolage Grotesque“ erkannt.
Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner).

**Neue Prüfungen – echte Befunde (im Bild bestätigt, behoben):**
- `low_contrast` (warn, 4,4 : 1) – dunkelroter Akzent `#9B0E23` auf Himmelblau, nur im **Querformat** (42 lu = keine „große Schrift“); auf Phone/Tablet (≥ 76 lu) galt 3 : 1.
  Fix `#850A20` (5,3 : 1). Die größenabhängige Schwelle ist richtig und hätte ich ohne Vitrine übersehen.
- `page_blends_with_store` (info) – Aubergine `#2A0E3D` verschmolz mit dem dunklen Store (alle 6 Targets) → `#451A66`.
- `element_in_corner` (info) – Möhre (Phone S. 3 unten links) und Melone (iPad S. 1 unten rechts) in der runden Store-Ecke → verschoben.
- `device_small` (info, play-phone S. 6) – Sammlungs-Phone nur 46 % der Seite → in der Variante nicht verkleinert.
- `seam_overlap` (info) – Melone über Naht 1|2 liegt auf iPad-Hochformat-Seite 2 über der unteren Rahmenecke des iPads. Gewollt → `allow: [seam_overlap]` an `t-melon`.
  Auf dem Phone-Canvas kam kein `seam_overlap` – dort liegt der Melonenteil auf Seite 2 neben dem Phone, nicht darauf (korrekt).
  Die Meldung nennt die Fläche (2976 / 4464 lu²) – gut zum Einschätzen.
- `screen_header_cut` an den drei oben angeschnittenen Koch-Geräten: diesmal per `allow` vorab gesetzt (wie in liquid-chrome bleibt offen, ob der Befund bei Anschnitt durch die
  **Seitenkante** überhaupt käme – ich habe es nicht ohne `allow` getestet).

**Nicht gemeldet (nur im Bild gesehen):**
- Früchte vor dem Gerät über UI: Aubergine lag auf Play-Tablet S. 4 auf den Koch-Buttons („Weiter“), Möhre auf dem „2 fehlende auf die Liste“-Button des hinteren Phones,
  Melone auf Tab-Leisten. Wie bekannt prüft Vitrine nur die Kopfzeile. Aubergine in der Play-Variante verschoben; Rest bewusst (nur Tab-Leiste/Rand).
- Play-Tablet S. 6: rahmenloses Phone (per `device_map` Android) lief rechts aus der Seite – kein `element_outside`, vermutlich weil `clip: page` gesetzt ist. Ein Hinweis
  „Gerät zu X % abgeschnitten“ (außer bei gewollt angeschnittenen Geräten) wäre nützlich. Per Override korrigiert.
- Leere Tablet-Screens: die Hochformat-iPad-Screens hatten anfangs große leere untere Hälften – kein MCP-Thema, aber ein „Screenshot hat großen einfarbigen Bereich“-Hinweis wäre hilfreich.

**Neue Funktionen:** `render_preview {view: gallery, background: dark}` + `max_edge: 2400` liefert eine gut lesbare Galerie; bei breiten Querformat-Galerien
reicht die Höhe (410–541 px) für Komposition, nicht für Details. `allow` pro Element: gut. `catalog_devices.frame` für die Geräteverhältnisse (Gerät-Oberkante → `at.y`).
Ein über die Naht ragendes **einzelnes** Bildelement (`clip: none`) passt bei jeder Lücke automatisch – kein Zwilling, keine Szene je Target nötig.

**Stolpersteine / Wünsche:**
- Mein Fehler: ein ungültiges `set_styles {styles: null}` in einem `apply_ops`-Block → ganzer Block abgelehnt (atomar, gut so); Fehlermeldung klar.
- `apply_ops` weiter nur inline (23–26 KB je Canvas). Wunsch unverändert: `apply_ops {file: …}`.
- Variante: Bereichs-Abbildung (y ab Kopfzone skalieren) weiter im Generator; ein Varianten-Feld `remap_y: {from: 470}` würde alle Positions-Overrides sparen.
- Bundled Inter nur 400/600: `weight: 700` im Text-Style wäre still auf 600 gefallen? (Ich habe vorher `catalog_fonts` gelesen und 600 gesetzt – ein Befund „Gewicht nicht verfügbar“ wäre hilfreich.)

## cutout-carnival (C04, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 3 Canvases × 6 Seiten, 6 Targets. Schreibaufrufe: `styles_update`, Strings + Phone (24 KB inline),
Phone-Fixes + Play-Variante (9 KB), Tablet (24 KB), Querformat (26 KB), danach 4 kleine Diff-Aufrufe (`diff_ops.py`). `render_preview` sofort,
`validate` (alle Targets × Sprachen, mit Pixelprüfung) wenige Sekunden, `export {pages: false, out_dir, html: false, wait_seconds: 120}` 12 Galerien in 12,8 s
ohne Hintergrund-Job. Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos (Querformat-Ordner), kein `allow`**. Leonardo nicht gebraucht.
Schrift per `npm pack @fontsource/dm-serif-display` (Regular + Italic, WOFF → TTF); nameID 1 ist dort schon „DM Serif Display“, Kursiv-Span (`italic: true`) nahm die Italic-TTF sofort.

**Unterbrechung:** Ein Server-Neustart stoppte den Lauf; laut Koordinator wartete ein Aufruf auf Freigabe. In meinem sichtbaren Verlauf hat KEIN Aufruf
auf eine Freigabe gewartet – alle kamen mit Ergebnis zurück (letzte vor der Meldung: Container-`Bash` `du -sh` über die gerenderten Screens). Danach weiter
nur mit Container-Bash, `device_commit_files`, `device_bash`, `vitrine-studio__*`; `project_open` fand Revision 2 unverändert vor.

**Neue Prüfungen – Befunde:**
- `page_blends_with_store` (info) – Creme `#FFEBC8` auf Seite 1/6 zu nah am weißen Store (alle 6 Targets) → `#FFE0AE`, weg.
- `seam_overlap` (info) – die Tänzerin über Naht 2|3 lag auf Seite 3 über der Koralle (1056 lu²). Richtig erkannt, Koralle verschoben statt `allow`.
- `device_small` (info, play-phone S. 6) – Werke-Phone 46 % → in der Play-Variante nicht verkleinert.
- **Geklärt (offene Frage aus liquid-chrome/fruit-pop):** `screen_header_cut` kommt NICHT, wenn ein Gerät nur durch die **obere Seitenkante** angeschnitten ist
  (Statusleiste außerhalb der Seite). Test: `allow` an den drei angeschnittenen Geräten entfernt → kein Befund. Das vorsorgliche `allow` ist also unnötig.
- Kein `low_contrast` trotz Akzentfarben auf Gelb/Rosa/Himmelblau – vorher selbst auf ≥ 4,5 : 1 gerechnet (DM Serif Display 400 ist dünner als die Grotesk-Vorlagen).

**Nicht gemeldet (nur im Bild gesehen):**
- Erster Wurf: rote Alge über Naht 1|2 lag auf Seite 2 auf dem **roten Panel** und verschwand fast – Farbgleichheit Form/Untergrund ist kein Befund
  (wäre ein Wunsch: „Element hebt sich kaum vom Untergrund ab“ für Bilder, analog `low_contrast`).
- Springer hinter Geräten (z 2) waren auf der Nachbarseite fast unsichtbar – kein MCP-Thema; Springer jetzt vor den Geräten (z 8), nur auf Rahmen/Rand.
- Formen über Screen-Inhalt prüft Vitrine wie bekannt nicht; im Bild kontrolliert (nur Rahmenkanten und Tab-Leisten berührt).
- Leere Bildschirmhälften in den Android-Tablet-Screens (1600×2560 ist schlanker als das iPad) – erst im Play-Galeriebild sichtbar; Screens neu gebaut.

**Rendering-Beobachtungen:**
- SVG mit `feTurbulence` + `feColorMatrix` + `feComposite` wird korrekt gerendert (Gouache-Struktur in den Formen) – gut für Papier-/Malstile ohne Leonardo.
- `fill: {kind: image, fit: tile}` mit transparentem PNG als zweites `background` je Seite (z −29) funktioniert als Papierkörnung über der Grundfarbe.
  In der verkleinerten Galerie wirkt die Körnung deutlich gröber als im Einzelbild (Moiré durch Verkleinern) – Körnung leiser wählen, als das Einzelbild verlangt.
- Galerie-PNGs werden mit Körnung groß (Play-Tablet quer 10,9 MB, gesamt `preview/` 67 MB) – bei Rauschtexturen die Vorschau ggf. als JPEG anbieten.
- Überschriebene Assets (gleicher Name, per `tar -xOf … > ziel`) übernahm Vitrine sofort.

**Wünsche:**
- Weiter `apply_ops {file: …}` (Canvases 24–26 KB inline).
- Ein Element-Typ „Pfad“ oder ein `tint` für Bild-Assets würde die 27 einfarbigen SVG-Kopien (Form × Farbe) sparen.
- Varianten-Bereichsabbildung (`remap_y`) wie in den Befunden zuvor – hier wieder im Generator gerechnet.

## trading-card (C05, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 2 Canvases × 6 Seiten (phone, tablet-land), 4 Targets. Schreibaufrufe: `styles_update` (5×, klein),
Strings (8 KB), Phone in 2 Blöcken (20 + 9 KB), Play-Variante (18 KB), Querformat in 2 Blöcken (13 + 16 KB), Play-Quer (21 KB), danach 5 Diff-Aufrufe
(1–21 KB; einer groß, weil `allow` + neue Namensbreiten auf beiden Canvases und Varianten). `render_preview` sofort, `validate` (alle Targets × Sprachen,
Pixelprüfung) wenige Sekunden, `export {pages: false, out_dir, html: false, wait_seconds: 120}` 8 Galerien in 13,8 s ohne Hintergrund-Job.
Keine Freigabe nötig, kein Abbruch. Schriften per `npm pack @fontsource/anton @fontsource/barlow-condensed` (WOFF → TTF, nameID 16/17), sofort erkannt.
Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner).

**Neue Prüfungen – echte Befunde (alle im Bild bestätigt, behoben):**
- `low_contrast` auf der Kontrastseite: Headline unten lag über den Kartenrückseiten an den Nähten 3|4 / 4|5 (1,8 : 1). Richtig – Rückseiten dort entfernt.
- `low_contrast` (4,1–4,4 : 1) an Stat-Labels in Blau/Violett auf Tinte → hellere Label-Farben je Team (`KEYCOL`) und deckende Boxen.
- `low_contrast` (3,4–3,8 : 1) an der Fußzeile der Querformat-Karten: Der **Geräteschatten des iPads** dunkelte die blaue/violette Fußzeile ab.
  Nur im 2752-px-Ausschnitt sichtbar – ohne Vitrine hätte ich das nie gefunden. Fix: kleinerer Schatten `devsm` für iPads in Karten.
- `low_contrast` (4,3 : 1) am roten Headline-Akzent im **Querformat** (52 lu = keine große Schrift), am Phone (118 lu) unauffällig → Span heller.
- `text_clipped` am Kartennamen der hinteren Karte (S. 3, Box ragte links aus der Seite) und `text_near_device` (Fußzeile der hinteren Karte unter dem vorderen Gerät) – beide richtig.
- `element_in_corner` (Play-Phone) an den Kartenrückseiten → in der Play-Variante 80 lu höher.
- `page_blends_with_store` (Play, #131314) → Seitenverlauf eine Stufe heller, danach weg.
- `device_small` (42–50 %): richtig gemessen, aber gewollt (Gerät = Kartenbild) → `allow: [device_small]` an allen Kartengeräten.

**Falsche bzw. zweifelhafte Alarme – wichtig für alle gedrehten Texte:**
- `low_contrast` 1,0–1,3 : 1 an **gedrehten** Kartennamen (Phone S. 3 „10K STEPS“, +7°; Querformat-Namen ±3–5°), obwohl der Text im Bild klar auf
  der Teamfarbe steht. Die Meldung verschwand, sobald ich die **Textbox schmaler** machte (740 → 600 bzw. 1080 → 760 Kartenmaß). Offenbar misst die Prüfung
  in der achsenparallelen Hülle (oder ungedrehten Box) der gedrehten Textbox – bei breiten Boxen reicht die bis in die schwarze Fensterkante. Wunsch:
  Messung nur unter den gedrehten Glyphen. Tipp: gedrehte Texte so schmal wie möglich (Textbreite + Rand), dann gibt es keine Scheinbefunde.
- Ebenso Mini-Karten-Etiketten (4,2–4,4 : 1) – nach schmalerer Box weg.

**Nicht gemeldet (nur im Bild geprüft):**
- Ob Karten über dem wichtigen Screen-Inhalt liegen: hier unkritisch, weil das Gerät *in* der Karte liegt; die hintere Karte auf S. 3 zeigt nur Kopf und Route.
- Teamfarbe auf gleicher Farbe: Die türkise Rückseite lag anfangs auf S. 4 unter der Headline – kam als `low_contrast`, gut. Gleiche Farbe Karte/Rückseite
  benachbart (Gold-Rückseite neben roter Karte) bewusst unterschiedlich gewählt.
- Geräte über dem Seitenrand: keine (Karten ragen nur 10–30 lu über den Rand, Geräte bleiben innen; `clip: page` an Geräten).
- `seam_overlap` / `element_outside` an den Rückseiten kamen gar nicht – vermutlich weil ich `allow` vorab gesetzt hatte (nicht ohne getestet).

**Neue Funktionen:** `render_preview {background: dark}` gut; `render_preview {out_file, inline: false}` + `device_stage_files` war der schnellste Weg,
einen Bildausschnitt in voller Auflösung zu prüfen (Fußzeilen-Schatten). `catalog_galleries` für Lücken/Radien.
Ein einzelnes Element über der Naht (Rückseite, `clip: none`) passt bei jeder Lücke – bestätigt.

**Stolpersteine / Wünsche:**
- `apply_ops` weiter nur inline; mit ~120 Elementen je Canvas (Karte = 12 Elemente) kamen 50 KB je Canvas zusammen → in 2 Blöcke geteilt.
  Wunsch unverändert: `apply_ops {file: …}`.
- Eine **Gruppe mit Skalierung** (Karte = Gruppe, in der Play-Variante `scale` statt 12 Overrides) würde hier ~70 % der Varianten-Ops sparen.
  Gruppen-`rotation` gibt es, Gruppen-`scale` nicht – das wäre für alle „Objekt mit Innenleben“-Vorlagen (Karten, Tickets, Polaroids) der größte Gewinn.
- Bild-Element mit Maske/Loch: Die Folie liegt als eigenes Bild unter einem SVG-Rahmen mit Loch – funktioniert, aber ein `clip_to: <element>` wäre einfacher.

## pop-art-panels (D01, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 3 Canvases × 6 Seiten, 6 Targets. Schreibaufrufe: `styles_update`, Strings (4 KB), Phone-Basis (20 KB),
Phone-Play + Korrekturen (14 KB), Tablet-Basis (17 KB) + Play (14 KB), Quer-Basis (16 KB) + Play (16 KB), danach 6 kleine Diff-Aufrufe (`diff_ops.py`).
Basis und Play-Variante habe ich bewusst **getrennt** gesendet (erst Basis ansehen, dann Variante) – hielt jeden Aufruf unter 20 KB.
`render_preview` sofort, `validate` (alle Targets × Sprachen, Pixelprüfung) wenige Sekunden, `export {pages: false, out_dir, html: false, wait_seconds: 120}`
12 Galerien in 18,8 s ohne Hintergrund-Job. Keine Freigabe nötig, kein Abbruch. Leonardo 6 × 0,012 $ = 0,072 $.
Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner).

**Neue Technik – Bild auf ein Panel zuschneiden ohne Clip-Funktion:** Freisteller-Gesicht (z −25) liegt zwischen dem Panel-Hintergrund (`background`, z −30)
und einem **Overlay-`background` (z −20) = dieselbe Seite mit Loch im Gesichts-Panel** (SVG `clipPath` evenodd). Das Gesicht wird exakt auf die schräge
Panelform zugeschnitten und bleibt ein austauschbares Bild-Element. Wichtig: Freisteller-Bildkanten, an denen das Motiv abgeschnitten ist (Haare oben,
Schultern unten), müssen außerhalb des Panels liegen – sonst sieht man eine gerade Kante (im ersten Wurf auf Play-Phone S. 4 passiert, im Bild gefunden,
nicht von Vitrine gemeldet). Lösung: Gesicht beginnt über der Panel-Oberkante; im Querformat Platzierung über die Augenhöhe je Figur.
Wunsch weiterhin: `clip_to: <element|polygon>` für Bilder – das Overlay braucht je Seite und Play-Höhe ein zweites SVG (hier 36 Overlays).

**Neue Prüfungen – Befunde:**
- `page_not_covered` (info) an 7 Gesichtsbildern im Querformat: **falscher Alarm** – Vitrine hält ein fast seitengroßes Bild für einen Seitenhintergrund mit
  Lücke, obwohl darunter ein vollflächiges `background` liegt. → `allow: [page_not_covered]` an den Gesichtern. Wunsch: Befund nur, wenn unter der Lücke
  tatsächlich die Canvas-Farbe sichtbar ist (gerendert prüfen).
- `element_in_corner` (info) an `face-date` (Play-Phone, Play-Tablet): **falscher Alarm** – die Bildecke ist vom Overlay verdeckt, sichtbar ist nur die
  runde Panel-Ecke. Wunsch: Ecken-Prüfung auf gerenderte, sichtbare Pixel des Elements beschränken (wie bei `low_contrast`). → `allow: [element_in_corner]`.
- `device_small` (info, Play-Phone 49 %, Play-Tablet 48 %, Seite 6): richtig – Matches-Phone in den Play-Varianten vergrößert statt `allow`.
- Kein `low_contrast`: Texte stehen nur in deckenden Kästen/Blasen (vorher Akzentrot `#B3121B` auf Gelb 5,0 : 1 gerechnet).
- `screen_header_covered` kam nicht, obwohl Gedankenblasen im ersten Wurf die linke obere Ecke des Phone-Screens (Logo-Zeile) berührten – wohl unter der
  Schwelle. Im Bild gesehen und Blasen 80–90 lu höher gesetzt.

**Nicht gemeldet (nur im Bild gesehen):**
- Gerade Bildkante des Freistellers innerhalb des Panels (s. o.) – für Vitrine ist das ein normales Bild; ein Hinweis „Bild mit deckender Kante endet sichtbar
  im Seiteninneren“ wäre für Freisteller nützlich.
- Gedankenblasen auf Screen-Inhalt (Profilfoto, Logo-Zeile) – bekannt: nur die Kopfzeile wird geprüft.
- Falsche Augenhöhe im Querformat-Streifen (Felix: Nase statt Augen) – kein MCP-Thema, über `EYE` je Figur gelöst.
- `seam_*` meldete nichts – korrekt, die kreuzenden Panels sind je Seite eigene SVGs mit waagerechter Reihengrenze an der Naht.

**Neue Funktionen:** `render_preview {view: gallery, background: dark}` und Einzelbild-Ansicht reichten für alle Prüfungen; `out_file` nicht gebraucht.
`export {pages: false}` schnell und zuverlässig. Projekt-Schrift (Bangers aus comic-burst) sofort erkannt.

**Stolpersteine / Wünsche:**
- Eigener Fehler: Geräte-Faktoren der Play-Varianten mit Präfix-IDs (`t-phone-matches`) statt Layout-Namen nachgeschlagen → wirkten nicht; erst durch
  `device_small` aufgefallen. Gute Absicherung durch die Prüfung.
- `apply_ops` weiter nur inline; Wunsch unverändert `apply_ops {file: …}`.
- Ein Platzhalter im Asset-Pfad je Variante (`ov-{variant}-{page}.svg`) würde hier 72 `fill`-Overrides sparen.
- Screens: 256-Farben-Quantisierung (Floyd-Steinberg) halbiert die Screenshot-Ordner (77 → 28 MB) ohne sichtbaren Verlust in den Galerien – Tipp für alle,
  die das 10-MB-Commit-Limit treffen.

## sunday-strip (D02, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 2 Canvases × 6 Seiten (phone, tablet-land), 4 Targets. Schreibaufrufe: `styles_update`, Strings (68 `set_string`),
Phone-Basis (~20 KB), Phone-Play-Overrides (~11 KB), Quer-Basis (~24 KB), Quer-Play (~11 KB), danach 5 kleine Korrekturen (1–3 KB, `split_ops.py`: Diff der
Basis + Overrides). Basis und Play-Variante getrennt gesendet (erst Basis ansehen). `render_preview` sofort, `validate` (alle Targets × Sprachen, Pixelprüfung)
wenige Sekunden, `export {pages: false, out_dir, html: false, wait_seconds: 120}` 8 Galerien in 11,7 s ohne Hintergrund-Job. Keine Freigabe nötig, kein Abbruch.
Schriften per `npm pack @fontsource/comic-neue @fontsource/patrick-hand` (WOFF → TTF, nameID 1/2/16/17 gesetzt) – `catalog_fonts` erkannte sofort
„Comic Neue“ und „Patrick Hand“ (Patrick Hand ist auf dem Mac zusätzlich installiert, die Projektschrift hat Vorrang). Leonardo 9 × 0,0194 $ = 0,1746 $.
Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner), **kein `allow`**.

**Prüfungen:** Diesmal kam **schon beim ersten `validate` kein einziger Befund** – Kontraste vorher gerechnet (Akzent `#A8321C` 5,4 : 1 auf Papier), Texte nur
auf Papier bzw. in deckenden Blasen, nichts über Nähten, Figuren innerhalb der Panels. Deshalb wenig über die Prüfungen gelernt; alle Korrekturen kamen aus dem Bild:
- **Geräte über dem Seitenrand (nicht gemeldet):** gekippte Phones (600 lu, ±4–5°) ragten auf S. 1, 2, 5, 6 bis zu 30 lu über die linke/rechte Seitenkante, das
  vordere Phone auf S. 3 deutlich. Kein `element_outside`, vermutlich wegen `clip: page`. Selbst gerechnet (halbe gedrehte Breite = w/2·cos + h/2·sin) und verschoben.
  Wunsch (wie fruit-pop): Info „Gerät seitlich zu N lu angeschnitten“, außer bei Anschnitt an Ober-/Unterkante.
- **Figur hinter dem Gerät (nicht gemeldet):** Auf Quer-S. 6 verschwand Wendy (z 3) komplett hinter dem iPad (z 5). Ein Hinweis „Bild zu > 90 % von einem
  Element mit höherem z verdeckt“ wäre nützlich.
- **Figuren über wichtigem Screen-Inhalt (nicht gemeldet, bekannt):** Pip lag auf S. 6 über dem Sonntagsplan-Zettel des Home-Screens → kleiner/weiter links.
  Bewusst belassen: Figuren vor der Seitenleiste der iPad-Screens bzw. vor dem Rand der Listen.
- **Bild an Panelrand (nicht gemeldet):** Schlafender Hund ragte im Querformat über die Kante der Feature-Wand – gewollt erlaubt wäre ok, hier verschoben.
- `screen_header_covered` kam nie – richtig, Blasen und Figuren liegen unter der Kopfzeile.

**Technik / Erfahrungen:**
- **Panel-relative Spezifikation** (u, v im Panel, Größe als Anteil der Panelhöhe) → Basis und Play-Variante aus denselben Angaben; die Overrides fallen automatisch
  ab. Ein Varianten-Feld „Position relativ zu einem Rechteck/Panel“ würde das in Vitrine selbst lösen.
- **Pose-Bögen in Leonardo:** Eine Figur je Bogen (4–6 Posen) bleibt in sich konsistent; Bögen mit mehreren Figuren mischen Stile und Figuren (2 von 9 verworfen).
  Freistellen per Rand-Flutung + Komponenten-Trennung; Löcher-Entfernung abgeschaltet (sonst verschwanden weiße Augen und Shirts).
- **Druck-Look per Skript** statt per Prompt (Tusche-Ebene, Farbebene mit Raster und Versatz) vereinheitlicht leicht unterschiedliche Leonardo-Stile.
- `render_preview` liefert bei breiten Quer-Galerien nur 412–541 px Höhe; Details nur über `view: image` je Seite – reicht, aber ein `pages: [a, b]` für Galerie-Ausschnitte wäre praktisch.
- `export` überspringt unveränderte Galerien (4 written, 4 unchanged) – angenehm schnell bei kleinen Korrekturen.

**Wünsche:** `apply_ops {file: …}` (weiter 20–24 KB inline je Canvas); Element-Typ „Sprechblase“ oder Span-`padding`/`radius` (hier wieder Bild + Text je Blase,
Größe vorab mit der TTF gemessen); Clip eines Bildes auf ein Panel/Rechteck (Figuren mussten komplett im Panel stehen); Hinweis auf seitlich angeschnittene Geräte.

## hero-landing (D03, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 3 Canvases × 6 Seiten, 6 Targets. Schreibaufrufe: `styles_update` (2×), Strings + Phone-Basis (18 KB),
Phone-Play (10 KB), Tablet-Basis (15 KB), Play-Tablet (11 KB), Quer-Basis (14 KB), Play-Quer (11 KB), danach ~10 Diff-Aufrufe (1–5 KB, `diff_ops.py`).
Basis und Play getrennt gesendet. `render_preview` sofort, `validate` (alle 6 Targets × 2 Sprachen, Pixelprüfung) wenige Sekunden, `export {pages: false,
out_dir, html: false, wait_seconds: 120}` 12 Galerien in 26,5 s ohne Hintergrund-Job. Keine Freigabe nötig, kein Abbruch, Vitrine antwortete immer.
Schriften: Bangers aus comic-burst, Barlow Condensed per `npm pack @fontsource/barlow-condensed` (WOFF → TTF, nameID 16/17) – sofort erkannt.
Leonardo 7 × (0,009–0,012 $) = 0,078 $. Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner), ein `allow` (`device_small`).

**Neue Prüfungen – echte Befunde (im Bild bestätigt, behoben):**
- `low_contrast` an allen Lautmalereien (1,0–1,9 : 1): gelber Text auf dem gelben/cremefarbenen Kern der Explosion. Richtig. Fix: Explosion mit **dunklem Kern**
  (Karmin/Pflaume/Petrol/Tusche) und Text ≤ 42 % der Burst-Breite.
- Danach `low_contrast` **genau 3,0 : 1** an allen Lautmalereien: gemessen wurde gegen den **eigenen harten Textschatten** (#E2384A, Versatz 9/10). Befund
  formal richtig (Glyphenkante liegt neben dem Schatten), aber überraschend – Tipp: Versatzschatten von Text immer dunkel wählen (#4A0612 → weg).
  Wunsch: eigenen Schatten/eigene Kontur des Textes nicht als „Hintergrund“ werten oder im Befund sagen, dass es der eigene Schatten ist.
- `low_contrast` (2,9 : 1) am Akzentrot der Kontrastseite (#9C0F24 auf Orange mit Tusche-Speed-Lines) → #7A0A1C, Speed-Lines heller.
- `low_contrast` (4,3–4,4 : 1) an Eyebrows auf Petrol-Panels (Gelb auf Petrol + weiße Speed-Lines) → Petrol dunkler bzw. Panelfarben getauscht.
- `low_contrast` (3,4 : 1) an der Quer-Headline der Kontrastseite: die **Beine der Heldin** (Bild, z 7) lagen hinter der roten Zeile. Sehr hilfreich – im kleinen
  Galeriebild nicht zu sehen.
- `element_in_corner` an drei Freisteller-Heldinnen (Fuß in der runden Store-Ecke) → nach innen versetzt. Diesmal echte Befunde (sichtbare Pixel in der Ecke).
- `screen_header_covered` (26 %): Explosion „KRAK!“ lag im Querformat auf der Kopfzeile des iPads → verlegt.
- `device_small` (48 % / 39 %) am rahmenlosen Profil-Phone der Tablet-Feature-Wand → gewollt, `allow: [device_small]` (Seite = Feature-Wand).

**Kein Befund nötig (getestet):** Flug-Heldin + Speed-Trail über der Naht 1|2 (einzelne Bild-Elemente mit `clip: none`): ich hatte `allow: [seam_overlap,
seam_split, element_outside]` vorab gesetzt, testweise entfernt → **kein** Befund auf allen Targets. Vorab-`allow` für Nahtelemente ist unnötig, solange sie
auf der Nachbarseite nur über Hintergrund liegen.

**Nicht gemeldet (nur im Bild gesehen):**
- **Fuß der Tritt-Pose über der Crew-Liste** (Zahlen und Namen) im Boss-Screen – Formen/Bilder über wichtigem Screen-Inhalt prüft Vitrine weiterhin nicht
  (nur die Kopfzeile). Gelöst durch Spiegeln der Seite (Heldin links, Gerät rechts) und kleinere Figur.
- „WHAM!“-Explosion auf dem Tablet über „Hey, Sam!“ (Titel unterhalb der Statusleiste, nicht als Kopfzeile erkannt) → verlegt.
- **Gekippte Geräte seitlich über dem Seitenrand** (Play-Tablet: Android-Tablet 672 lu breit, +5°; rahmenloses Profil-Phone) – wie bekannt kein Befund; per
  Rechnung (halbe gedrehte Breite) und Bild korrigiert.
- Gleiche Farbe auf gleicher Farbe: gelbe Lautmalerei auf gelbem Burst wurde **doch** gemeldet (weil Text) – bei Bild-auf-Bild (Burst auf gelber Kontrastseite)
  natürlich nicht; dort Burst bewusst in Karmin/Orange.

**Erfahrungen / Technik:**
- Panels je Seite per shapely (Seite mit runden Ecken, Teilungslinien als gepufferte Rinne abgezogen, Farbe per Punkt-Zuordnung), Teilungen als Bruchteile der
  Seitenhöhe → Play-SVGs ohne Verzerrung aus derselben Spezifikation. Speed-Lines als Keile mit radialer Maske funktionieren im Vitrine-SVG-Renderer.
- Layout-Generator mit Faktoren je Variante (y × Höhenverhältnis, Geräte/Figur/Text eigene Faktoren) → Overrides fallen automatisch ab; Diff-Ops 1–5 KB.
- Leonardo: eine Pose je Bild, gemeinsame wortgleiche Figurenbeschreibung → Kostüm über 6 Bilder konsistent (Gesicht leicht schwankend). Die erste Fassung
  (weißes Haar, dunkle Haut, Trikot) erinnerte an eine bekannte Heldin → verworfen; Signatur-Kritzel in 1 von 7 Bildern.
- Screens: 100 PNGs, 256 Farben → 125 MB auf 18 MB; Übertragung als 2 Tar-Pakete < 10 MB, `tar -x` in leere Ordner ok, Überschreiben nur per `tar -xOf … > ziel`.

**Wünsche:** `apply_ops {file: …}` (weiter 10–18 KB inline je Block); Prüfung „Bild/Form überdeckt N % des Screen-Inhalts“ (nicht nur Kopfzeile);
Info „Gerät seitlich zu N lu angeschnitten“; eigener Textschatten nicht als Hintergrund im Kontrasttest; ein Bild-`asset`-Platzhalter je Variante
(`bg-{variant}-{page}.svg`) würde 36 `fill`-Overrides sparen.

## shonen-speed (D04, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 3 Canvases × 6 Seiten, 6 Targets. Schreibaufrufe: `set_styles` + Strings (6 KB), Phone-Basis (13 KB),
Play-Variante (9 KB), Tablet-Basis (14 KB) + Play (10 KB), Quer-Basis (14 KB) + Play (9 KB), danach 7 kleine Diff-Aufrufe (1–3 KB, `diff_ops.py`). Basis und
Play getrennt gesendet (Basis erst ansehen). `render_preview` sofort, `validate` (alle 6 Targets × 2 Sprachen, Pixelprüfung) wenige Sekunden,
`export {pages: false, out_dir, html: false, wait_seconds: 120}` 12 Galerien in 17,9 s ohne Hintergrund-Job (zweiter Lauf 1,7 s, 6 unverändert übersprungen).
Keine Freigabe nötig, kein Abbruch, Vitrine antwortete immer. Leonardo 6 × (0,009–0,012 $) = 0,063 $.
Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner), **kein `allow`**.

**Japanische Schrift / `styles.fonts.fallback` (neu, funktioniert):**
- fontsource liefert Zen Kaku Gothic New / Noto Sans JP in 120 Unicode-Stücken plus je einer Datei `…-japanese-<w>-normal.woff2` (≈ 6 900 Zeichen: Kana +
  gängige Kanji). `japanese` + `latin` + `latin-ext` mit `fontTools.merge.Merger` zu **einer TTF** zusammengeführt (2,1–2,8 MB), Name-Tabelle 1/2/4/6/16/17 gesetzt
  → `catalog_fonts` erkannte „Zen Kaku Gothic New“ und „Noto Sans JP“ sofort als Projektschriften.
- `styles.fonts` lässt sich **nicht** mit `styles_update` setzen (kein Feld `fonts`) – nur über `apply_ops {op: set_styles}` mit dem kompletten Styles-Objekt.
  Wunsch: `fonts` in `styles_update` (merge wie `text`).
- Fallback `Noto Sans JP` für Zen Kaku Gothic New **und** Inter gesetzt. Kana/Kanji (auch senkrecht als „ein Zeichen je Zeile“, vollbreites ！) wurden in allen
  Previews und Exporten korrekt gerendert; ob im Einzelfall Zen oder Noto zeichnet, sieht man nicht (beide haben die Zeichen). Ein Befund/Info
  „Zeichen X aus Fallback-Schrift Y“ wäre zur Kontrolle nützlich.
- Senkrechter Text (Tategaki) gibt es nicht – per `\n` zwischen jedem Zeichen gelöst (eigene String-Keys `kana.<seite>.v`). Wunsch: `writing_mode: vertical`.

**Neue Prüfungen – echte Befunde (alle im Bild bestätigt, behoben):**
- `low_contrast` (1,4–1,6 : 1) an Headline und Kana auf S. 6: der **helle Energie-Burst** am Fluchtpunkt lag hinter dem Text → Fluchtpunkt verlegt. Richtig, mit Ort.
- `low_contrast` (4,2 : 1) am cyanfarbenen Eyebrow über einer hellen Speedline (38 lu = keine „große Schrift“) → **dunkles `glow` im Text-Style** (`radius 5, spread 3`).
  Danach `low_contrast` 1,6 : 1 am **Tinten-Eyebrow der Kontrastseite**: gemessen gegen den eigenen dunklen Glow (wie der eigene Schatten in hero-landing) –
  Glow dort per Element-`glow {opacity: 0}` abgeschaltet. Wunsch wie gehabt: eigenen Schatten/Glow nicht als Hintergrund werten bzw. im Befund benennen.
- `text_clipped` an quer gesetzter Kana-Deko, die aus der Seite lief (S. 6, Querformat S. 2) → verschoben. Sinnvoll – ich hatte hier bewusst kein `allow` gesetzt.
- `text_near_device` (warn auf Play-Phone 2 %, info 2–14 lu) an Kana-Spalten und einer Querformat-Headline neben gekippten Geräten → verschoben.
- `seam_overlap` (info, 800 lu²): Fuß der Flug-Pose ragte auf Seite 1 über die Ecke des iPads (Querformat) → iPad kleiner/weiter links. Gut.
- `element_in_corner` (info): Schuh der Läuferin in der runden iPhone-Ecke → nach innen.
- `device_small` (info, Play-Tablet S. 6, 44 %): rahmenloses Profil-Phone → in der Play-Variante × 1,35 statt `allow` (vorher vorsorglich gesetztes `allow` entfernt,
  auf iPad kam ohne `allow` kein Befund).
- `page_blends_with_store` (info, Play-Tablet S. 6, #131314) → Randfarbe des Verlaufs eine Stufe heller.

**Nicht gemeldet (nur im Bild gesehen / selbst gerechnet):**
- **Gekippte Geräte seitlich über dem Seitenrand** (S. 1, 3, 5, 6 bis 43 lu) – wie bekannt kein Befund. Jetzt **im Generator automatisch** gelöst:
  halbe gedrehte Breite `w/2·cos + h/2·sin`, x auf `[hw+18, 1000−hw−18]` geklemmt. Wunsch weiterhin: Info „Gerät seitlich zu N lu angeschnitten“.
- **Figur über wichtigem Screen-Inhalt:** Läuferin deckt auf S. 1/3/5 jeweils das untere Viertel des Screens (Liste/Antwort C–D) – bewusst so gelassen
  (Kopf, Hero-Karte, Frage und richtige Antwort bleiben frei), aber nur im Bild prüfbar.
- **Burst-Kontinuität über die Naht 1|2:** kein `seam_*`-Befund (richtig, je Seite eigenes SVG). Gesichert über gleichen Seed + Fluchtpunkt bei `1000 + Lücke/2`
  bzw. `−Lücke/2` mit der Lücke je Target (45 / 71,1 / 40 / 22,5 lu aus `catalog_galleries`). Wunsch: `{gap}`-Platzhalter oder Canvas-weites SVG, das Vitrine
  pro Target mit der echten Lücke zerschneidet.
- Zwei Instanzen derselben Figur auf einer Seite (Läuferin S. 1 + Füße der Flug-Pose von S. 2) – Geschmacksfrage, kein MCP-Thema; Flug zu 75 % auf Seite 2 gelegt.

**Erfahrungen / Technik:**
- Energie-Aura per Abstandsfeld + gerichtetem, niederfrequentem Rauschen (`np.roll` entlang der Bewegungsrichtung), in 2 harte Cel-Bänder quantisiert –
  sieht deutlich mehr nach Anime aus als ein weicher Glow (erster Versuch: weich = „Nebel“).
- Freistellen mit Rand-Flutung über „hell ODER hellgrau ungesättigt“: nimmt Leonardos grauen Bodenschatten mit, Lineart stoppt die Flutung an weißen Schuhen/T-Shirt.
- Leonardo hielt das Figurendesign über 6 Einzelbilder stabil (wortgleiche Beschreibung); einziges Abweichen: Kopfhörer auf dem Kopf statt um den Hals (Flug).
- `render_preview {view: canvas}` war für breite Querformat-Sets nützlicher als `gallery` (Seitenbeschriftung + Nähte schraffiert).

**Wünsche:** `apply_ops {file: …}` (weiter 9–14 KB inline je Block); `fonts` in `styles_update`; `writing_mode: vertical` für CJK; Info zu seitlich
angeschnittenen Geräten; „Bild überdeckt N % des Screen-Inhalts“; Lücken-Platzhalter für nahtübergreifende Hintergründe.

## manga-tone (D05, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 2 Canvases × 6 Seiten (phone, tablet-land), 4 Targets. Schreibaufrufe: `set_styles` + Strings (6 KB),
Phone-Basis (18 KB), Phone-Play (12 KB), Querformat-Basis (18 KB), Quer-Play (13 KB), danach 6 kleine Diff-Aufrufe (1–9 KB, `diff_ops.py`). Basis und Play
getrennt gesendet. `render_preview` sofort, `validate` (alle Targets × Sprachen, Pixelprüfung) wenige Sekunden, `export {pages: false, out_dir, html: false}`
8 Galerien in 7,6 s ohne Hintergrund-Job. Keine Freigabe nötig, kein Abbruch, Vitrine antwortete immer. Leonardo 7 × 0,009 $ = 0,063 $.
Schrift per `npm pack @fontsource/m-plus-rounded-1c` (japanese + latin + latin-ext → eine TTF, `mkfont.py` aus shonen-speed mit „ExtraBold“ ergänzt),
Noto Sans JP aus shonen-speed kopiert – `catalog_fonts` erkannte beide sofort. Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner), **kein `allow`**.

**Prüfungen – echte Befunde (alle im Bild bestätigt, behoben):**
- `low_contrast` (warn, 1,6–2,1 : 1) am weißen Text im roten Stempel (よむ/つづく): Der Text war höher als der Stempel bzw. lief über dessen **weißen
  Innenrahmen**. Richtig. Fix: Stempelgröße aus Schriftgröße und Zeichenzahl, Innenrahmen entfernt.
- `low_contrast` (4,2 : 1) danach auf Play: Weiß auf Rot 5,2 : 1 rechnerisch, aber am Glyphenrand gegen Kantenglättung gemessen; bei 54 lu galt 4,5 : 1.
  Fix: Stempeltext ≥ 64 lu (fett ≥ 62 lu → 3 : 1). Die größenabhängige Schwelle steuert hier das Layout.
- `low_contrast` (3,5 : 1) am Rot der Nacht-Headline im Querformat (58 lu): Rot auf Schwarz reicht nur für große Schrift → Headline 72 lu (Play 63 lu).
- `page_blends_with_store` (info): Papier `#EDE8DD` am hellen Store, schwarze Nachtseite am dunklen Store → Papier `#E2DBCC`, Nachtränder `#3B3834`. Weg.
- `element_in_corner` (info): Halbfiguren mit geradem Schnitt unten standen in der runden Store-Ecke unten rechts. Richtig – Generator klemmt jetzt die
  deckende Unterkante (aus dem Alpha-Kanal gemessen) aus der Ecke.
- `screen_header_covered` (info, nur Play-Tablet quer, 26–54 %): Erzählkasten über der Kopfzeile des Android-Tablets, weil die Kästen in der niedrigeren
  Play-Seite nicht mitschrumpften. Richtig; Play-Geräte kleiner und 30 lu tiefer.
- `text_clipped` (warn): senkrechtes ドキドキ ragte im Querformat oben aus der Seite → verschoben.

**Nicht gemeldet (nur im Bild gesehen):**
- Figuren über dem unteren Screen-Rand (Mara über Eingabefeld/Tab-Leiste, Theo/Ava über Tab-Leiste) – wie bekannt nur Kopfzeile geprüft. Bewusst
  nur Tab-Leiste/Composer berührt.
- Gerade Bildkante eines Freistellers (Lenas Ellbogen am Bildrand von Leonardo abgeschnitten) – sichtbar als senkrechte Kante; hier auf die Rinne gelegt.
- Ob die Naht-Bänder an den Nähten zusammenpassen, prüft Vitrine nicht (je Seite eigenes SVG, keine `seam_*`-Meldung – korrekt). Gesichert über die
  globale Szene mit der Lücke je Target; im Galeriebild aller 4 Targets kontrolliert.
- Moiré: feine Rasterpunkte (15 lu Phone, 8 lu Querformat) erzeugen in verkleinerten `render_preview`-Galerien leichte Muster, im Einzelbild nicht.

**Erfahrungen / Technik:**
- Rasterfolie als SVG-Kreise in globalen Koordinaten (Seite als `translate(-X)` in die Szene) – gleiche Phase über jede Naht ohne Muster-Offsets;
  Vitrine rendert 700–900 KB große SVGs je Seite ohne spürbare Verzögerung (gesamt 13 MB, als tar.gz nur 1–3 MB → Commit-Limit kein Problem).
- Panel-Ecken: alle Zellen mit einer runden Seiten-Maske (Radius = Store-Radius − Rand) schneiden → keine Tusche-Linie wird von der Store-Ecke gekappt.
- `device_commit_files` überschreibt bestehende Dateien direkt (hanko.svg), ohne Löschrecht – angenehmer als `tar -xOf` für Einzeldateien.
- Senkrechter Text (ein Zeichen je Zeile) mit M PLUS Rounded 1c sauber; Langvokal „ー“ bewusst vermieden (steht senkrecht falsch) – Wunsch weiter
  `writing_mode: vertical`.

**Wünsche:** `apply_ops {file: …}` (weiter 12–18 KB inline je Block); Element-Typ „Sprechblase“ (hier wieder Bild + Text, Textgröße vorab gemessen);
`clip_to` für Bilder (Figuren an der Panelkante); Info „Bild überdeckt N % des Screen-Inhalts“; Hinweis auf Freisteller mit gerader Kante im Seiteninneren.

## pastel-slice (D06, Neubau)

**Ablauf:** `project_create` (minimal, en/de) sofort. 3 Canvases × 6 Seiten, 6 Targets. Schreibaufrufe: Setup (Canvases, Varianten, Targets, `set_styles`,
Strings – 1 Block, 11 KB), Phone-Basis (10 KB), Phone-Play (5 KB), Tablet-Basis (12 KB), Tablet-Play (5 KB), Quer-Basis (12 KB) + Quer-Play/Styles (8 KB),
danach 7 kleine Korrekturen. `render_preview` sofort, `validate` (alle 6 Targets × 2 Sprachen, Pixelprüfung) wenige Sekunden, `export {pages: false, out_dir,
html: false, wait_seconds: 120}` 12 Galerien in 14 s ohne Hintergrund-Job. Keine Freigabe nötig, Vitrine antwortete immer. Ein Server-Neustart (nicht Vitrine)
unterbrach zwei Leonardo-Aufrufe; danach normal weiter. Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner), **kein `allow`**.

**Neue Technik – Panorama als EIN Bild-Element je Canvas:** `scene` (image, page 0, `clip: none`, z −30, Breite = 6 Seiten + 5 Lücken + 80 lu, Höhe = Seitenhöhe,
`fit: stretch`). Vitrine legt die Seiten mit der Lücke des Targets darüber und schneidet die Lücken heraus → die Szene (Masten, Leitungen, Straße, Häuser)
läuft an **jeder** Naht exakt weiter, ohne 36 Seiten-SVGs. Play-Varianten: ein Override `{asset, width, height, at}` mit eigenem Bild (andere Lücke und
Seitenhöhe). Viel einfacher als die Seiten-Ausschnitte aus shonen-speed/manga-tone. Wichtig: `at.y` von unten verankerten Elementen in Play-Overrides
gilt für die **Canvas**-Seitenhöhe (Vitrine addiert Δ danach) – erster Wurf hatte die Play-Seitenhöhe eingesetzt, im Generator korrigiert.

**Getestet: `allow` für das Panorama ist unnötig.** Ich hatte `page_not_covered, element_in_corner, element_outside, seam_overlap, seam_split` vorab gesetzt
(Fehlalarm-Warnung aus pop-art-panels), danach auf allen drei Canvases entfernt → **kein einziger Befund**. Ein seitenbreites Hintergrundbild unter allem
löst also keine Fehlalarme aus (anders als die Gesichter in pop-art-panels, die nur fast seitengroß waren).

**Prüfungen – echte Befunde (alle im Bild bestätigt, behoben):**
- `low_contrast` an der Kontrastseite 6 (Creme auf Dämmerung, 2,1–3,1 : 1): der Tageszeit-Verlauf war links auf Seite 6 noch zu hell → Dämmerung früher
  (Übergang liegt jetzt fast ganz in der Naht 5|6 + leichter dunkler Glow im Style).
- `low_contrast` an senkrechter Kana-Deko (またあした über rosa Horizont 1,3 : 1; ありがとう auf Play über dem Rahmen des hinteren Phones 1,7 : 1, dazu
  `text_near_device`) → verschoben bzw. ≥ 62 lu (fett → 3 : 1). Die größenabhängige Schwelle steuert auch hier das Layout.
- **Querformat:** alle Headlines < 62 lu → 4,5 : 1 nötig. Auf Play quer (Seite nur 562 lu hoch) ragten Bäume/Häuser der Szene in die Textspalte
  (1,6–4,1 : 1 an 5 Headlines und 2 Kana) – sehr nützlich, im kleinen Galeriebild kaum zu sehen. Fix im Panorama: Play quer mit 12 statt 10 schmaleren
  Szenen (niedrigeres Band), Himmel im Querformat mit stärkerer Kurve (oben satter), Kana in den freien Himmel. Zwei Rest-Befunde 4,0–4,4 : 1 per Element-`glow`.
- Keine `device_small`, `page_blends_with_store`, `element_in_corner`, `screen_header_*` – die ruhige Komposition hat davon nichts ausgelöst.

**Nicht gemeldet (nur im Bild gesehen):**
- Die Figur vor dem Café (S. 1) verschwindet fast ganz hinter dem Phone – wie in sunday-strip: ein Hinweis „Bild zu > 90 % verdeckt“ wäre nützlich. Hier
  bewusst belassen, die Figur ist auf S. 4 (Bank) und S. 5 (Dachterrasse) sichtbar; bei Bildern *im* Panorama kann Vitrine das ohnehin nicht wissen.
- Ob die Szene hinter Text unruhig ist, merkt `low_contrast` nur, wenn es den Kontrast drückt – Leitungen quer durch eine Headline wären „ok“. Hier durch
  Layout vermieden (Headlines liegen im Himmel über den Masten).
- Gekippte Geräte seitlich: im Generator geklemmt (halbe gedrehte Breite), wie in shonen-speed.

**Stolperstein (wichtig für alle): `device_commit_files` schrieb einmal veraltete Inhalte.** Ich hatte zwei Panorama-JPGs im Container neu erzeugt und unter
**demselben `stagedPath`** erneut committet – Antwort „written“, aber auf dem Mac lag eine ältere Fassung (md5 geprüft; `validate` meldete exakt die alten
Werte). Mit **neuem Dateinamen** im Container committet und per `cat > ziel` umkopiert → korrekt. Tipp: bei wiederholten Commits derselben Datei immer
einen neuen Staging-Namen verwenden und `md5sum` auf beiden Seiten vergleichen.

**Erfahrungen / Technik:**
- Leonardo liefert für „frontal elevation, buildings fill the full width, stand on a sidewalk at the bottom, plain flat pale cream background above the rooftops“
  sehr gut stückelbare Straßenansichten im Anime-Hintergrundstil. „isolated in the centre“ gibt dagegen winzige Vignetten (verworfen). Flussufer wurden
  zweimal perspektivisch; erst „orthographic frontal elevation like a flat theatre backdrop“ half.
- Himmel freistellen: Flutung von oben **plus** eingeschlossene Himmelsstücke zwischen Leitungen (Komponenten mit mittlerem Abstand < 7 zur Himmelfarbe),
  sonst bleiben cremefarbene Inseln; cremefarbene Hauswände (Abstand ~24) bleiben dabei stehen. Kantenfarbe entmischt (kein Creme-Saum vor Blau).
- Masten + durchhängende Leitungen in globalen Koordinaten verdecken die Übergänge zwischen den Leonardo-Szenen und verbinden die Seiten – der eigentliche
  „Panorama-Kleber“. Szenen-Übergänge liegen bei Phone/Tablet in bzw. neben den Store-Lücken.
- Pseudo-Schrift auf Schildern (5 von 10 Szenen) per Inpainting (OpenCV Telea) in Rechtecken entfernt.

**Wünsche:** `apply_ops {file: …}` (weiter 10–12 KB inline je Block); Info „Bild zu N % verdeckt“; Info „Element liegt über unruhigem Bildbereich hinter Text“
(Varianz statt nur Kontrast); im Commit-Tool ein Inhalts-Hash in der Antwort.

## paper-calm (B06, Überarbeitung 13.11.2026)

**Ablauf:** `project_open` auf das bestehende Projekt (Rev. 7), dann 1 Aufruf Styles + 6 Strings (`set_styles`), je Canvas EIN `apply_ops`
(Phone 27 KB, Querformat 29 KB inline: `remove_element` der alten Elemente + `update_element` der Seitenhintergründe + `add_element` +
`set_variant_override` für die bestehenden Varianten `play`/`play-land`), danach 3 kleine Diff-Aufrufe (`diffops.py`). `remove_element`
räumt die Varianten-Overrides der entfernten Elemente offenbar mit auf (keine Fehlermeldung, keine Reste im Ergebnis). `render_preview`
sofort, `validate` (4 Targets × 2 Sprachen, Pixelprüfung) wenige Sekunden, `export {pages: false, out_dir, html: false}` 8 Galerien in 9,7 s.
Keine Freigabe nötig, Vitrine antwortete immer. Leonardo nicht verwendet (0 $). Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner).

**Unterbrechung:** Ein Server-Neustart (nicht Vitrine) unterbrach den Lauf zwischen dem Styles-Aufruf (Rev. 9) und dem Phone-Aufruf.
Danach meldete `project_open` Rev. 11 mit unverändertem Element-Stand (Styles schon neu) – zwei Revisionen ohne sichtbare Änderung,
vermutlich Speichern beim Öffnen/Schließen. Kein Datenverlust; Phone-Aufruf danach normal.

**Naht ohne Zwilling (Kernpunkt der Überarbeitung):** Das alte Häkchen-Kästchen war ein Zwilling (`seam-a` auf S. 1 bei x 1000, `seam-b` auf
S. 2 bei x 0, je `clip: page`) – das passt nur, wenn die Lücke 0 wäre; bei echter Store-Lücke fehlt kein Streifen, sondern die Hälften
sitzen um die Lückenbreite versetzt (Thorstens Befund). Jetzt Blatt + Kästchen + Häkchen je EIN Element auf S. 1 mit `clip: none`:
Vitrine schneidet die Lücke jedes Targets heraus, die Form läuft geometrisch korrekt weiter (iPhone 45 lu, Play, iPad, Play-Tablet geprüft).
`validate` meldete **kein** `seam_overlap`/`seam_split` – obwohl das Nahtblatt auf S. 2 hinter dem Phone liegt und das Kästchen über dem Blatt.
Bestätigt den Befund aus fruit-pop/hero-landing: einzelne Nahtelemente brauchen kein vorsorgliches `allow`. Wunsch: `validate` könnte
Zwillinge erkennen (zwei gleich große/gleich gedrehte Elemente bei x≈1000 und x≈0 auf Nachbarseiten mit `clip: page`) und `seam_split` o. ä. melden.

**Prüfungen – Befunde:**
- **Geändert gegenüber den bisherigen Befunden:** `screen_header_cut` (info) kam jetzt für die **oben durch die Seitenkante** angeschnittenen
  Geräte der blauen S. 4 (Phone: Schnitt 82 lu unter der Oberkante, Play 52 lu; iPad 24 / 20 lu). In cutout-carnival (12.11.) kam der Befund in
  genau diesem Fall noch NICHT, und in Fassung 1 dieser Vorlage auch nicht. Die Prüfung zählt den Seitenrand also inzwischen mit. Gewollt
  (nur Statusleiste + Zurück-Zeile weg, Titel sichtbar) → `allow: [screen_header_cut]` an `phone-project` und `l-ipad-project`.
- `text_near_device` (warn, Play-Phone DE, 12 % → 4 %): die längere DE-Unterzeile „Aufgaben diese Woche erledigt“ auf dem „15“-Blatt lief
  hinter das Logbuch-Phone. Richtig, nur auf Play (niedrigere Seite) und nur DE. Fix: Play-Phone tiefer, Blatt kleiner.
- `low_contrast` (warn, 4,4 : 1) an der Querformat-Headline S. 2: Blau auf Weiß wäre 6,6 : 1, aber die Kante + Schatten des Blattes lag
  unter der Akzentzeile. Richtig erkannt; Blatt nach links.
- **Kein** `page_blends_with_store` für die neue Tintenseite `#26324A` (gegen #000 bzw. #131314 nur ~1,6 : 1) – vermutlich, weil das
  große weiße Blatt genug Fläche abgrenzt. Im dunklen Galeriebild geprüft: die Seite hebt sich ab.

**Nicht gemeldet (nur im Bild/Rechnung):**
- Gekippte Geräte seitlich über dem Rand: im Generator per `clampx` (halbe gedrehte Breite) verhindert, im Bild kontrolliert.
- Gedrehter Text auf gedrehtem Blatt (große „15“, pivot um die Blattmitte): kein Scheinbefund `low_contrast` (Boxen schmal gehalten,
  Tipp aus trading-card befolgt).
- Weiß auf Weiß: Nahtblatt und Blattstapel überlappen sich auf S. 1 ohne Befund – nur durch die Schatten unterscheidbar; im Bild ok.

**Wünsche:** `apply_ops {file: …}` (weiter 27–29 KB inline); Info „Zwilling über der Naht“ (s. o.); im Befund `screen_header_cut`
unterscheiden, ob der Schnitt von der Seitenkante, einer Lücke oder einem Element kommt.

## notebook-grid (A09, Überarbeitung 13.11.2026)

**Ablauf:** Backup `cp -R notebook-grid notebook-grid-v1`, `project_open` (Rev. 9). Seitenbilder (36 JPGs) im Container neu gerendert
(`scene2.py`), als 3 Tar-Pakete < 10 MB mit frischen Namen committet, per `tar -xOf … > ziel` überschrieben, md5 aller 36 Dateien geprüft –
Vitrine übernahm sie sofort. Danach 4 kleine `apply_ops`/`element_update` (Rev. 10–14). `render_preview` sofort, `validate` wenige Sekunden,
`export {pages: false, html: false}` 12 Galerien in 18 s. Keine Freigabe nötig. Leonardo nicht verwendet (0 $).
Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner).

**`element_in_corner` hat das Problem NICHT gemeldet.** Die eckigen Papierblätter steckten in seitengroßen Hintergrundbildern
(`image`, 1000 × Seitenhöhe, z −30) – die Prüfung sieht nur ein seitenfüllendes Bild, nicht das Motiv darin. Auch Karten/Tapes, deren Ecke
genau auf dem Store-Radius lag, kamen nicht (offenbar nur Pixel, die die Store-Ecke tatsächlich abschneidet). Das Problem „eckige Form
innerhalb der runden Store-Ecke“ ist also nur im Bild bzw. per eigener Rechnung zu finden. Lösung hier: Papier mit Radius
`Store-Radius − Einbandbreite` (konzentrisch) je Target-Variante, Werte aus `catalog_galleries` (iOS basis width; Play basis height →
Radius 40 = 4 % der Seitenhöhe, d. h. 71 lu Phone/Tablet hoch, 22,5 lu quer), dazu ein Prüfskript (`check_corners.py`), das die gedrehten
Ecken aller Bild-/Form-Elemente gegen das runde Papier rechnet. Wunsch: ein Befund „Element hat eine eckige Kante nahe der runden
Store-Ecke“ bzw. eine Option `corner_inset`, die Inhalt gegen eine *eingerückte* Store-Ecke prüft (Rahmen-/Passepartout-Vorlagen).

**WICHTIG – `set_variant_override` ERSETZT das ganze Override eines Elements** (kein Merge): Ich hatte nur `{at}` gesendet, danach waren
`width`/`height` (Karten, Tapes) und `pivot` (gedrehte Karteikarte) der Play-Varianten weg – ohne Fehlermeldung. Gefunden über den
YAML-Vergleich mit dem Backup, repariert mit vollständigen Overrides (altes Override + neues `at`). Tipp für alle: vor
`set_variant_override` das bestehende Override per `element_get {target}` (`override`) lesen und komplett mitschicken – oder
`element_update {scope: variant}` verwenden (laut Doku Patch-Semantik; hier nicht getestet). Wunsch: `merge: true` oder ein Hinweis
in der Antwort, welche Felder entfallen sind.

**Neue Befunde gegenüber dem Bau am 12.11. (ohne Änderung am Projekt):** `validate` meldete jetzt `low_contrast` (2,5 : 1) und
`text_near_device` für die Randnotiz auf iPad S. 1 sowie `text_near_device` (13 lu, info) auf Play quer S. 2 – die Prüfungen sind
offenbar strenger/genauer geworden. Beide echt (Notiz berührte den iPad-Schatten), behoben.

**Sonst:** `export` mit `out_dir` + `cat` nach `preview/` zuverlässig (md5 gleich). `layout_report {page}` gab die Box der Karten
(Höhe aus Bildverhältnis) – nützlich für die Eckenrechnung.

## quiet-oak (B16, Überarbeitung 13.11.2026)

**Ablauf:** Backup `cp -R quiet-oak quiet-oak-v1`, `project_open` (Rev. 12). 6 Leonardo-Bilder ($0,078), Freistellen im Container, ein Tar < 2 MB
mit frischem Namen committet, per `tar -xOf … > ziel` verteilt, md5 geprüft. Je Canvas EIN `apply_ops` (11–12 KB: `remove_element` der 6 alten
Objekte + `update_element` aller Elemente + `add_element` der Requisiten + alle Play-Overrides vollständig), danach 5 kleine Diff-Aufrufe
(Rev. 13–21) und ein `styles_update`. `render_preview` sofort, `validate` wenige Sekunden, `export {pages: false, html: false}` 12 Galerien in 13 s.
Keine Freigabe nötig. Ergebnis: **0 Fehler, 0 Warnungen, 2 Infos** (Querformat-Ordner).

**Neue Befunde ohne Änderung an der Stelle (Prüfungen strenger als am 11.11.):**
- `seam_overlap` (info) für das **Regal-Panorama** (`shelf`, ein Bild über alle Seiten) auf Seite 2, weil es dort das hinter der Kante versenkte
  Gerät überdeckt – genau das ist die Idee. Am 11.11. kam der Befund nicht. Gewollt → `allow: [seam_overlap]` an den drei Regal-Elementen.
- `low_contrast` (warn, 4,5 : 1) am Eyebrow „WEEK“ (26 lu) – unverändert seit Fassung 1, jetzt gemeldet (vermutlich dunkle Washi-Faser genau
  unter dem Text). Fix: `muted` eine Stufe dunkler.
- `device_small` (info, Play-Phone S. 6, 47 %) und `text_near_device` (info, Play quer DE S. 2, 8 lu) – ebenfalls alt, jetzt gemeldet; behoben.

**Nahtelemente:** Bleistift (1|2) und Eichenzweig (3|4) sind je ein Bild mit `clip: none` – passt bei jeder Lücke. Der Bleistift bekam
`seam_overlap` (er liegt auf S. 2 vor dem versenkten Gerät), der Zweig nicht (liegt auf S. 4 neben dem Gerät) – korrekt unterschieden.

**`set_variant_override` ersetzt das ganze Override** (Befund notebook-grid) – hier von Anfang an berücksichtigt: Generator sendet je Element
das komplette Play-Override (`anchor_y`, `width`, `height`, `at`); Sonderpositionen je Variante über ein `vfix`-Feld im Generator. Kein Verlust.

**Nicht gemeldet (nur im Bild):** Requisit über Screen-Inhalt (Teeschale über den unteren Terminzeilen des Phones S. 1, Washi-Kärtchen über der
Tab-Leiste S. 5, Play-Tablet: Zweigblätter über der Tab-Leiste des Fokus-iPads). Wie bekannt prüft Vitrine nur die Kopfzeile. Wunsch wie bei
seed-to-harvest: Info „Bild deckt N % des Screens“ ab Schwelle. Maßstab Requisit ↔ Gerät prüft natürlich niemand – hier über cm je lu im Generator.

**Sonst:** `render_preview` mit `format` habe ich einmal versehentlich mit nacktem `png` aufgerufen (Client-JSON-Fehler, wie bekannt) – ohne
`format` problemlos. Leonardo-CDN-Download im Container braucht einen Browser-`User-Agent` (sonst HTTP 403).

## sunday-strip (D02, Überarbeitung 13.11.2026)

**Anlass:** Kleine „Bild fehlt“-Icons oben rechts in den Setup-Panels S. 3/5. **Ursache:** kein MCP-/Asset-Fehler, sondern eine in die
Panel-SVGs eingebackene Bilderrahmen-Requisite (Rechteck mit Bergzacken), die exakt wie das übliche Bild-Platzhalter-Symbol aussieht.
Durch Wanduhr ersetzt; 8 Panel-SVGs per `cat > ziel` im Projekt überschrieben (md5 geprüft), Vitrine übernahm sie sofort im nächsten
`render_preview` – kein `apply_ops` nötig, Revision blieb gleich. Generator lief im device_bash-VM (Python 3.10 + PIL) mit temporärer
`fig/`-Kopie und reproduzierte vorher alle 88 SVGs byte-gleich → sicherer Diff, nur 8 Dateien geändert.
**Prüfungen:** `validate` meldete das Icon natürlich nicht (ist gewolltes Motiv im Hintergrundbild) – nur im Bild auffindbar.
Tipp für alle: in Deko keine Motive verwenden, die wie UI-Systemsymbole wirken (Bild-Platzhalter, Warndreieck, Ladekreis, „kaputtes Bild“).
`export {pages: false}` 8 Galerien in 11,8 s; 0 Fehler, 0 Warnungen, 2 Infos.
