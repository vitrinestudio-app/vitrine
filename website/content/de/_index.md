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
  note: "Version 1.0 für macOS, Windows und Linux. Der Quellcode liegt auf GitHub."
  notePending: "Version 1.0.0 erscheint gerade – die Downloads gehen mit dem Release online. Der Quellcode liegt auf GitHub."
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
  alt: "Echte Renderings aus einem Projekt aus der Vorlage Warm Stay: Zeilen auf Englisch, Deutsch und Französisch; Spalten für das iPhone (App Store), ein Android-Smartphone (Google Play), das iPad im Querformat (App Store), ein Mac-Banner für die Website und die Feature-Grafik für Google Play."
  points:
    - "**Fallback-Ketten**: `fr-CA` erbt von `fr`, fehlende Screenshots kommen aus der Standardsprache"
    - "**Anpassungen je Sprache** und optionale automatische Silbentrennung für lange Wörter"
    - "**Zwei Hochformat-Seiten → ein Querformat-Bild** für iPad-Sätze"
    - "**Prüfung** je Ausgabeziel × Sprache: fehlende Screenshots, überlaufende Texte, Store-Regeln"
  count: 114
  countLabel: "Bilder aus diesem einen Projekt: 3 Sprachen × 6 Store-Sätze + 2 Banner"
  devices:
    - {name: "iPhone", store: "App Store", target: "ios-iphone-69", shape: "phone"}
    - {name: "Android", store: "Google Play", target: "play-phone", shape: "android"}
    - {name: "iPad", store: "App Store", target: "ios-ipad-13-landscape", shape: "landscape"}
    - {name: "Mac", store: "Website", target: "web-banner", shape: "mac"}
    - {name: "Feature-Grafik", store: "Google Play", target: "play-feature-graphic", shape: "banner"}
  langs:
    - {code: "EN", lang: "en"}
    - {code: "DE", lang: "de"}
    - {code: "FR", lang: "fr"}

editor:
  eyebrow: "Der Editor"
  title: "Ein echter Desktop-Editor – die Vorschau *ist* der Export"
  lead: "Jedes Pixel, das du siehst, rendert dieselbe Rust-Pipeline, die auch deine Dateien schreibt. Jede Änderung wird sofort als YAML gespeichert."
  alt: "Der Vitrine-Studio-Editor: links die Ebenen, in der Mitte sechs Seiten einer Synthwave-Musik-App auf einer durchgehenden Leinwand, rechts ein ausgewähltes Smartphone mit seinen Geräte-Einstellungen im Inspektor."
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
      title: "Schriften"
      text: "Mitgelieferte Familien mit freier Lizenz wie Inter, Montserrat und Playfair Display, dazu deine eigenen: Schriftdateien im Projekt oder die auf deinem Rechner installierten – auf Wunsch im Projekt gespeichert, damit CI und Team genauso rendern."
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
  lead: "Drei Vorlagen aus der Galerie – eine Dating-App im Pop-Art-Comic, ein Synthwave-Musikplayer und eine Wetter-App im Stil alter Nationalpark-Plakate – so gerendert, wie die Stores sie zeigen. Je ein Layout, jedes Format."
  tablistLabel: "Beispiel-Ausgaben"
  note: "Alle Geräterahmen oben zeichnet Vitrine Studio selbst – keine Hersteller-Grafiken. Die App-Bildschirme darin sind die Beispiel-Screenshots der Vorlagen."
  tabs:
    - label: "iPhone · Pop-Art"
      image: "iphone-gallery-en.jpg"
      alt: "Sechs iPhone-Screenshots einer Dating-App in Comic-Panels, jede Überschrift in einem gelben Textkasten."
      caption: "iPhone-6.9″-Satz aus der Vorlage Pop-Art Panels: Comic-Panels mit Sprechblasen, ein Smartphone pro Seite."
    - label: "iPad · Deutsch"
      image: "ipad-gallery-de.jpg"
      alt: "Drei iPad-Screenshots im Querformat einer Wetter-App auf Deutsch, im Stil alter Nationalpark-Plakate."
      caption: "Die Vorlage Park Poster als iPad-Querformat-Satz auf Deutsch – die ersten drei von sechs Bildern."
    - label: "iPhone · Français"
      image: "iphone-gallery-fr.jpg"
      alt: "Der iPhone-Screenshot-Satz der Wetter-App im Plakat-Stil auf Französisch."
      caption: "Dieselbe Vorlage auf Französisch: Die längeren Wörter brechen um, das Layout bleibt."
    - label: "Google Play"
      image: "play-gallery-en.jpg"
      alt: "Sechs Google-Play-Screenshots einer Musik-App vor einem Neon-Synthwave-Sonnenuntergang mit Gitterboden."
      caption: "Synthwave Sunset als Google-Play-Satz: Der Geräte-Tausch zeichnet Android-Smartphones, eine Variante passt die kürzeren Seiten an."
    - label: "iPad · quer"
      image: "ipad-landscape-gallery-en.jpg"
      alt: "Drei iPad-Screenshots der Synthwave-Musik-App im Querformat."
      caption: "Die eigene Querformat-Leinwand der Vorlage für das iPad – die ersten drei von sechs Bildern."
    - label: "Blog-Header"
      image: "blog-header-en.jpg"
      alt: "Ein breites Banner mit vier Synthwave-Überschriften und schrägen Smartphones vor einer Neon-Sonne."
      caption: "Ein Blog-Header aus der iPhone-Leinwand – ein Rechteck über vier Seiten, ohne Lücken."

