---
title: Vorlagen & Galerie
description: "Projekte aus Vorlagen starten, eigene Leinwände als Vorlage speichern, in ein Projekt einfügen, .vitrine-template-Dateien weitergeben und in der Galerie veröffentlichen."
weight: 120
icon: template
---

Eine **Vorlage** besteht aus einer oder mehreren Leinwänden – z. B. Phone,
Tablet und Feature-Grafik als abgestimmtes Set – mit ihren Zielen und
Varianten, den Stilen, den Texten in allen Sprachen, Bildern und Schriften.
Sie ist eine einzige **`.vitrine-template`**-Datei.

## Mitgelieferte und eigene Vorlagen {#built-in-and-your-own-templates}

- **Phone** – drei Seiten mit je einer Überschrift und einem Phone; Ziele
  iPhone 6,9″, Google-Play-Phone und ein Blog-Header. Die Vorgabe.
- **Minimal** – eine iPhone-Seite.
- **Deine eigenen** Vorlagen – gespeichert, importiert oder heruntergeladen –
  liegen im App-Datenordner (`templates/`) und stehen in jedem Projekt zur
  Verfügung.

## Neues Projekt aus einer Vorlage {#new-project-from-a-template}

Startbildschirm → **Neues Projekt** → eine Vorlagenkarte wählen (oder **Datei
importieren…** für eine `.vitrine-template`-Datei) → Name, Speicherort,
Sprachen → **Erstellen**.

- Alle Leinwände der Vorlage kommen mit.
- **Texte:** Jede deiner Sprachen bekommt den Text der Vorlage in derselben
  Sprache, sonst in ihrer Basissprache (`de-AT` ← `de`), sonst in der
  Standardsprache der Vorlage – gemeldet als „kopiert, zu übersetzen“.
- **Screenshots bleiben leer.** Die Platzhalter-Screens der Vorlage werden
  nicht kopiert, damit die Prüfung weiter nach deinen echten Screenshots fragt
  und kein Platzhalter im Store landen kann.

```sh
vitrine new my-app --template phone --lang en --lang de
vitrine new my-app --template ./bold-gradient.vitrine-template
```

## Als Vorlage speichern {#save-as-template}

**Leinwände & Ausgabeziele** → eine Leinwand → **Als Vorlage speichern…**:
wähle die *Leinwände*, gib einen *Namen* (die *Id* ergibt sich daraus),
*Beschreibung*, *Autor* und *Schlagwörter* an und optional **Auch als Datei
speichern**, um sie weiterzugeben. Erneutes Speichern mit derselben Id ersetzt
die Vorlage.

Was hineinkommt:

- die Leinwände, die Ziele, die sie zeigen, ihre Varianten, alle Stile;
- nur die Texte, die die Leinwände verwenden, in allen Sprachen;
- die verwendeten Bilder und Schriften aus dem Ordner `fonts/` des Projekts,
  die ein Stil nennt;
- **nicht** deine Screenshots (ersetzt durch erzeugte Platzhalter-Screens im
  Seitenverhältnis des Geräts), Store-Einstellungen, Import-Zuordnungen oder
  importierte Hersteller-Rahmen (Vorschauen nutzen die eingebauten Rahmen).

**Schriften:** Nur Schriften, die die SIL Open Font License angeben, dürfen in
eine Vorlage; andere werden abgelehnt („darf nicht weitergegeben werden“ oder
„Lizenz unklar“). Die Lizenzdatei wird übernommen oder erzeugt
(`fonts/<name>-OFL.txt`). Mitgelieferte Schriften brauchen nichts.

```sh
vitrine template save --name "Bold gradient" --canvas phone --tag dark -o bold.vitrine-template
```

## Aus Vorlage einfügen {#insert-from-template}

Leinwände einer Vorlage in ein **bestehendes** Projekt übernehmen:
**Leinwände & Ausgabeziele → Neue Leinwand … → Aus Vorlage**. Wähle die
Vorlage und ihre Leinwände; der Dialog zeigt, was passieren wird. Das Ganze ist
ein Rückgängig-Schritt.

