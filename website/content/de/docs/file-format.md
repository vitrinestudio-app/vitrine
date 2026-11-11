---
title: Projekt-Dateiformat
description: "Der Projektordner und jeder Abschnitt der project.yaml – Sprachen, Stile, Leinwände, Elemente, Varianten, Ziele, Store – plus die Übersetzungsdateien, mit Beispielen aus dem Fernwood-Projekt."
weight: 150
icon: file
---

Du musst die Dateien nie anfassen – App und CLI schreiben sie. Sie sind aber
schlichtes, lesbares YAML: Du kannst Änderungen in Git prüfen, sie per Skript
bearbeiten oder schnell etwas im Texteditor korrigieren. Die Beispiele auf
dieser Seite stammen aus *Fernwood*, dem Beispielprojekt im Repository
(`tests/fixtures/example`).

## Der Projektordner {#the-project-folder}

| Pfad | Inhalt | In Git? |
|---|---|---|
| `project.yaml` | das Layout: Sprachen, Stile, Leinwände, Ziele, Store-Zuordnung | ja |
| `i18n/<lang>.yaml` | die Texte einer Sprache | ja |
| `screenshots/<lang>/[<variant>/]<key>.png` | deine App-Screenshots ([Details](../screenshots/)) | ja |
| `assets/` | Bilder für Bild-Elemente und Bildfüllungen | ja |
| `fonts/` | Projektschriften (`.ttf`, `.otf`) | ja |
| `frames/<id>/` | eigene Frame-Pakete, die mit dem Projekt reisen | ja |
| `export/` | exportierte Bilder (Build-Ergebnis) | nein |
| `.cache/` | Render-Cache, Upload-Bilder, Upload-Protokoll `uploads.sqlite` | nein |

`vitrine new` schreibt eine `.gitignore` mit `export/` und `.cache/`.

## Wie die Dateien geschrieben werden {#how-the-files-are-written}

- **Kanonische Form:** Die App schreibt immer dieselbe Anordnung und
  Reihenfolge, damit eine Änderung als kleiner Diff erscheint.
- **Atomares Speichern:** Jede Änderung wird in eine temporäre Datei
  geschrieben und dann an ihren Platz verschoben – ein Absturz hinterlässt nie
  eine halbe Datei.
- **`revision`** zählt bei jedem Speichern hoch. Hat sich die Datei auf der
  Platte inzwischen geändert (anderer Editor, `git pull`), lädt die App neu,
  statt sie zu überschreiben.
- **Unbekannte Felder sind Fehler** – ein Tippfehler wird mit der Liste der
  erlaubten Felder gemeldet, nie stillschweigend ignoriert.

## `project.yaml` auf einen Blick {#projectyaml-at-a-glance}

```yaml
format: vitrine/1
revision: 1
name: Fernwood
languages:
  default: en
  available: [en, de, fr, fr-CA]
  # fallback: {nl: de}          # optional; fr-CA → fr geht automatisch
styles:
  colors:
    accent: "#F6C453"
    paper: "#FFFFFF"
  text:
    headline: {font: Inter Display, weight: 700, size: 90, line_height: 1.08, color: "@paper", hyphenate: true}
    sub: {font: Inter, weight: 400, size: 40, line_height: 1.3, color: "@soft"}
  spans:
    accent: {color: "@accent"}
  shadows:
    card:
      layers:
      - {color: "#0000004D", blur: 40, y: 22}
  # frame: detailed               # Standardrahmen: stylized | detailed | imported:apple | pack:<id>
  # fonts: {Inter: {fallback: [Noto Sans JP]}}
canvases:
- id: phone
  page: {aspect: 1320:2868, count: 6, ids: [plan, beds, companions, neighbours, harvest, calendar]}
  backgrounds:
  - kind: linear-gradient
    angle: 115
    stops: [["#0B3324", 0], ["#145A3C", 0.5], ["#1D5C68", 1]]
  elements:
  - id: title-plan
    kind: text
    page: 0
    text: plan.title
    style: headline
    box: {top: 170, left: 70, right: 70}
    align: center
    fit: {shrink_to: 64, max_lines: 2}
  - id: phone-plan
    kind: device
    page: 0
    anchor_y: bottom
    at: {x: 500, y: 1390}
    width: 720
    model: iphone-island
    color: natural-titanium
    screenshot: home
    shadow: soft
  variants:
    tablet-landscape:
      content_scale: 0.92
      overrides:
        phone-plan: {at: {x: 500, y: 1683}}
targets:
- id: ios-iphone-69
  canvas: phone
  size: [1320, 2868]
  screenshots: ios-phone
  gallery: appstore-iphone
  store: {apple: {display_type: APP_IPHONE_67}}
store:
  apple: {account: team-a, app_id: "1234567890", bundle_id: com.example.fernwood}
  google: {account: play, package_name: com.example.fernwood}
```

