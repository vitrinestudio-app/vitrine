---
title: Ziele & Store-Formate
linkTitle: Ziele & Store-Formate
description: "Store-Formate aus Presets hinzufügen, eine Leinwand mit Ankern, Gruppierung und Varianten an andere Größen anpassen, Banner und App-Store-Creatives erstellen und Uhren-Ziele einrichten."
weight: 60
icon: devices
---

Ein **Ziel** (Ausgabeziel) ist eine Ausgabe: welche Leinwand, in welcher
Pixelgröße, mit welchen Frames, Lücken und welchem Store-Slot. Jedes Ziel wird
in jeder Sprache des Projekts gerendert (oder in den Sprachen, die du dafür
auswählst).

## Ein Ziel hinzufügen {#adding-a-target}

Öffne **Leinwände & Ziele …** (☰-Menü oder der Knopf neben der
Ziel-Auswahl) → eine Leinwand → **Ziel hinzufügen**. Wähle ein Preset – das
Filterfeld hilft – und **Mit dieser Größe hinzufügen**, wenn ein Preset mehrere
Größen anbietet. Jedes Preset bringt die exakten Größen des Stores mit, den
Store-Slot, ein passendes Gerät und das Galerie-Profil. Plaketten zeigen, ob
der Store den Satz **verlangt** (*Pflicht*, *bedingt*, *optional*) und wie gut
die Daten geprüft sind.

{{< placeholder "Leinwände & Ziele: die Liste Ziel hinzufügen mit Presets, Plaketten und Größen." >}}

### Presets {#presets}

| Preset | Größe (px, Vorgabe zuerst) | Hinweise |
|---|---|---|
| `ios-iphone-69` | 1320×2868, 1290×2796 | iPhone 6,9″/6,7″ – wird als Apples 6,7″-Satz hochgeladen |
| `ios-iphone-65` | 1284×2778, 1242×2688 | iPhone 6,5″ |
| `ios-iphone-63` | 1206×2622, 1179×2556 | iPhone 6,3″ – **der Pflicht-Satz für iPhone** |
| `ios-iphone-61` | 1170×2532, 1125×2436, 1080×2340 | iPhone 6,1″/5,8″ |
| `ios-iphone-55`, `ios-iphone-47` | 1242×2208, 750×1334 | iPhones mit Home-Button |
| `ios-iphone-duo-inner`, `-outer` | 2007×2853, 1398×2034 | iPhone Duo, nur über die App Asset Library |
| `ios-ipad-13` | 2064×2752, 2048×2732 | iPad 13″ – Pflicht, wenn deine App auf dem iPad läuft |
| `ios-ipad-129`, `ios-ipad-11`, `ios-ipad-105` | | ältere/kleinere iPads |
| `mac` | 2880×1800, 2560×1600, 1440×900, 1280×800 | Mac App Store, nur 16:10 |
| `watch-ultra`, `watch-series`, `watch-series-7`, `-4`, `-3` | 422×514 … 312×390 | nur Screenshot |
| `play-phone` | 1080×1920, 1440×2560 | Google-Play-Smartphone 9:16 |
| `play-phone-2x1` | 1080×2160, 1440×2880 | Google-Play-Smartphone 1:2 |
| `play-tablet-7`, `play-tablet-10`, `play-chromebook` | | Google Play, große Bildschirme |
| `play-wear` | 1024×1024, 384×384 | Wear OS, nur Screenshot |
| `play-feature-graphic` | 1024×500 | Banner, **Pflicht** für die Veröffentlichung bei Google Play |
| `ios-header` | 3840×1646 | App-Store-Produktseiten-Header (21:9) |
| `ios-search-results` | 3840×2560 … 1920×1280 | App-Store-Suchergebnisse (3:2) |
| `ios-creative-universal` | 5244×2950 | App-Store-Universal-Creative (16:9) |
| `banner-blog-header`, `banner-open-graph`, `banner-web-hero` | 2400×1000, 1200×630, 2880×1200 | für deine Website und Social Media |

Jedes Preset hat auch eine Querformat-Größe, wo der Store es erlaubt (*Breite
und Höhe tauschen*). Bei Apple *brauchst* du nur einen iPhone-Satz (6,3″) und
für iPad-Apps einen iPad-Satz (13″); App Store Connect skaliert die anderen
herunter. Google Play will 2–8 Bilder pro Gerätetyp und mindestens vier mit
1080 px oder mehr, damit die App für Empfehlungen infrage kommt.

## Das Ziel-Formular {#the-target-form}

