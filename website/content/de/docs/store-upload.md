---
title: Store-Upload
description: "App-Store-Connect-API-Schlüssel und Google-Play-Dienstkonto einrichten, das Projekt verbinden und dann hochladen: planen → bestätigen → hochladen."
weight: 110
icon: store
---

{{< callout status >}}
Der Store-Upload ist fertig gebaut und gegen simulierte Stores getestet. Tests
mit echten Store-Konten laufen gerade – kontrolliere nach deinen ersten
Uploads das Ergebnis in App Store Connect und der Play Console, und melde
bitte alles, was dir merkwürdig vorkommt.
{{< /callout >}}

Vitrine Studio lädt deine Screenshots, die Feature-Grafik für Google Play und
die App-Store-Creatives direkt hoch. Das läuft immer in drei
Schritten: **planen → bestätigen → hochladen**. Nichts landet in einem Store
ohne einen Plan, den du gesehen und bestätigt hast.

## Überblick {#overview}

1. Einmal pro Computer: Store-Konten in **App-Einstellungen → Stores**
   hinzufügen.
2. Einmal pro Projekt: die App in **Projekteinstellungen → Stores**
   verbinden.
3. Jedes Mal: **☰ → In die Stores hochladen…**, Plan prüfen, **Bestätigen &
   hochladen**.

## 1. Store-Konten (einmal pro Computer) {#1-store-accounts-once-per-computer}

Konten gehören zu deinem Computer, nicht zum Projekt: Ein Projekt kann per
Git geteilt werden, die Schlüssel bleiben bei dir. Der Schlüssel selbst liegt
im **Schlüsselbund des Systems** (macOS-Schlüsselbund, Windows
Anmeldeinformationsverwaltung, Linux Secret Service, Dienst
`app.vitrinestudio`); die App-Einstellungen verweisen nur darauf. Vitrine
Studio schreibt Schlüssel nie ins Projekt oder in Protokolle und zeigt ihren
Inhalt nie an.

Deine Schlüssel verlassen den Computer nicht: Vitrine Studio schickt sie an
keinen eigenen Server und in keine Cloud. Die App signiert mit dem Schlüssel
lokal eine kurzlebige Anmeldung und sendet nur diese direkt an Apple (App
Store Connect API) bzw. Google (Google-Anmeldung und Play Developer API).

### App-Store-Connect-API-Schlüssel {#app-store-connect-api-key}

Diese Schritte stehen auch in der App (**Wie bekomme ich einen
App-Store-Connect-Schlüssel?**, mit Knöpfen, die die richtigen Seiten öffnen):

1. Melde dich bei App Store Connect mit einem Konto mit der Rolle **Admin**
   an – nur Admins können Team-Schlüssel erstellen.
2. Öffne **Benutzer und Zugriff → Integrationen → App Store Connect API →
   Team-Schlüssel** (englisch: *Users and Access → Integrations → App Store
   Connect API → Team Keys*).
3. Klicke auf **API-Schlüssel generieren** (oder +), gib einen Namen ein
   (z. B. „Vitrine Studio“) und wähle den Zugriff **App-Manager**. Klicke
   auf **Generieren**.
4. Lade den Schlüssel herunter (`AuthKey_<Schlüssel-ID>.p8`). Apple erlaubt
   den Download **nur einmal** – bewahre die Datei gut auf.
5. Notiere die **Issuer-ID** (über der Schlüsselliste) und die
   **Schlüssel-ID** (in der Zeile des Schlüssels).
6. In Vitrine Studio: **App-Einstellungen → Stores → App-Store-Connect-Schlüssel
   hinzufügen …**, die `.p8`-Datei wählen und die Issuer-ID eingeben. Die
   Schlüssel-ID kommt aus dem Dateinamen. Optional gibst du dem Konto einen
   *Namen* (z. B. dein Team).

Schlüssel verloren oder öffentlich geworden: Widerrufe ihn auf derselben Seite
und erstelle einen neuen. Individuelle API-Schlüssel (ohne Issuer-ID) werden
noch nicht unterstützt – nimm einen Team-Schlüssel.

### Google-Play-Dienstkonto {#google-play-service-account}

