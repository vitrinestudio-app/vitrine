# Vitrine Studio – Konzept

> Website: vitrinestudio.app · Slug/Binary/Crate-Präfix: `vitrine`
>
> Open-Source-Desktop-Programm (GPL-3.0-or-later, kostenlos) zum Erstellen von App-Store- und Play-Store-Screenshots mit Device-Frames, Texten, Übersetzungen, Panorama-Sets, Store-Upload und MCP-Schnittstelle.
>
> Stand: November 2026 · Status: Entwurf

---

## 1. Ziel

Ein lokales, schnelles Werkzeug, mit dem man Store-Screenshots einmal gestaltet und dann für alle Sprachen, Geräteklassen und Stores automatisch rendert und hochlädt.

**Leitlinien**

1. **Einmal gestalten, überall ausgeben.** Ein Gerät, ein Text, ein Hintergrund existiert genau einmal, egal über wie viele Export-Bilder er läuft.
2. **Lokal und offen.** Projekt = Ordner mit lesbaren Textdateien und Bildern. Kein Konto, keine Cloud, Git-tauglich.
3. **Headless zuerst.** Alles, was die GUI kann, kann auch die CLI und der MCP-Server, weil alle drei dieselbe Operations-API im `core` benutzen.
4. **WYSIWYG über einen einzigen Renderer.** Vorschau und Export kommen aus derselben Rust-Render-Pipeline, damit Textumbrüche und Positionen in der Vorschau exakt dem Export entsprechen.
5. **Schnell.** Vorschau-Updates unter 50 ms pro Seite bei Vorschauauflösung, kompletter Export eines Projekts (5 Seiten × 3 Ziele × 10 Sprachen) in Sekunden, nicht Minuten.

**Bewusst nicht enthalten (Nicht-Ziele, vorerst)**

- App-Preview-Videos
- Screenshots aufnehmen (Simulator-/Geräte-Steuerung). Dafür gibt es fastlane snapshot, Xcode-UI-Tests usw.; Vitrine Studio importiert deren Ergebnisse.
- 3D-Perspektive à la Rotato – kommt erst als letzter, optionaler Schritt (Phase 7). Normale 2D-Rotation von Geräten und allen anderen Elementen ist dagegen von Anfang an enthalten.
- Cloud-Sync, Team-Funktionen, Konten

---

## 2. Kernidee: Leinwand statt Einzelbilder

Bisherige Tools denken in **Export-Bildern**: Jedes Bild ist ein Container, darauf liegen Gerät und Text. Läuft ein Gerät über zwei Bilder, muss es zweimal angelegt und ausgerichtet werden.

Vitrine Studio dreht das um:

```
Leinwand (Canvas) ─ ein durchgehendes Querformat
┌─────────────┬─────────────┬─────────────┬─────────────┬─────────────┐
│   Seite 1   │   Seite 2   │   Seite 3   │   Seite 4   │   Seite 5   │
│  ┌───────┐  │             │             │             │             │
│  │ Titel │  │  ┌───────┐  │             │   ┌─────────┴───┐         │
│  └───────┘  │  │ Titel │  │             │   │   Tablet    │         │
│        ┌────┼──┐└───────┘ │  ┌──────┐   │   │  (über 4+5) │         │
│        │ Handy │          │  │Laptop│   │   └─────────┬───┘         │
│        │(über1+2)         │  └──────┘   │             │             │
│        └────┼──┘          │             │             │             │
│  Hintergrund läuft durchgehend über alle Seiten ───────────────────▶│
└─────────────┴─────────────┴─────────────┴─────────────┴─────────────┘
```

- Die **Leinwand** ist die oberste Ebene. Hintergrund und Geräte liegen in Leinwand-Koordinaten und werden beim Export einfach in Seiten geschnitten.
- **Seiten** sind Ausschnitte der Leinwand mit festem Seitenverhältnis. Jede Seite wird zu einem Export-Bild. Zwischen den Seiten liegen Lücken wie in der Store-Galerie, damit Übergänge im Store optisch stimmen (3.3).
- Die ganze Leinwand oder ein Teil davon lässt sich zusätzlich als **Banner** exportieren, z. B. als Header für einen Blog-Post.
- **Texte** sind standardmäßig an eine Seite gebunden und werden dort mit Rändern (Margins), Ausrichtung und automatischer Größenanpassung gesetzt. Sie können aber auch frei auf der Leinwand liegen.
- **Geräte mischen** ist selbstverständlich: Jedes Gerät-Element hat sein eigenes Modell (Handy mit Home-Taste, Handy mit Dynamic Island, Tablet, Laptop …) und seine eigene Ausrichtung.
- **Mehrere Geräte pro Seite** ebenso: Elemente sind einfach Objekte auf der Leinwand.

### Begriffe

| Begriff | Bedeutung |
|---|---|
| **Projekt** | Ordner mit `project.yaml`, Übersetzungen, Assets. Enthält eine oder mehrere Leinwände. |
| **Leinwand (Canvas)** | Durchgehende Fläche aus *n* gleich großen Seiten. Typisch: eine Leinwand für Handys, eine für Tablets, eine für Mac. |
| **Seite (Page)** | Ein Ausschnitt der Leinwand. Normalerweise ein Export-Bild; bei Seitengruppierung bilden mehrere Seiten zusammen ein Export-Bild (3.4). |
| **Lücke (Gap)** | Abstand zwischen zwei Export-Bildern, wie ihn der Store bei der Anzeige einfügt. Inhalt in der Lücke wird in Seiten-Exports weggelassen. |
| **Heimatseite** | Die Seite, an der ein Element hängt. Positionen werden relativ zur Heimatseite gespeichert; das Element darf beliebig in Nachbarseiten hineinragen. |
| **Variante** | Abgeleitete Geometrie einer Leinwand für ein anderes Format (z. B. Tablet quer: 2 Hochformat-Seiten → 1 Querformat-Bild). Inhalte bleiben gemeinsam, nur Position/Größe/Rotation werden überschrieben. |
| **Element** | Hintergrund, Gerät, Text, Bild, Form. Liegt auf der Leinwand, optional an eine Seite gebunden. |
| **Asset** | Screenshot, Hintergrundbild, Logo, Schrift, eigener Device-Frame. Screenshots haben Varianten pro Sprache und Plattform. |
| **Ausgabeziel (Target)** | Rendert eine Leinwand in eine konkrete Pixelgröße, optional mit Gerätetausch, und ist optional einem Store-Slot zugeordnet. |
| **Sprache** | Sprachcode mit Fallback-Kette. |

---

