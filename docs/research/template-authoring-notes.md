# Template authoring notes (internal)

Internal working notes for building **gallery templates** with an AI
assistant over MCP (the owner's template sessions, 62 templates by
2026-11-13). Not shipped: the user-facing agent skill in
`integrations/skill/` covers store screenshots for the user's *own* app
and does not link here. Use this file together with that skill (design,
copy, store and QA rules live there) when building or reworking templates.

Moved out of the skill on 2026-11-13 (template-authoring reference, the
"Templates only" QA block, gallery listing fields, generator folder
conventions, asset provenance rules, the remote-Mac workflow and the
template target table).

## What a template is

A gallery template is a complete Vitrine project for a **fictional app**
that demonstrates a style and a set structure. Users open it, replace
screenshots, texts, colours and assets. It must look finished out of the box
and be easy to adapt.

## Folder layout

One folder per template, named with the template slug (kebab-case):

```
<slug>/
  projekt/           # the Vitrine project (project_create), incl. assets/, screenshots/<lang>/<variant>/, fonts/
  assets/src/        # generator scripts (build_ops.py, screen HTML, SVG generators), image-generation prompts
  preview/           # gallery previews <target>_<lang>.png (from export)
  README.md          # see below
  _unused/           # leftovers (if deleting is blocked)
```

Project name = template name in Title Case from the slug (`polaroid-wall` →
"Polaroid Wall"), not the fictional app's name. Fictional app names must be
unique across all templates. `.cache/` (op files) is not part of the project
content and never goes into templates.

## What every template contains

- **Canvases:** phone (portrait) and tablet – landscape only, or portrait and
  landscape, as the plan says. The tablet canvas gets its own composition
  (text beside the device, two devices, wider panoramas) – not a scaled
  phone.
- **Pages:** 5–6 per canvas (the first three strongest); one feature wall or
  mosaic as closer if the style allows.
- **Targets:** App Store and Google Play with device frames
  (platform-appropriate frames per store). No separate frameless Play
  variant unless requested.
- **Languages:** English (default/fallback) and German, both complete.
  German must fit without overrides where possible; overrides only for
  single words/lines, documented in the README.
- **Text slots** with names a user understands (eyebrow, headline, subline),
  consistent across pages so a user can retype quickly.
- **Bundled fonts only.** Max two families.
- **Fictional app:** name, icon (simple SVG), 5–6 realistic UI screens with
  plausible data – no real brands, people, logos or trademarks anywhere.
- **Placeholder copy** follows the copy rules of the skill and fits the
  style's voice; English and German both fit without overrides if possible.
  Never lorem ipsum – users copy the tone they see.
- **Validation:** 0 errors, 0 warnings for every target × language; treat
  overflow and contrast warnings as errors.

## Standard target set used for templates

| Target | Canvas | Variant | Size | Screenshots | device_map |
|---|---|---|---|---|---|
| `ios-iphone-69` | phone | – | 1320×2868 | ios-phone | – |
| `play-phone` | phone | `play` | 1080×1920 | android-phone | iphone-island → android-punchhole |
| `ios-ipad-13` | tablet | – | 2064×2752 | ios-tablet | – |
| `play-tablet-10` | tablet | `play-tablet` | 9:16 | android-tablet | ipad-modern → android-tablet |
| `ios-ipad-13-landscape` | tablet-land | – | 2752×2064 | ios-tablet-land | – |
| `play-tablet-10-landscape` | tablet-land | `play-land` | 2560×1600 | android-tablet-land | ipad-modern → android-tablet |

Notes (checked against the code 2026-11-13):
- `ios-tablet-land` and `android-tablet-land` are own screenshot folders, not
  catalog variants (the catalog has `ios-phone`, `android-phone`,
  `ios-tablet`, `android-tablet`, `mac`, `web`, `watch`, …); own folders are
  fine when they exist.
- The `play-tablet-10` preset's landscape sizes are 16:9 (2560×1440,
  3840×2160, 1920×1080); 2560×1600 (16:10) is not one of them – check with
  `validate` / `catalog_targets` before relying on it.
- The App Store catalog marks the iPhone 6.3″ set (`ios-iphone-63`) as
  required and 6.9″ as optional (ASC media manager).
- Portrait and landscape tablet targets share one store slot: only one of
  them may be an *upload* target (readiness `store_slot_conflict`, validate
  `store_slot_duplicate`).

Screenshot sizes used for the fictional screens:

| Variant folder | Viewport @ scale | PNG |
|---|---|---|
| `ios-phone` | 440×956 @3 | 1320×2868 |
| `android-phone` | 432×960 @2.5 | 1080×2400 |
| `ios-tablet` | 1032×1376 @2 | 2064×2752 |
| `android-tablet` | 800×1280 @2 | 1600×2560 |
| `ios-tablet-land` | 1376×1032 @2 | 2752×2064 |
| `android-tablet-land` | 1280×800 @2 | 2560×1600 |

