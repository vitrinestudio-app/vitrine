---
title: Store upload
description: "Set up an App Store Connect API key and a Google Play service account, then upload with plan → confirm → upload."
weight: 60
icon: upload
---

> **Status:** built and tested against fake stores; real-run testing with live
> store accounts is in progress.

## Accounts live on your computer

Store accounts are added once in **App settings → Stores**. The keys go into
the system keychain (macOS Keychain, Windows Credential Manager, Linux Secret
Service) – never into the project, never into logs. A project only names the
account it uses, so it can be shared through Git.

### App Store Connect API key

1. App Store Connect → **Users and Access** → **Integrations** → App Store
   Connect API → Team Keys → *Generate API Key*. Creating team keys needs an
   Admin account.
2. Give it the role **App Manager** (enough for screenshots).
3. Download the `.p8` file – Apple lets you do this **only once**. Note the
   **Issuer ID** shown above the key list.
4. In Vitrine Studio: *Add App Store Connect key…*, pick the file, enter the
   issuer ID. *Test* checks the key read-only.

### Google Play service account

1. Google Cloud console → enable the **Google Play Android Developer API** →
   IAM & Admin → Service accounts → create one → Keys → *Add key* → JSON.
2. Play Console → **Users and permissions** → invite the service account's
   e-mail with *Manage store presence* and *View app information* for your
   app(s).
3. In Vitrine Studio: *Add Google service account…* and pick the JSON file.

The app explains both step by step, with buttons that open the right consoles.

## Link the project

**Project settings → Stores**: choose the account and the app (the App Store
app picker lists every app your key can see; for Google Play type the package
name and click *Check*). Store languages per project language are filled in
from sensible defaults (`de → de-DE`, `he → iw-IL` on Play).

## Plan → confirm → upload

1. **Plan.** Renders the store targets, hashes every image and compares with
   the last upload – or, with *Check store state*, with the store itself.
   Unchanged sets are skipped; problems are listed with what blocks them.
2. **Confirm.** Nothing is sent before you confirm. The confirmation is only
   valid for that plan and project state (30 minutes).
3. **Upload.** Progress per set, cancel at any time; to resume, plan and upload
   again – finished sets are skipped.

```sh
vitrine upload plan --store apple --remote     # prints the plan and a confirm token
vitrine upload execute --confirm <token>       # --yes for scripts
vitrine upload log --last
```

- **App Store:** screenshot sets, and the App Asset Library for iPhone Duo and
  the App Store header and search-results creatives. Apple processes each
  image after the upload; still processing is not an error.
- **Google Play:** one edit for all languages, committed at the end – an error
  or cancel discards it, so nothing changes in Play. The feature graphic is
  uploaded with the screenshots.
- **AI assistants** can plan uploads; executing one needs your click on
  *Confirm upload* in the app.