## 3. Datenmodell

### 3.1 Projektordner

```
grove-eco.vitrine/
├── project.yaml              # Leinwände, Elemente, Stile, Ziele, Sprachen
├── i18n/
│   ├── en.yaml               # Texte pro Sprache (Schlüssel → Text)
│   ├── de.yaml
│   └── fr.yaml
├── screenshots/
│   ├── en/                   # Standardsprache (Fallback für alle anderen)
│   │   ├── ios-phone/home.png
│   │   ├── ios-phone/planner.png
│   │   └── android-phone/home.png
│   ├── de/
│   │   └── ios-phone/home.png   # nur wo eine Sprachvariante existiert
│   └── fr/ …
├── assets/                   # Hintergründe, Logos, Deko-Bilder
├── fonts/                    # projektlokale Schriften (für reproduzierbaren Export)
├── frames/                   # optionale eigene Frame-Pakete
├── export/                   # Ausgabe (gitignore)
└── .cache/                   # Thumbnails, Render-Cache, Undo-Historie (gitignore)
```

**Warum YAML-Dateien statt SQLite für das Projekt?** Das Projekt soll diffbar, versionierbar, von Hand und von Claude lesbar sein. SQLite wird nur für app-weite Daten (zuletzt geöffnete Projekte, Thumbnail-/Render-Cache, persistierte Undo-Historie) verwendet.

Auf macOS kann der Ordner später als Dokument-Paket registriert werden, sodass er im Finder wie eine Datei erscheint.

### 3.2 Koordinatensystem, Einheiten und Heimatseite

- Logische Einheit **lu**: Eine Seite ist **1000 lu breit**. Die Seitenhöhe ergibt sich aus dem Seitenverhältnis (z. B. 1320:2868 → 2172,7 lu).
- Jedes Element hat eine **Heimatseite** (`page`) und eine Position relativ zu deren linker oberer Ecke. Ein Gerät mit `page: 0` und `x: 1000` liegt mit seinem Mittelpunkt genau auf der rechten Kante von Seite 1 und ragt in Seite 2 hinein.
- Warum relativ zur Seite statt absolut auf der Leinwand? Lückenbreite (3.3) und Anordnung der Seiten (Gruppierung zu Querformat, 3.4) unterscheiden sich je nach Ziel. Weil jedes Element an seiner Heimatseite hängt, bleibt es an der richtigen Stelle, und Übergänge in die Nachbarseite stimmen automatisch.
- **Rotation** um beliebige Winkel ist für jedes Element von Anfang an enthalten (Drehpunkt standardmäßig in der Mitte, frei verschiebbar). Geräte haben zusätzlich die Ausrichtung portrait/landscape.
- Alle Größen (Schriftgrößen, Ränder, Schatten, Eckradien) sind in lu. Dadurch ist das Layout **unabhängig von der Exportauflösung**: Export = Skalierung von lu auf Pixel.
- Screenshots werden erst beim Rendern mit hochwertigem Resampling (Lanczos3) in die Zielgröße gebracht. Die Auflösung der Screenshots bestimmt nur die Qualität, nie das Layout. Wird ein Screenshot um mehr als einen Schwellwert hochskaliert, gibt es eine Warnung.

### 3.3 Lücken zwischen den Seiten (Store-Galerie)

Die Stores zeigen Screenshots mit Abständen nebeneinander. Schneidet man die Leinwand lückenlos in Bilder, wirken Geräte, Formen oder diagonale Linien, die über zwei Bilder laufen, im Store versetzt, weil die Lücke den Verlauf unterbricht.

Deshalb wird die Leinwand physisch **mit Lücken** aufgebaut, und der Inhalt in den Lücken wird in Seiten-Exports weggelassen. Beispiel mit 320 px breiten Seiten und 20 px Lücke:

```
ohne Lücke:  Seite 1 = x 1–320    Seite 2 = x 321–640    Seite 3 = x 641–960
mit Lücke:   Seite 1 = x 1–320    Seite 2 = x 341–660    Seite 3 = x 681–1000
                       x 321–340 und 661–680 liegen in Lücken und werden nicht exportiert
```

- Die Lückenbreite ist Teil des **Galerie-Profils** eines Ziels (App Store iPhone, App Store iPad, Mac App Store, Play Store …) und wird relativ zur Seitenbreite in lu angegeben. Stores, App und Website zeigen unterschiedliche Abstände; die Presets werden einmal an echten Store-Ansichten ausgemessen und sind pro Ziel überschreibbar. `0` schaltet Lücken ab.
- Da Elemente an ihrer Heimatseite hängen, verschiebt eine andere Lückenbreite nur den Ausschnitt der Nachbarseite; Übergänge bleiben stimmig.
- Hintergründe laufen durchgehend über Seiten **und** Lücken.
- Der Editor zeigt Lücken schraffiert, Inhalte darin halbtransparent, damit man sieht, was im Store „verschwindet“.
- Die Validierung warnt, wenn Text in einer Lücke liegt.

### 3.4 Ein Layout, mehrere Formate: Anker, Gruppierung, Varianten

Ausgabeziele haben nicht immer exakt dasselbe Seitenverhältnis wie die Leinwand. Beispiel: iPhone 6,9″ (1320 × 2868, ≈ 1 : 2,17) gegenüber Google Play (lange Seite höchstens doppelt so lang wie die kurze, also ≤ 1 : 2).

Lösung: Die Seitenbreite bleibt 1000 lu, die **Seitenhöhe passt sich dem Ziel an**. Jedes Element hat einen vertikalen Anker:

- `top`: Abstand zur Oberkante bleibt (typisch für Überschriften)
- `bottom`: Abstand zur Unterkante bleibt (typisch für Geräte, die unten aus dem Bild laufen)
- `center`: relative Position zur Mitte bleibt
- `stretch`: Element skaliert mit der Höhe (Hintergründe)

Damit lässt sich eine Leinwand für iOS und Android gemeinsam nutzen. Bei zu großer Abweichung (Schwellwert konfigurierbar) schlägt die Validierung eine Variante vor (siehe unten).

**Seitengruppierung: aus Hochformat wird Querformat.** Ein Ziel kann mehrere Seiten zu einem Export-Bild zusammenfassen (`group: 2`). Aus 6 Handy-Seiten werden so 3 Tablet-Querformat-Bilder. Jede ursprüngliche Seite wird zu einer **Spalte** des Querformat-Bildes: Ein Text mit Heimatseite 2 steht in der rechten Spalte von Bild 1, seine Ränder gelten relativ zur Spalte. Lücken gibt es dann nur zwischen den Querformat-Bildern, nicht zwischen den Spalten.

