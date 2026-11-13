---
title: Vitrine Studio
description: "Open-Source-Desktop-App für App-Store- und Google-Play-Screenshots: einmal auf einer durchgehenden Leinwand gestalten, für jede Sprache, jedes Gerät und jeden Store rendern, direkt hochladen – mit MCP-Server für KI-Assistenten."

hero:
  eyebrow: "Open Source · macOS, Windows, Linux"
  title:
    - "Store-Screenshots,"
    - "einmal gestaltet."
    - "Überall gerendert."
  free:
    big: "100 % kostenlos"
    rest: "– ja, wirklich."
    note: "Kein Abo, kein Konto, kein Wasserzeichen, keine Bezahlversion. Kostenlos und Open Source."
  lead: "Vitrine Studio ist eine Desktop-App für App-Store- und Google-Play-Screenshots. Du gestaltest eine durchgehende Leinwand – Hintergründe und Geräte laufen über die Seiten hinweg wie in der Store-Galerie – und renderst sie für jede Sprache, jede Gerätegröße und jeden Store. Dann lädst du sie hoch oder überlässt die Fleißarbeit deinem KI-Assistenten."
  ctaGithub: "Auf GitHub ansehen"
  note: "Pre-Alpha: Installer sind noch nicht veröffentlicht. Der Quellcode liegt auf GitHub."
  pages:
    - ["Einmal", "gestalten"]
    - ["Jede", "Sprache"]
    - ["Jedes", "Gerät"]
    - ["Jeder", "Store"]
    - ["Direkt", "hochladen"]
    - ["Frei &", "Open Source"]
  notesLabel: "Was die Animation zeigt"
  notes:
    - "Ein Hintergrund über alle Seiten"
    - "Geräte frei drehen und überlappen"
    - "Ein Gerät über zwei Seiten"
    - "Smartphones, Laptops und Tablets gemischt"
    - "Lücken wie im Store – was in einer Lücke liegt, fällt weg"
  facts:
    - "Open Source, GPL-3.0-or-later"
    - "Kein Konto, keine Cloud"
    - "Projektordner aus reinem YAML"
    - "Gebaut mit Rust + Tauri"

idea:
  eyebrow: "So funktioniert’s"
  title: "Eine Leinwand gestalten statt vieler Einzelbilder"
  lead: "Statt jeden Screenshot von Hand zu bauen, legst du einen Streifen aus Seiten an. Den Rest erledigt Vitrine Studio – für jedes Ausgabeziel, das du hinzufügst."
  steps:
    - title: "Einmal gestalten"
      text: "Eine Leinwand aus Store-Seiten. Ein Handy über zwei Screenshots gibt es genau **einmal**, Hintergründe laufen über alle Seiten, und die Lücken entsprechen der Store-Galerie – jeder Übergang passt."
    - title: "Jede Kombination rendern"
      text: "Sprachen × Gerätegrößen × Stores aus einem Layout. Anker, Seitengruppierung und Varianten passen es an andere Seitenverhältnisse an – Inhalte pflegst du nie doppelt."
    - title: "Exportieren oder hochladen"
      text: "PNG/JPEG im eigenen oder im fastlane-Layout, farbverwaltet und inkrementell – oder direkt zu App Store Connect und Google Play, immer als **planen → bestätigen → hochladen**."