| Feld | Bedeutung |
|---|---|
| **Id** | der Name des Ziels, zugleich der Name des Exportordners |
| **Art** | *Seiten* (ein Bild pro Seite oder Seitengruppe) oder *Banner* (ein Bild aus einem Bereich) |
| **Größe (px)** | die Exportgröße eines Bildes |
| **Seiten pro Bild** | `2` setzt zwei Hochformat-Seiten nebeneinander in ein Querformat-Bild |
| **Variante** | welche Variante der Leinwand dieses Ziel nutzt; *keine* = Basis-Layout |
| **Galerie-Profil** | Lücke und Eckenradius des Stores, für die Lücken auf der Leinwand und die Galerie-Vorschau |
| **Screenshots** | der Screenshot-Ordner (*Screenshot-Variante*), z. B. `ios-phone`, `android-phone`, `ios-tablet` |
| **Geräte-Tausch** | Gerätemodelle für dieses Ziel ersetzen, z. B. Smartphone mit Island → Android-Smartphone |
| **Sprachen** | *alle* oder nur einige |
| **Format** | PNG oder JPEG mit einer *Qualität* |
| **Store-Slot** | wohin die Bilder hochgeladen werden: App-Store-Display-Typ oder Google-Play-Bildtyp |
| **Nur Screenshot** | den Screenshot jeder Seite allein exportieren (Uhren) |
| **Sicherer Bereich** | Abstände in % des Bildes, gestrichelt auf der Leinwand |

Unter dem Formular werden die Store-Regeln des Ziels geprüft (Größen, Anzahl,
Formate).

## Eine Leinwand, viele Formate {#one-canvas-many-formats}

Für eine andere Gerätegröße brauchst du selten eine zweite Leinwand.
Stattdessen:

### Andere Seitenverhältnisse: Anker {#different-aspect-ratios-anchors}

Eine Smartphone-Leinwand, die fürs iPhone gestaltet ist (Seitenverhältnis
1320:2868), wird auch für Google Play (1080:2160) gerendert: Die Seite wird
etwas kürzer. Jedes Element hält den Abstand zu der Kante, die sein **Anker**
nennt – Überschriften oben bleiben oben (*Oben*), Smartphones unten bleiben
unten (*Unten*). Ist die Seite eines Ziels viel höher oder kürzer als die
Leinwandseite, schlägt die Prüfung eine Variante vor (`variant_suggested`).

### Querformat aus Hochformat-Seiten: Gruppierung {#landscape-from-portrait-pages-grouping}

Der iPad-Querformat-Satz des Beispielprojekts nutzt die **Smartphone**-Leinwand:
`Seiten pro Bild: 2` setzt die Seiten 1+2, 3+4 und 5+6 in drei
Querformat-Bilder. Der Geräte-Tausch zeichnet iPads statt iPhones, und eine
Variante verschiebt die Geräte ein wenig. Die exportierten Dateinamen nennen
beide Seiten: `01_plan+beds.png`.

{{< shot src="images/ipad-gallery-de.jpg" alt="iPad-Querformat-Galerie des Beispielprojekts: Jedes Bild zeigt zwei Smartphone-Seiten nebeneinander." caption="`ios-ipad-13` im Querformat mit `Seiten pro Bild: 2`, der Variante `tablet-landscape` und einem Geräte-Tausch zum iPad." >}}

### Umordnen: Varianten {#rearranging-variants}

