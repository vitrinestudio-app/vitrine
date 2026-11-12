---
title: Geräte & Frames
description: "Eingebaute Geräte-Frames, Apple Product Bezels, die du selbst importierst, eigene Frame-Pakete, Geräte-Tausch pro Ziel, Foldables und Uhren."
weight: 50
icon: frame
---

Ein **Geräte**-Element zeigt einen deiner Screenshots in einem Geräte-Frame.
Du wählst ein **Modell** (die Form: Smartphone mit Dynamic Island,
Android-Tablet, Notebook …) und einen **Frame** (wie es gezeichnet wird). Es
gibt drei Arten von Frames:

1. **Eingebaute Frames** – von Vitrine Studio selbst gezeichnet, immer
   verfügbar.
2. **Apple Product Bezels** – Apples offizielle Gerätebilder, die du selbst
   herunterlädst und einmal pro Rechner importierst.
3. **Eigene Frame-Pakete** – beliebige PNG- oder SVG-Frames mit einer kleinen
   Beschreibungsdatei.

## Eingebaute Geräte {#built-in-devices}

| Klasse | Modelle |
|---|---|
| Smartphones | Phone with island, Phone with notch, Phone with home button, Android phone with punch-hole camera, Foldable phone (open / folded) |
| Tablets | Tablet without home button, Tablet with home button, Android tablet |
| Laptops | Notebook (16:10), Laptop (16:9) |
| Desktops | All-in-one desktop (16:9), Monitor (16:9) |
| Uhren | Rugged watch, Watch |
| Browser | Browser window |
| Ohne Rahmen | No frame (rounded screenshot) |

Die eingebauten Frames sind Vitrine Studios eigene Grafik, gezeichnet nach
Maßen – sie sehen aus wie die echten Geräte, enthalten aber keine Grafiken der
Hersteller. Zwei Stile:

- **Detailliert** – Rahmen, Glaskante, Tasten und Kamera; die Vorgabe.
- **Stilisiert** – flach und schlicht, wenige Details.

Jedes Modell hat seine eigenen Farben (*Black titanium*, *Natural titanium*,
*Midnight*, *Silver*, *Sage* …), und du kannst jedes `#RRGGBB` eintippen.

**Browserfenster:** Jedes Gerät kann *Fensterrahmen → Browser* bekommen, mit
einer *URL* und einem *Hell/Dunkel*-Look – praktisch für Web-Apps und
Mac-Screenshots auf einer Website.

## Ein Gerät einrichten {#setting-up-a-device}

Wähle ein Gerät aus und sieh in den Inspektor:

- **Modell** und **Rahmen** (*Projekt* = die Projektvorgabe, *Stilisiert*,
  *Detailliert*, *Apple* oder ein Frame-Paket).
- **Farbe** – die Farbfelder des gewählten Frames.
- **Ausrichtung** – *Hochformat* oder *Querformat*.
- **Screenshot** – der **Schlüssel** des Screenshots, der gezeigt wird (siehe
  [Screenshots](../screenshots/)); *Einpassung* legt fest, wie ein Screenshot
  mit anderem Seitenverhältnis das Display füllt.
- **Schatten** – ein Schatten-Preset (*soft*, *hard* oder eins aus deinem
  Projekt).
- Größe: Du stellst die **Breite** ein; die Höhe folgt dem Gerät.

Den Standard-Frame des Projekts findest du unter **Projekteinstellungen →
Allgemein → Frame-Stil**; neue Projekte starten mit der Vorgabe aus den
App-Einstellungen (*Frame-Stil neuer Projekte*).

## Geräte-Tausch pro Ziel {#device-swap-per-target}

Eine Leinwand, je Store andere Geräte: Der **Geräte-Tausch** eines Ziels
(*Leinwände & Ausgabeziele* → Ziel) ersetzt Modelle nur für dieses Ziel. Das
Google-Play-Ziel der Vorlage *Phone* tauscht das iPhone gegen das
Android-Smartphone; ein iPad-Ziel kann es gegen ein Tablet tauschen. Variante,
Screenshots und Layout bleiben gleich.

## Apple Product Bezels {#apple-product-bezels}