matrix:
  eyebrow: "Ein Layout, viele Formate"
  title: "Jede Sprache. Jedes Gerät. Jeder Store."
  lead: "Füge eine Sprache oder ein Ausgabeziel hinzu, und der ganze Satz entsteht – übersetzt, neu angeordnet und auf überlaufende Texte geprüft."
  alt: "Ein Raster gerenderter Screenshots: vier Sprachen mal vier Geräteformate, alle aus einem Layout."
  points:
    - "**Fallback-Ketten**: `fr-CA` erbt von `fr`, fehlende Screenshots kommen aus der Standardsprache"
    - "**Anpassungen je Sprache** und optionale automatische Silbentrennung für lange Wörter"
    - "**Zwei Hochformat-Seiten → ein Querformat-Bild** für iPad-Sätze"
    - "**Prüfung** je Ausgabeziel × Sprache: fehlende Screenshots, überlaufende Texte, Store-Regeln"
  countLabel: "Bilder aus einem Layout – und es werden mehr"
  devices:
    - {name: "iPhone 6.9″", shape: "phone"}
    - {name: "Android", shape: "android"}
    - {name: "iPad 13″", shape: "landscape"}
    - {name: "Mac", shape: "mac"}
  langs:
    - {code: "EN", headline: "Plan your garden", headline2: "Know the partners"}
    - {code: "DE", headline: "Plane deinen Garten", headline2: "Finde Partner"}
    - {code: "FR", headline: "Planifiez votre jardin", headline2: "Trouvez les alliés"}
    - {code: "ES", headline: "Planifica tu jardín", headline2: "Conoce los aliados"}

editor:
  eyebrow: "Der Editor"
  title: "Ein echter Desktop-Editor – die Vorschau *ist* der Export"
  lead: "Jedes Pixel, das du siehst, rendert dieselbe Rust-Pipeline, die auch deine Dateien schreibt. Jede Änderung wird sofort als YAML gespeichert."
  alt: "Der Vitrine-Studio-Editor im dunklen Modus: links Leinwände und Ebenen, in der Mitte sechs Seiten einer Garten-App auf einer Leinwand, rechts die Leinwand-Einstellungen im Inspektor."
  alt2: "Eine ausgewählte Überschrift auf der Leinwand mit ihren Text-Einstellungen im Inspektor."
  points:
    - title: "Direkt bearbeiten."
      text: "Verschieben, skalieren und drehen mit Einrasten, Texte direkt tippen, Screenshots per Drag & Drop auf Geräte ziehen."
    - title: "Vier Ansichten."
      text: "Die durchgehende Leinwand mit schraffierten Lücken, einzelne Seiten, die Store-Galerie und alle Sprachen nebeneinander."
    - title: "Alles rückgängig."
      text: "Ein Verlauf für deine Änderungen und die deines KI-Assistenten – jeder Schritt beschriftet."

features:
  eyebrow: "Funktionen"
  title: "Alles zwischen „Screenshots gemacht“ und „live im Store“"
  lead: "Für App-Entwickler, die in vielen Sprachen und auf mehr als einer Plattform veröffentlichen."
  items:
    - icon: canvas
      title: "Durchgehende Leinwand"
      text: "Hintergründe, Bänder und Geräte laufen über die Seitenlücken. Was in einer Lücke liegt, fehlt im Export – genau wie der Store es zeigt."
    - icon: devices
      title: "Jede Geräteklasse"
      text: "iPhone (auch das faltbare iPhone Duo), iPad, Mac, Android-Smartphones und -Tablets, Apple Watch und Wear OS, Browserfenster und Notebooks."
    - icon: frame
      title: "Geräterahmen, dreifach"
      text: "Eingebaute, parametrisch gezeichnete Rahmen, selbst heruntergeladene Apple Product Bezels oder eigene Rahmenpakete (PNG/SVG + `frame.yaml`)."
    - icon: store
      title: "Store-Formate als Vorlagen"
      text: "Screenshot-Sätze für App Store und Google Play, App-Store-Header und Suchergebnis-Motive, die Play-Feature-Grafik, Web-Slides und Banner – aus derselben Leinwand."
    - icon: languages
      title: "Sprachen & Übersetzer"
      text: "Fallback-Ketten, Anpassungen je Sprache, automatische Silbentrennung und CSV/XLIFF-Austausch, der dein Markup in Übersetzungstools schützt."
    - icon: formats
      title: "Ein Layout, viele Formate"
      text: "Vertikale Anker, Seitengruppierung und Varianten passen ein Design an andere Seitenverhältnisse und an Querformat-Sätze an."
    - icon: folder
      title: "Reine Ordner, Git-freundlich"
      text: "Ein Projekt ist ein Ordner mit YAML-Dateien und Bildern. Kein Konto, keine Cloud, kein Lock-in – vergleichen, verzweigen, reviewen."
    - icon: palette
      title: "Farbverwaltete Ausgabe"
      text: "Display-P3-Screenshots werden einmal nach sRGB umgerechnet; Exporte sind deckend, inkrementell und kommen mit einer `index.html`-Übersicht."
    - icon: fonts
      title: "Mitgelieferte Schriftfamilien"
      text: "Inter, Montserrat, Playfair Display, Lora, Bebas Neue und mehr – alle unter SIL OFL, immer verfügbar, ohne Schriftdateien im Projekt."
    - icon: terminal
      title: "CLI für CI"
      text: "`vitrine validate`, `export` und `upload` laufen in Skripten und Pipelines, mit klaren Exit-Codes und JSON-Ausgabe."
    - icon: sparkle
      title: "KI-Assistenten per MCP"
      text: "Claude und andere MCP-Clients gestalten, übersetzen, prüfen Vorschauen und exportieren – live in deinem Editor, und jeden Schritt kannst du wie deinen eigenen zurücknehmen."
    - icon: template
      title: "Vorlagen"
      text: "Mit einer Vorlage starten, eigene Leinwände als Vorlage speichern und teilen – oder direkt in der App in der Community-Galerie stöbern."