ai:
  eyebrow: "KI-Assistenten"
  title: "Dein Assistent bearbeitet, was du siehst"
  lead: "Vitrine Studio hat einen eingebauten MCP-Server. Claude – oder jeder andere MCP-Client – legt Projekte an, fügt Geräte und Texte hinzu, übersetzt, sieht sich gerenderte Vorschauen an, prüft und exportiert. Die Änderungen erscheinen live im Editor."
  points:
    - "**Zu Claude Desktop hinzufügen** mit einem Klick in den App-Einstellungen; Claude Code und andere Clients mit einem Befehl"
    - "Jede Änderung ist **ein Rückgängig-Schritt**, beschriftet mit dem Namen des Assistenten"
    - "Der Assistent **sieht das Ergebnis**: `render_preview` liefert das gerenderte PNG"
    - "Läuft **lokal** auf 127.0.0.1 mit Token – Store-Uploads brauchen trotzdem **deinen Klick**"
    - "Nicht nur kleine Änderungen: Die ersten [Vorlagen der Galerie](galerie/) – rund 50 – hat **Claude Opus 5.5 über MCP** gestaltet. Ein Assistent kann den Großteil der eigentlichen Gestaltung übernehmen"
    - "Der quelloffene **[Agent-Skill](docs/ai-mcp/#agent-skill)** bringt jedem Assistenten, der das Agent-Skills-Format liest, den ganzen Ablauf bei – vom Text bis zu den Store-Regeln"
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