```
Handy hoch:   [ S1 ] ␣ [ S2 ] ␣ [ S3 ] ␣ [ S4 ] ␣ [ S5 ] ␣ [ S6 ]
iPad quer:    [ S1 | S2 ]  ␣  [ S3 | S4 ]  ␣  [ S5 | S6 ]
```

Rechnerisch passen zwei Hochformat-Seiten aber nicht direkt in ein Querformat: Zwei iPhone-Seiten nebeneinander ergeben etwa 0,92 : 1, ein iPad quer ist 4 : 3 (≈ 1,33 : 1). Eine Spalte ist also deutlich niedriger als die Handy-Seite (1000 × 1500 lu statt 1000 × 2173 lu). Dafür gibt es drei Mittel, die zusammenwirken:

1. **Anker** wie oben: Überschrift bleibt oben, Gerät unten.
2. **Inhalts-Skalierung** pro Variante (z. B. Geräte × 0,75), damit Geräte in die niedrigere Spalte passen.
3. **Varianten mit Overrides:** Für ein Format (z. B. `tablet-landscape`) kann jedes Element abweichende Position, Größe, Rotation oder Heimatseite bekommen. Alles andere – Screenshot-Zuordnung, Texte, Übersetzungen, Stile – bleibt gemeinsam und wird nur einmal gepflegt. Beim Anlegen wird eine Variante automatisch vorbelegt (Gruppierung + Anker + Skalierung), danach wird nur noch nachjustiert.

Zusammen mit dem Gerätetausch pro Ziel (`device_map`, 3.7) wird aus der Handy-Leinwand ohne doppelte Pflege ein iPad-Querformat-Set mit Tablet-Frames.

### 3.5 Elemente

Gemeinsame Eigenschaften: `id`, `kind`, `page` (Heimatseite), Position (`at`, relativ zur Heimatseite), Größe, `rotation` (Grad), `pivot` (Drehpunkt), `opacity`, `z` (Ebene), `anchor_y`, `clip` (`none` | `page`; Standard: Text `page`, alles andere `none`), `visible_in` (Liste von Zielen/Sprachen, optional).

| Typ | Spezifische Eigenschaften |
|---|---|
| **background** | Farbe, Verlauf (linear/radial, mehrere Stopps), Bild (cover/contain/tile, Position), auf der ganzen Leinwand oder pro Seite. Mehrere Hintergrund-Ebenen stapelbar. |
| **device** | Modell, Ausrichtung (portrait/landscape), Farbvariante, Screenshot-Schlüssel, Breite (Höhe folgt aus dem Modell), Schatten, Einpassung des Screenshots (`cover`/`contain`/`stretch`). |
| **text** | Übersetzungsschlüssel, Stil, Textbox (Ränder zur Seite oder freie Box), Ausrichtung horizontal/vertikal, Auto-Fit (`shrink_to` Mindestgröße), max. Zeilen, Inline-Formatierung. |
| **image** | Asset (PNG/JPEG/SVG), optional mit Sprachvarianten (z. B. Badges „Neu“/„New“). |
| **shape** | Rechteck/abgerundet, Kreis, Linie; Füllung, Kontur, Schatten. Für Hervorhebungen, Pfeile, Sprechblasen. |
| **group** | Gruppiert Elemente für gemeinsames Verschieben/Rotieren/Skalieren (`scale` um den Gruppen-Drehpunkt, auch als Varianten-Override; ADR 0030). |

**Text an Seiten binden.** Ein Text mit `page: 2` und `box: { top: 120, left: 80, right: 80 }` liegt auf Seite 3 mit 80 lu Rand links und rechts. Er wird an den Seitengrenzen beschnitten (`clip: page`) und kann nicht versehentlich in die Nachbarseite laufen. Geräte haben standardmäßig `clip: none` und laufen frei über Seitengrenzen.

**Inline-Formatierung** in Übersetzungen, bewusst minimal und übersetzerfreundlich:

```
Plane deine Beete **in Minuten**
Mit [accent]Mischkultur[/] zu mehr Ernte
```

`**…**` = fett, `_…_` = kursiv, `[stilname]…[/]` = benannter Span-Stil (Farbe, Gewicht, Hintergrund-Highlight). Zeilenumbruch mit `\n`, sonst automatischer Umbruch.

### 3.6 Stile

Projektweite Tokens, damit Änderungen zentral bleiben:

- **Farben**: benannte Farben (`primary`, `accent`, `text`), referenziert mit `@name`
- **Textstile**: Schrift, Gewicht, Größe, Zeilenhöhe, Laufweite, Farbe, Groß-/Kleinschreibung, Schatten, Kontur (Outline), Leuchten (Glow); Span-Stile können Kontur, Leuchten und Schatten eigens setzen
- **Schatten-Presets**: `soft`, `hard`, `none`
- Element-Eigenschaften überschreiben Stilwerte.

### 3.7 Beispiel `project.yaml`

```yaml
format: vitrine/1
name: grove.eco

languages:
  default: en
  available: [en, de, fr, es]
  fallback:            # zusätzlich zur automatischen Kette de-AT → de → default
    pt-BR: pt

styles:
  colors:
    primary: "#2F6B3A"
    accent:  "#E8A33D"
    text:    "#1D1D1F"
  text:
    headline: { font: "Inter", weight: 800, size: 72, line_height: 1.08, color: "@text" }
    sub:      { font: "Inter", weight: 500, size: 38, line_height: 1.25, color: "@text" }
  spans:
    accent:   { color: "@accent" }

canvases:
  - id: phone
    page: { aspect: "1320:2868", count: 5 }
    backgrounds:
      - kind: linear-gradient
        angle: 160
        stops: [["#EAF4E4", 0.0], ["#B7D9A6", 1.0]]   # läuft über alle 5 Seiten
    elements:
      - id: hero-phone
        kind: device
        model: iphone-island
        orientation: portrait
        screenshot: home              # Asset-Schlüssel
        page: 0                       # Heimatseite: Seite 1
        at: { x: 1000, y: 2172 }      # Mitte auf der rechten Kante von Seite 1 → ragt in Seite 2
        anchor_y: bottom
        width: 820
        rotation: -8
        shadow: soft

      - id: title-1
        kind: text
        page: 0
        text: hero.title              # Schlüssel in i18n/*.yaml
        style: headline
        box: { top: 140, left: 80, right: 80 }
        align: center
        anchor_y: top
        fit: { shrink_to: 52, max_lines: 3 }

    variants:
      tablet-landscape:               # für Ziele mit group: 2
        content_scale: 0.75
        overrides:
          hero-phone: { at: { x: 1000, y: 1500 }, rotation: -4 }
          title-1:    { box: { top: 90, left: 70, right: 70 } }

targets:
  - id: ios-phone-69
    canvas: phone
    size: [1320, 2868]
    screenshots: ios-phone            # welche Screenshot-Variante
    gallery: appstore-iphone          # Lücken-Preset
    store: { apple: { display_type: APP_IPHONE_67 } }   # gegen aktuelle API prüfen

  - id: android-phone
    canvas: phone
    size: [1080, 2160]                # ≤ 2:1, Anker passen Layout an
    screenshots: android-phone
    device_map: { iphone-island: android-punchhole }    # Gerätetausch
    gallery: play-phone
    store: { google: { image_type: phoneScreenshots } }

  - id: ios-ipad-13-landscape
    canvas: phone
    variant: tablet-landscape
    group: 2                          # 2 Handy-Seiten → 1 Querformat-Bild
    size: [2752, 2064]
    screenshots: ios-tablet
    device_map: { iphone-island: ipad-modern }
    gallery: appstore-ipad
    store: { apple: { display_type: APP_IPAD_PRO_3GEN_129 } }

  - id: blog-header
    kind: banner                      # ein durchgehendes Bild statt Einzelseiten
    canvas: phone
    region: { pages: [0, 2] }         # Seiten 1–3; alternativ freies Rechteck
    size: [2400, 1000]
    fit: cover
    gaps: none                        # lückenlos durchgehend
```