showcase:
  eyebrow: "Gemacht mit Vitrine Studio"
  title: "Aus einer YAML-Datei in jeden Store"
  lead: "Das Beispielprojekt – ein erfundener Gartenplaner – so gerendert, wie die Stores es zeigen. Gleiches Layout, gleiche Datei."
  tablistLabel: "Beispiel-Ausgaben"
  note: "Alle Geräterahmen oben zeichnet Vitrine Studio selbst – keine Hersteller-Grafiken. Die App-Bildschirme darin sind für das Beispiel generiert."
  tabs:
    - label: "iPhone · English"
      image: "iphone-gallery-en.jpg"
      alt: "Sechs iPhone-Screenshots einer Garten-App auf einer grünen Leinwand; ein schräges Handy überspannt die Lücke zwischen Seite drei und vier."
      caption: "iPhone-6.9″-Satz: eine durchgehende Leinwand, ein Handy über den Seiten 3 und 4."
    - label: "iPad · Deutsch"
      image: "ipad-gallery-de.jpg"
      alt: "Drei iPad-Screenshots im Querformat auf Deutsch, jeweils aus zwei Handy-Seiten."
      caption: "Dieselben Handy-Seiten als iPad-Querformat-Satz auf Deutsch – zwei Seiten pro Bild, Tablet-Rahmen, angepasste Positionen."
    - label: "iPhone · Français"
      image: "iphone-gallery-fr.jpg"
      alt: "Der iPhone-Screenshot-Satz auf Französisch."
      caption: "Auf Französisch, mit französischen App-Screenshots, wo es sie gibt, und englischen als Ersatz."
    - label: "Mac App Store"
      image: "mac-gallery-en.jpg"
      alt: "Drei Mac-App-Store-Screenshots mit einem Desktop-App-Fenster auf grünem Hintergrund."
      caption: "Mac-App-Store-Satz (16:10) aus einer eigenen Leinwand."
    - label: "Web"
      image: "web-gallery-en.jpg"
      alt: "Zwei Web-Slides: ein Browserfenster und ein Notebook mit einer Web-App."
      caption: "Web-Slides: ein Browserfenster und ein Notebook mit Browser-Leiste."
    - label: "Blog-Header"
      image: "blog-header-de.jpg"
      alt: "Ein breites Blog-Headerbild, ohne Lücken aus der Handy-Leinwand geschnitten."
      caption: "Ein Blog-Header aus derselben Leinwand – ohne Lücken."

