---
title: Project file format
description: "The project folder and every section of project.yaml – languages, styles, canvases, elements, variants, targets, store – plus the translation files, with examples from the Fernwood project."
weight: 150
icon: file
---

You never have to touch the files – the app and the CLI write them. But they
are plain, readable YAML, so you can review changes in Git, script them, or fix
something quickly in a text editor. Examples on this page come from
*Fernwood*, the example project in the repository
(`tests/fixtures/example`).

## The project folder

| Path | Content | In Git? |
|---|---|---|
| `project.yaml` | the layout: languages, styles, canvases, targets, store mapping | yes |
| `i18n/<lang>.yaml` | the texts of one language | yes |
| `screenshots/<lang>/[<variant>/]<key>.png` | your app screenshots ([details](../screenshots/)) | yes |
| `assets/` | images for image elements and image fills | yes |
| `fonts/` | project fonts (`.ttf`, `.otf`) | yes |
| `frames/<id>/` | own frame packs that travel with the project | yes |
| `export/` | exported images (build output) | no |
| `.cache/` | render cache, upload images, upload log `uploads.sqlite` | no |

`vitrine new` writes a `.gitignore` with `export/` and `.cache/`.

## How the files are written

- **Canonical form:** the app always writes the same layout and order, so a
  change shows up as a small diff.
- **Atomic saves:** every change is written to a temporary file and moved into
  place – a crash never leaves half a file.
- **`revision`** counts up with every save. If the file changed on disk
  meanwhile (another editor, `git pull`), the app reloads instead of
  overwriting it.
- **Unknown fields are errors** – a typo is reported with the list of allowed
  fields, never silently ignored.

## `project.yaml` at a glance

```yaml
format: vitrine/1
revision: 1
name: Fernwood
languages:
  default: en
  available: [en, de, fr, fr-CA]
  # fallback: {nl: de}          # optional; fr-CA → fr is automatic
styles:
  colors:
    accent: "#F6C453"
    paper: "#FFFFFF"
  text:
    headline: {font: Inter Display, weight: 700, size: 90, line_height: 1.08, color: "@paper", hyphenate: true}
    sub: {font: Inter, weight: 400, size: 40, line_height: 1.3, color: "@soft"}
  spans:
    accent: {color: "@accent"}
  shadows:
    card:
      layers:
      - {color: "#0000004D", blur: 40, y: 22}
  # frame: detailed               # default frame: stylized | detailed | imported:apple | pack:<id>
  # fonts: {Inter: {fallback: [Noto Sans JP]}}
canvases:
- id: phone
  page: {aspect: 1320:2868, count: 6, ids: [plan, beds, companions, neighbours, harvest, calendar]}
  backgrounds:
  - kind: linear-gradient
    angle: 115
    stops: [["#0B3324", 0], ["#145A3C", 0.5], ["#1D5C68", 1]]
  elements:
  - id: title-plan
    kind: text
    page: 0
    text: plan.title
    style: headline
    box: {top: 170, left: 70, right: 70}
    align: center
    fit: {shrink_to: 64, max_lines: 2}
  - id: phone-plan
    kind: device
    page: 0
    anchor_y: bottom
    at: {x: 500, y: 1390}
    width: 720
    model: iphone-island
    color: natural-titanium
    screenshot: home
    shadow: soft
  variants:
    tablet-landscape:
      content_scale: 0.92
      overrides:
        phone-plan: {at: {x: 500, y: 1683}}
targets:
- id: ios-iphone-69
  canvas: phone
  size: [1320, 2868]
  screenshots: ios-phone
  gallery: appstore-iphone
  store: {apple: {display_type: APP_IPHONE_67}}
store:
  apple: {account: team-a, app_id: "1234567890", bundle_id: com.example.fernwood}
  google: {account: play, package_name: com.example.fernwood}
```

## `languages`

| Field | Meaning |
|---|---|
| `default` | the default language: source for translators, last fallback |
| `available` | all languages, in display order |
| `fallback` | optional extra fallbacks, e.g. `{nl: de}`; regional codes fall back to their base language automatically |

## `styles`

| Field | Meaning |
|---|---|
| `colors` | named colours, used as `@name` |
| `text` | text styles: `font`, `weight`, `italic`, `size`, `line_height`, `tracking`, `color`, `background`, `transform` (`none`/`uppercase`/`lowercase`), `shadow`, `stroke`, `glow`, `hyphenate` |
| `spans` | inline styles for `[name]…[/]`: `color`, `weight`, `italic`, `font`, `background`, `stroke`, `glow`, `shadow` |
| `shadows` | shadow presets: `{layers: [{color, blur, x, y}]}`; `soft`, `hard`, `none` are built in |
| `frame` | the project's default device frame |
| `fonts` | per family: `fallback` families for missing characters |

## `canvases`

| Field | Meaning |
|---|---|
| `id` | the canvas id |
| `page` | `aspect` (`width:height` of one page), `count`, optional `ids` (name the export files) |
| `backgrounds` | fills running across the whole canvas, bottom first |
| `elements` | the elements, painting order: first = bottom |
| `variants` | per variant: `content_scale` and `overrides` (element id → fields) |

**Fills** are a colour (`"#0B3324"`, `"@deep"`), a `linear-gradient` (`angle`
in degrees, 0 = to top, 90 = to right; `stops: [[colour, 0..1], …]`), a
`radial-gradient` (`center`, `radius`, `stops`) or an `image` (`asset`, `fit`:
`cover`/`contain`/`stretch`/`tile`, `focus`).

## Elements

Every element has an `id`, a `kind` and a home `page` (0-based). Units are lu
(a page is 1000 lu wide); rotation in degrees, positive = clockwise.