Braucht ein Format Elemente an anderer Stelle, lege eine **Variante** dafür an
(Varianten-Auswahl → **Neue Variante …**), hake die Ziele unter **Gezeigt von**
an und verschiebe die Elemente, während die Variante angezeigt wird.
Gespeichert werden nur die Unterschiede. Siehe
[Varianten](../editor/#variants).

## Banner {#banners}

Ein **Banner**-Ziel rendert **ein** Bild aus einem Bereich einer Leinwand
statt eines Bildes pro Seite – die Google-Play-Feature-Grafik,
App-Store-Creatives, einen Blog-Header, ein Open-Graph-Bild.

### Banner aus dieser Leinwand {#banner-from-this-canvas}

Am schnellsten geht es über **Leinwände & Ziele** → eine Leinwand →
**Banner aus dieser Leinwand**. Angeboten werden:

- **Google-Play-Feature-Grafik** (1024×500)
- **App-Store-Produktseiten-Header** (21:9)
- **App-Store-Suchergebnisse** (3:2)
- **App-Store-Universal-Creative (16:9)**

Für jedes schlägt es so viele Seiten vor, wie zum Seitenverhältnis des Banners
passen (etwa 4 Smartphone-Seiten für die Feature-Grafik, 5 für den Header, 3
für die Suchergebnisse) – **Ab Seite** verschiebt den Bereich – und zeigt
*schon da: …*, wenn die Leinwand schon eins hat. **Hinzufügen** legt das Ziel
an: lückenlos, bildfüllend (*cover*), mit denselben Screenshots, demselben
Geräte-Tausch und derselben Variante wie das Screenshot-Ziel dieses Stores auf
der Leinwand – das Play-Banner zeigt also die Android-Geräte.

{{< shot src="images/blog-header-de.jpg" alt="Ein breites Banner mit der deutschen Überschrift und Smartphones aus dem Beispielprojekt." caption="Ein Banner-Ziel (`blog-header`, Deutsch), ausgeschnitten aus der Smartphone-Leinwand." >}}

### Banner-Einstellungen {#banner-settings}

- **Bereich:** *ganze Leinwand*, ein *Seitenbereich* (*von Seite* … *bis*)
  oder ein *freies Rechteck* (Seite, x, y, Breite, Höhe in lu).
- **Einpassung:** *cover (Ausschnitt)* oder *contain (Hintergrund
  erweitern)*.
- **Lücken:** *keine* (lückenlos) oder *wie im Store*.

Banner wollen oft eine eigene Anordnung: Gib dem Banner-Ziel eine eigene
Variante (das Beispiel nutzt `banner`), etwa um die Untertitel auszublenden.

### Google-Play-Feature-Grafik {#google-play-feature-graphic}

Pflicht für die Veröffentlichung bei Google Play: 1024×500, JPEG oder 24-Bit-PNG
ohne Transparenz. Exporte aus Vitrine Studio sind immer deckend. Googles
Empfehlungen: wichtige Inhalte (Logo, Name, Slogan) weg von den Rändern, kein
„Beste“ oder „#1“, keine Play-Plaketten, und möglichst keine Geräte – nutze eine
Variante ohne Geräte, wenn du dich streng daran hältst. Sie wird zusammen mit
den Screenshots hochgeladen, eine pro Sprache.

### App-Store-Creatives (Header und Suchergebnisse) {#app-store-creatives-header-and-search-results}

Optionale Bilder für den App Store ab iOS 27, eins pro Lokalisierung: der
**Produktseiten-Header** (21:9, 3840×1646), das Bild für die
**Suchergebnisse** (3:2, 1920×1280 bis 3840×2560) oder ein **universelles**
Creative (16:9-PNG, 5244×2950), das beides abdeckt. Apples Regeln: eine kurze,
lokalisierte Aussage, die das Bild ergänzt; keine Preise, URLs, ©, nicht
belegbaren Auszeichnungen, anderen Plattformen oder Apple-Auszeichnungen;
geeignet ab 4 Jahren. Sie werden nur über die App Asset Library hochgeladen.

**Sicherer Bereich:** Apple definiert ihn nur in seinen Design-Vorlagen, ohne
Zahlen. Wenn du ihn kennst, setze **Sicherer Bereich** (oben/rechts/unten/links
in %) am Ziel: Die Leinwand zeigt ihn gestrichelt, und die Prüfung warnt mit
`text_outside_safe_area`.

## Ziele „Nur Screenshot“ {#screenshot-only-targets}

**Nur Screenshot** (*Screenshot jeder Seite allein exportieren*) ist bei allen
Uhren-Presets und bei `play-wear` vorab eingeschaltet. Jede Seite exportiert
den Screenshot ihres **größten Geräts** allein, bildfüllend – ohne Rahmen,
Hintergrund, Text, Schatten oder Drehung –, weil die Stores die nackte
Uhren-Oberfläche wollen (Apple zeichnet seine eigene Uhren-Maske). Nutze den
Screenshot-Ordner des Ziels (`watch`, `wear`), damit die richtigen Aufnahmen
gewählt werden. Eine Seite ohne Gerät bleibt leer (`screenshot_only_empty`).

Für die Apple Watch reicht eine Größe für alle Lokalisierungen; wähle die, für
die du Aufnahmen hast.

## Store-Größen, die Vitrine Studio nicht anbietet {#store-sizes-vitrine-studio-does-not-offer}

Einige Größen auf Apples Hilfeseite (1260×2736, 1512×2316) lehnt der Uploader
von App Store Connect ab, deshalb bieten die Presets sie nicht an.
Store-spezifische Werte sind Datendateien mit Quellen und Datum im Repository
(`crates/core/data/targets.yaml`).