ai:
  eyebrow: "KI-Assistenten"
  title: "Dein Assistent bearbeitet, was du siehst"
  lead: "Vitrine Studio hat einen eingebauten MCP-Server. Claude – oder jeder andere MCP-Client – legt Projekte an, fügt Geräte und Texte hinzu, übersetzt, sieht sich gerenderte Vorschauen an, prüft und exportiert. Die Änderungen erscheinen live im Editor."
  points:
    - "**Zu Claude Desktop hinzufügen** mit einem Klick in den App-Einstellungen; Claude Code und andere Clients mit einem Befehl"
    - "Jede Änderung ist **ein Rückgängig-Schritt**, beschriftet mit dem Namen des Assistenten"
    - "Der Assistent **sieht das Ergebnis**: `render_preview` liefert das gerenderte PNG"
    - "Läuft **lokal** auf 127.0.0.1 mit Token – Store-Uploads brauchen trotzdem **deinen Klick**"
  more: "KI-Assistent einrichten"
  chat:
    label: "Ein Chat mit einem KI-Assistenten, der fehlende Texte ins Französische übersetzt und Seite 2 prüft."
    client: "Claude Desktop"
    user: "Übersetze alles Fehlende ins Französische und prüf dann Seite 2 auf Überläufe."
    tools:
      - {name: "strings_missing", note: "fr · 14 Schlüssel"}
      - {name: "strings_set", note: "14 Texte"}
      - {name: "render_preview", note: "Seite 2 · fr"}
      - {name: "validate", note: "fr · ok"}
    reply: "Fertig – 14 Texte übersetzt. Seite 2 passt; eine Überschrift wurde automatisch getrennt, schau sie dir kurz an."
    toast: "Claude Desktop: Französische Texte setzen · Rückgängig"

fastlane:
  eyebrow: "Du nutzt fastlane?"
  title: "Passt in deine fastlane-Lane"
  lead: "snapshot, screengrab, deliver und supply bleiben. Vitrine Studio liest deine Aufnahmen und schreibt genau die Ordner, die deliver und supply hochladen."
  terminalLabel: "Terminal: fastlane-Screenshots importieren und im fastlane-Layout exportieren."
  more: "Nutzung mit fastlane"
  points:
    - "Importiert ganze `fastlane/`-Ordner – snapshot- **und** screengrab-Ausgaben, Store-Locales, `01Home`-Namen"
    - "Überspringt frameit-Reste (`_framed.png`, `Framefile.json`) mit Begründung"
    - "Exportiert das **deliver/supply-Layout** mit Store-Locale-Ordnern, Konfliktprüfung und Limits"
    - "Optional lädt Vitrine Studio die Screenshots selbst hoch – deliver und supply übertragen weiter Binary und Metadaten"
  terminal:
    - {comment: "1 · Aufnahmen holen (frameit-Dateien werden ignoriert)"}
    - {cmd: "vitrine import screenshots fastlane/"}
    - {out: "✓ 48 screenshots · 4 languages · 2 devices"}
    - {comment: "2 · einmal in der App gestalten, dann für deliver/supply exportieren"}
    - {cmd: "vitrine export --layout fastlane --out fastlane"}
    - {out: "✓ screenshots/de-DE/01_ios-iphone-69.png …"}
    - {out: "✓ metadata/android/de-DE/images/phoneScreenshots/ …"}
    - {comment: "3 · hochladen wie bisher"}
    - {cmd: "fastlane deliver"}