## `languages` {#languages}

| Feld | Bedeutung |
|---|---|
| `default` | die Standardsprache: Quelle für Übersetzer, letzter Fallback |
| `available` | alle Sprachen, in Anzeigereihenfolge |
| `fallback` | optionale zusätzliche Fallbacks, z. B. `{nl: de}`; regionale Codes fallen automatisch auf ihre Basissprache zurück |

## `styles` {#styles}

| Feld | Bedeutung |
|---|---|
| `colors` | benannte Farben, verwendet als `@name` |
| `text` | Textstile: `font`, `weight`, `italic`, `size`, `line_height`, `tracking`, `color`, `background`, `transform` (`none`/`uppercase`/`lowercase`), `shadow`, `stroke`, `glow`, `hyphenate` |
| `spans` | Inline-Stile für `[name]…[/]`: `color`, `weight`, `italic`, `font`, `background`, `stroke`, `glow`, `shadow` |
| `shadows` | Schatten-Vorlagen: `{layers: [{color, blur, x, y}]}`; `soft`, `hard`, `none` sind eingebaut |
| `frame` | der Standard-Geräterahmen des Projekts |
| `fonts` | pro Familie: `fallback`-Familien für fehlende Zeichen |

## `canvases` {#canvases}

| Feld | Bedeutung |
|---|---|
| `id` | die Id der Leinwand |
| `page` | `aspect` (`Breite:Höhe` einer Seite), `count`, optional `ids` (benennen die Exportdateien) |
| `backgrounds` | Füllungen über die ganze Leinwand, unterste zuerst |
| `elements` | die Elemente in Zeichenreihenfolge: erstes = unten |
| `variants` | pro Variante: `content_scale` und `overrides` (Element-Id → Felder) |

**Füllungen** sind eine Farbe (`"#0B3324"`, `"@deep"`), ein `linear-gradient`
(`angle` in Grad, 0 = nach oben, 90 = nach rechts; `stops: [[farbe, 0..1], …]`),
ein `radial-gradient` (`center`, `radius`, `stops`) oder ein `image` (`asset`,
`fit`: `cover`/`contain`/`stretch`/`tile`, `focus`).

## Elemente {#elements}

Jedes Element hat eine `id`, eine `kind` und eine Heimatseite `page`
(0-basiert). Einheiten sind lu (eine Seite ist 1000 lu breit); Drehung in
Grad, positiv = im Uhrzeigersinn.

**Gemeinsame Felder:**

| Feld | Bedeutung |
|---|---|
| `page` | Heimatseite (0-basiert); `at` bezieht sich darauf |
| `at` | `{x, y}` – Mittelpunkt des Elements, gemessen von der linken oberen Ecke der Heimatseite |
| `width`, `height` | Größe in lu |
| `rotation`, `pivot` | Grad; Drehpunkt als Versatz vom Mittelpunkt |
| `anchor_y` | `top` (Vorgabe), `bottom`, `center`, `stretch` – zu welcher Kante das Element seinen Abstand hält, wenn die Seitenhöhe eines Ziels abweicht |
| `opacity`, `z` | 0..1; Ebene (gezeichnet wird nach z, dann nach Listenreihenfolge) |
| `clip` | `page` (Vorgabe für Texte) oder `none` |
| `visible`, `visible_in` | ausblenden; oder nur in bestimmten `targets` / `langs` zeigen |
| `lock_page`, `locked` | Editor-Hilfen: Heimatseite beim Ziehen behalten; auf der Leinwand nicht auswählbar |

**Pro Art:**

| Art | Felder |
|---|---|
| `device` | `model` (Pflicht), `orientation`, `frame`, `bezel`, `color`, `fold` (`open`/`folded`), `chrome` (`none`/`browser`), `url`, `appearance` (`light`/`dark`), `screenshot` (der Schlüssel), `fit` (`cover`/`contain`/`stretch`), `shadow`; die Höhe ergibt sich aus dem Modell |
| `text` | `text` (der Schlüssel, Pflicht), `style`, `box` (`{top, left, right, bottom}` Abstände zu den Seitenrändern – ohne `bottom` wächst die Box nach unten), `align`, `valign`, `fit` (`{shrink_to, max_lines}`), `color`, `font_size`, `stroke`, `glow`, `shadow`, `hyphenate`, `langs` |
| `image` | `asset` (z. B. `assets/badge.png`), `fit`, `shadow` |
| `shape` | `shape` (`rect`/`ellipse`/`line`), `radius`, `fill`, `stroke`, `shadow` |
| `background` | `fill` – füllt die Heimatseite |
| `group` | `children` – Elemente, die gemeinsam verschoben und gedreht werden |

