---
title: Privacy policy
slug: privacy
eyebrow: "Legal"
description: "Privacy policy of Vitrine Studio: website, template gallery (api.vitrinestudio.app) and desktop app."
noindex: true
---

{{< callout note >}}
This text is a translation for information only. Only the German original
version, the [Datenschutzerklärung](/de/datenschutz/), is legally binding.
{{< /callout >}}

{{< callout warn >}}
**Before going live:** items marked “[OPEN: …]” still have to be filled in
by the owner.
{{< /callout >}}

This privacy policy informs you about the nature, scope and purpose of the
processing of your personal data. It applies to all services of Vitrine
Studio by Looking4Cache UG (haftungsbeschränkt) (hereinafter “we” or “our”).
These are:

- the website https://www.vitrinestudio.app, hereinafter “website”;
- the template gallery with its interface https://api.vitrinestudio.app,
  hereinafter “gallery”;
- the desktop app and the command line tool Vitrine Studio, hereinafter
  “app”.

The collection and processing of your personal data is carried out in
compliance with the applicable data protection regulations, in particular
the General Data Protection Regulation (hereinafter “GDPR”).

**In short:** the website sets no cookies and uses no analytics,
advertising or tracking services. The app runs on your computer, needs no
account and sends us no usage data. We process personal data mainly in the
technically necessary access logs and – if you upload templates – your
GitHub username.

## 1. Name and address of the controller

The controller within the meaning of the GDPR and other national data
protection laws of the member states as well as other data protection
provisions is:

Looking4Cache UG (haftungsbeschränkt)<br>
Oberer Wasen 12<br>
74626 Bretzfeld<br>
Germany<br>
Email: [OPEN: contact address, as in the [legal notice](/legal-notice/)]

We have not appointed a data protection officer, as we are not required to
do so. [OPEN: please confirm]

## 2. Legal bases of the processing

In accordance with Art. 13 GDPR, we state the legal basis of each
processing in its section. We rely on:

- **Art. 6(1)(b) GDPR** – the processing is necessary for the performance of
  the user relationship you enter into with us when you sign in and publish
  templates in the gallery (see the
  [terms of use of the template gallery](/terms/));
- **Art. 6(1)(c) GDPR** – the processing is necessary for compliance with a
  legal obligation (e.g. handling notices of illegal content under the
  Digital Services Act);
- **Art. 6(1)(f) GDPR** – the processing is necessary for our legitimate
  interests (secure and stable operation, protection against abuse and
  overload).

We do not ask for consent under Art. 6(1)(a) GDPR because we do not carry
out any processing that requires consent. We store or read information on
your device only where this is strictly necessary for a service you
explicitly requested (§ 25(2) no. 2 TDDDG).

## 3. Website

### 3.1. Hosting and web server log files

The website consists of static pages. It is hosted by [OPEN: hosting
provider / data centre, name and address, server location; data processing
agreement under Art. 28 GDPR].

Every access to the website is recorded by the web server in log files.
The following is stored:

- IP address [OPEN: full or shortened/anonymised]
- date and time
- requested URL, HTTP method and status, amount of data transferred
- referrer URL (the previously visited page), if your browser sends it
- browser identification (user agent)

[OPEN: check the scope of logging against the actual web server
configuration.]

The purpose of logging is to deliver the website, to trace errors and to
fend off attacks. The data is not linked to your person.

**Legal basis:** our legitimate interest in secure and stable operation,
Art. 6(1)(f) GDPR.

**Storage period:** the logs are deleted automatically after [OPEN: number]
days.

### 3.2. No cookies, local storage for the colour scheme

The website sets no cookies. If you explicitly switch between the light and
dark scheme with the toggle in the header, your browser stores that choice
in its local storage (`localStorage`, entry `theme` with the value `light`
or `dark`) so the page looks the same on your next visit. The entry stays on
your device, is not transmitted to us and can be deleted at any time in your
browser settings.

**Legal basis:** § 25(2) no. 2 TDDDG (a function you requested) and
Art. 6(1)(f) GDPR, insofar as any personal reference exists.

### 3.3. No third-party services

The website's fonts, images, style sheets and scripts are served from our
own server. We use no analytics or statistics services, no advertising, no
social media plugins and no embedded videos or maps. Visiting the website
therefore transmits no data to third parties.

### 3.4. Template gallery on the website

The gallery page and the home page show templates from the gallery. For
this, your browser loads the list of templates and the preview images
directly from our server api.vitrinestudio.app. No cookies or credentials
are sent. Section 4.1 applies to these requests.

