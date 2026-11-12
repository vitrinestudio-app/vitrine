# 0023 – Templates: package format, merge rules, gallery backend

- Status: accepted (gallery backend renamed and GitHub login added: ADR 0025)
- Date: 2026-11-11

## Context

KONZEPT §9 "Vorlagen" (duplicate a canvas, save as template, new project
from template) and phase 6.3. User decisions (phase-6.md 6.3, STATUS.md):
a template may hold **several canvases** (Phone + Tablet + feature graphic
as one set) with their targets and variants, sharing styles and texts;
"new project" takes all canvases, "insert" lets the user pick; templates
are always **CC0**; fonts only under the **OFL**, with a licence file that
is kept or generated from the font's name table; the web gallery runs on
an own backend (Rust/axum, SQLite, Docker, `api.vitrinestudio.app`) with
GitHub login and moderation.

## Decision

### Package `.vitrine-template`

A zip of a project subset plus a manifest; the same layout extracted is
what the app-wide store keeps (`<app data>/templates/<id>/`).

```text
template.yaml                  manifest
project.yaml                   vitrine/1 project: the canvases, their targets
                               and variants, styles; no store:/import:
i18n/<lang>.yaml               only the keys the canvases use
screenshots/<lang>/<key>.png   generated placeholders (core::template::placeholder)
assets/…                       PNG/JPEG/SVG used by the canvases
fonts/<file>.ttf|otf           OFL fonts named by the styles
fonts/<stem>-OFL.txt           their licence (kept from the project or generated)
previews/<canvas>.png          gallery preview per canvas (rendered)
```

`template.yaml` (`core::template::Manifest`, `deny_unknown_fields`):
`format: vitrine-template/1`, `id` (`[a-z0-9][a-z0-9-]*`, ≤ 64), `name`,
`description`, `author`, `tags` (≤ 12, id-like), `version` (free form,
default `1.0.0`), `license: CC0-1.0` (the only value accepted),
`project_format: vitrine/1`, `canvases` (`id`, `aspect`, `pages`,
`targets`, `preview`) in project order, `fonts` (`file`, `family`,
`license: OFL-1.1`, `license_file`, `copyright`), `generator`.

Built-in templates (`phone`, `minimal`, formerly `core::templates`) are the
same format, embedded from `crates/core/templates/<id>/`; their placeholders
are generated on first use and they carry no stored previews (rendered on
demand).

### Reading a package safely (`Package::from_zip_bytes`, `from_dir`)

- Limits (`Limits::default()`): 50 MB packed, 150 MB unpacked, 25 MB per
  file (raised 2026-11-12, see the amendment below) (read with `take(limit + 1)`, the declared size is not trusted),
  500 files, path depth 6.
- Paths: relative, `/`-separated, no `..`/`.`/empty components, no
  absolute paths, backslashes, colons, control characters or hidden
  components; names differing only in case are refused. Zip symlink
  entries and symlinks in store folders are refused. `__MACOSX/`,
  `.DS_Store`, `Thumbs.db` are skipped; a single wrapper folder is
  accepted.
- Only the file kinds of the table above, checked by content: PNG/JPEG
  magic bytes, SVG as UTF-8 without `<script`, event handler attributes,
  `<!ENTITY`, `<foreignObject`, `javascript:` or external `href`s, fonts
  parsed by skrifa, text files UTF-8 ≤ 2 MB.
- Fonts: every font must be listed in the manifest and declare the OFL in
  its name table (ID 13 licence description or ID 14 URL mentioning the
  Open Font License / OFL / openfontlicense.org); its licence file must be
  the OFL 1.1 text. A font declaring nothing is refused as "licence
  unclear", any other licence as "may not be redistributed".
- The project subset must load (`project_from_yaml`), match the manifest's
  canvases, contain no `store:`/`import:` settings, have every referenced
  asset, and validate with `core::validate` (bundled + package fonts, the
  package files as assets) without errors – except findings that depend on
  the user's later content (store rules, screenshots, text fit, unknown
  fonts, imported frames).

### Saving (`core::template::extract`)