automation:
  eyebrow: "CLI & Automatisierung"
  title: "Passt in dein CI/CD"
  lead: "Das Kommandozeilenwerkzeug `vitrine` kann mit deinem Projekt alles, was die App kann – ohne Oberfläche, in jeder Pipeline. Prüf die Screenshots bei jedem Pull Request, exportiere sie als Build-Artefakte und lade sie aus dem Release-Job hoch."
  terminalLabel: "Terminal: Store-Screenshots in einem CI-Job prüfen, exportieren und hochladen."
  more: "Doku zur Automatisierung"
  points:
    - "**Jede CI:** `validate`, `export` und `upload` mit klaren Exit-Codes, GitHub-Annotationen, GitLab Code Quality und JUnit-Berichten – [Überblick](docs/automation/)"
    - "**Container-Image** `ghcr.io/vitrinestudio-app/vitrine-cli` für GitLab, Jenkins, CircleCI und andere"
    - "**GitHub Actions:** [Workflow-Beispiele](docs/automation/github-actions/); eine fertige Action ist in Arbeit (Entwurf)"
    - "**GitLab CI:** [Job-Beispiele](docs/automation/gitlab-ci/); CI/CD-Komponenten sind in Arbeit (Entwurf)"
    - "**fastlane:** importiert snapshot- und screengrab-Ausgaben, exportiert das deliver/supply-Layout – [Nutzung mit fastlane](docs/automation/fastlane/); ein Plugin ist in Arbeit (Entwurf)"
    - "**Andere Systeme:** Jenkins, Xcode Cloud, Bitrise, Codemagic und mehr – [Hinweise](docs/automation/other-systems/)"
  terminal:
    - {comment: "1 · jeden Pull Request prüfen (Annotationen im Diff)"}
    - {cmd: "vitrine validate --format github --strict"}
    - {comment: "2 · als Build-Artefakt exportieren – oder in die fastlane-Ordner"}
    - {cmd: "vitrine export --out export"}
    - {cmd: "vitrine export --layout fastlane --out fastlane"}
    - {comment: "3 · aus dem Release-Job hochladen: planen, dann bestätigen"}
    - {cmd: "vitrine upload plan --remote --format json > plan.json"}
    - {cmd: "token=$(jq -r .confirm_token plan.json)"}
    - {cmd: "vitrine upload execute --confirm \"$token\" --yes"}

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
  note: "App Store Connect (Screenshots für jedes Gerät, iPhone Duo sowie Header- und Suchergebnis-Motive, über die App Asset Library) und Google Play (Screenshots und Feature-Grafik). **Stand:** Der App-Store-Upload ist mit einem echten Konto für Screenshots sowie Header- und Suchergebnis-Motive geprüft; Google Play und die übrigen Apple-Geräteklassen sind bisher gegen nachgebildete Stores getestet."

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
  funding:
    title: "Wie die Entwicklung finanziert wird"
    text: "Vitrine Studio bleibt kostenlos. Wir denken über faire Wege nach, die Entwicklung zu finanzieren – zum Beispiel eine in die App integrierte KI-Bildgenerierung oder später direkten Support zu fairen Preisen. Sponsoren sind herzlich willkommen, große Firmen genauso wie kleine Beiträge."
    aside: "Und ganz ehrlich: Mit einem MacBook mit mehr als 16 GB RAM und einer Festplatte, die nicht ständig voll ist, wäre die Entwicklung deutlich entspannter gelaufen."
    github: "Auf GitHub sponsern"
    liberapay: "Über Liberapay spenden"
    soon: "Sponsoring über GitHub Sponsors und Liberapay startet bald. Firmen bekommen eine ordentliche Rechnung."

faq:
  eyebrow: "FAQ"
  title: "Fragen & Antworten"
  items:
    - q: "Ist Vitrine Studio wirklich 100 % kostenlos?"
      open: true
      a: "Ja – 100 % kostenlos, wirklich. Vitrine Studio ist freie Open-Source-Software unter der GPL-3.0-or-later. Es gibt kein Abo, keine Bezahlversion, kein Konto und kein Wasserzeichen auf deinen Screenshots – und keine Begrenzung bei Sprachen, Geräten oder Exporten. Auch für kommerzielle Apps darfst du sie nutzen. Wie wir die Entwicklung finanzieren wollen: [hier](#funding)."
    - q: "Wann kann ich sie herunterladen?"
      a: "Jetzt: Version 1.0 für macOS (signiert und notarisiert), Windows und Linux liegt auf der [Download-Seite](/de/download/). Du kannst die App auch mit Rust [aus dem Quellcode bauen](https://github.com/vitrinestudio-app/vitrine)."
      aPending: "Version 1.0.0 erscheint gerade; die Installer für macOS, Windows und Linux gehen mit dem Release auf der [Download-Seite](/de/download/) online. Bis dahin kannst du die App mit Rust [aus dem Quellcode bauen](https://github.com/vitrinestudio-app/vitrine)."
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