`i18n/de.yaml`:

```yaml
hero.title: "Plane deine Beete **in Minuten**"
hero.sub: "Mit [accent]Mischkultur[/] zu mehr Ernte"
```

**Gerätetausch pro Ziel** (`device_map`): Dieselbe Leinwand rendert für iOS mit iPhone-Frames und iOS-Screenshots und für Android mit Android-Frames und Android-Screenshots. Die Elementbreite bleibt, die Höhe folgt aus dem Ersatzmodell.

---

## 4. Geräte und Device-Frames

### 4.1 Gerätekatalog

Ein versionierter, datengetriebener Katalog (`core/data/devices.yaml`), damit neue Geräte ohne Code-Änderung ergänzt werden können:

| Modell-ID | Klasse | Merkmale |
|---|---|---|
| `iphone-home` | Handy mit Home-Taste | Ränder oben/unten, runder Home-Button |
| `iphone-notch` | Handy mit Notch | |
| `iphone-island` | Handy mit Dynamic Island | |
| `android-punchhole` | Android-Handy | Kamera-Loch, schmale Ränder |
| `ipad-home` / `ipad-modern` | Tablet | mit/ohne Home-Taste |
| `android-tablet` | Tablet | |
| `macbook` | Laptop | Display + Unterteil mit Scharnier |
| `generic-laptop` | Laptop | |
| `browser` | Browser-Fenster | für Web-Apps: Adressleiste mit frei wählbarer URL, Tabs, hell/dunkel |
| `imac` / `generic-monitor` | Desktop | für Mac-Apps und Web-Apps |
| `none` | rahmenlos | nur abgerundeter Screenshot mit Schatten |

Jedes Modell definiert: Außenmaße, Display-Rechteck, Eckradien von Gehäuse und Display, Aussparungen (Notch/Island/Kamera), Farbvarianten, unterstützte Ausrichtungen, erwartetes Screenshot-Seitenverhältnis.

**Darstellungsstil:** Jedes eingebaute Modell gibt es in zwei Stilen, projektweit wählbar und pro Element überschreibbar:

- `stylized` – schlicht und flächig, modern, wenig Details
- `detailed` – nah am echten Gerät: Glanzkanten, Tasten, Materialverläufe, Kameradetails

Dazu kommen importierte Frames als dritte Quelle (4.2).

**Web-Apps** lassen sich auf zwei Arten zeigen: als eigenständiges `browser`-Fenster oder in einem Laptop-/Desktop-Frame mit eingeblendetem Browser-Rahmen (`chrome: browser` am Gerät). Die Browser-Leiste wird von Vitrine Studio gezeichnet; der Screenshot zeigt nur den Seiteninhalt.

### 4.2 Frames: drei Quellen

**Lizenzfrage:** Die Gerätebilder aus den Apple Design Resources (und ähnliche Herstellerbilder) dürfen in eigenen Marketingmaterialien verwendet, aber nicht ohne Weiteres in einem Open-Source-Projekt weiterverteilt werden. Deshalb:

1. **Eingebaute Frames werden parametrisch gerendert** (Vektorpfade aus den Katalogdaten: Gehäuse, Rand, Glanzkante, Tasten, Kamera). Sie erinnern an die echten Geräte, sind aber eigene Grafik, in den Stilen `stylized` und `detailed`. Farbvarianten sind damit trivial (Schwarz, Silber, Titan, frei wählbar).
2. **Import der Apple Design Resources:** Der Nutzer lädt die Gerätebilder („Product Bezels“) selbst bei Apple herunter und zieht das Archiv oder den entpackten Ordner auf Vitrine Studio (ein DMG wird unter macOS direkt eingebunden, auf anderen Systemen vorher entpackt). Der Importer
   - erkennt Gerät, Farbe und Ausrichtung anhand der Dateinamen über eine pflegbare Zuordnungstabelle (`core/data/frame-import/apple.yaml`), damit neue Apple-Pakete ohne Code-Änderung unterstützt werden können,
   - ermittelt Display-Rechteck und Eckmaske automatisch (transparente Fläche im Inneren des Frames, per Flood-Fill von der Bildmitte),
   - zeigt nicht zuordenbare Dateien zur manuellen Zuordnung an,
   - legt die Frames **app-weit** ab, nicht im Projekt und nicht im Repository.

   Projekte speichern nur Modell und Quelle (`frame: imported:apple`). Fehlt das Paket auf einem anderen Rechner, verwendet Vitrine Studio den parametrischen Frame und weist in der Validierung darauf hin.
3. **Eigene Frame-Pakete:** Beliebige PNG- oder SVG-Frames mit einer kleinen `frame.yaml` (Display-Rechteck, Maske/Eckradius, Modell-Zuordnung) in `frames/` des Projekts oder app-weit. Vitrine Studio verteilt keine Herstellerbilder mit.

### 4.3 Screenshot im Frame

