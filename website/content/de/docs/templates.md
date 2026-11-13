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
- **Beispiel-Screenshots:** Manche Vorlagen bringen eigene Dummy-Screenshots
  ihres Autors mit, damit du das Design sofort ausprobieren kannst. Sie werden
  nach `screenshots/` kopiert und in `screenshots/samples.yaml` vermerkt. Die
  Prüfung zeigt den Hinweis `sample_screenshot` („durch eigenen ersetzen“), der
  Export funktioniert, aber ein **Store-Upload lehnt** jedes Screenshot-Set ab, das noch
  ein Beispiel enthält. Ersetze eine Datei durch deinen eigenen Screenshot
  (gleicher Name) – dann ist sie kein Beispiel mehr, sonst ist nichts zu tun.

```sh
vitrine new my-app --template phone --lang en --lang de
vitrine new my-app --template ./bold-gradient.vitrine-template
```

## Als Vorlage speichern {#save-as-template}

**Leinwände & Ziele** → eine Leinwand → **Als Vorlage speichern…**:
wähle die *Leinwände*, gib einen *Namen* (die *Id* ergibt sich daraus),
*Beschreibung* und *Autor* an, wähle die *Schlagwörter*, optional **Meine
Screenshots als Beispiel-Screenshots mitnehmen** und **Auch als Datei
speichern**, um sie weiterzugeben. Erneutes Speichern mit derselben Id ersetzt
die Vorlage.

Was hineinkommt:

- die Leinwände, die Ziele, die sie zeigen, ihre Varianten, alle Stile;
- nur die Texte, die die Leinwände verwenden, in allen Sprachen;
- die verwendeten Bilder und Schriften aus dem Ordner `fonts/` des Projekts,
  die ein Stil nennt;
- **nicht** deine Screenshots (ersetzt durch erzeugte Platzhalter-Screens im
  Seitenverhältnis des Geräts) – außer du nimmst sie als
  **Beispiel-Screenshots** mit: Dann kommen die Screenshots, die die
  Leinwände verwenden (PNG/JPEG, höchstens 10 MB und 6000 px je Seite),
  als JPEG (Qualität 60) neu kodiert hinein, damit die Vorlage klein bleibt
  (Bilder mit transparenten Pixeln bleiben PNG). Mach das nur mit Screenshots, die dir gehören (z. B.
  einer Demo-App); in der Galerie werden sie als CC0 veröffentlicht;
- **nicht** Store-Einstellungen, Import-Zuordnungen oder importierte
  Hersteller-Rahmen (Vorschauen nutzen die eingebauten Rahmen).

**Schlagwörter** stammen aus einer festen Liste in drei Gruppen – keine
freien Wörter –, damit die Galerie zuverlässig filtern kann:

| Gruppe | Anzahl | Werte |
|---|---|---|
| Stil (`--style`) | 1–2 | minimal, bold, playful, elegant, realistic, illustrated, editorial, retro, futuristic, organic |
| Farbe (`--color`) | 1–2 | light, dark, colorful, pastel, monochrome, gradient, neon, earthy |
| App-Kategorie (`--use`) | 1–3 | games, productivity, business, finance, health-fitness, sports, education, kids, food-drink, travel, navigation, social, lifestyle, shopping, entertainment, music, photo-video, news, weather, utilities, developer-tools, medical, home-garden, books, events |

In der App erscheinen sie mit deutschen Namen (z. B. „Minimalistisch“,
„Farbverlauf“, „Gesundheit & Fitness“). Eine lokale Vorlage darf Gruppen leer
lassen; die Galerie braucht je Gruppe mindestens eines. Die Geräte (phone,
tablet, watch, mac, web, tv, banner) ergeben sich aus den Ausgabezielen – du
setzt sie nicht selbst. `vitrine template tags` listet alles auf.

**Schriften:** Nur Schriften, die die SIL Open Font License angeben, dürfen in
eine Vorlage; andere werden abgelehnt („darf nicht weitergegeben werden“ oder
„Lizenz unklar“). Die Lizenzdatei wird übernommen oder erzeugt
(`fonts/<name>-OFL.txt`). Mitgelieferte Schriften brauchen nichts.

Auf der Kommandozeile speichert `vitrine template save` **alle Leinwände**
des Projekts. Willst du nur einige, nenne sie mit `--canvas` – einmal pro
Leinwand (`--canvas phone --canvas tablet`). Die Schlagwörter setzt du mit
`--style`, `--color` und `--use`; jede Option darf mehrfach vorkommen oder
mehrere Werte mit Komma nehmen (`--color gradient --color dark` ist dasselbe
wie `--color gradient,dark`). Freie Schlagwörter (`--tag`) gibt es nicht
mehr – nur die Werte aus der Tabelle oben.

