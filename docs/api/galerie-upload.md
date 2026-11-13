# Galerie-Upload der Vorlagen

Ziel: Jede fertige Vorlage aus `Vorlagen/<slug>/projekt/` als Paket `.vitrine-template` speichern, mit Tags und
Beschreibung, und in die Galerie unter api.vitrinestudio.app hochladen. Die Vorlagen erscheinen dort als
**offizielle** Vorlagen. Erst Thorsten gibt sie frei.

**Weg:** über die CLI `vitrine`, ausgeführt mit `device_bash` auf dem Mac. Die MCP-Schnittstelle kann Vorlagen zwar speichern
(`template_save` mit denselben Feldern), aber nicht hochladen. Und die CLI braucht kein geöffnetes Projekt. Nimm deshalb
für alles die CLI.

## Voraussetzungen (erledigt Thorsten, nicht du)

- Server aktualisiert und Upload-Limit für die Masse hochgesetzt (Anzahl pro Stunde). Paketgröße bis 100 MB
  (`VITRINE_API_MAX_UPLOAD_MB=100`, nginx `client_max_body_size 110m`). polaroid-wall hat ~50 MB.
- CLI gebaut: `VITRINE=/Users/thorsten/Dev/arrangia/target/release/vitrine`. Prüfe das mit `"$VITRINE" --version`.
- Upload-Token liegt in der Datei `~/.config/vitrine/official-token`. **Nie ausgeben, nie in Dateien oder Notizen kopieren.**

Gemeinsame Variablen für alle Schritte:

```sh
VITRINE=/Users/thorsten/Dev/arrangia/target/release/vitrine
V="$HOME/mnt/App Vitrine-Studio/Vorlagen"
OUT="$V/_dist"                          # die fertigen Pakete
export VITRINE_APP_DIR="$V/_dist/app"   # eigener Vorlagen-Speicher, nicht Thorstens App
mkdir -p "$OUT"
```

## 1. Metadaten je Vorlage festlegen

Lege `_plan/galerie-meta.tsv` an, eine Zeile je Vorlage, durch Tabs getrennt:

```
slug	name	style	color	use	description
aurora-glass	Aurora Glass	elegant,futuristic	pastel,light	productivity,lifestyle	Light, airy glass-on-aurora look: ...
```

- **slug** ist der Ordnername und zugleich die Vorlagen-ID. Er ist dauerhaft und wird nicht mehr geändert.
  Lass `seed-to-harvest-old` weg.
- **name** ist der Projektname aus `_plan/status.md`.
- **Tags** sind feste Werte, keine freien Wörter. Die Werte innerhalb einer Spalte trennst du mit Kommas.
  Eine falsch geschriebene Tag-ID lehnt die CLI ab.
  - `style` 1–2, `color` 1–2, `use` 1–3 Werte. Alle erlaubten IDs mit Beschriftung und Hinweis stehen unten unter
    **Tag-Liste**; `"$VITRINE" template tags` gibt dieselbe Liste aus.
  - Wähle `use` nach „Passt zu“ im README, nicht nur nach der Fake-App. Die Leute sollen die Vorlage für ihre eigene
    App finden.
  - `color` ergibt sich aus dem Modus: H → light, D → dark, M → das Kennzeichnende (z. B. gradient, colorful).
  - Geräte-Tags (phone, tablet, watch, mac, banner) setzt die CLI selbst aus den Targets und Canvases.
- **description** ist Englisch, weil die Galerie international ist. 1–3 Sätze, ohne Werbesprache. Sie ist die Info zur
  Verwendung:
  1. Wie die Vorlage aussieht, z. B. Panorama über alle Seiten oder Kontrastseite.
  2. Für welche Art App sie passt.
  3. Was drin ist: Phone und Tablet hoch/quer, 6 Seiten, EN + DE, Beispiel-Screenshots zum Ersetzen.

  Beispiel: „Light glass-on-aurora look with one panorama across all six pages and a night contrast page. Suits AI
  assistants, notes and calm productivity apps. Phone, tablet portrait and landscape, EN + DE, with sample
  screenshots to replace.“

