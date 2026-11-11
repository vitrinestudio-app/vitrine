---
title: Templates & frames
description: "Start from templates, save and share your own, import Apple bezels and build frame packs."
weight: 100
icon: template
---

## Templates

A template is one or more canvases – e.g. phone, tablet and feature graphic as
one coordinated set – with their targets and variants, the styles, the texts
in every language, images and fonts. It is a `.vitrine-template` file.

- **Built in:** `phone` (three pages: iPhone 6.9″, Google Play, blog banner)
  and `minimal` (one page, iPhone).
- **New project from a template:** start screen → *New project* → pick a card,
  or `vitrine new my-app --template <name|file>`. Screenshots stay empty, so
  no placeholder can end up in the store.
- **Save as template:** *Canvases & targets* → a canvas → *Save as template…*
  (`vitrine template save --name N`). Your screenshots are replaced by
  generated placeholders; store settings stay behind.
- **Insert from template** adds canvases to an open project in one undo step;
  equal style names are kept apart (`accent` → `accent-2`) or merged.
- Licence: templates are **CC0**; fonts inside keep their OFL licence. Fonts
  without an OFL declaration are refused.
- Imports are checked: size limits, only known file kinds, no paths outside
  the package, SVG without scripts.

### Community gallery

A gallery at `api.vitrinestudio.app` is being set up: browsing and
downloading need no account, publishing needs a GitHub login, uploads are
moderated. Today: `vitrine gallery list`, `download`, `login`, `upload`;
browsing from the app follows.

## Device frames

Three sources, chosen per device or as the project default:

1. **Built-in frames**, drawn parametrically from catalog data (styles
   *stylized* and *detailed*) – iPhone, iPad, Mac, Android, watches, browser
   windows, notebooks. No vendor artwork.
2. **Apple Product Bezels** – download them yourself from Apple Design
   Resources, then *Import*. Open the DMG in Finder and accept the licence
   first, then import the mounted volume. They are stored app-wide, never in
   the project; projects just say `frame: imported:apple`. On a computer
   without them the built-in frame is used.
3. **Own frame packs** – PNG or SVG frames plus a `frame.yaml` (screen
   rectangle, mask or detected screen, colours, models). Keep them in the
   project's `frames/` folder (travels with Git) or app-wide.

```sh
vitrine frames import /Volumes/Bezel-iPhone-18 --dry-run
vitrine frames import clay-pack.zip --to project
vitrine frames list --packs
```

Check the terms of vendor kits before using them in store listings – some
allow mock-ups of your own app only, some must not be redistributed. Keep
vendor packs app-wide, never in shared projects.