### 3.5. Links to GitHub

The source code, the app downloads and the container image are hosted on
GitHub (see section 6). The website only links to them. A connection to
GitHub is made only when you click such a link, and GitHub's privacy
provisions apply.

## 4. Template gallery (api.vitrinestudio.app)

In the gallery you can browse and download templates without an account.
To upload your own templates, you sign in with your GitHub account. The
gallery is hosted by [OPEN: hosting provider / data centre, name and
address, server location; data processing agreement under Art. 28 GDPR].

### 4.1. Requests, logs and rate limiting

Every request to the gallery – from the website, the app or the command
line – passes a web server in front of it (reverse proxy) that logs
requests as described in section 3.1. [OPEN: scope and storage period of
these logs.] The app and the command line send the program identifier
`VitrineStudio/<version number>` as user agent.

To protect against abuse and overload, the gallery limits the number of
requests per sender and hour (e.g. reads, uploads, reports, sign-in
attempts). To do so, it counts requests per IP address in a one-hour
window. These counters are kept in memory only, are never written to disk
and are lost whenever the server restarts.

Downloading a template increases the template's download counter; who
downloaded it is not stored.

**Legal basis:** our legitimate interest in secure and stable operation,
Art. 6(1)(f) GDPR.

### 4.2. Sign-in with GitHub

You sign in via GitHub (in the app and the command line with a code you
confirm on github.com; on websites via a redirect to GitHub). You sign in
directly with GitHub; we never receive your password. We request no
additional permissions and receive from GitHub only your **public
username** and your **numeric GitHub ID**. We discard GitHub's access token
immediately after reading these two values.

We then create our own session key. On our server we store only a
cryptographic hash of it together with the username, GitHub ID, creation
and expiry time. The key itself is kept by the app or command line in your
operating system's keychain.

When you sign in via a website, the server sets a technically necessary
cookie (`vitrine_oauth_state`) for at most ten minutes that protects the
sign-in against forgery; it is no longer needed after you return from
GitHub.

**Legal basis:** Art. 6(1)(b) GDPR (providing the upload function); for the
sign-in cookie also § 25(2) no. 2 TDDDG.

**Storage period:** a session is valid for [OPEN: 90 days, unless the server
setting was changed]. Signing out deletes it immediately; expired sessions
are no longer accepted and are deleted from the server at the next sign-in.
Used or expired intermediate sign-in states are deleted as well.

GitHub is responsible for the sign-in at GitHub (see section 6).

### 4.3. Uploaded templates

When you upload a template, we store the template package (name,
description, optional author name, tags, layouts, fonts and – only if you
confirm they are your own – sample screenshots) together with your GitHub
username, your GitHub ID and the time of the upload. The server
automatically replaces any other screenshots with placeholders.

Templates are published only after we have reviewed them. The gallery then
publicly shows: the content of the template, name, description, author
name, **your GitHub username as uploader**, timestamps and the number of
downloads. Published templates are licensed under CC0 1.0 and can be
downloaded and reused by anyone. Please do not include personal data of
third parties in templates.

**Legal basis:** Art. 6(1)(b) GDPR (publishing the template at your
request; the GitHub ID ensures that only you can upload new versions of
your templates).

**Storage period:** as long as the template is offered in the gallery. On
request by email we delete your templates and the related data. We delete
rejected templates [OPEN: period, e.g. within 30 days]. We cannot recall
copies that have already been downloaded.

### 4.4. Reports of templates

You can report templates that infringe rights or are illegal. We store the
reported template, the chosen reason, your free text and the time – not
your IP address and no account. If you include contact details in the free
text, we process them to be able to reply to you.

**Legal basis:** Art. 6(1)(c) GDPR (obligations under the Digital Services
Act) and Art. 6(1)(f) GDPR (protecting the gallery against illegal
content).

**Storage period:** until the report has been dealt with and then
[OPEN: period].

## 5. Desktop app and command line

The app runs entirely on your computer. It needs no account, your projects
stay as files in the folders you choose, and it contains no telemetry, no
usage statistics and no automatic error reporting. It connects to the
internet only in the following cases:

- **Update check:** at most once a day at start-up, the desktop app asks
  GitHub (github.com, Vitrine Studio releases) whether a new version is
  available; only after you agree does it download the update from there.
  GitHub receives the usual connection data (e.g. IP address, time); we
  receive no data. The automatic check can be switched off in the app's
  settings.
- **Template gallery:** only when you browse, download or upload templates
  or sign in; section 4 applies.