upload:
  eyebrow: "Store-Upload"
  title: "Direkt zu App Store Connect und Google Play"
  lead: "Nie wieder 60 Dateien in einen Browser ziehen. Vitrine Studio plant den Upload, zeigt dir genau, was sich ändert, und schickt erst dann – aus der App, der CLI oder von deinem KI-Assistenten."
  steps:
    - title: "Planen"
      text: "Rendert die Store-Ausgabeziele, berechnet Prüfsummen und vergleicht mit dem letzten Upload – oder mit dem Store selbst. Unveränderte Sätze werden übersprungen."
    - title: "Bestätigen"
      text: "Du siehst jeden Satz mit seiner Aktion: hochladen, ersetzen, unverändert, übersprungen oder blockiert – und warum. Nichts geht raus, bevor du bestätigst."
    - title: "Hochladen"
      text: "Fortschritt je Satz, jederzeit abbrechen, fortsetzen durch erneutes Planen. Schlüssel bleiben im Schlüsselbund deines Systems."
  planLabel: "Eine Upload-Plan-Tabelle mit Ausgabezielen, Sprachen, Store-Locales und Aktionen."
  planTitle: "Upload-Plan · Fernwood"
  columns: ["Ziel", "Sprache", "Store", "Aktion", "Bilder"]
  rows:
    - {target: "ios-iphone-69", lang: "en", locale: "en-US", action: "Unverändert", kind: "same", images: "6"}
    - {target: "ios-iphone-69", lang: "de", locale: "de-DE", action: "Ersetzen", kind: "replace", images: "6"}
    - {target: "ios-iphone-69", lang: "fr", locale: "fr-FR", action: "Hochladen", kind: "upload", images: "6"}
    - {target: "ios-ipad-13-landscape", lang: "fr-CA", locale: "fr-CA", action: "Blockiert", kind: "blocked", images: "3"}
  planFoot: "1 blockiert: Screenshot fehlt"
  confirm: "Bestätigen & hochladen"
  note: "App Store Connect (Screenshot-Sätze und die App Asset Library für iPhone Duo, Header- und Suchergebnis-Motive) und Google Play (Screenshots und Feature-Grafik). **Stand:** fertig gebaut und gegen nachgebildete Stores getestet; Tests mit echten Konten laufen."

templates:
  eyebrow: "Vorlagen"
  title: "Mit einer Vorlage starten – oder deine teilen"
  lead: "Eine Vorlage enthält eine oder mehrere Leinwände mit Ausgabezielen, Varianten, Stilen, Texten und Schriften – ein abgestimmter Satz aus Handy, Tablet und Feature-Grafik in einer Datei."
  cards:
    - {name: "Phone", style: "phone", text: "Drei Seiten mit Überschrift und Gerät: iPhone 6.9″, Google Play und ein Blog-Banner.", meta: "Eingebaut"}
    - {name: "Minimal", style: "minimal", text: "Eine Seite, ein Gerät – der schnellste Weg zum ersten Bild.", meta: "Eingebaut"}
    - {name: "Deine eigene", style: "own", text: "Jede Leinwand als Vorlage speichern, für andere Apps wiederverwenden, in ein Projekt einfügen.", meta: ".vitrine-template"}
  points:
    - "Vorlagen sind **CC0** – enthaltene Schriften behalten ihre OFL-Lizenz"
    - "Sicherer Import: Größenlimits, kein ausführbarer Inhalt, jede Datei geprüft"
    - "**Community-Galerie** – stöbern, filtern und laden ohne Konto"

oss:
  eyebrow: "Open Source"
  title: "Zum Lesen, Bauen und Behalten"
  lead: "Vitrine Studio ist freie Software unter der GNU GPL v3 oder neuer. Deine Projekte sind normale Dateien auf deiner Festplatte – sie überdauern jede App-Version."
  points:
    - icon: heart
      title: "Kostenlos, GPL-3.0-or-later"
      text: "Kein Abo, kein Konto, kein Wasserzeichen. Code lesen, bauen, verändern."
    - icon: lock
      title: "Lokal zuerst"
      text: "Projekte verlassen deinen Rechner nur, wenn du in einen Store hochlädst oder eine Vorlage teilst. Store-Schlüssel liegen im Schlüsselbund des Systems."
    - icon: git
      title: "Beiträge willkommen"
      text: "Pull Requests brauchen einmalig eine [Contributor License Agreement](https://github.com/vitrinestudio-app/vitrine/blob/main/CLA.md), unterschrieben im Pull Request."
  yaml: |
    format: vitrine/1
    name: Fernwood
    languages:
      default: en
      available: [en, de, fr, fr-CA]
    canvases:
    - id: phone
      page: {aspect: 1320:2868, count: 6}
      elements:
      - id: title-plan
        kind: text
        page: 0
        text: plan.title
        style: headline
      - id: phone-plan
        kind: device
        model: iphone-island
        screenshot: home
  yamlCaption: "Ein Projekt ist lesbares YAML – vergleichen, reviewen, skripten."