Selected canvases (default all) with the targets showing them, all styles,
the strings their texts use (texts, language and variant overrides), image
assets (files outside `assets/` are moved there, references follow), the
project fonts whose family the styles name (OFL check; licence kept from
`<stem>-OFL.txt`, `<stem>.txt`, `OFL.txt` or `LICENSE.txt` next to it if
that is the OFL text, else generated: name ID 0 copyright, plus `with
Reserved Font Name "…"` when ID 0/13 state one, plus the OFL 1.1 body
taken from `assets/fonts/OFL.txt`), and generated placeholder screenshots
for every screenshot key (device screen aspect from the catalog) instead
of the project's screenshots. `store:` and `import:` are dropped. Previews
are rendered by `vitrine_render::template::add_previews` (store gallery of
the first page target per canvas, else its banner; default language;
parametric frames only – never imported vendor bezels).

### New project from a template (`instantiate::create_project`)

All canvases, targets, styles, assets and fonts; texts are mapped to the
chosen languages: same language, else its primary subtag (`de-AT` → `de`),
else a template language with that primary subtag, else the template's
default language (reported as "copied, to translate"). Language overrides
and `visible_in.langs` keep entries that apply to a chosen language
(prefix rule as in resolve), target `langs` are filtered (empty → all).
**Placeholder screenshots are not copied**: the editor shows its own
placeholder and validation keeps reporting missing screenshots, so a
placeholder can never reach a store upload.

### Insert canvases into an open project – merge rules (`insert::plan_insert`)

Renames are applied with the ops API to a scratch copy of the template
project (so every reference follows), then turned into one transaction
for the project: `SetStyles` (merged), `SetString`s, `AddCanvas`es,
`AddTarget`s – **one undo step** ("Insert … from template …"). Files are
written before the ops and removed again if the ops fail.

