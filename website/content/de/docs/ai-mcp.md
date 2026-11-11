---
title: KI-Assistenten (MCP)
description: "Claude Desktop, Claude Code oder einen anderen MCP-Client verbinden: Der Assistent gestaltet, übersetzt, prüft und exportiert im geöffneten Projekt – live und rückgängig machbar."
weight: 130
icon: sparkle
---

Vitrine Studio hat einen eingebauten [MCP](https://modelcontextprotocol.io)-Server.
Ein Assistent wie Claude arbeitet am **Projekt, das du gerade geöffnet hast**:
Du siehst jede Änderung live auf der Leinwand, und jede Änderung ist ein
Rückgängig-Schritt mit dem Namen des Assistenten – „Claude Desktop: Move
title“.

## Claude Desktop {#claude-desktop}

1. Starte Vitrine Studio.
2. **App-Einstellungen → KI-Assistenten (MCP) → Zu Claude Desktop
   hinzufügen…** Der Dialog zeigt die Konfigurationsdatei, den Eintrag, der
   hinzukommt, und den Status (*Noch nicht eingerichtet*, *Bereits
   eingerichtet*, *Mit anderem Pfad eingerichtet – wird aktualisiert*).
   **Konfiguration schreiben** legt vorher eine Sicherungskopie an und lässt
   deine anderen Einträge in Ruhe.
3. Starte Claude Desktop neu.

Claude Desktop startet dann `vitrine mcp`, das sich mit der laufenden App
verbindet – oder sie startet, wenn sie geschlossen ist (*Bei Bedarf
starten*).

Der Eintrag, falls du ihn lieber von Hand einträgst (der genaue Pfad steht in
den Einstellungen):

```json
{
  "mcpServers": {
    "vitrine-studio": {
      "command": "/Applications/Vitrine Studio.app/Contents/MacOS/vitrine",
      "args": ["mcp"]
    }
  }
}
```

Konfigurationsdatei: macOS `~/Library/Application Support/Claude/claude_desktop_config.json`,
Windows `%APPDATA%\Claude\claude_desktop_config.json`, Linux
`~/.config/Claude/claude_desktop_config.json`.

{{< placeholder "App-Einstellungen → KI-Assistenten (MCP): Verbindungsstatus, Zu Claude Desktop hinzufügen, der Befehl für Claude Code." >}}

## Claude Code {#claude-code}

Kopiere den Befehl aus **App-Einstellungen → KI-Assistenten (MCP) → Claude
Code → Über den Befehl vitrine verbinden** (empfohlen), z. B.:

```sh
claude mcp add vitrine-studio -- "/Applications/Vitrine Studio.app/Contents/MacOS/vitrine" mcp
```

(Mit [installiertem Kommandozeilen-Tool](../install/#the-command-line-tool):
`claude mcp add vitrine-studio -- vitrine mcp`.) *Direkt per HTTP* gibt es
auch, aber Port und Token ändern sich bei jedem Start der App.

## Andere MCP-Clients {#other-mcp-clients}

Jeder MCP-Client funktioniert. Am besten: ein stdio-Server namens
`vitrine-studio` mit dem Befehl `vitrine` und dem Argument `mcp` (**Andere
MCP-Clients → Befehl (stdio)** zeigt den vollständigen Pfad). Clients, die
Streamable HTTP sprechen, können die URL `http://127.0.0.1:<port>/mcp` mit dem
Header `Authorization: Bearer <token>` aus den Einstellungen nutzen – gültig
bis zum Neustart der App; mit einem **Festen Port** bleibt der Port gleich.

## So wird verbunden {#how-it-connects}

- Die App bietet den MCP-Zugang **nur auf diesem Computer** an (127.0.0.1),
  mit einem geheimen Token, das sich bei jedem Start ändert. Ausschalten kannst
  du ihn mit **KI-Assistenten über MCP verbinden lassen**.
- `vitrine mcp` (das, was Claude Desktop startet) leitet an die laufende App
  weiter; läuft keine und ist *Bei Bedarf starten* an, startet es die App (und
  wartet bis zu 20 Sekunden).
- Ohne App – CI, ein Server, *Bei Bedarf starten* aus – arbeitet `vitrine mcp
  --headless --project <ordner>` direkt auf dem Ordner, ohne Live-Ansicht.

## Was du in der App siehst {#what-you-see-in-the-app}

- Die **Assistenten-Anzeige** in der oberen Leiste: grau = aus, *MCP* =
  bereit, mit dem Namen des Clients, wenn einer verbunden ist. Ein Klick
  öffnet **Assistenten-Aktivität** – jeder Schritt mit Uhrzeit; Klick wählt die
  geänderten Elemente aus, der letzte Schritt lässt sich rückgängig machen.
- Geänderte Elemente werden auf der Leinwand kurz hervorgehoben, mit einem
  kleinen Hinweis „Claude Desktop: …“ (Klick wählt sie aus).
- Ein Projekt, das der Assistent öffnet oder anlegt, öffnet sich in der App.

## Was ein Assistent kann {#what-an-assistant-can-do}

- **Anlegen und strukturieren:** Projekte (auch aus Vorlagen), Leinwände,
  Ziele, Banner, Sprachen, Varianten.
- **Gestalten:** Elemente hinzufügen, verschieben, skalieren, drehen,
  gruppieren und umstylen; Stile bearbeiten; Overrides pro Variante und pro
  Sprache.
- **Übersetzen:** fehlende Texte finden, Übersetzungen schreiben, CSV/XLIFF
  austauschen.
- **Das Ergebnis sehen:** `render_preview` liefert das gerenderte Bild – ein
  exportiertes Bild, die ganze Leinwand mit Lücken oder die Store-Galerie –
  plus die Probleme dieses Ziels und dieser Sprache. `layout_report` zeigt, wie
  jeder Text eingepasst wurde.
- **Prüfen und erzeugen:** prüfen, exportieren, Screenshots und Schriften
  importieren.
- **Vorlagen:** auflisten, speichern, einfügen.
- **Uploads:** planen und – mit deiner Bestätigung – ausführen (siehe unten).

Werkzeuge richten sich nach dem, **was du gerade ansiehst**: Ohne Ziel oder
Sprache nehmen sie die aktuelle Ansicht der App, und `selection_get` liest
deine Auswahl – so funktioniert „mach *das hier* 20 % kleiner“.

## Prompts zum Ausprobieren {#prompts-to-try}

- „Leg ein Vitrine-Studio-Projekt in ~/Screens/Fernwood (en, de) für einen
  Gartenplaner an, drei iPhone-Seiten mit je Überschrift und Phone, und zeig
  mir die Galerie.“
- „Übersetze alles Fehlende ins Französische und prüf dann Seite 2 auf
  Französisch auf Überläufe.“
- „Ich habe den Titel ausgewählt – mach ihn nur auf Deutsch 20 % kleiner.“
- „Füge ein Ziel iPad 13″ quer mit zwei Seiten pro Bild hinzu und behebe
  alles, was sich überlappt.“
- „Exportiere iPhone und iPad für alle Sprachen.“

## Uploads {#uploads}

Einen Upload-Plan (`upload_plan`) kann ein Assistent immer erstellen – das ist
ein Probelauf. Um ihn **auszuführen**, zeigt dir Vitrine Studio einen Dialog
„*Claude Desktop möchte in die Stores hochladen*“ mit dem Plan; nur dein
Klick auf **Upload bestätigen** startet ihn. Abgelehnt oder 5 Minuten
unbeantwortet → es passiert nichts.

Headless (`vitrine mcp --headless`) sind Uploads durch Assistenten aus. Sie
lassen sich nur einschalten, indem du `mcp_allow_headless_upload: true` in die
`settings.yaml` der App schreibst – überleg dir das gut.

## Datenschutz und Sicherheit {#privacy-and-safety}

- Die MCP-Werkzeuge machen **keine eigenen Netzwerkanfragen** – nur die
  Store-Upload-Werkzeuge sprechen mit den Stores. Dateien werden nur im
  Projektordner geschrieben und an Pfaden, die der Assistent ausdrücklich
  nennt (Exportordner, Übersetzungsdateien).
- **Store-Schlüssel** lassen sich über MCP weder lesen noch importieren noch
  ändern; die Store-Zuordnung ist für Assistenten schreibgeschützt.
- Installierte Geräterahmen sind schreibgeschützt.
- Was der Assistent von deinem Projekt sieht, geht wie jeder andere
  Gesprächsinhalt an seinen Anbieter – es gelten die üblichen Regeln deines
  KI-Anbieters.