faq:
  eyebrow: "FAQ"
  title: "Fragen & Antworten"
  items:
    - q: "Ist Vitrine Studio wirklich 100 % kostenlos?"
      open: true
      a: "Ja – 100 % kostenlos, wirklich. Vitrine Studio ist freie Open-Source-Software unter der GPL-3.0-or-later. Es gibt kein Abo, keine Bezahlversion, kein Konto und kein Wasserzeichen auf deinen Screenshots – und keine Begrenzung bei Sprachen, Geräten oder Exporten. Auch für kommerzielle Apps darfst du sie nutzen."
    - q: "Wann kann ich sie herunterladen?"
      a: "Vitrine Studio ist im Pre-Alpha-Stadium. Signierte Installer für macOS, Windows und Linux sind in Vorbereitung; bis dahin kannst du die App mit Rust [aus dem Quellcode bauen](https://github.com/vitrinestudio-app/vitrine)."
    - q: "Brauche ich ein Konto oder eine Internetverbindung?"
      a: "Nein. Projekte sind lokale Ordner. Das Netz wird nur beim Hochladen in einen Store oder für die Vorlagen-Galerie genutzt – eine Vorlage dort zu veröffentlichen braucht einen GitHub-Login, stöbern und herunterladen nicht."
    - q: "Sind Apple-Geräterahmen dabei?"
      a: "Vitrine Studio zeichnet seine Geräterahmen selbst und liefert keine Hersteller-Grafiken mit. Apples Product Bezels kannst du selbst importieren (von Apple Design Resources geladen) oder eigene Rahmenpakete verwenden."
    - q: "Kann ich mein fastlane-Setup behalten?"
      a: "Ja. Vitrine Studio importiert snapshot- und screengrab-Ausgaben und exportiert genau das Ordnerlayout, das deliver und supply hochladen – Vitrine Studio übernimmt in deiner Lane die Gestaltung der Screenshots, alles andere bleibt bei fastlane."
    - q: "Welche KI-Assistenten funktionieren damit?"
      a: "Jeder MCP-Client. Claude Desktop richtest du mit einem Klick in den App-Einstellungen ein, Claude Code mit einem Befehl. Assistenten können Store-Uploads planen, ausführen erst nach deiner Bestätigung in der App."
    - q: "Welche Sprachen werden unterstützt?"
      a: "Jede Sprache, die deine Schriften abdecken. Automatische Silbentrennung ist für Englisch, Deutsch, Französisch, Spanisch, Niederländisch, Italienisch, Dänisch, Portugiesisch, Norwegisch und Finnisch dabei. Rechts-nach-links-Layouts werden noch nicht gespiegelt; für CJK-Schriften fügst du eine Schrift wie Noto Sans zum Projekt hinzu."
    - q: "Bin ich an Vitrine Studio gebunden?"
      a: "Nein. Ein Projekt ist ein Ordner mit lesbaren YAML-Dateien, deinen Screenshots und Bildern. Exporte sind ganz normale PNG-/JPEG-Dateien."

cta:
  badge: "100 % kostenlos – ja, wirklich"
  title: "Bessere Store-Seiten in jeder Sprache"
  lead: "Kostenlos und Open Source. Kein Abo, kein Konto, kein Wasserzeichen – einmal gestalten, überall rendern."
  docs: "Zur Dokumentation"
---
