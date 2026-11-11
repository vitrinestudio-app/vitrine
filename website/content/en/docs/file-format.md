---
title: File format
description: "What is inside project.yaml and the i18n files – readable YAML you can diff, review and script."
weight: 90
icon: folder
---

A project is plain YAML in a canonical form, so diffs stay small. The app
rewrites it atomically after every change and reloads it when it changes on
disk.

## `project.yaml`

```yaml
format: vitrine/1
revision: 1
name: Fernwood
languages:
  default: en
  available: [en, de, fr, fr-CA]
styles:
  colors:
    accent: "#F6C453"
    ink: "#17251C"
  text:
    headline: {font: Inter Display, weight: 700, size: 90, color: "@paper", hyphenate: true}
  spans:
    accent: {color: "@accent"}
canvases:
- id: phone
  page: {aspect: 1320:2868, count: 6, ids: [plan, beds, companions, neighbours, harvest, calendar]}
  backgrounds:
  - kind: linear-gradient
    angle: 115
    stops: [["#0B3324", 0], ["#1D5C68", 1]]
  elements:
  - id: title-plan
    kind: text
    page: 0
    text: plan.title          # key in i18n/<lang>.yaml
    style: headline
    box: {top: 170, left: 70, right: 70}
    fit: {shrink_to: 64, max_lines: 2}
  - id: phone-plan
    kind: device
    page: 0
    anchor_y: bottom
    at: {x: 500, y: 1390}     # lu, relative to the home page
    width: 720
    model: iphone-island
    screenshot: home          # screenshots/<lang>/<variant>/home.png
targets:
- id: ios-iphone-69
  canvas: phone
  # preset, size, variant, device map, store slot …
```

| Key | Meaning |
|---|---|
| `languages` | default language, available languages, optional `fallback` chains |
| `styles` | colours, text styles, span styles, shadows, the default frame style |
| `canvases` | pages (aspect, count, ids), backgrounds, elements, variants |
| `targets` | which canvas, at which size, with which variant; store slot |
| `store` | App Store / Google Play mapping and store locales |

Element kinds: `text`, `device`, `image`, `shape`, `background` and `group`.
Positions and sizes are in **lu** (a page is 1000 lu wide); rotation is in
degrees, positive = clockwise.

## `i18n/<lang>.yaml`

```yaml
plan.title: Your garden, **planned in minutes**
beds.title: Plan every bed by **drag & drop**
callout.score_label: Companion\-score
```

One flat map of keys per language, with inline markup. A regional file (e.g.
`fr-CA.yaml`) only needs the keys that differ.

## Frame packs

Own frames are a folder with PNG/SVG images and a `frame.yaml`
(format `vitrine-frame/1`): name, licence, the models they stand for, and per
frame the file, colour, orientation and screen area (detected, a rectangle
with corner radius, or a mask). See [Templates & frames](../templates-frames/).