## Inspired templates

Templates may borrow the *design language* of well-known apps or design
movements (colour logic, type treatment, composition). They must not
reproduce their logos, wordmarks, mascots/characters, screenshots, slogans
or product names. The inspiration is recorded in the internal plan, not in
the template's public name or description. If a finished template would be
mistaken for the original app's real screenshots, it is too close – change
it.

## Assets and provenance

| Asset | How | Notes |
|---|---|---|
| Shapes, patterns, icons, doodles, badges, simple illustrations, app icons | SVG, hand-written | Crisp, tiny, recolourable; use 2+ tones and a shadow for volume |
| Fictional app UI screens | HTML/CSS rendered to PNG at device resolution | Real text = sharp and translatable; render each language |
| Backgrounds and style elements that are hard to draw (photos, painterly skies, textures, 3D props) | Image generation (e.g. Leonardo) | Never for UI. No text in the image, no recognisable real people, no brand objects. Generate only on a plan that grants ownership/redistribution rights; record tool, model and prompt |

- Templates are CC0-1.0 (`template_save`): every asset must be
  CC0-compatible (own SVG/HTML, generated images on a plan granting rights).
  Fonts inside keep the OFL.
- Keep source files (SVG, HTML) next to the output so the template can be
  maintained.
- Props: one consistent prop set per template – same light direction, camera
  angle and material.

## README.md per template

- Name, one-line description, mood keywords, suggested app categories.
- Set structure and the job of each page.
- Fonts, palette (hex with roles), density level.
- How to adapt: which elements are recolourable, which assets to replace,
  where the cross-page moment is.
- Known per-language overrides.
- Asset provenance (SVG by hand / HTML render / generated with which tool)
  and licence.

## Build loop

1. Write the brief (from the plan): app, structure, palette, fonts, layouts
   per page.
2. Create assets and fictional screens first (sizes above).
3. `project_create {dir, name, template: "minimal", langs: ["en","de"]}`;
   copy screens and assets into the project with the shell.
4. Generator script (`assets/src/build_ops.py`) → ops per canvas incl.
   targets, variants, overrides and strings → `.cache/ops/<canvas>.json` →
   `apply_ops {file}`. Phone canvas → render → fix. Then tablet canvas(es) →
   render → fix. Keep the previous output to diff.
5. Play variants with `device_map`; German filled; `validate` to 0/0; render
   the gallery for each target × language on light and dark store background
   and look at it.
6. Run the QA gate of the skill plus the template checklist below, export
   previews (`export {pages: false, html: false}` writes one `_gallery.png`
   per target × language), write the README.
7. Reworking an existing template: back it up first (`cp -R <slug>
   <slug>-v1`), `project_open`, change via ops or `element_update {scope:
   "variant"}`, add a dated "Rework" section to the README.

## QA – templates only

- [ ] Phone and tablet canvases (tablet orientation as planned), App Store
      and Play targets, EN (default) and DE.
- [ ] No real brand, logo, character, person or trademark anywhere.
- [ ] Placeholder copy follows the copy rules and the style's voice.
- [ ] README written, previews saved, asset provenance noted.
- [ ] 0 errors / 0 warnings in every target × language; every `allow`
      intended and explainable.

## Gallery listing fields

`slug` (folder = permanent id) · `name` · `style` 1–2 · `color` 1–2 · `use`
1–3 – **fixed tag ids only** (`crates/core/data/template-tags.yaml`; MCP
`template_save` has them as enums); device tags are derived by the CLI ·
`description`: plain English, 1–3 sentences – look, suitable app types,
what's included (phone, tablet portrait/landscape, 6 pages, EN + DE, sample
screenshots to replace). Uploading accepts the gallery terms
(https://www.vitrinestudio.app/terms/).

## Working on a remote Mac (cloud session + desktop bridge)

- Build files (screens, SVGs, ops JSON) in the cloud container, transfer with
  `device_commit_files` (≤ ~10 MB per file; tar bundles, unpack with
  `tar -xOf … > target`), verify with `md5sum`. Use fresh staging file names
  – re-using one can deliver stale content.
- Deleting can be blocked on the user's machine: move leftovers to
  `_unused/` and truncate with `: > file`.
- Fonts: `npm pack @fontsource/<name>` (+ WOFF→TTF with fontTools) instead of
  raw downloads.
- Fictional app screens: HTML/CSS rendered with Playwright/Chrome at the
  sizes above, per language; look at a contact sheet before importing.
- Many files: copying straight into `assets/` and `screenshots/…` with the
  shell is faster than `assets_import` (one result line per file); Vitrine
  picks them up at once. Overwriting an asset file takes effect at the next
  render without any op.