1. Lege in der [Google Cloud Console](https://console.cloud.google.com/) ein
   Projekt an oder wähle ein vorhandenes.
2. Aktiviere für dieses Projekt die **Google Play Android Developer API**
   (*APIs & Dienste → Bibliothek*).
3. **IAM & Verwaltung → Dienstkonten → Dienstkonto erstellen** (Name z. B.
   „vitrine-upload“).
4. Öffne das Dienstkonto → **Schlüssel → Schlüssel hinzufügen → Neuen
   Schlüssel erstellen → JSON**. Die JSON-Datei wird heruntergeladen – gut
   aufbewahren.
5. In der **Play Console: Nutzer und Berechtigungen → Neue Nutzer einladen**
   → die E-Mail-Adresse des Dienstkontos eingeben
   (`…@….iam.gserviceaccount.com`).
6. Unter **App-Berechtigungen** deine App(s) hinzufügen und **App-Informationen
   ansehen (Lesezugriff)** sowie **Store-Präsenz verwalten** (Texte und Bilder
   des Store-Eintrags) erlauben. Einladung senden.
7. Neue Berechtigungen können bis zu 24 Stunden brauchen, bis sie wirken.
8. In Vitrine Studio: **App-Einstellungen → Stores → Google-Dienstkonto
   hinzufügen …** und die JSON-Datei wählen.

### Ein Konto testen {#testing-an-account}

**Testen** neben einem Konto prüft es, ohne etwas zu ändern:

- **App Store:** signiert ein Token und listet deine Apps – „Schlüssel
  akzeptiert, N Apps sichtbar“. 0 Apps heißt: Rolle oder App-Zugriff des
  Schlüssels sind zu eng.
- **Google Play:** zeigt nur, dass Google den Schlüssel akzeptiert. Ob das
  Dienstkonto deine App bearbeiten darf, zeigt **Prüfen** in den
  Projekteinstellungen (unten).

Typische Fehler: *Schlüssel abgelehnt* (falsche Issuer-ID oder Schlüssel-ID,
widerrufener Schlüssel oder falsch gehende Uhr deines Computers), *kein
Zugriff* (Rolle zu niedrig), *nicht erreichbar – offline?* oder ein
Serverproblem bzw. Anfragelimit (später noch einmal versuchen).

**Konto entfernen** löscht auch den Schlüssel aus dem Schlüsselbund. Projekte
behalten den Verweis und zeigen „nicht gefunden“, bis du wieder ein Konto mit
dieser Id hinzufügst.

{{< shot src="images/store-accounts.jpg" alt="App-Einstellungen, Abschnitt Stores: ein App-Store-Connect-Schlüssel und ein Google-Play-Dienstkonto, je mit Testen-Knopf, und die aufgeklappte Hilfe für App-Store-Connect-Schlüssel." caption="App-Einstellungen → **Stores**: zwei Konten und die Hilfe für App-Store-Connect-Schlüssel." >}}

## 2. Das Projekt verbinden {#2-link-the-project}

**Projekteinstellungen → Stores**:

- **App Store:** **App Store einrichten**, das *Konto* wählen, dann **App
  wählen …** – die Liste zeigt jede App, die der Schlüssel sieht, und füllt
  *Apple-ID* (die numerische ID aus App Store Connect → App-Informationen)
  und *Bundle-ID* aus. Stelle die *Plattform* ein: *iOS / iPadOS* oder
  *macOS*. (Du kannst die IDs auch eintippen.)
- **Google Play:** **Google Play einrichten**, das *Konto* wählen und den
  *Paketnamen* eintippen (steht in der Play Console unter dem App-Namen –
  Google kann die Apps eines Dienstkontos nicht auflisten). **Prüfen** fragt
  Google Play, ob es das Paket gibt und das Dienstkonto es bearbeiten darf:
  Es öffnet einen vorübergehenden Entwurf, liest Titel und Standardsprache der
  App und löscht den Entwurf wieder – es ändert sich nichts.
- **Store-Sprachen:** als welche Store-Sprache jede Projektsprache
  hochgeladen wird. Leer = die eingebaute Vorgabe (`de → de-DE`,
  `fr-CA → fr-CA`, Hebräisch bei Play `iw-IL`, bei Apple `he`). Zwei
  Projektsprachen können sich keine Store-Sprache teilen (`de` und `de-AT` →
  ordne `de-AT` anders zu oder lass es weg).

Hochgeladen werden nur die hier eingetragenen Stores. Welche Bilder wohin
gehen, bestimmt der **Store-Slot** jedes Ziels (App-Store-Display-Typ /
Play-Bildtyp) – die Presets setzen ihn für dich.

{{< shot src="images/project-stores.jpg" alt="Projekteinstellungen, Tab Stores: Konto, App, Apple-ID, Bundle-ID und Plattform für den App Store; Konto und Paketname für Google Play; Store-Sprachen." caption="Projekteinstellungen → **Stores**: das Projekt mit beiden Stores verbunden." >}}

## 3. Planen, bestätigen, hochladen {#3-plan-confirm-upload}

**☰ → In die Stores hochladen…** (oder **Hochladen…** im Export-Dialog). Pro
Store, mit dem das Projekt verbunden ist, gibt es einen Reiter.

### Bevor hochgeladen werden kann {#before-you-can-upload}

Fehlt noch etwas, listet der Dialog die Schritte in der richtigen Reihenfolge,
jeweils mit einem Knopf: Konto hinzufügen (*App-Einstellungen öffnen*),
Projekt verbinden oder Store-Sprachen korrigieren (*Projekteinstellungen
öffnen*), Zielen einen Store-Slot geben (*Ziele öffnen*). Die Liste
aktualisiert sich, wenn du den geöffneten Dialog schließt. Fehlt ein Satz,
den der Store verlangt (der iPhone-6,3″-Satz, die Play-Smartphone-Screenshots
oder die Feature-Grafik), erscheint das als Hinweis: Die anderen Bilder
werden trotzdem hochgeladen, aber ohne ihn reicht der Store nicht ein bzw.
veröffentlicht nicht.

### Der Plan {#the-plan}

Der Plan rendert die Store-Ziele (inkrementell wie der Export), bringt die
Bilder in Store-Reihenfolge, erstellt Fingerabdrücke und vergleicht sie mit
dem letzten erfolgreichen Upload. Die Tabelle zeigt pro Ziel und Sprache die
*Store-Sprache*, die *Aktion* und die Zahl der Bilder:

| Aktion | Bedeutung |
|---|---|
| **Hochladen** | ein neues Set |
| **Ersetzen** | das Set hat sich geändert und wird ersetzt |
| **Unverändert** | dieselben Bilder wie beim letzten Mal – übersprungen |
| **Übersprungen** | z. B. eine Sprache ohne Store-Sprache oder ein Apple-Ziel der anderen Plattform |
| **Blockiert** | kann nicht hochgeladen werden: Prüffehler (fehlende Screenshots, falsche Größe, zu viele Bilder), Datei zu groß, zwei Ziele oder Sprachen im selben Store-Set |

Optionen:

- **Store-Stand prüfen** fragt zusätzlich den Store, was dort gerade liegt
  (braucht Netzwerk und den Schlüssel). Apple: die App, eine bearbeitbare
  Version, die vorhandenen Lokalisierungen und Screenshots mit ihren
  Prüfsummen; Google: öffnet eine vorübergehende Bearbeitung (wird wieder
  gelöscht) und vergleicht die Bilder. Ein Set, das der Store schon genau so
  hat, zeigt *Unverändert*.
- **Auch unveränderte Sets hochladen** erzwingt einen vollständigen Upload.

{{< shot src="images/upload-plan.jpg" alt="Der Upload-Plan für den App Store: sechs Sätze (zwei Ziele × drei Sprachen) mit der Aktion Hochladen, und der Knopf Bestätigen & hochladen." caption="Der Plan: sechs Sätze zum Hochladen, noch nichts gesendet." >}}

### Bestätigen und hochladen {#confirm-and-upload}

**Bestätigen & hochladen** fragt noch einmal nach („… Set(s) mit … Bild(ern)
gehen zu … und ersetzen die Screenshots dort“) und zeigt dann den Fortschritt
pro Set. **Upload abbrechen** stoppt jederzeit. Eine Bestätigung gehört
genau zu ihrem Plan: Sie gilt 30 Minuten und nur für einen Upload. Vor dem
Hochladen prüft Vitrine Studio, ob Projekt und Bilder noch genau dem Stand des
Plans entsprechen. Hast du seitdem etwas geändert oder sind die 30 Minuten
vorbei, wird nichts hochgeladen – erstelle dann einfach einen neuen Plan. So
landet nie etwas anderes im Store als das, was du im Plan gesehen hast.

- **App Store:** Jedes Set enthält danach genau die geplanten Bilder in der
  geplanten Reihenfolge: Screenshots mit demselben Inhalt bleiben, die übrigen
  werden gelöscht, die fehlenden hochgeladen, dann wird die Reihenfolge
  gesetzt. Fehlende Lokalisierungen und Sets werden angelegt.
- **Google Play:** eine Bearbeitung für alle Sprachen; pro Sprache und Bildtyp
  werden alle Bilder ersetzt, dann wird die Bearbeitung einmal übernommen.
  Jeder Fehler und jeder Abbruch verwirft die Bearbeitung – in Play ändert sich
  dann nichts.

**Fortsetzen** nach einem Fehler oder Abbruch: einfach neu planen und
hochladen – fertige Sets sind *Unverändert* und werden übersprungen.

## Besonderheiten bei Apple {#apple-specifics}

**Es braucht eine bearbeitbare Version.** Apple nimmt Screenshot-Änderungen nur
für eine App-Store-Version an, die noch bearbeitet werden kann („Zur Einreichung
vorbereiten“, abgelehnt …). Sonst sagt der Plan *Keine bearbeitbare
App-Store-Version. Lege in App Store Connect eine neue Version an (z. B. 2.1.1)
und plane dann erneut.* – mit dem Knopf **App Store Connect öffnen**.

**Verarbeitung.** Apple verarbeitet jedes hochgeladene Bild – meist in
Sekunden, manchmal über zehn Minuten. Vitrine Studio wartet bis zu 15 Minuten
pro Set. Bilder, die danach noch verarbeitet werden, sind **kein** Fehler: Das
Set zeigt *Hochgeladen, Apple verarbeitet*; die Bilder sind in App Store
Connect und werden nicht erneut hochgeladen. Prüfe später mit **Verarbeitung
prüfen**. Kann Apple ein Bild nicht verarbeiten (z. B. falsche Maße), schlägt
das Set mit Apples Meldung fehl, und der nächste Lauf lädt nur dieses Bild
hoch.

**Anfragelimits.** Apple erlaubt etwa 3500 Anfragen pro Stunde und Schlüssel.
Bei einem Limit pausiert der Lauf (*Pausiert*, bis zu 15 Minuten) und versucht
es dann erneut.

Außerdem will Apple jede Sprache auch in den App-Informationen haben, bevor du
einreichst – App Store Connect zeigt das an.

### Upload-Verfahren für den App Store {#app-store-upload-method}

Vitrine Studio lädt über Apples **App Asset Library** zu App Store Connect
hoch – für jedes Gerät (iPhone, iPad, Mac, Apple Watch, Apple TV, Vision Pro,
iMessage, iPhone Duo) und für die Creatives **Kopfzeile** und
**Suchergebnisse**. Es gibt nichts auszuwählen. (Frühere Versionen konnten
Apples ältere Screenshot-Set-Schnittstelle nutzen; Apple hat sie als veraltet
markiert, und Vitrine Studio verwendet sie nicht mehr. Eine alte Zeile
`upload_flow` in `project.yaml` wird ignoriert und verschwindet beim nächsten
Speichern.)

Der API-Schlüssel braucht Zugriff auf die Asset Library der App. Ohne ihn
meldet der Plan (mit *Store-Stand prüfen*) *App Asset Library is not
available*, und in den App Store wird nichts hochgeladen – prüfe die Rolle des
Schlüssels in App Store Connect.

In der Asset Library heißen
von Vitrine Studio hochgeladene Bilder `<Datei> [<Fingerabdruck>]`; gleiche
Bilder werden über Sprachen hinweg wiederverwendet, und alte, von Vitrine
Studio hochgeladene Bilder werden entfernt, sobald nichts sie mehr nutzt (von
Hand hochgeladene Bilder bleiben).

**App-Store-Asset-Typen anzeigen** im Upload-Dialog (oder `vitrine upload
refdata`) listet nur lesend, was App Store Connect für deinen Schlüssel meldet
und wie Vitrine Studio seine Ziele darauf abbildet.

## Besonderheiten bei Google Play {#google-play-specifics}

- Bilder: PNG oder JPEG, bis 15 MB pro Bild, 2–8 pro Gerätetyp; die
  Reihenfolge beim Upload ist die Reihenfolge im Store.
- Die **Feature-Grafik** (`play-feature-graphic`) wird in derselben
  Bearbeitung hochgeladen, eine pro Sprache.
- Zum Veröffentlichen braucht Play mindestens 2 Smartphone-Screenshots und
  die Feature-Grafik: Die Prüfung warnt, wenn ein Satz zu wenige Bilder hat
  (`store_count`) oder dem Projekt ein Smartphone- oder Feature-Grafik-Ziel
  fehlt (`required_store_set_missing`).
- Lehnt Play die Übernahme ab und verlangt *Änderungen nicht zur Prüfung
  senden*, lade mit dieser Option erneut hoch (`--changes-not-sent-for-review`
  auf der Kommandozeile).

## Das Upload-Protokoll {#the-upload-log}

**Upload-Protokoll** im Dialog listet frühere Läufe mit ihren Sets und
Dateien. Alles wird in `<project>/.cache/uploads.sqlite` festgehalten – Läufe,
Sets, Dateien mit ihren Store-IDs und ihrem Zustand sowie die HTTP-Anfragen
(Methode, Pfad, Status) – nie Schlüssel oder Tokens. Ein Lauf ohne Fortschritt
über 10 Minuten wird gestoppt; der Lauf einer App, die während eines Uploads
geschlossen wurde, erscheint als *Unterbrochen*. Einfach neu planen und
hochladen.

## Auf der Kommandozeile {#on-the-command-line}

```sh
vitrine upload plan                                  # alle verbundenen Stores; gibt ein Bestätigungs-Token aus
vitrine upload plan --store apple --remote           # mit dem Store-Stand
vitrine upload plan --target play-phone --lang de    # nur ein Teil
vitrine upload execute --confirm <token>             # fragt im Terminal j/N
vitrine upload execute --confirm <token> --yes       # für Skripte
vitrine upload log --last --events
```

Exit-Codes: `plan` 1 = etwas ist blockiert; `execute` 0 = fertig, 1 = teilweise
fertig, fehlgeschlagen oder abgebrochen, 2 = abgelehnt (Token, Schlüssel). Die
CLI nutzt die Konten der App auf diesem Computer (`--app-dir` /
`VITRINE_APP_DIR` wählt einen anderen App-Datenordner).

## KI-Assistenten und Uploads {#ai-assistants-and-uploads}

Ein Assistent kann jederzeit einen Plan erstellen (`upload_plan`) – das ist
ein Probelauf. Um ihn **auszuführen**, braucht er deinen Klick: Vitrine Studio
zeigt „*Claude Desktop* möchte in die Stores hochladen“ mit dem Plan, und nur
**Upload bestätigen** startet ihn (unbeantwortete Anfragen werden nach 5
Minuten abgelehnt). Läuft die App nicht, können Assistenten gar nicht
hochladen, es sei denn, du erlaubst es ausdrücklich in der
Einstellungsdatei. Siehe [KI-Assistenten](../ai-mcp/#uploads).

## Fehlerbehebung {#troubleshooting}

| Meldung | Was tun |
|---|---|
| *Keine bearbeitbare App-Store-Version* | in App Store Connect eine neue Version anlegen, neu planen |
| *Schlüssel abgelehnt* | Issuer-ID / Schlüssel-ID prüfen, ob der Schlüssel widerrufen wurde, und die Uhr deines Computers |
| 0 Apps sichtbar | Rolle oder App-Zugriff des Schlüssels zu eng (App-Manager oder Admin) |
| *Google Play kennt dieses Paket nicht, oder das Dienstkonto hat keinen Zugriff* | Paketnamen prüfen; das Dienstkonto in der Play Console einladen |
| *keine Berechtigung für diese App* (Play) | dem Dienstkonto *Store-Präsenz verwalten* für die App geben; bis zu 24 h warten |
| *Blockierte* Sets | die Prüffehler dieses Ziels / dieser Sprache beheben oder den Plan eingrenzen |
| *Anfragelimit – später erneut versuchen* | warten, dann neu planen und hochladen |
| *Apple verarbeitet noch N Bild(er)* | nichts zu tun; später mit *Verarbeitung prüfen* nachsehen |