- Maske mit Display-Eckradius; Aussparungen (Island, Notch) werden über dem Screenshot gezeichnet.
- Einpassung `cover` (Standard), `contain`, `stretch`. Weicht das Seitenverhältnis des Screenshots um mehr als ca. 2 % vom Display ab, gibt es eine Warnung (falsches Gerät gewählt?).
- Querformat: Frame und Display werden gedreht, der Screenshot wird nicht gedreht (er ist bereits quer).
- Ohne Screenshot zeigt die Vorschau einen Platzhalter mit dem Asset-Schlüssel.

---

## 5. Mehrsprachigkeit

### 5.0 Standardsprache

- Die **Standardsprache** wird in der App pro Projekt eingestellt (Projekteinstellungen, jederzeit änderbar). Sie ist der Fallback für Texte und Screenshots.
- In den App-Einstellungen lässt sich festlegen, welche Standardsprache neue Projekte bekommen.
- Vor einem Wechsel der Standardsprache zeigt Vitrine Studio an, welche Texte und Screenshots in der neuen Sprache fehlen und damit keinen Fallback mehr hätten.

### 5.1 Texte

- Alle Texte sind Schlüssel; die Inhalte stehen in `i18n/<sprache>.yaml`.
- **Fallback-Kette**: `de-AT` → `de` → Standardsprache. Fehlt ein Text in einer Sprache, wird er in der Standardsprache gerendert und die Validierung meldet ihn.
- **Auto-Fit pro Sprache**: Ist der deutsche Text länger, schrumpft er bis `shrink_to`. Reicht das nicht, gibt es einen Überlauf-Fehler für genau diese Sprache und Seite (in der GUI rot markiert).
- **Optionale Overrides pro Sprache**: Für Sonderfälle kann ein Text-Element pro Sprache abweichende Stil- oder Box-Werte bekommen (z. B. kleinere Schrift nur für Finnisch).
- **Austausch mit Übersetzern**: Export/Import als CSV und XLIFF; Übersetzung per Claude über MCP.
- RTL (Arabisch, Hebräisch): Textshaping und Bidi kommen von der Text-Engine; gespiegelte Layouts sind ein späteres Thema.
- Schriften mit Fallback für CJK, Kyrillisch usw. (projektweite Fallback-Liste pro Schrift).

### 5.2 Screenshots

- Auflösung des Asset-Schlüssels `home` für Sprache `de`, Variante `ios-phone`:
  `screenshots/de/ios-phone/home.png` → `screenshots/de/home.png` → `screenshots/<standardsprache>/ios-phone/home.png` → `screenshots/<standardsprache>/home.png`
  (Eine Datei direkt im Sprachordner ohne Varianten-Ordner gilt für alle Varianten.)
- Verwendete Fallbacks werden in der Validierung als Info ausgewiesen („fr nutzt Standard-Screenshot für planner“).
- **Import** mit Zuordnungsregeln:
  - Ordnerkonvention von Vitrine Studio (s. o.)
  - **fastlane snapshot**-Ausgabe (`<lang>/<Gerätename>-<name>.png`) mit konfigurierbarer Zuordnung Gerätename → Variante
  - Drag & Drop in der GUI mit Erkennung über Dateiname und Pixelgröße
- Ordnerüberwachung: Werden Screenshots neu erzeugt, aktualisiert sich die Vorschau automatisch.

---

## 6. Rendering und Export

### 6.1 Ein Renderer für alles

```
project.yaml + i18n + Assets
        │
        ▼
  core::resolve(target, language)   → aufgelöste Szene (Anker, Fallbacks, Gerätetausch, Stile)
        │
        ▼
  core::layout                      → Textlayout (Shaping, Umbruch, Auto-Fit), Bounding-Boxes
        │
        ▼
  render::rasterize(scene, scale)   → RGBA-Pixmap der ganzen Leinwand oder einer Seite
        │
        ├─▶ GUI-Vorschau (verkleinert, gecacht pro Seite)
        ├─▶ MCP render_preview (PNG an Claude)
        └─▶ Export (Seiten schneiden, Alpha entfernen, PNG/JPEG schreiben)
```

Die GUI rendert **nicht** selbst im Browser. Sie zeigt die Pixmaps aus Rust und legt nur Interaktions-Overlays (Auswahlrahmen, Anfasser, Hilfslinien) als HTML/SVG darüber. Dadurch sind Textumbrüche in Vorschau und Export identisch.

### 6.2 Bibliotheken (Vorschlag, in Phase 0 per Spike bestätigen)

| Aufgabe | Crate | Anmerkung |
|---|---|---|
| Rasterisierung | `tiny-skia` | reines Rust, Pfade, Verläufe, Masken, Anti-Aliasing |
| Textlayout | `cosmic-text` | Shaping, Bidi, Umbruch, Font-Fallback; Alternative `parley` im Spike vergleichen |
| Schriftverwaltung | `fontdb` | Systemschriften + Projekt- + mitgelieferte OFL-Schriften |
| SVG-Assets | `resvg` / `usvg` | SVG-Hintergründe, Logos, SVG-Frame-Pakete |
| Bilder | `image`, `fast_image_resize` | Decoding, Lanczos3-Skalierung |
| PNG schreiben | `png` (+ optional `oxipng`) | 24-Bit, sRGB-Chunk |
| Weichzeichnen (Schatten) | eigene Box-Blur-Implementierung (3 Durchläufe ≈ Gauß) | tiny-skia hat keinen Blur |
| Farbmanagement | `qcms` (später) | Display-P3-Screenshots → sRGB |

### 6.3 Export

Drei Export-Arten:

1. **Seiten** (Store-Screenshots): pro Ziel × Sprache × Seite (bzw. Seitengruppe) ein Bild. Inhalt in Lücken wird weggelassen (3.3). Dateiname per Vorlage, Standard:
   `export/<target>/<lang>/<nn>_<page-id>.png`
2. **Banner**: ein durchgehendes Bild aus der ganzen Leinwand, einem Seitenbereich oder einem freien Rechteck, in beliebiger Zielgröße, z. B. Blog-Header (2400 × 1000), Social-Media-Vorschau (1200 × 630) oder Website-Hero.
   - Einpassung `cover` (Ausschnitt) oder `contain` (der durchgehende Hintergrund wird dann erweitert)
   - Lücken wahlweise `none` (lückenlos durchgehend) oder `as_store`. Weil Elemente an ihren Heimatseiten hängen, bleibt das Layout auch ohne Lücken stimmig.
   - Optional andere Variante oder eigene Overrides, z. B. Texte ausblenden und nur Geräte zeigen.
3. **Store-Galerie-Vorschau**: Seiten nebeneinander mit den Lücken und Eckrundungen des Galerie-Profils, so wie sie im Store erscheinen. Zur Kontrolle der Übergänge und zum Teilen mit anderen.