- **Store upload:** when you upload screenshots to the App Store or Google
  Play, the app connects with your own access keys directly from your
  computer to App Store Connect (Apple) or the Google Play Developer API
  (Google). This data does not pass through our servers; we receive none of
  it. Apple or Google are responsible for the processing there under your
  own agreements with them. The app stores your access keys only in your
  operating system's keychain and the upload log in the project folder.
- **Links:** help and console links (e.g. to Apple or Google) open in your
  browser.

The interface for AI assistants (MCP) is reachable only on your own
computer (127.0.0.1) and protected by a key. What an AI assistant you
connect sends to its provider is governed by that provider's terms; we
receive none of it. The “Add to Claude Desktop” function only changes a
configuration file on your computer.

## 6. GitHub

The source code, issue reports, contributions, the app downloads and the
container image (ghcr.io) are provided on GitHub; signing in to the gallery
also happens via GitHub. When you use GitHub, GitHub processes your data as
an independent controller under its own terms and privacy statement. Data
may be transferred to the USA in the process. GitHub has committed to the
EU-US Data Privacy Framework (DPF) and is certified; the DPF ensures an
adequate level of data protection for processing in the USA. The
certification can be viewed here:
https://www.dataprivacyframework.gov/s/participant-search

GitHub, Inc., 88 Colin P. Kelly Jr. Street, San Francisco, CA 94107, USA<br>
Website: https://github.com<br>
Privacy statement: https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement

## 7. Recipients and transfers to third countries

We do not sell data or pass it on for advertising. Recipients of personal
data are only the hosting providers of the website and the gallery
(sections 3.1 and 4), with whom we conclude data processing agreements, and
authorities where we are legally obliged to disclose. We do not transfer
data to third countries [OPEN: confirm once the hosting providers are
settled]; for GitHub see section 6.

## 8. Storage period

Unless stated otherwise in the individual sections, we delete data as soon
as it is no longer needed for its purpose and no statutory retention
obligation prevents deletion. If a statutory retention obligation prevents
deletion, processing is restricted: the data is blocked and not used for
other purposes.

## 9. Your rights

Under the applicable laws you have various rights regarding your personal
data. If you wish to exercise these rights, please send your request by
email or by post, clearly identifying yourself, to the address given in
section 1. For requests about the gallery, your GitHub username helps us.

- **Access (Art. 15 GDPR):** you can ask for confirmation whether we process
  data concerning you, and for access to that data including a copy, its
  purposes, recipients, storage period and origin, and your further rights.
- **Rectification (Art. 16 GDPR):** you can have inaccurate data corrected
  and incomplete data completed.
- **Erasure (Art. 17 GDPR):** you can ask for your data to be erased without
  undue delay, in particular when it is no longer needed for its purpose or
  was processed unlawfully. If we have made the data public, we inform
  other controllers of your request as far as possible.
- **Restriction of processing (Art. 18 GDPR):** under the statutory
  conditions you can ask us to restrict the processing of your data.
- **Data portability (Art. 20 GDPR):** you can receive the data you provided
  to us on the basis of a contract in a structured, commonly used and
  machine-readable format, or have it transmitted to another controller.
- **Objection (Art. 21 GDPR):** on grounds relating to your particular
  situation, you can object at any time to processing based on
  Art. 6(1)(f) GDPR. We then no longer process the data unless we can
  demonstrate compelling legitimate grounds or the processing serves the
  establishment, exercise or defence of legal claims.
- **Complaint to a supervisory authority (Art. 77 GDPR):** you have the
  right to lodge a complaint with a supervisory authority, in particular in
  the member state of your habitual residence, place of work or place of the
  alleged infringement. The authority responsible for us is: Der
  Landesbeauftragte für den Datenschutz und die Informationsfreiheit
  Baden-Württemberg, Lautenschlagerstraße 20, 70173 Stuttgart, Germany,
  https://www.baden-wuerttemberg.datenschutz.de

## 10. Automated decision-making including profiling

There is no automated decision-making including profiling within the
meaning of Art. 22 GDPR. The server checks uploaded templates technically
(e.g. file types and sizes); we decide on publication ourselves.

## 11. Obligation to provide data

You are not obliged to provide us with any data. Without the connection
data, however, the website and the gallery cannot be delivered, and without
signing in with GitHub you cannot upload templates.

## 12. Security measures

In accordance with Art. 32 GDPR we take appropriate technical and
organisational measures. The website and the gallery transmit data only in
encrypted form (HTTPS). We store session keys only as hashes.

## 13. Changes

We adapt this privacy policy when our services or the legal situation
change. The version published here applies.

Last updated: 13 November 2026