Ein Text ohne `box` ist eine freie Box, zentriert bei `at` mit der Breite
`width`.

**Sprach-Overrides** (`langs`, nur bei Texten) ändern Felder für eine
Sprache; `de` gilt auch für `de-AT`:

```yaml
langs:
  de: {box: {top: 170, left: 40, right: 40}, fit: {shrink_to: 76, max_lines: 2}}
```

Erlaubt: `at`, `width`, `height`, `anchor_y`, `style`, `box`, `align`,
`valign`, `fit`, `color`, `font_size`, `stroke`, `glow`, `shadow`,
`hyphenate`. Reihenfolge der Anwendung: Basis → Varianten-Override →
Sprach-Override. Overrides setzen nur Werte; sie enthalten nie `null`.

## `targets` {#targets}

```yaml
- id: ios-ipad-13-landscape
  canvas: phone
  variant: tablet-landscape
  group: 2
  size: [2752, 2064]
  screenshots: ios-tablet
  gallery: appstore-ipad
  device_map: {iphone-island: ipad-modern}
  store: {apple: {display_type: APP_IPAD_PRO_3GEN_129}}
- id: play-feature-graphic
  kind: banner
  canvas: phone
  variant: banner
  size: [1024, 500]
  screenshots: android-phone
  device_map: {iphone-island: android-punchhole}
  region: {rect: {page: 0, x: 0, y: 40, width: 3000, height: 1465}}
  fit: cover
  gaps: none
  store: {google: {image_type: featureGraphic}}
```

| Feld | Bedeutung |
|---|---|
| `kind` | `pages` (Vorgabe) oder `banner` |
| `canvas`, `variant` | was das Ziel zeigt |
| `size` | Exportgröße eines Bildes in px |
| `group` | Seiten pro Bild |
| `screenshots` | Ordner der Screenshot-Variante |
| `gallery` | Galerie-Profil (Lücken, Eckenradius) |
| `device_map` | Modelltausch für dieses Ziel |
| `langs` | zu exportierende Sprachen (Vorgabe: alle) |
| `format`, `quality` | `png` (Vorgabe) oder `jpeg` mit Qualität 1–100 |
| `store` | `{apple: {display_type}}` oder `{google: {image_type}}`; Creatives haben zusätzlich `placement` |
| `screenshot_only` | den Screenshot jeder Seite allein exportieren |
| `safe_area` | `{top, right, bottom, left}` in % des Bildes |
| `region`, `fit`, `gaps` | Banner: `{pages: [a, b]}` oder `{rect: {page, x, y, width, height}}`; `cover`/`contain`; `none`/`as_store` |

## `store` {#store}

```yaml
store:
  apple: {account: team-a, app_id: "1234567890", bundle_id: com.example.app, platform: ios}
  # apple.upload_flow: auto (Vorgabe) | asset_library | screenshot_sets
  google: {account: play, package_name: com.example.app}
  locales:
    apple: {en: en-GB}
    google: {he: iw-IL}
```

`account` ist die Id eines Kontos in den App-Einstellungen auf deinem
Computer – der Schlüssel selbst steht nie im Projekt. Hochgeladen werden nur
die hier eingetragenen Stores.

## `import` {#import}

Gespeicherte Zuordnungen für den Screenshot-Import, damit erneute Importe ohne
Rückfragen laufen:

```yaml
import:
  fastlane:
    devices: {"iPhone 16 Pro Max": ios-phone}
    locales: {de-DE: de}
    keys: {01_home: home}
```

## Übersetzungsdateien: `i18n/<lang>.yaml` {#translation-files-i18nlangyaml}

Pro Sprache eine flache Zuordnung von Schlüsseln zu Texten, mit
[Markup](../languages/#text-markup):

```yaml
beds.title: Plan every bed **by drag & drop**
callout.bad: "**Fennel**\nbetter keep apart"
companions.title: Know your [accent]plant partners[/]
```

Schlüssel dürfen Punkte enthalten. Eine regionale Datei (`fr-CA.yaml`)
braucht nur die Schlüssel, die abweichen. Setze einen Text in
Anführungszeichen, wenn er `: ` enthält oder mit `"`, `[` oder `*` beginnt.

## Ältere Projekte {#older-projects}

- Ein Projekt aus *Arrangia* (dem früheren Namen von Vitrine Studio, `format:
  arrangia/1`) wird mit einem Hinweis abgelehnt: Ändere diese Zeile in
  `format: vitrine/1` – das ist die ganze Migration.
- Eine `project.yaml` mit dem alten Schlüssel `strips:` wird abgelehnt:
  Benenne `strips:` in `canvases:` um und in jedem Ziel `strip:` in `canvas:`.