Für alle Arten gilt:

- **Keine Alphakanäle** (App Store und Play wollen deckende Bilder); PNG 24-Bit oder JPEG mit Qualität.
- Die Exportauflösung kommt allein aus dem Ziel; die Screenshots werden passend skaliert.
- Parallelisiert über `rayon`, inkrementell (Hash über aufgelöste Szene + Assets → unveränderte Bilder werden nicht neu gerendert).

### 6.4 Ausgabeziel-Presets

Ein Preset-Katalog (`core/data/targets.yaml`) mit den aktuellen Store-Größen, z. B.:

- Apple iPhone 6,9″ (1320 × 2868), 6,5″ (1284 × 2778); iPad 13″ hoch und quer (2064 × 2752 / 2752 × 2064)
- Mac App Store (16 : 10: 1280 × 800, 1440 × 900, 2560 × 1600, 2880 × 1800)
- Google Play Phone (z. B. 1080 × 1920 / 1080 × 2160), 7″- und 10″-Tablet, Chromebook, Feature Graphic (1024 × 500)
- Banner: Blog-Header, Open-Graph/Social (1200 × 630), Website-Hero, frei

Die Werte und Store-Regeln (Anzahl pro Set, erlaubte Größen, Seitenverhältnisse) werden bei der Umsetzung gegen die aktuelle Dokumentation von Apple und Google geprüft und sind als Daten hinterlegt, nicht im Code.

---

## 7. Store-Anbindung

### 7.1 Gemeinsamer Ablauf

1. **Plan** (Dry Run): Für jedes Ziel und jede Sprache wird ermittelt, welches Store-Set ersetzt wird, welche Bilder sich gegenüber dem letzten Upload geändert haben (Hash) und ob Store-Regeln verletzt sind.
2. **Anzeige** des Plans in GUI/CLI/MCP mit Bestätigung.
3. **Ausführung** mit Fortschritt, Wiederaufnahme bei Abbruch und Protokoll in `.cache/uploads.sqlite`.

Upload passiert nie ohne vorherigen Plan und ausdrückliche Bestätigung, auch nicht über MCP (dort mit einem `confirm_token` aus dem Plan).

### 7.2 Apple – App Store Connect API

- Authentifizierung: API-Schlüssel (.p8, Key ID, Issuer ID) → JWT (ES256), kurzlebig. Der Schlüsselinhalt liegt nach dem Import im System-Keychain, in der Konfiguration steht nur der Verweis.
- Ablauf: App → bearbeitbare App-Store-Version → Lokalisierungen → Screenshot-Sets je Display-Typ → alte Screenshots löschen → neue reservieren, Teile hochladen, mit MD5-Prüfsumme bestätigen → Verarbeitungsstatus abfragen → Reihenfolge setzen.
- Sprachzuordnung Projekt → Apple-Locale (`de` → `de-DE`), konfigurierbar.
- Display-Typen, Grenzen und Endpunkte bei der Umsetzung gegen die aktuelle API-Dokumentation verifizieren.

### 7.3 Google – Play Developer API (Android Publisher v3)

- Authentifizierung: Service-Account-JSON → OAuth2-JWT-Bearer → Access-Token. JSON im Keychain.
- Ablauf: Edit anlegen → pro Sprache und Bildtyp `deleteall` → Bilder hochladen → Edit committen.
- Bildtypen u. a. `phoneScreenshots`, `sevenInchScreenshots`, `tenInchScreenshots`, `featureGraphic`.

### 7.4 Technik

Kein SDK, direkte REST-Aufrufe mit `reqwest` + rustls; JWT mit `jsonwebtoken`. Eigenes Crate `publish`, damit `core` frei von Netzwerkcode bleibt. Tests gegen aufgezeichnete Antworten (Mock-Server mit `wiremock`).

---

## 8. MCP-Schnittstelle

### 8.1 Betrieb

- `vitrine mcp [--project <pfad>]` startet einen MCP-Server über stdio (Crate `rmcp`, offizielles Rust-SDK).
- Einbindung in Claude Desktop / Claude Code über die übliche MCP-Konfiguration; die GUI zeigt den passenden Konfigurationsschnipsel zum Kopieren.
- Der Server nutzt dieselbe Operations-API wie GUI und CLI, also gelten dieselben Validierungen und es gibt keine Sonderwege.

### 8.2 Tools (erste Version)

| Tool | Zweck |
|---|---|
| `project_open`, `project_create`, `project_summary` | Projekt öffnen/anlegen, kompakte Übersicht (Leinwände, Seiten, Elemente, Sprachen, Ziele) |
| `catalog_devices`, `catalog_targets`, `catalog_fonts` | verfügbare Geräte, Ziel-Presets, Schriften |
| `canvas_add`, `canvas_update` | Leinwand anlegen/ändern (Seitenzahl, Seitenverhältnis, Hintergrund) |
| `element_add`, `element_update`, `element_remove`, `element_list` | Elemente bearbeiten |
| `apply_ops` | mehrere Operationen atomar in einem Schritt (ein Undo-Schritt) |
| `strings_get`, `strings_set`, `strings_missing` | Übersetzungen lesen, setzen, fehlende auflisten (für Übersetzen durch Claude) |
| `assets_list`, `screenshots_import` | Assets und Screenshot-Varianten |
| `render_preview` | **gibt ein PNG als Bildinhalt zurück** (Seite, Leinwand oder Store-Galerie, Sprache, Ziel, max. Kantenlänge), damit Claude das Ergebnis sieht |
| `layout_report` | berechnete Bounding-Boxes pro Seite, Überlappungen, Text-Überläufe, gewählte Schriftgrößen als Text |
| `validate` | alle Warnungen und Fehler |
| `export` | Export für Ziele/Sprachen |
| `upload_plan`, `upload_execute` | Store-Upload mit Plan und Bestätigungs-Token |

Resources: `project.yaml`, Übersetzungsdateien, Gerätekatalog (lesend).

### 8.3 Gleichzeitiges Arbeiten von GUI und MCP

- Das Projekt auf der Platte ist die Quelle der Wahrheit. Jede Operation wird sofort atomar gespeichert (temporäre Datei + Rename); es gibt keinen „ungespeicherten“ Zustand.
- `project.yaml` trägt eine Revisionsnummer. Schreibzugriffe prüfen sie (optimistische Sperre) und laden bei Konflikt neu.
- Die GUI überwacht den Projektordner (`notify`) und lädt externe Änderungen live nach. Claude kann also arbeiten, während man in der GUI zusieht.