Apple veröffentlicht offizielle Gerätebilder („Product Bezels“) in seinen
[Design Resources](https://developer.apple.com/design/resources/). Apples
Lizenz erlaubt nicht, dass Vitrine Studio sie mitliefert – du lädst sie also
selbst herunter und importierst sie einmal. Danach funktionieren sie in jedem
Projekt auf diesem Rechner.

1. Lade das Bezel-Paket für deine Geräte herunter (z. B.
   *Bezel-iPhone-18.dmg*) von developer.apple.com/design/resources.
2. **Öffne das DMG im Finder und akzeptiere die Lizenz.** Das Disk-Image
   enthält eine Lizenzvereinbarung; die `.dmg`-Datei direkt zu importieren
   schlägt fehl. Nach dem Akzeptieren ist das Volume eingebunden (z. B.
   `/Volumes/Bezel-iPhone-18`).
3. **App-Einstellungen → Frame-Pakete → Verwalten…** → **Ordner wählen…** und
   das eingebundene Volume auswählen (eine `.zip` oder ein entpackter Ordner
   geht auch: **.dmg oder .zip wählen…**, oder zieh es ins Fenster).
4. Prüfe die Liste – Gerät, Farbe und Ausrichtung werden aus den Dateinamen
   erkannt, der Bildschirmbereich wird erkannt – und klicke **Importieren**.

{{< placeholder "Der Dialog Frame-Pakete nach dem Analysieren eines Apple-Bezel-Pakets." >}}

So nutzt du sie:

- Für das ganze Projekt: **Projekteinstellungen → Allgemein → Frame-Stil →
  Apple (importiert)**.
- Für ein einzelnes Gerät: Inspektor → **Rahmen → Apple**.

Mit einem Apple-Frame zeigen die Farbfelder die Farben des Pakets (z. B.
Black, Burgundy, Glacier, Silver beim iPhone 18). Sind mehrere Größen
installiert (iPhone 18 Pro und Pro Max), wählt **Bezel → Automatisch** die,
die zur Bildgröße des Ziels passt; du kannst pro Gerät eine festlegen.

Gut zu wissen:

- Bezels liegen im App-Datenordner; das Projekt sagt nur
  `frame: imported:apple`. Auf einem Rechner ohne die Bezels (bei Kollegen, in
  CI) wird der eingebaute detaillierte Frame gezeichnet, und die Prüfung
  vermerkt `imported_frame_missing` – außer du
  [speicherst die verwendeten Bezels im Projekt](#apple-bezels-in-ci).
- Eine Farbe, die dort nicht installiert ist, weicht auf eine installierte aus
  (`imported_color_fallback`).
- Der Import-Bericht trennt Dateien, die **bewusst übersprungen** werden (z. B.
  Marketing-Komposits im iPhone-Duo-Paket), Dateien, die du **ignorieren**
  wolltest, und **nicht erkannte** Dateien, die du von Hand zuordnen kannst.
- Apples Bedingungen erlauben die Bezels für Mock-ups von Apps auf
  Apple-Plattformen – lies die Lizenzdatei im Paket.

Auf der Kommandozeile:

```sh
vitrine frames import /Volumes/Bezel-iPhone-18 --dry-run   # zeigen, was importiert würde
vitrine frames import /Volumes/Bezel-iPhone-18
vitrine frames list
vitrine frames remove --bezel "iPhone 18 Pro"              # oder --model, --color, --all
```

## Apple-Bezels in CI: im Projekt speichern {#apple-bezels-in-ci}

Ein Build-Server hat keine importierten Bezels; `vitrine export` würde dort die
eingebauten Rahmen zeichnen. Damit dieselben Bilder wie auf deinem Mac
entstehen, speicherst du die Bezels, die dein Projekt verwendet, **im
Projekt**:

- **Projekteinstellungen → Frames → Verwendete Geräterahmen im Projekt
  speichern** oder `vitrine frames embed`.
- Kopiert werden nur die Bezels, die deine Ziele tatsächlich zeichnen (Modell ×
  Farbe × Ausrichtung × Größe), unverändert, nach `frames/imported/apple/` –
  meist wenige MB. **Alle Größen der verwendeten Modelle** behält z. B. auch
  das iPhone Pro neben dem Pro Max.
- Der Ordner enthält eine `README.md` mit dem Lizenzhinweis und, wenn das
  Paket mit ihr importiert wurde, Apples Lizenzdatei
  (`apple/licenses/<paket>/License.rtf`).
- Frames werden dann zuerst im Projekt gesucht, dann im Frame-Speicher dieses
  Rechners, sonst wird der eingebaute Rahmen gezeichnet. Der Inspektor zeigt,
  woher ein Bezel kommt (*im Projekt gespeichert* / *Frame-Speicher dieses
  Rechners*).
- Ändern sich Geräte, Farben oder Ziele, zeigt der Tab, was eine
  Aktualisierung täte (neu, neu importiert, nicht mehr verwendet) – klicke
  **Gespeicherte Frames aktualisieren** oder führe `vitrine frames embed`
  erneut aus. Auch die Prüfung meldet es: `imported_frame_not_stored` (ein
  Ziel zeichnet Bezels, die nur dieser Rechner hat), `stored_frames_stale`
  (unbenutzte oder veraltete Kopien) und `imported_frames_stored`
  (Lizenz-Erinnerung).
- **Aus dem Projekt entfernen** / `vitrine frames unembed` löscht den Ordner.

```sh
vitrine frames embed --dry-run      # was gespeichert würde, mit Größen
vitrine frames embed                # speichern oder aktualisieren (--all-sizes, --used-only)
vitrine frames unembed              # frames/imported/ entfernen
```

{{< callout warn >}}
**Lizenz: nur private Repositorys.** Apples Lizenz erlaubt die Bezels in
Mock-ups, aber nicht ihre Weitergabe. Gespeicherte Frames sind in einem
privaten Repository in Ordnung; veröffentliche das Projekt nie mit ihnen – kein
öffentliches Repository, kein weitergegebenes Archiv. Vorlagen und
Galerie-Uploads enthalten sie nie. Für ein öffentliches Repository trägst du
`frames/imported/` in die `.gitignore` ein (CI zeichnet dann die eingebauten
Rahmen) oder entfernst die gespeicherten Frames.
{{< /callout >}}

## Eigene Frame-Pakete {#your-own-frame-packs}

Jeder Geräte-Frame, den du verwenden darfst – deine eigene Zeichnung, ein
CC0-Kit, ein Export aus einem Design-Tool – kann ein **Frame-Paket** werden:
ein Ordner mit PNG- oder SVG-Frames und einer `frame.yaml`.

```yaml
format: vitrine-frame/1
name: Clay phone
author: Jane Doe
license: CC0-1.0
models: [iphone-island, android-punchhole]   # weglassen = passt zu jedem Gerät
frames:
  - file: clay-portrait.png
    color: Sand
    orientation: portrait
  - file: clay-portrait-night.png
    color: Night
```

- **Bildschirmbereich:** wird automatisch aus dem transparenten Bereich um die
  Bildmitte erkannt (wie bei Apples Bezels); oder gib `screen: {x, y, w, h}`
  mit einem Eck-`radius` an; oder ein `mask`-Bild. Ein Frame mit gemaltem
  (deckendem) Bildschirm braucht `cut_screen: true`.
- **Mehrere Größen:** Füge Frames mit unterschiedlichen `bezel`-Namen hinzu;
  sie werden wie Apples ausgewählt (nach Zielgröße, dann Screenshot-Größe).
- **Ein Paket ohne `models`** passt zu jedem Gerät – nützlich für Geräte, die
  der Katalog nicht kennt; die Höhe des Elements folgt dem Frame.
- Unbekannte Felder sind Fehler, Tippfehler werden also gemeldet.

Importiere es unter **App-Einstellungen → Frame-Pakete** (Ordner oder `.zip`)
und wähle bei **Importieren nach** *Diesen Rechner (alle Projekte)* oder
*Dieses Projekt (Ordner frames/, reist mit)*. Oder auf der Kommandozeile:

```sh
vitrine frames import clay-pack/ --to project     # oder --to app (Vorgabe); --id zum Umbenennen
vitrine frames list --packs
vitrine frames remove --pack clay-pack --from project
```

Geräte nutzen es dann über **Rahmen → Paket: Clay phone**
(`frame: pack:clay-pack`). Ein Projekt-Paket verdeckt ein app-weites Paket mit
derselben Id. Einen Paket-Ordner von Hand in den Ordner `frames/` des Projekts
zu kopieren, geht auch; nach Änderungen an den Dateien öffnest du das Projekt
neu oder importierst das Paket noch einmal.

Die vollständige Feld-Referenz steht im Repository:
[`docs/guide/frame-packs.md`](https://github.com/vitrinestudio-app/vitrine/blob/main/docs/guide/frame-packs.md).

{{< callout warn >}}
Prüfe die Bedingungen von Hersteller-Kits, bevor du sie in Store-Einträgen
verwendest. Apples Bezels sind für Mock-ups von Apps auf Apple-Plattformen und
dürfen nicht weitergegeben werden; Ausgaben von Googles Device Art Generator
dürfen nicht in Play-Einträgen verwendet werden. Halte Hersteller-Pakete
app-weit; Apple-Bezels kommen nur als [gespeicherte Frames](#apple-bezels-in-ci)
eines privaten Repositorys ins Projekt.
{{< /callout >}}

## Foldables: iPhone Duo {#foldables-iphone-duo}

Zwei Modelle, eins pro Zustand: *Foldable phone (open)* (`iphone-duo`, inneres
Display, von Natur aus quer) und *Foldable phone (folded)*
(`iphone-duo-folded`, äußeres Display). Im Inspektor wechselt **Faltung**
(*Aufgeklappt* / *Zugeklappt*) zwischen beiden – auch nach einem
Geräte-Tausch. Die Presets `ios-iphone-duo-inner` und `ios-iphone-duo-outer`
exportieren die passenden Größen; Apple nimmt sie nur über die App Asset
Library an (siehe [Store-Upload](../store-upload/#app-store-upload-method)).

## Uhren und Wear OS {#watches-and-wear-os}

App Store und Google Play wollen für Uhren **reine App-Screenshots** – ohne
Rahmen, Text und Hintergrund. Die Uhren-Presets (`watch-ultra`,
`watch-series`, …) und `play-wear` sind deshalb **Nur Screenshot**: Jede Seite
exportiert den Screenshot ihres größten Geräts allein, bildfüllend. Ein
Marketing-Layout mit den Uhren-Frames kannst du trotzdem auf einer anderen
Leinwand gestalten, z. B. für deine Website. Siehe
[Ziele & Store-Formate](../targets/#screenshot-only-targets).
