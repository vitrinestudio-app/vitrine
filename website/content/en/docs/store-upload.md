---
title: Store upload
description: "Set up an App Store Connect API key and a Google Play service account, link your project, then upload with plan → confirm → upload."
weight: 110
icon: store
---

{{< callout status >}}
Store upload is built and tested against simulated stores. Testing with live
store accounts is in progress – check the result in App Store Connect and Play
Console after your first uploads, and please report anything unexpected.
{{< /callout >}}

Vitrine Studio uploads your screenshots, the Google Play feature graphic and
the App Store creatives directly. It always works in
three steps: **plan → confirm → upload**. Nothing reaches a store without a
plan you have seen and confirmed.

## Overview

1. Once per computer: add your store accounts in **App settings → Stores**.
2. Once per project: link the app in **Project settings → Stores**.
3. Each time: **☰ → Upload to stores…**, check the plan, **Confirm &
   upload**.

## 1. Store accounts (once per computer)

Accounts belong to your computer, not to the project: a project can be shared
through Git while the keys stay with you. The key itself goes into the
**system keychain** (macOS Keychain, Windows Credential Manager, Linux Secret
Service, service `app.vitrinestudio`); the app settings only reference it.
Vitrine Studio never writes keys into the project or into logs, and never
shows their content.

Your keys never leave your computer: Vitrine Studio sends them to no server of
its own and to no cloud. The app uses the key to sign a short-lived sign-in
locally and sends only that, directly to Apple (App Store Connect API) or
Google (Google sign-in and the Play Developer API).

### App Store Connect API key

The app has these steps too (**How do I get an App Store Connect key?**, with
buttons that open the right pages):

1. Sign in to App Store Connect with an account that has the **Admin** role –
   only Admins can create team keys.
2. Open **Users and Access → Integrations → App Store Connect API → Team
   Keys**.
3. Click **Generate API Key** (or +), enter a name (e.g. "Vitrine Studio") and
   choose the access **App Manager**. Click **Generate**.
4. Download the key (`AuthKey_<Key ID>.p8`). Apple lets you download it **only
   once** – keep the file safe.
