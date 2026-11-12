---
title: Templates & gallery
description: "Start projects from templates, save your own canvases as templates, insert them into a project, share .vitrine-template files and publish to the gallery."
weight: 120
icon: template
---

A **template** is one or more canvases – e.g. phone, tablet and feature graphic
as one coordinated set – with their targets and variants, the styles, the
texts in every language, images and fonts. It is a single
**`.vitrine-template`** file.

## Built-in and your own templates

- **Phone** – three pages with a headline and a phone each; targets iPhone
  6.9″, Google Play phone and a blog header. The default.
- **Minimal** – one iPhone page.
- **Your own** templates – saved, imported or downloaded – are kept in the app
  data folder (`templates/`) and available in every project.

## New project from a template

Start screen → **New project** → pick a template card (or **Import file…**
for a `.vitrine-template` file) → name, location, languages → **Create**.

- All canvases of the template come along.
- **Texts:** each of your languages gets the template's text in the same
  language, else in its base language (`de-AT` ← `de`), else the template's
  default language – reported as "copied, to translate".
- **Screenshots stay empty.** The template's placeholder screens are not
  copied, so validation keeps asking for your real screenshots and no
  placeholder can end up in the store.
- **Sample screenshots:** some templates bring the author's own dummy app
  screenshots so you can try the design right away. They are copied into
  `screenshots/` and listed in `screenshots/samples.yaml`. Validation shows
  the info `sample_screenshot` ("replace it with your own"), export works,
  but a **store upload refuses** every set that still contains a sample.
  Replace a file with your own screenshot (same name) and it is no longer a
  sample – nothing else to do.

```sh
vitrine new my-app --template phone --lang en --lang de
vitrine new my-app --template ./bold-gradient.vitrine-template
```

## Save as template

**Canvases & targets** → a canvas → **Save as template…**: choose the
*Canvases*, give a *Name* (the *Id* follows from it), *Description*, *Author*,
pick the *Tags*, optionally **Include my screenshots as sample screenshots**
and **Also save as a file** to share it. Saving again with the same id
replaces the template.

What goes in:

- the canvases, the targets showing them, their variants, all styles;
- only the texts the canvases use, in all languages;
- the images they use, and fonts from the project's `fonts/` folder that a
  style names;
- **not** your screenshots (replaced by generated placeholder screens in the
  device's aspect) – unless you include them as **sample screenshots**: then
  the screenshots the canvases use (PNG/JPEG, at most 10 MB and 6000 px per
  side) go in as they are. Only do this with screenshots that are yours
  (e.g. of a demo app); in the gallery they are published as CC0;
- **not** store settings, import mappings or imported vendor frames
  (previews use the built-in frames).

**Tags** come from a fixed list in three groups – no free words – so the
gallery can filter reliably:

| Group | How many | Values |
|---|---|---|
| Style (`--style`) | 1–2 | minimal, bold, playful, elegant, realistic, illustrated, editorial, retro, futuristic, organic |
| Colour (`--color`) | 1–2 | light, dark, colorful, pastel, monochrome, gradient, neon, earthy |
| App category (`--use`) | 1–3 | games, productivity, business, finance, health-fitness, sports, education, kids, food-drink, travel, navigation, social, lifestyle, shopping, entertainment, music, photo-video, news, weather, utilities, developer-tools, medical, home-garden, books, events |

A local template may leave groups empty; the gallery needs at least one per
group. The devices (phone, tablet, watch, mac, web, tv, banner) are worked
out from the targets – you don't set them. `vitrine template tags` lists
everything.

**Fonts:** only fonts that declare the SIL Open Font License may go into a
template; others are refused ("may not be redistributed" or "licence
unclear"). The licence file is kept or generated (`fonts/<name>-OFL.txt`).
Bundled fonts need nothing.

```sh
vitrine template save --name "Bold gradient" --canvas phone \
  --style bold --color gradient,dark --use travel -o bold.vitrine-template
vitrine template save --name "Demo set" --style minimal --color light --use productivity \
  --sample-screenshots                     # with your own screenshots as samples
vitrine template tags                      # all tag values
```

## Insert from template

Add canvases of a template to an **existing** project: **Canvases &
targets → + New canvas → From template**. Pick the template and its canvases;
the dialog shows what will happen. It's one undo step.

- Canvas and target ids that already exist get `-2`
  (`ios-iphone-69 → ios-iphone-69-2`), element ids the next free number.
- **Same style names:** a colour or style the project defines differently is
  either kept separately (*Keep both* – renamed, e.g. `accent-2`; the default)
  or replaced by the project's (*Use this project's*). Identical styles are
  shared. Your project's frame style stays.
- Texts with the same key and the same content are shared; others become
  `<template>.<key>`.
- Images and fonts are copied; identical files are reused.

```sh
vitrine template insert bold-gradient --canvas phone --styles use-project --dry-run
```

## Managing templates

```sh
vitrine template list
vitrine template import bold.vitrine-template [--replace]
vitrine template export bold-gradient -o bold.vitrine-template
vitrine template remove bold-gradient          # built-in templates can't be removed
```

Every template file is checked before it's used: size limits (50 MB packed,
150 MB unpacked), only known file kinds checked by content, no paths outside
the package or hidden files, SVG without scripts or external links, and the
project inside must load and validate.

## Licence

Templates are **CC0** (public domain) – anyone may use them for anything.
Fonts inside keep their OFL licence. Only save templates from material you
may share this way.

## The template gallery

{{< callout status >}}
The gallery at `api.vitrinestudio.app` is being set up. Browsing it from the
app and the website is not built yet; the command line can already list,
download and upload.
{{< /callout >}}

- **Browsing and downloading** need no account.
- **Publishing** needs a GitHub login and complete tags. The server checks
  every upload again, replaces all screenshots with its own placeholders,
  renders the previews itself, and queues the template for moderation.
  Templates of the official account are marked *official*.
- **Sample screenshots** stay in the template only when you confirm they are
  your own and may be published as CC0 (`--confirm-own-screenshots`);
  without the confirmation the server replaces them with placeholders.
- The gallery filters by style, colour, app category and device
  (`vitrine gallery list --style bold --use games,kids --devices phone`).

```sh
vitrine gallery list --query garden              # approved templates
vitrine gallery download bold-gradient           # → bold-gradient.vitrine-template
vitrine template import bold-gradient.vitrine-template

vitrine gallery login                            # GitHub device login
vitrine template export bold-gradient -o bold.vitrine-template
vitrine gallery upload bold.vitrine-template     # status "pending" until approved
vitrine gallery logout
```

**Many templates at once:** `vitrine gallery upload` takes several files.
`--dry-run` runs every local check (package, tags, sample screenshots) and
prints a summary table without sending anything; a real run continues after
a failed file and ends with the same summary (exit code 1 if any failed).

```sh
vitrine gallery upload dist/*.vitrine-template --dry-run --confirm-own-screenshots
vitrine gallery upload dist/*.vitrine-template --confirm-own-screenshots
```

`gallery login` prints a code and the address `https://github.com/login/device`;
enter the code there. No GitHub permissions are requested – only your public
login name and id are read. The session (90 days) is kept in the system
keychain. Your templates stay yours if you rename your GitHub account. Uploading
an id that belongs to someone else fails with "id taken"; uploading the same
version twice asks you to raise the version (`template save --version`).