```sh
# alle Leinwände des Projekts
vitrine template save --name "Bold gradient" \
  --style bold --color gradient --color dark --use travel -o bold.vitrine-template
# nur die Leinwände phone und tablet
vitrine template save --name "Calm set" --canvas phone --canvas tablet \
  --style minimal --color light,pastel --use productivity,health-fitness
# mit eigenen Screenshots als Beispiele
vitrine template save --name "Demo set" --style minimal --color light --use productivity \
  --sample-screenshots
vitrine template tags                      # alle Schlagwörter
```

## Aus Vorlage einfügen {#insert-from-template}

Leinwände einer Vorlage in ein **bestehendes** Projekt übernehmen:
**Leinwände & Ziele → Neue Leinwand … → Aus Vorlage**. Wähle die
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

Jede Vorlagendatei wird vor der Verwendung geprüft: Größengrenzen (250 MB
gepackt, 500 MB entpackt; der Galerie-Server hat ein eigenes Upload-Limit,
standardmäßig 50 MB), nur bekannte Dateiarten, anhand des Inhalts
geprüft, keine Pfade außerhalb des Pakets oder versteckte Dateien, SVG ohne
Skripte oder externe Links, und das enthaltene Projekt muss sich laden und
prüfen lassen.

## Lizenz {#licence}

Vorlagen stehen unter **CC0** (gemeinfrei) – jeder darf sie für alles
verwenden. Enthaltene Schriften behalten ihre OFL-Lizenz. Speichere Vorlagen
nur aus Material, das du so weitergeben darfst.

## Die Vorlagen-Galerie {#the-template-gallery}

{{< callout status >}}
Die Galerie wird gerade gefüllt. Durchsuchen kannst du sie auf der Website
([Vorlagen-Galerie](/de/galerie/)) oder mit `vitrine gallery list`; in der App
selbst gibt es das Durchsuchen noch nicht – Vorlage herunterladen und im
Dialog *Neues Projekt* **Datei importieren…** wählen.
{{< /callout >}}

- **Durchsuchen und Herunterladen** brauchen kein Konto.
- **Veröffentlichen** braucht eine GitHub-Anmeldung und vollständige
  Schlagwörter. Der Server prüft jeden Upload noch einmal, ersetzt alle
  Screenshots durch eigene Platzhalter, rendert die Vorschauen selbst und
  stellt die Vorlage zur Moderation ein. Vorlagen des offiziellen Kontos sind
  als *official* markiert.
- **Beispiel-Screenshots** bleiben nur in der Vorlage, wenn du bestätigst,
  dass es deine eigenen sind und sie als CC0 veröffentlicht werden dürfen
  (`--confirm-own-screenshots`); ohne Bestätigung ersetzt der Server sie
  durch Platzhalter.
- Die Galerie filtert nach Stil, Farbe, App-Kategorie und Gerät
  (`vitrine gallery list --style bold --use games,kids --devices phone`).

```sh
vitrine gallery list --query garden              # freigegebene Vorlagen
vitrine gallery download bold-gradient           # → bold-gradient.vitrine-template
vitrine template import bold-gradient.vitrine-template

vitrine gallery login                            # GitHub-Anmeldung per Gerätecode
vitrine template export bold-gradient -o bold.vitrine-template
vitrine gallery upload bold.vitrine-template     # Status „pending“ bis zur Freigabe
vitrine gallery logout
```

**Viele Vorlagen auf einmal:** `vitrine gallery upload` nimmt mehrere Dateien.
`--dry-run` führt alle lokalen Prüfungen aus (Paket, Schlagwörter,
Beispiel-Screenshots) und zeigt eine Übersichtstabelle, ohne etwas zu senden;
ein echter Lauf macht nach einer fehlerhaften Datei weiter und endet mit
derselben Übersicht (Exit-Code 1, wenn eine Datei scheiterte).

```sh
vitrine gallery upload dist/*.vitrine-template --dry-run --confirm-own-screenshots
vitrine gallery upload dist/*.vitrine-template --confirm-own-screenshots
```

`gallery login` zeigt einen Code und die Adresse
`https://github.com/login/device`; gib den Code dort ein. Es werden keine
GitHub-Berechtigungen angefragt – gelesen werden nur dein öffentlicher
Benutzername und deine Id. Die Sitzung (90 Tage) liegt im Schlüsselbund des
Systems. Deine Vorlagen bleiben deine, auch wenn du dein GitHub-Konto
umbenennst. Eine Id hochzuladen, die jemand anderem gehört, scheitert mit „id
taken“; dieselbe Version zweimal hochzuladen fordert dich auf, die Version zu
erhöhen (`template save --version`).