- Leinwand- und Ziel-Ids, die es schon gibt, bekommen `-2`
  (`ios-iphone-69 → ios-iphone-69-2`), Element-Ids die nächste freie Nummer.
- **Gleiche Stilnamen:** Eine Farbe oder ein Stil, den das Projekt anders
  definiert, wird entweder getrennt behalten (*Beide behalten* – umbenannt,
  z. B. `accent-2`; die Vorgabe) oder durch den des Projekts ersetzt (*Die
  dieses Projekts*). Gleiche Stile werden geteilt. Der Rahmenstil deines
  Projekts bleibt.
- Texte mit gleichem Schlüssel und gleichem Inhalt werden geteilt; andere
  werden zu `<vorlage>.<schlüssel>`.
- Bilder und Schriften werden kopiert; gleiche Dateien werden wiederverwendet.

```sh
vitrine template insert bold-gradient --canvas phone --styles use-project --dry-run
```

## Vorlagen verwalten {#managing-templates}

```sh
vitrine template list
vitrine template import bold.vitrine-template [--replace]
vitrine template export bold-gradient -o bold.vitrine-template
vitrine template remove bold-gradient          # mitgelieferte Vorlagen lassen sich nicht entfernen
```

Jede Vorlagendatei wird vor der Verwendung geprüft: Größengrenzen (50 MB
gepackt, 150 MB entpackt), nur bekannte Dateiarten, anhand des Inhalts
geprüft, keine Pfade außerhalb des Pakets oder versteckte Dateien, SVG ohne
Skripte oder externe Links, und das enthaltene Projekt muss sich laden und
prüfen lassen.

## Lizenz {#licence}

Vorlagen stehen unter **CC0** (gemeinfrei) – jeder darf sie für alles
verwenden. Enthaltene Schriften behalten ihre OFL-Lizenz. Speichere Vorlagen
nur aus Material, das du so weitergeben darfst.

## Die Vorlagen-Galerie {#the-template-gallery}

{{< callout status >}}
Die Galerie unter `api.vitrinestudio.app` wird gerade aufgebaut. Durchsuchen
in der App und auf der Website gibt es noch nicht; auf der Kommandozeile kannst
du schon auflisten, herunterladen und hochladen.
{{< /callout >}}

- **Durchsuchen und Herunterladen** brauchen kein Konto.
- **Veröffentlichen** braucht eine GitHub-Anmeldung. Der Server prüft jeden
  Upload noch einmal, ersetzt alle Screenshots durch eigene Platzhalter,
  rendert die Vorschauen selbst und stellt die Vorlage zur Moderation ein.
  Vorlagen des offiziellen Kontos sind als *official* markiert.

```sh
vitrine gallery list --query garden              # freigegebene Vorlagen
vitrine gallery download bold-gradient           # → bold-gradient.vitrine-template
vitrine template import bold-gradient.vitrine-template

vitrine gallery login                            # GitHub-Anmeldung per Gerätecode
vitrine template export bold-gradient -o bold.vitrine-template
vitrine gallery upload bold.vitrine-template     # Status „pending“ bis zur Freigabe
vitrine gallery logout
```

`gallery login` zeigt einen Code und die Adresse
`https://github.com/login/device`; gib den Code dort ein. Es werden keine
GitHub-Berechtigungen angefragt – gelesen werden nur dein öffentlicher
Benutzername und deine Id. Die Sitzung (90 Tage) liegt im Schlüsselbund des
Systems. Deine Vorlagen bleiben deine, auch wenn du dein GitHub-Konto
umbenennst. Eine Id hochzuladen, die jemand anderem gehört, scheitert mit „id
taken“; dieselbe Version zweimal hochzuladen fordert dich auf, die Version zu
erhöhen (`template save --version`).