---

## 9. Benutzeroberfläche

### 9.1 Aufbau

```
┌──────────────────────────────────────────────────────────────────────────┐
│ Projekt ▾  Leinwand: phone ▾  Ziel: ios-phone-69 ▾  Sprache: de ▾  ⚠ 3   │
├───────────────┬───────────────────────────────────────────┬──────────────┤
│ Leinwände     │                                           │ Inspektor    │
│  ▸ phone      │   [ Seite 1 ][ Seite 2 ][ Seite 3 ] …     │  Position    │
│  ▸ tablet     │      durchgehende Leinwand, Zoom/Pan      │  Größe       │
│ Ebenen        │                                           │  Gerät       │
│  ▸ hero-phone │                                           │  Screenshot  │
│  ▸ title-1    │                                           │  Text/Stil   │
│ Assets        │                                           │  Anker       │
│ Übersetzungen │                                           │              │
├───────────────┴───────────────────────────────────────────┴──────────────┤
│ Ansicht: Leinwand | Seiten | Store-Galerie | Alle Sprachen (Raster)      │
└──────────────────────────────────────────────────────────────────────────┘
```

### 9.2 Wichtige Bedienelemente

- **Direktes Bearbeiten** auf der Leinwand: Verschieben, Skalieren, Einrasten an Seitenrändern, Seitenmitten, Rändern und anderen Elementen; Hilfslinien.
- **Drehen** mit Drehanfasser, Einrasten bei 0°, ±5°, ±15°, ±45°, ±90° (mit Umschalt-Taste frei); Winkel auch numerisch im Inspektor.
- **Heimatseite** wird beim Verschieben automatisch auf die Seite gesetzt, auf der der Elementmittelpunkt liegt; manuell festlegbar.
- **Lücken** schraffiert, Inhalt darin halbtransparent; Galerie-Profil im Ziel-Umschalter wählbar.
- **Varianten-Umschalter**: zeigt die Leinwand z. B. als iPad-Querformat-Gruppierung; Änderungen in dieser Ansicht werden als Overrides der Variante gespeichert und sind im Inspektor als solche markiert (zurücksetzbar).
- **Text direkt bearbeiten** per Doppelklick in der aktuellen Sprache.
- **Sprachumschalter** jederzeit; Ansicht „Alle Sprachen“ zeigt ein Raster aller Sprachen einer Seite und markiert Überläufe.
- **Übersetzungstabelle**: Schlüssel × Sprachen, fehlende Einträge markiert, CSV/XLIFF-Austausch.
- **Ziel-Umschalter**: zeigt dieselbe Leinwand im Seitenverhältnis und mit den Geräten des gewählten Ziels.
- **Undo/Redo** über die Operations-Historie.
- **Projekteinstellungen**: Standardsprache, Sprachen, Frame-Stil (stylized/detailed/importiert), Ziele, Galerie-Profile, Schriften.
- **App-Einstellungen**: Vorgaben für neue Projekte (u. a. Standardsprache), importierte Frame-Pakete, Store-Zugänge (Keychain).
- **Vorlagen**: Leinwand duplizieren, als Vorlage speichern, neues Projekt aus Vorlage.
- UI-Sprachen Englisch und Deutsch (JSON-Kataloge).

### 9.3 Vorschau-Pipeline

- Rendern in Rust auf Vorschauauflösung, Seite für Seite gecacht, nur geänderte Seiten neu.
- Übertragung als Binärdaten über einen eigenen Tauri-Command (`tauri::ipc::Response`, kein Base64).
- Beim Ziehen wird mit reduzierter Auflösung gerendert, beim Loslassen in voller Vorschauqualität.

---

## 10. Architektur und Tech-Stack

### 10.1 Cargo-Workspace

```
vitrine/
├── rust-toolchain.toml
├── Cargo.toml                 # Workspace, gemeinsame Dependencies und Lints
├── crates/
│   ├── core/       # Modell, YAML-IO, Operations-API, Auflösung, Layout, Validierung, Kataloge
│   ├── render/     # Rasterisierung, Frames, Text-Rendering, Export-Encoding
│   ├── publish/    # App Store Connect, Google Play, Keychain-Zugriff
│   ├── mcp/        # MCP-Server (rmcp) auf Basis von core/render/publish
│   ├── cli/        # clap: new, render, export, validate, import, strings, upload, mcp
│   └── gui/        # Tauri 2, Frontend in Vanilla HTML/CSS/JS (ES-Module)
├── assets/fonts/   # mitgelieferte OFL-Schriften (Inter u. a.)
├── docs/
│   ├── KONZEPT.md
│   └── adr/        # Architekturentscheidungen
└── tests/fixtures/ # Beispielprojekte, Referenzbilder
```

Gegenüber dem bisherigen Stack (core/cli/gui) kommen `render`, `publish` und `mcp` hinzu: Rendering ist groß genug für ein eigenes Crate, Netzwerkcode bleibt aus `core` heraus, und der MCP-Server lässt sich getrennt testen.

### 10.2 Operations-API (Herzstück)

Jede Änderung am Projekt ist eine serialisierbare Operation:

```rust
pub enum Op {
    AddElement { canvas: CanvasId, element: Element },
    UpdateElement { id: ElementId, patch: ElementPatch },
    RemoveElement { id: ElementId },
    SetString { lang: Lang, key: Key, value: Option<String> },
    AddCanvas { canvas: Canvas },
    // …
}
```

- `Project::apply(op) -> Result<Inverse>` validiert, wendet an und liefert die Gegenoperation (für Undo).
- GUI, CLI und MCP rufen ausschließlich diese API auf.
- Transaktionen (`apply_ops`) bündeln mehrere Operationen zu einem Undo-Schritt.
- Die Historie wird optional in `.cache/history.sqlite` persistiert.

### 10.3 Übernommene Stack-Entscheidungen

- Rust Edition 2024, Toolchain gepinnt
- Tauri 2 (ohne Tray/Autostart), Frontend ohne Framework, ohne npm, ohne Build-Schritt
- Web-Ansicht nur mit `core:default`; Datei- und Systemzugriffe ausschließlich über eigene Tauri-Commands (Datei-Dialoge über Rust-Seite)
- i18n der Oberfläche über JSON-Kataloge (en/de)
- tokio für Async (Upload, Dateiüberwachung), reqwest + rustls
- SQLite (rusqlite, bundled) nur für Cache, Historie, Upload-Protokoll, zuletzt geöffnete Projekte
- Projekt und App-Konfiguration als YAML über serde
- Geheimnisse ausschließlich im System-Keychain (`keyring`), in Konfigurationen nur Verweise
- macOS, Windows, Linux