5. Note the **Issuer ID** (shown above the key list) and the **Key ID** (in
   the key's row).
6. In Vitrine Studio: **App settings → Stores → Add App Store Connect key…**,
   choose the `.p8` file and enter the Issuer ID. The Key ID is read from the
   file name. Optionally give the account a *Name* (e.g. your team).

Lost or leaked key: revoke it on the same page and create a new one. Individual
API keys (without Issuer ID) are not supported yet – use a team key.

### Google Play service account

1. In the [Google Cloud console](https://console.cloud.google.com/), create a
   project or choose an existing one.
2. Enable the **Google Play Android Developer API** for this project
   (*APIs & Services → Library*).
3. **IAM & Admin → Service accounts → Create service account** (name e.g.
   "vitrine-upload").
4. Open the service account → **Keys → Add key → Create new key → JSON**. The
   JSON file is downloaded – keep it safe.
5. In **Play Console: Users and permissions → Invite new users** → enter the
   service account's e-mail address (`…@….iam.gserviceaccount.com`).
6. Under **App permissions** add your app(s) and allow **View app information
   (read-only)** and **Manage store presence** (store listing texts and
   images). Send the invitation.
7. New permissions can take up to 24 hours to become active.
8. In Vitrine Studio: **App settings → Stores → Add Google service account…**
   and choose the JSON file.

### Testing an account

**Test** next to an account checks it without changing anything:

- **App Store:** signs a token and lists your apps – "key accepted, N apps
  visible". 0 apps means the key's role or app access is too narrow.
- **Google Play:** only proves that Google accepts the key. Whether the
  service account may edit your app shows **Check** in the project settings
  (below).

Typical errors: *key rejected* (wrong Issuer ID or Key ID, revoked key, or
your computer's clock is wrong), *no access* (role too low), *cannot reach … –
offline?*, or a server problem / rate limit (try again later).

**Remove account** also deletes the key from the keychain. Projects keep the
reference and show "not found" until you add an account with that id again.

{{< shot src="images/store-accounts.jpg" alt="App settings, section Stores: an App Store Connect key and a Google Play service account, each with a Test button, and the expanded help for App Store Connect keys." caption="App settings → **Stores**: two accounts and the help for App Store Connect keys." >}}

## 2. Link the project

**Project settings → Stores**:

- **App Store:** **Set up App Store**, choose the *Account*, then **Choose
  app…** – it lists every app the key can see and fills in the *Apple ID*
  (the numeric id from App Store Connect → App Information) and the *Bundle
  ID*. Set the *Platform*: *iOS / iPadOS* or *macOS*. (You can also type the
  ids.)
- **Google Play:** **Set up Google Play**, choose the *Account* and type the
  *Package name* (shown under the app name in Play Console – Google can't list
  a service account's apps). **Check** asks Google Play whether the package
  exists and the service account may edit it: it opens a temporary draft,
  reads the app's title and default language, and deletes the draft again –
  nothing changes.
- **Store languages:** which store language each project language is uploaded
  as. Empty = the built-in default (`de → de-DE`, `fr-CA → fr-CA`, Hebrew on
  Play `iw-IL`, on Apple `he`). Two project languages can't share one store
  language (`de` and `de-AT` → map `de-AT` elsewhere or leave it out).

Only stores listed here are uploaded. Which images go where comes from each
target's **Store slot** (App Store display type / Play image type) – the presets
set it for you.

{{< shot src="images/project-stores.jpg" alt="Project settings, Stores tab: account, app, Apple ID, bundle ID and platform for the App Store; account and package name for Google Play; store languages." caption="Project settings → **Stores**: the project linked to both stores." >}}

## 3. Plan, confirm, upload

**☰ → Upload to stores…** (or **Upload…** in the export dialog). One tab per
store the project is linked to.

### Before you can upload

If something is missing, the dialog lists the steps in order, each with a
button: add an account (*Open app settings*), link the project or fix store
languages (*Open project settings*), give targets a store slot (*Open
targets*). The list refreshes when you close the opened dialog.

### The plan

The plan renders the store targets (incrementally, like the export), puts the
images in store order, fingerprints them and compares them with the last
successful upload. The table shows per target and language the *Store locale*,
the *Action* and the number of images:

| Action | Meaning |
|---|---|
| **Upload** | a new set |
| **Replace** | the set changed and will be replaced |
| **Unchanged** | same images as last time – skipped |
| **Skipped** | e.g. a language without store language, or an Apple target of the other platform |
| **Blocked** | can't be uploaded: validation errors (missing screenshots, wrong size, too many images), file too large, two targets or languages on one store set |

Options:

- **Check store state** also asks the store what it holds now (needs network
  and the key). Apple: the app, an editable version, the existing
  localizations and screenshots with their checksums; Google: opens a
  temporary edit (deleted again) and compares the images. A set the store
  already has identically shows *Unchanged*.
- **Upload unchanged sets too** forces a complete upload.

{{< shot src="images/upload-plan.jpg" alt="The upload plan for the App Store: six sets (two targets × three languages) marked Upload, and the Confirm & upload button." caption="The plan: six sets to upload, nothing sent yet." >}}

### Confirm and upload

**Confirm & upload** asks once more ("… set(s) with … image(s) go to … and
replace the screenshots there"), then shows progress per set. **Cancel
upload** stops at any time. A confirmation belongs to
exactly one plan: it is valid for 30 minutes and for one upload. Before
uploading, Vitrine Studio checks that the project and the images are still
exactly as they were in the plan. If you changed something since, or the 30
minutes are over, nothing is uploaded – just make a new plan. That way nothing
reaches the store except what you saw in the plan.

- **App Store:** each set is made to hold exactly the planned images in
  order: screenshots already there with the same content are kept, the others
  deleted, the missing ones uploaded, then the order is set. Missing
  localizations and sets are created.
- **Google Play:** one edit for all languages; per language and image type
  all images are replaced, then the edit is committed once. Any error or
  cancel discards the edit – nothing changes in Play.

**To resume** after an error or cancel: plan and upload again – finished sets
are *Unchanged* and skipped.

## Apple specifics

**An editable version is needed.** Apple only accepts screenshot changes on an
App Store version that can still be edited ("Prepare for Submission",
rejected …). Otherwise the plan says *No editable App Store version. Create a
new version (e.g. 2.1.1) in App Store Connect, then plan again.* with a button
**Open App Store Connect**.

**Processing.** Apple processes each uploaded image – usually seconds,
sometimes over ten minutes. Vitrine Studio waits up to 15 minutes per set.
Images still processing after that are **not** an error: the set shows
*Uploaded, Apple processing*; they are in App Store Connect and won't be
uploaded again. Check later with **Check processing**. If Apple fails to
process an image (e.g. wrong dimensions), the set fails with Apple's message
and the next run uploads only that image.

**Rate limits.** Apple allows about 3500 requests per hour per key. On a rate
limit the run pauses (*Paused*, up to 15 minutes) and retries.

Apple also wants each language in the app information before you submit – App
Store Connect shows that.

### App Store upload method {#app-store-upload-method}

Vitrine Studio uploads to App Store Connect through Apple's **App Asset
Library** – for every device (iPhone, iPad, Mac, Apple Watch, Apple TV,
Vision Pro, iMessage, iPhone Duo) and for the **header** and **search
results** creatives. There is nothing to choose. (Earlier versions could use
Apple's older screenshot-set interface; Apple deprecated it, and Vitrine
Studio no longer uses it. An old `upload_flow` line in `project.yaml` is
ignored and disappears on the next save.)

The API key needs access to the app's asset library. Without it the plan (with
*Check store state*) says *App Asset Library is not available* and nothing is
uploaded to the App Store – check the key's role in App Store Connect.

In the asset library, images
uploaded by Vitrine Studio are named `<file> [<fingerprint>]`; identical images
are reused across languages, and old images it uploaded are removed once
nothing uses them (images you uploaded by hand stay).

**Show App Store asset types** in the upload dialog (or `vitrine upload
refdata`) lists, read-only, what App Store Connect reports for your key and how
Vitrine Studio maps its targets.

## Google Play specifics

- Images: PNG or JPEG, up to 15 MB each, 2–8 per device type; the order of
  the upload is the order in the store.
- The **feature graphic** (`play-feature-graphic`) is uploaded in the same
  edit, one per language.
- If Play refuses the commit and asks for *changes not sent for review*,
  upload again with that option (`--changes-not-sent-for-review` on the
  command line).

## The upload log

**Upload log** in the dialog lists past runs with their sets and files.
Everything is recorded in `<project>/.cache/uploads.sqlite` – runs, sets,
files with their store ids and state, and the HTTP requests (method, path,
status) – never keys or tokens. A run without progress for 10 minutes is
stopped; a run of an app that was closed during an upload shows as
*Interrupted*. Just plan and upload again.

## On the command line

```sh
vitrine upload plan                                  # all linked stores; prints a confirm token
vitrine upload plan --store apple --remote           # with the store state
vitrine upload plan --target play-phone --lang de    # only part of it
vitrine upload execute --confirm <token>             # asks y/N in a terminal
vitrine upload execute --confirm <token> --yes       # for scripts
vitrine upload log --last --events
```

Exit codes: `plan` 1 = something is blocked; `execute` 0 = completed, 1 =
partly done, failed or cancelled, 2 = refused (token, keys). The CLI uses the
accounts of the app on this computer (`--app-dir` / `VITRINE_APP_DIR` selects
another app data folder).

## AI assistants and uploads

An assistant can make a plan (`upload_plan`) at any time – it's a dry run. To
**execute** one, it needs your click: Vitrine Studio shows "*Claude Desktop*
wants to upload to the stores" with the plan, and only **Confirm upload**
starts it (unanswered requests are declined after 5 minutes). Without the app
running, assistants can't upload at all unless you explicitly allow it in the
settings file. See [AI assistants](../ai-mcp/#uploads).

## Troubleshooting

| Message | What to do |
|---|---|
| *No editable App Store version* | create a new version in App Store Connect, plan again |
| *key rejected* | check Issuer ID / Key ID, whether the key was revoked, and your computer's clock |
| 0 apps visible | the key's role or app access is too narrow (App Manager or Admin) |
| *Google Play does not know this package, or the service account has no access* | check the package name; invite the service account in Play Console |
| *no permission for this app* (Play) | give the service account *Manage store presence* for the app; wait up to 24 h |
| *Blocked* sets | fix the validation errors of that target/language, or narrow the plan |
| *rate limited – try again later* | wait, then plan and upload again |
| *Apple is still processing N image(s)* | nothing to do; check later with *Check processing* |