**Common fields:**

| Field | Meaning |
|---|---|
| `page` | home page (0-based); `at` is relative to it |
| `at` | `{x, y}` – centre of the element, from the top-left corner of the home page |
| `width`, `height` | size in lu |
| `rotation`, `pivot` | degrees; pivot as offset from the centre |
| `anchor_y` | `top` (default), `bottom`, `center`, `stretch` – which edge it keeps its distance to when a target's page height differs |
| `opacity`, `z` | 0..1; layer (painting order is z, then list order) |
| `clip` | `page` (default for texts) or `none` |
| `visible`, `visible_in` | hide it; or show it only in some `targets` / `langs` |
| `lock_page`, `locked` | editor helpers: keep the home page when dragged; not selectable on the canvas |

**Per kind:**

| Kind | Fields |
|---|---|
| `device` | `model` (required), `orientation`, `frame`, `bezel`, `color`, `fold` (`open`/`folded`), `chrome` (`none`/`browser`), `url`, `appearance` (`light`/`dark`), `screenshot` (the key), `fit` (`cover`/`contain`/`stretch`), `shadow`; the height follows the model |
| `text` | `text` (the key, required), `style`, `box` (`{top, left, right, bottom}` margins to the page edges – without `bottom` the box grows down), `align`, `valign`, `fit` (`{shrink_to, max_lines}`), `color`, `font_size`, `stroke`, `glow`, `shadow`, `hyphenate`, `langs` |
| `image` | `asset` (e.g. `assets/badge.png`), `fit`, `shadow`, `clip_to` (id of a `rect`/`ellipse` shape on the canvas: the image is cut to its outline; a shape with `visible: false` is only the mask) |
| `shape` | `shape` (`rect`/`ellipse`/`line`), `radius`, `fill`, `stroke`, `shadow` |
| `background` | `fill` – fills the home page |
| `group` | `children` – elements moved, rotated and scaled together; `scale` (0.1–10, default 1) scales them around the group's pivot |

A text without `box` is a free box centred at `at` with `width`.

**Language overrides** (`langs`, texts only) change fields for one language;
`de` also applies to `de-AT`:

```yaml
langs:
  de: {box: {top: 170, left: 40, right: 40}, fit: {shrink_to: 76, max_lines: 2}}
```

Allowed: `at`, `width`, `height`, `anchor_y`, `style`, `box`, `align`,
`valign`, `fit`, `color`, `font_size`, `stroke`, `glow`, `shadow`,
`hyphenate`. Order of application: base → variant override → language
override. Overrides only set values; they never contain `null`.

## `targets`

```yaml
- id: ios-ipad-13-landscape
  canvas: phone
  variant: tablet-landscape
  group: 2
  size: [2752, 2064]
  screenshots: ios-tablet
  gallery: appstore-ipad
  device_map: {iphone-island: ipad-modern}
  store: {apple: {display_type: APP_IPAD_PRO_3GEN_129}}
- id: play-feature-graphic
  kind: banner
  canvas: phone
  variant: banner
  size: [1024, 500]
  screenshots: android-phone
  device_map: {iphone-island: android-punchhole}
  region: {rect: {page: 0, x: 0, y: 40, width: 3000, height: 1465}}
  fit: cover
  gaps: none
  store: {google: {image_type: featureGraphic}}
```

| Field | Meaning |
|---|---|
| `kind` | `pages` (default) or `banner` |
| `canvas`, `variant` | what it shows |
| `size` | export size of one image in px |
| `group` | pages per image |
| `screenshots` | screenshot variant folder |
| `gallery` | gallery profile (gaps, corner radius) |
| `device_map` | model swaps for this target |
| `langs` | languages to export (default: all) |
| `format`, `quality` | `png` (default) or `jpeg` with quality 1–100 |
| `store` | `{apple: {display_type}}` or `{google: {image_type}}`; creatives add `placement` |
| `screenshot_only` | export each page's screenshot alone |
| `safe_area` | `{top, right, bottom, left}` in % of the image |
| `region`, `fit`, `gaps` | banners: `{pages: [a, b]}` or `{rect: {page, x, y, width, height}}`; `cover`/`contain`; `none`/`as_store` |

## `store`

```yaml
store:
  apple: {account: team-a, app_id: "1234567890", bundle_id: com.example.app, platform: ios}
  # apple.upload_flow: auto (default) | asset_library | screenshot_sets
  google: {account: play, package_name: com.example.app}
  locales:
    apple: {en: en-GB}
    google: {he: iw-IL}
```

`account` is the id of an account in the app settings on your computer – the
key itself is never in the project. Only stores listed here are uploaded.

## `import`

Saved screenshot import mappings, so re-imports run without questions:

```yaml
import:
  fastlane:
    devices: {"iPhone 16 Pro Max": ios-phone}
    locales: {de-DE: de}
    keys: {01_home: home}
```

## Translation files: `i18n/<lang>.yaml`

One flat map of keys to texts per language, with [markup](../languages/#text-markup):

```yaml
beds.title: Plan every bed **by drag & drop**
callout.bad: "**Fennel**\nbetter keep apart"
companions.title: Know your [accent]plant partners[/]
```

Keys may contain dots. A regional file (`fr-CA.yaml`) only needs the keys that
differ. Quote a text that contains `: ` or starts with `"`, `[` or `*`.

## Older projects

- A project from *Arrangia* (Vitrine Studio's earlier name, `format:
  arrangia/1`) is refused with a hint: change that line to `format: vitrine/1`
  – that's the whole migration.
- A `project.yaml` with the old key `strips:` is refused: rename `strips:` to
  `canvases:` and `strip:` to `canvas:` in every target.