1. **Styles**, in the order colours → shadows → span styles → text styles
   (so a renamed colour makes a style that uses it differ, too):
   - name only in the template → added;
   - same name, equal definition → shared;
   - same name, different definition → **rename** (default): the
     template's becomes `<name>-2` (next free) in the inserted canvases
     and texts (span names in translations follow), or **use_project**:
     the inserted canvases use the project's definition;
   - a template shadow named like a built-in preset (`soft`, `hard`,
     `none`) that the project does not redefine is a conflict too (adding
     it would change the project's existing elements);
   - the project's frame style and font settings win; template font
     settings for families the project does not configure are added.
2. **Ids**: a canvas or target id the project uses gets `-2`, `-3` …
   appended (`ios-iphone-69` → `ios-iphone-69-2`; numbers in these ids
   mean something); element ids follow the duplicate-canvas rule
   (`title-1` → `title-2`).
3. **Texts**: languages mapped as for a new project. A key the project
   already has is shared when the texts are equal in every language, else
   the template's key becomes `<template id>.<key>`.
4. **Files**: assets and fonts (+ licence files) are copied to the same
   paths; an identical existing file is reused, a different one gets
   `-2` (references follow).

Commands: `template_list|save|import|export|remove|insert_plan|insert`,
image command `template_preview`; `project_new {template}` takes a
built-in name, store id or file. CLI `vitrine template
save|list|import|export|remove|insert|new`, `vitrine new --template
<name|file>`. MCP `template_list|save|insert|import`, `project_create
{template}`.

### Gallery backend (`crates/api-server`, formerly `crates/gallery-server`)

Binary `vitrine-api` (axum 0.8, rusqlite, files on disk, config via
`VITRINE_API_*` environment variables) for `api.vitrinestudio.app`,
deployed by the owner with Docker behind his reverse proxy. It reuses
`core` (the checks above, `regenerate_placeholders`: uploaded screenshots
are always replaced by generated ones) and `render` (trusted previews
rendered on the server, the uploaded `previews/` are discarded).
Implemented: health, list/search, get, download, preview images (public,
no login), upload with validation (stored as `pending` unless
`VITRINE_API_AUTO_APPROVE`), abuse reports, an admin moderation queue
(approve/reject, bearer token from the environment), rate limiting.
Renamed to `api-server` and given GitHub login (device + web flow, own
session tokens, ownership by GitHub id, official templates) in ADR 0025.
API: `docs/api/API.md`; deployment, GDPR and Impressum:
`docs/api/DEPLOY.md`.

## Consequences

- One format for built-in, saved, imported and gallery templates; the
  store is plain folders.
- Placeholders are cheap (a few KB each) and deterministic, so server
  previews equal local ones.
- Merge rules never change the project's existing look: conflicting styles
  are renamed unless the user asks for the project's styles.
- Not done yet: a GUI "browse gallery" (client and app commands exist,
  ADR 0025), template versions/updates, frame
  packs in templates (6.4), font subsetting.

## Amendment 2026-11-12: sample screenshots, tag facets, sprites

Owner decisions (STATUS.md decision log); details in `docs/api/API.md`.

- **Sample screenshots.** `template.yaml` may list `samples:` – package
  paths under `screenshots/` (PNG/JPEG, ≤ 10 MB, ≤ 6000 px per side,
  checked with the package) of the author's own app screenshots, CC0 like
  the rest. `extract` packs them with `SaveOptions::sample_screenshots`
  (the used keys in every template language, variant folders included;
  placeholders only for keys without one and never where a sample sits at
  the same place). New project and insert copy them (default language
  from the serving template language, plus languages the template has;
  insert never overwrites an existing screenshot of that name, any
  extension) and list them in `screenshots/samples.yaml` with size and
  SHA-256 (`core::samples`): a file is a sample only while its content
  matches, so replacing it un-marks it. Validation: info
  `sample_screenshot` (per language, via `AssetLookup::is_sample`); the
  upload plan turns it into a blocking problem; export is allowed. The
  server keeps samples only when the uploader confirms
  (`?confirm_own_screenshots=true`, CLI `--confirm-own-screenshots`),
  else `regenerate_placeholders(false)` replaces them as before.
- **Tags.** `tags:` is a facet map `{style, color, use}` checked against
  `crates/core/data/template-tags.yaml` (`core::template::tags`: max per
  facet always, min with `check(true)` for the gallery); `devices` is
  derived (`tags::devices`) and never stored in the manifest. Old flat
  lists are still read (known values sorted into facets, others in
  `dropped`, refused by the server). This replaces "`tags` (≤ 12, id-like)".
- **Previews and sprites.** `vitrine_render::template::sprites` renders a
  canvas's preview target page by page (exact page viewports, no gaps)
  at a given page height and integer scales; the API server encodes them
  as WebP (ADR 0025 amendment).

## Amendment 2026-11-12: package size, JPEG samples

Templates saved with sample screenshots from real projects reached
80–210 MB (full-size PNG screenshots) and failed the 50 MB limit.

- **Sample screenshots as JPEG.** `vitrine_render::template::compress_samples`
  (run by the app command, CLI and MCP save after `extract`) re-encodes every
  sample as sRGB JPEG quality 60 (`SAMPLE_JPEG_QUALITY`, owner decision):
  `screenshots/en/home.png` → `home.jpg`, which the screenshot lookup finds
  in the same place. Samples with transparent pixels stay PNG (JPEG has no
  alpha; reported in `SaveReport::samples_png`), as do files the JPEG would
  not shrink. `extract` packs one file per key and folder (the one the lookup
  picks) so `.png` → `.jpg` never collides. Assets, fonts, placeholders and
  previews are untouched. The report gives `samples_bytes_original` /
  `samples_bytes`.
- **Limits.** `Limits::for_package(max)`: unpacked ≤ 2 × packed (at least
  150 MB), 50 MB per file, 1000 files, depth 6. `Limits::default()` is the
  client limit (`CLIENT_PACKAGE_MB` = 250 MB packed, 500 MB unpacked) used by
  the app, CLI and gallery download (`publish::gallery::MAX_DOWNLOAD`), so a
  local check never refuses what a server with a raised upload limit accepts.
  The server checks uploads with `for_package(VITRINE_API_MAX_UPLOAD_MB)`
  (ADR 0025 amendment). Zip-bomb protection is unchanged (sizes counted
  while reading, declared sizes not trusted).