### 10.4 Lizenz

**GPL-3.0-or-later** (entschieden am 11. November 2026, ADR 0022), damit Forks offen bleiben. Beiträge nur mit einer Contributor License Agreement im Lizenz-Stil (`CLA.md`): Beitragende behalten ihr Urheberrecht und räumen dem Inhaber (Looking4Cache UG (haftungsbeschränkt)) ein einfaches, unbefristetes, unwiderrufliches Nutzungsrecht ein, auch zur Lizenzierung unter anderen Lizenzen, plus eine Patentlizenz. Name, Logo und App-Icons stehen nicht unter der GPL. Mitgelieferte Schriften unter SIL OFL, Silbentrennmuster unter MIT/BSD; alle Rust-Abhängigkeiten GPL-3-kompatibel. Keine SPDX-Kopfzeilen pro Datei. Keine Herstellergrafiken im Repository.

---

## 11. Qualität und Tests

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test` grün vor jedem Commit.
- **Golden-Image-Tests**: Beispielprojekte in `tests/fixtures` werden gerendert und mit Referenzbildern verglichen (Toleranz pro Pixel und Anteil abweichender Pixel). Nur mitgelieferte Schriften verwenden, damit die Tests auf allen Plattformen gleich laufen. Ein Hilfsbefehl aktualisiert Referenzbilder bewusst.
- **Snapshot-Tests** (`insta`) für YAML-Serialisierung, aufgelöste Szenen, Layout-Reports.
- **Property-Tests** (`proptest`) für Anker-/Koordinatenumrechnung und Op/Inverse-Paare (apply + inverse = Ausgangszustand).
- **Round-Trip-Tests**: Laden → Speichern ergibt dieselbe Datei (stabile Feldreihenfolge, keine unnötigen Diffs).
- **Store-Clients** gegen Mock-Server.
- **MCP**: Integrationstest, der den Server als Subprozess startet und Tools aufruft.
- CI (GitHub Actions) auf macOS, Windows, Linux.

---

## 12. Roadmap

| Phase | Inhalt | Fertig, wenn … |
|---|---|---|
| **0 – Spike** | Rendering-Prototyp: Verlauf über Seiten und Lücken; parametrisches iPhone (stilisiert und detailliert) mit Screenshot, Maske, Rotation und Schatten; Tablet quer; Text mit Umbruch, Inline-Span und Auto-Fit; Seiten-Export mit Lücken, Banner-Export, 2-in-1-Gruppierung. Vergleich cosmic-text vs. parley. | ADR mit Entscheidung; Beispielbilder sauber; Übergänge über Lücken stimmen in der Galerie-Vorschau; Renderzeit gemessen. |
| **1 – Core + CLI** | Datenmodell mit Heimatseiten, Lücken/Galerie-Profilen, Ankern, Gruppierung und Varianten; YAML-IO; Operations-API; Auflösung; Layout; Validierung; Gerätekatalog Handy (Home-Taste, Notch, Dynamic Island, Android), Tablet, Laptop, Desktop, Browser in beiden Stilen; Ziel-Presets iOS, iPadOS, macOS, Play, Banner; Export Seiten, Banner, Galerie-Vorschau; CLI `new/render/export/validate`. | Beispielprojekt (Handy hoch, iPad quer per Gruppierung, Mac, Web-App im Browser- und Laptop-Frame, Blog-Banner) wird headless in alle Ziele exportiert; Golden-Tests grün. |
| **2 – GUI-Editor** | Tauri-App, Leinwand-Ansicht mit Lücken, Auswahl/Verschieben/Skalieren/Drehen, Inspektor, Ebenen, Varianten-Umschalter, Undo/Redo, Asset-Verwaltung, Projekt- und App-Einstellungen (Standardsprache), Live-Reload. | Ein Projekt lässt sich vollständig in der GUI erstellen und exportieren. |
| **3 – Mehrsprachigkeit & Import** | i18n-Dateien, Übersetzungstabelle, Auto-Fit pro Sprache, Screenshot-Varianten, Import (Ordnerkonvention, fastlane), CSV/XLIFF, Sprachraster-Ansicht, Import der Apple Design Resources. | 3 Sprachen × 3 Ziele mit Fallbacks korrekt exportiert, Überläufe gemeldet; Apple-Bezels importiert und verwendet. |
| **4 – MCP** | MCP-Server mit den Tools aus Abschnitt 8, `render_preview` als Bild. | Claude kann ein Projekt von null anlegen, gestalten, übersetzen und exportieren. |
| **5 – Store-Upload** | App Store Connect, danach Google Play; Plan/Dry-Run, Keychain, Upload-Protokoll. | Upload in eine Test-App für beide Stores funktioniert inkl. Reihenfolge. |
| **6 – Feinschliff** | Vorlagen, eigene Frame-Pakete, Farbmanagement P3 → sRGB, Feature Graphic, Installer/Signierung, Website vitrinestudio.app, Doku. | Release 1.0. |
| **7 – 3D (optional)** | Perspektivische Neigung von Geräten (Mesh-Warp), Presets wie „leicht gekippt“. | nach 1.0 |

---

## 13. Entscheidungen und offene Fragen

**Entschieden (9. November 2026)**

- Name: Vitrine Studio (Kurzform Vitrine, CLI `vitrine`), Website www.vitrinestudio.app, API api.vitrinestudio.app (am 11. November 2026 umbenannt, vorher „Arrangia“; Crates.io-Namen noch prüfen)
- Rotation von Elementen von Anfang an; 3D-Perspektive optional als letzter Schritt
- Export als Einzelseiten und als durchgehendes Banner
- Lücken der Store-Galerie werden beim Seiten-Export berücksichtigt
- Abweichende Seitenverhältnisse über Anker; Querformat aus Hochformat über Seitengruppierung und Varianten
- Frame-Optik: stilisiert und detailliert, auswählbar; zusätzlich Import der Apple Design Resources
- Mac-App-Screenshots von Anfang an
- Web-Apps: Browser-Frame und Laptop-Frame (auch kombiniert)
- Standardsprache pro Projekt in der App einstellbar
- Lizenz: GPL-3.0-or-later mit CLA im Lizenz-Stil (am 11. November 2026, siehe 10.4)

**Offen**

1. **Lückenbreiten der Galerie-Profile:** müssen einmal an echten Store-Ansichten ausgemessen werden (App-Store-App auf iPhone und iPad, App-Store-Website, Mac App Store, Play-Store-App und -Website).