Zeig Thorsten die fertige Tabelle, bevor du Pakete baust.

## 2. Prüfen und Pakete speichern

Je Vorlage:

```sh
P="$V/$slug/projekt"
"$VITRINE" --project "$P" validate          # Fehler (Exit 1) erst beheben; Warnungen notieren
"$VITRINE" --project "$P" template save \
  --id "$slug" --name "$name" --description "$description" --author "Vitrine Studio" \
  --style "$style" --color "$color" --use "$use" \
  --sample-screenshots --version 1.0.0 --replace \
  -o "$OUT/$slug.vitrine-template"
```

- Ohne `--canvas` kommen alle Canvases ins Paket (Phone, Tablet hoch, Tablet quer). So ist es gewollt.
- `--sample-screenshots` packt die Fake-App-Screens als Beispiel-Screenshots mit ein. Sie sind selbst erzeugt und
  werden als CC0 veröffentlicht. Nutzer sehen damit sofort ein fertiges Ergebnis und ersetzen die Screens dann durch
  ihre eigenen.
- Bricht `save` wegen einer Schrift ab, melde das. `--skip-unlicensed-fonts` nimmst du nicht ohne Rückfrage.
- Am besten schreibst du eine Schleife über `galerie-meta.tsv` (`while IFS=$'\t' read -r slug name style color use description`).
  Sie soll nach einem Fehler weiterlaufen und am Ende auflisten, was fehlgeschlagen ist.

## 3. Trockenlauf (sendet nichts)

```sh
"$VITRINE" gallery upload "$OUT"/*.vitrine-template --dry-run --confirm-own-screenshots
```

Die Tabelle zeigt je Paket: ID, Version, Canvases, Tags, Geräte, Anzahl der Beispiel-Screenshots, Größe und Ergebnis.
Alles muss `ok` sein. Beispiel-Screenshots werden automatisch als JPEG gepackt. Üblich sind 5–40 MB je Paket.
Pakete über 90 MB nennst du Thorsten vor dem Upload.

## 4. Hochladen

```sh
VITRINE_OFFICIAL_TOKEN="$(cat ~/.config/vitrine/official-token)" \
  "$VITRINE" gallery upload "$OUT"/*.vitrine-template --confirm-own-screenshots \
  --token-env VITRINE_OFFICIAL_TOKEN --format json > "$OUT/upload-result.json"
```

- Der Upload läuft nach einem Fehler weiter. Ein Paket mit Status `error` lädst du einzeln noch einmal hoch.
  Bei einem Rate-Limit (429) wartest du, statt es sofort zu wiederholen.
- `version_exists` heißt, die Vorlage ist schon oben. Für eine geänderte Fassung nimmst du die Version
  `1.0.1` und baust das Paket neu.
- Mit `--confirm-own-screenshots` bestätigst du, dass alles in den Paketen selbst erstellt ist und als CC0
  veröffentlicht werden darf. Leonardo-Bilder und eigene SVGs gehören dazu. Fremde Fotos, Logos oder
  Markenzeichen dürfen nicht drin sein.

## 5. Abschluss

- Freigeben (`gallery admin approve`) macht nur Thorsten. Du hast kein Admin-Token.
- Bericht an Thorsten:
  - wie viele Pakete hochgeladen wurden und welche fehlten (mit Grund),
  - Paketgrößen auffällig groß/klein,
  - Warnungen aus `validate`,
  - die Tabelle `galerie-meta.tsv`.
- `_dist/app` darfst du danach löschen. Die Pakete in `_dist/*.vitrine-template` bleiben liegen.

## Tag-Liste (vollständig)

Stand `crates/core/data/template-tags.yaml`. In die Befehle kommt immer die **ID** (linke Spalte), nie die Beschriftung.
Die Beschriftungen zeigen App und Website-Galerie an.

### `style`: Stil

1–2 Werte je Vorlage (Pflicht für die Galerie). `--style a,b`.

