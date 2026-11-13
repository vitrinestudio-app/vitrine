# Authoring gallery templates

A gallery template is a complete Vitrine project for a **fictional app** that demonstrates a
style and a set structure. Users open it, replace screenshots, texts, colours and assets.
It must look finished out of the box and be easy to adapt.

## Folder layout

One folder per template, named with the template slug (kebab-case):

```
<slug>/
  projekt/           # the Vitrine project (project_create), incl. assets/, screenshots/<lang>/<variant>/, fonts/
  assets/src/        # generator scripts (build_ops.py, screen HTML, SVG generators), Leonardo prompts
  preview/           # gallery previews <target>_<lang>.png (from export)
  README.md          # see below
  _unused/           # leftovers (if deleting is blocked)
```

Project name = template name in Title Case from the slug (`polaroid-wall` → "Polaroid Wall"),
not the fictional app's name. Fictional app names must be unique across all templates.
`.cache/` (op files) is not part of the project content.

## What every template contains

- **Canvases:** phone (portrait) and tablet – landscape only, or portrait and landscape, as
  the plan says. The tablet canvas gets its own composition (text beside the device, two
  devices, wider panoramas) – not a scaled phone.
- **Pages:** 5–6 per canvas (the first three strongest); one feature wall or mosaic as
  closer if the style allows.
- **Targets:** App Store and Google Play with device frames (platform-appropriate frames per
  store). No separate frameless Play variant unless requested.
- **Languages:** English (default/fallback) and German, both complete. German must fit
  without overrides where possible; overrides only for single words/lines, documented in the
  README.
- **Text slots** with names a user understands (eyebrow, headline, subline), consistent across
  pages so a user can retype quickly.
- **Bundled fonts only.** Max two families.
- **Fictional app:** name, icon (simple SVG), 5–6 realistic UI screens with plausible data –
  no real brands, people, logos or trademarks anywhere.

## Inspired templates

Templates may borrow the *design language* of well-known apps or design movements (colour
logic, type treatment, composition). They must not reproduce their logos, wordmarks,
mascots/characters, screenshots, slogans or product names. The inspiration is recorded in
the internal plan, not in the template's public name or description. If a finished template
would be mistaken for the original app's real screenshots, it is too close – change it.

## Assets

| Asset | How | Notes |
|---|---|---|
| Shapes, patterns, icons, doodles, badges, simple illustrations, app icons | SVG, hand-written | Crisp, tiny, recolourable; use 2+ tones and a shadow for volume |
| Fictional app UI screens | HTML/CSS rendered to PNG at device resolution | Real text = sharp and translatable; render each language |
| Backgrounds and style elements that are hard to draw (photos, painterly skies, textures, 3D props) | Image generation (e.g. Leonardo) | Never for UI. No text in the image, no recognisable real people, no brand objects. Generate only on a plan that grants ownership/redistribution rights; record tool, model and prompt |

Keep source files (SVG, HTML) next to the output so the template can be maintained.

## README.md per template

- Name, one-line description, mood keywords, suggested app categories.
- Set structure and the job of each page.
- Fonts, palette (hex with roles), density level.
- How to adapt: which elements are recolourable, which assets to replace, where the
  cross-page moment is.
- Known per-language overrides.
- Asset provenance (SVG by hand / HTML render / generated with which tool) and licence.

## Build loop

1. Write the brief (from the plan): app, structure, palette, fonts, layouts per page.
2. Create assets and fictional screens first (sizes in `vitrine-technical.md` §3).
3. `project_create {dir, name, template: "minimal", langs: ["en","de"]}`; copy screens and
   assets into the project with the shell.
4. Generator script → ops per canvas incl. targets, variants, overrides and strings →
   `apply_ops {file}`. Phone canvas → render → fix. Then tablet canvas(es) → render → fix.
5. Play variants with `device_map`; German filled; `validate` to 0/0; render the gallery for
   each target × language on light and dark store background and look at it.
6. Run the QA gate (`qa.md`), export previews, write README.
7. Reworking an existing template: back it up first (`cp -R <slug> <slug>-v1`), `project_open`,
   change via ops or `element_update {scope: "variant"}`, add a dated "Rework" section to the
   README.

## Gallery listing fields

`slug` (folder, permanent id) · `name` · `style` 1–2 · `color` 1–2 · `use` 1–3 (fixed tag
ids only; devices are tagged by the CLI) · `description`: English, 1–3 plain sentences – look,
suitable app types, what's included (phone, tablet portrait/landscape, 6 pages, EN + DE,
sample screenshots to replace).