| ID | Englisch | Deutsch | Wann |
|---|---|---|---|
| `minimal` | Minimal | Minimalistisch | viel Weißraum, wenige Elemente, ruhig |
| `bold` | Bold | Kräftig | große Typo, starke Kontraste, laut |
| `playful` | Playful | Verspielt | verspielt, Sticker, Kritzel, gekippte Elemente |
| `elegant` | Elegant | Elegant | edel, Serifen, feine Linien, gedeckte Akzente |
| `realistic` | Realistic | Realistisch | Fotos, echte Objekte, 3D, Schatten |
| `illustrated` | Illustrated | Illustriert | gezeichnete Figuren, Illustrationen, SVG-Szenen |
| `editorial` | Editorial | Redaktionell | Magazin-Look, Raster, Kolumnen, Bildunterschriften |
| `retro` | Retro | Retro | 80er/90er, Papier, Druck, Pixel, Vintage |
| `futuristic` | Futuristic | Futuristisch | Glas, Neon-Licht, Tech, Sci-Fi |
| `organic` | Organic | Organisch | Naturformen, Pflanzen, Wasser, weiche Kurven |

### `color`: Farbe

1–2 Werte je Vorlage (Pflicht für die Galerie). `--color a,b`.

| ID | Englisch | Deutsch | Wann |
|---|---|---|---|
| `light` | Light | Hell | überwiegend helle Seiten |
| `dark` | Dark | Dunkel | überwiegend dunkle Seiten |
| `colorful` | Colourful | Bunt | viele kräftige Farben |
| `pastel` | Pastel | Pastell | weiche, helle Farbtöne |
| `monochrome` | Monochrome | Monochrom | eine Farbe oder Schwarz/Weiß plus höchstens ein Akzent |
| `gradient` | Gradient | Farbverlauf | Farbverläufe prägen das Bild |
| `neon` | Neon | Neon | leuchtende Farben, meist auf Dunkel |
| `earthy` | Earthy | Erdtöne | Braun, Grün, Terrakotta, Sand |

### `use`: App-Kategorie

1–3 Werte je Vorlage (Pflicht für die Galerie). `--use a,b,c`. Nach den Apps wählen, zu denen die Vorlage passt, nicht nur nach der Fake-App.

| ID | Englisch | Deutsch |
|---|---|---|
| `games` | Games | Spiele |
| `productivity` | Productivity | Produktivität |
| `business` | Business | Business |
| `finance` | Finance | Finanzen |
| `health-fitness` | Health & fitness | Gesundheit & Fitness |
| `sports` | Sports | Sport |
| `education` | Education | Bildung |
| `kids` | Kids | Kinder |
| `food-drink` | Food & drink | Essen & Trinken |
| `travel` | Travel | Reisen |
| `navigation` | Navigation | Navigation |
| `social` | Social | Soziale Netzwerke |
| `lifestyle` | Lifestyle | Lifestyle |
| `shopping` | Shopping | Shopping |
| `entertainment` | Entertainment | Unterhaltung |
| `music` | Music | Musik |
| `photo-video` | Photo & video | Foto & Video |
| `news` | News | Nachrichten |
| `weather` | Weather | Wetter |
| `utilities` | Utilities | Dienstprogramme |
| `developer-tools` | Developer tools | Entwickler-Tools |
| `medical` | Medical | Medizin |
| `home-garden` | Home & garden | Haus & Garten |
| `books` | Books | Bücher |
| `events` | Events | Veranstaltungen |

### `devices`: Geräte (automatisch)

Nicht setzen. Die CLI leitet sie aus den Targets und Canvases der Vorlage ab und zeigt sie im Trockenlauf an.

| ID | Englisch | Deutsch | Wann |
|---|---|---|---|
| `phone` | Phone | Smartphone | iPhone- oder Android-Targets, Phone-Geräte |
| `tablet` | Tablet | Tablet | iPad-/Android-Tablet-Targets, Tablet-Geräte |
| `watch` | Watch | Uhr | Apple Watch / Wear OS |
| `mac` | Mac & desktop | Mac & Desktop | Mac-Targets, Laptop/Desktop-Geräte |
| `web` | Web | Web | Browser-Geräte, Web-Targets |
| `tv` | TV | TV | (derzeit nicht vergeben) |
| `banner` | Banner | Banner | Banner-Targets (z. B. Blog-Header) |
