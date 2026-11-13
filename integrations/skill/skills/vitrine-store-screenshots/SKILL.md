---
name: vitrine-store-screenshots
description: Design, localize, check, export and upload App Store and Google Play screenshot sets for the user's own app with Vitrine Studio through its MCP server (vitrine-studio) – benefits and copy, layout, panoramas across pages, languages, store rules, validation, export and store upload. Use for app store screenshots, Play Store graphics, ASO marketing images, feature graphics or any work in a Vitrine Studio project.
license: MIT
---

# Store screenshots with Vitrine Studio

You help the user build the store screenshots of **their own app** in **Vitrine Studio**, using
its MCP server (`vitrine-studio`). Vitrine works on **one continuous canvas per device class**:
pages sit side by side with the store's gallery gaps between them, so a device or background
can cross a page boundary and still line up in the store. Texts and screenshots are resolved
per language with fallback chains; variants adapt a canvas to other targets (other page
heights, landscape images from grouped pages, Android frames for Play).

## Core principle

Screenshots are **ads, not documentation**. Each page sells exactly one idea – an outcome, a
moment, or a pain that goes away – and the real app UI proves it. The first three pages carry
the decision: they show in search results and are often all a visitor sees.

## Workflow

1. **Get oriented.** Read `references/vitrine-technical.md` and, before the first edit,
   `docs_get` for `overview`, `elements`, `ops` and `scopes`; `docs_get` wins on conflicts.
   If `apply_ops` has no `file` parameter, the app is older than this skill: ask the user to
   update Vitrine Studio (inline `ops` still work). Never edit project YAML by hand while the
   app has the project open – go through the tools so changes are live and undoable.
2. **Start the project.** Open the user's project (`project_open`) or create one with
   `project_create {dir, name, template, langs}`: blank (`minimal`), the built-in `phone`
   starter, or a template from `template_list` (built-ins, saved, imported and templates the
   user downloaded from the online gallery). `template_insert` adds a template's canvases to an
   open project (`dry_run` first). Browsing the online gallery happens in the app
   (New project → Template gallery); there is no gallery tool over MCP. Then
   `project_summary` for canvases, pages, element ids, targets and languages.
3. **Bring in the user's screenshots.** `screenshots_import {dir, dry_run: true}` plans a
   folder import (fastlane output, language folders or flat), show the plan, then run it
   without `dry_run`. Screenshots live in `screenshots/<lang>/<variant>/<key>.png`; a device
   element shows the screenshot with its `screenshot` key. Single files with their own key:
   `assets_import {files: [...]}`. Screenshots that came with a template are **samples**
   (listed in `screenshots/samples.yaml`, validate info `sample_screenshot`): export works,
   but store upload refuses every set that still uses one – replace them with the user's own
   (a replaced file stops being a sample automatically).
4. **Find the benefits.** From the user, the code or the store listing: who is it for, why do
   they install, what is different. Write 3–5 benefits, each verb-first and specific. Get them
   confirmed before designing. Choose a set structure and page jobs
   (`references/design-rules.md` §Story): 1 hero · 2 differentiator · 3 proof/ecosystem ·
   4+ features · last = breadth.
5. **Write copy first.** Three options per page (paint a moment / state an outcome / kill a
   pain), pick one, 3–5 words per line, max two lines (`references/copy.md`).
6. **Set the visual system once** – or adapt the template's: replace its colours with a
   palette from the app icon (`styles_update`, one accent), keep or swap its fonts (max two
   families), retype its texts (`strings_set`), swap its decoration assets. Headline ≈ 7–10 %
   of the page width, consistent margins, device scale and frame style. **Vary layouts** –
   never the same composition twice in a row. Use the continuous canvas deliberately: at most
   one cross-page moment in a 5–7 page set, two in 8–10; never text, faces, ratings or key UI
   across a gap.
7. **Languages.** `language_add`, then `strings_missing {lang}` → `strings_set` (or
   `strings_export` / `strings_import` for translators). Design for the longest language;
   rewrite before shrinking; use hyphenation or per-language overrides only where needed.
   The screenshot UI language should match the copy language.
8. **Store variants.** Default: the same framed design for both stores, Android frames on
   Play via the target's `device_map`, and a variant with adjusted positions for Play's lower
   pages. Offer a frameless, low-text Play variant only if the user wants it
   (`references/store-specs.md`). Google Play needs a feature graphic (`banner_add {canvas, preset:
   "play-feature-graphic"}`).
9. **Build and check.** Big changes as op files (`apply_ops {file}`), single override fields
   with `element_update {scope: "variant"}`. After every visual change `render_preview` and
   **look at it**. `validate` every target × language to 0 errors (aim for 0 warnings), render
   the gallery on the light and dark store background and run the QA gate in
   `references/qa.md`, including the thumbnail test and the checks validate can't do.
10. **Export.** `export` writes `<project>/export/` (incremental; `layout: "fastlane"` for
    fastlane deliver/supply folders; `pages: false` for gallery previews only). Long exports
    continue in the background: poll `export_status {job}`.
11. **Store upload – only when the user asks.** Store accounts are set up by the user in the
    app (App settings → Stores) and the project's store mapping in Project settings → Stores;
    neither can be changed over MCP (`store_config_get` reads them). `upload_plan` (dry run;
    `remote: true` also reads the store) → show the user the plan, its readiness steps and
    problems → only after the user says yes, `upload_execute {confirm_token}` → the user must
    also click "Confirm upload" in the app → follow with `upload_status`. Never upload on your
    own, never retry a declined upload. Details: `references/vitrine-technical.md` §8.

Hand over with a gallery preview and a list of open points (sample or missing screenshots,
claims to verify, languages not yet reviewed). Designs can be saved for reuse with
`template_save`; sharing a template in the online gallery has its own terms
(https://www.vitrinestudio.app/terms/).

## Hard rules

- Real UI with realistic data; no empty states, loading screens, settings or login as content.
- No prices, discounts, URLs, ©, "#1/best/top/new/free", award or Editor's-Choice badges,
  calls to action, or other platforms' names, devices or store badges.
- Proof (ratings, user counts, press) only when true and permitted – and never on Play.
- Text contrast ≥ 4.5:1 against the worst pixel underneath, in every language.
- Imported Apple bezels stay upright and unmodified; tilt and crop only parametric frames.
- Don't present unverifiable conversion statistics as fact. Apple's own published tests show
  single-digit effects; good fundamentals beat tricks.
- Store uploads change the public listing: plan → the user's explicit yes → execute, every time.

## Technical essentials (details in `references/vitrine-technical.md`)

- Pages are 1000 lu wide on one continuous canvas; the store gap is cut out of anything that
  crosses a seam. Cross a seam with **one** element and `clip: none`, never with twin halves;
  panoramas as a canvas `backgrounds` image or one wide element – no per-page slices.
- `at.y` is always in canvas page height; other page heights are handled by `anchor_y`.
- One upload target per store slot (portrait and landscape tablet share a slot).
- `set_variant_override` replaces an element's whole override (check
  `dropped_override_fields`); `element_update {scope: "variant"}` merges.
- Design checks on devices, pages and corners run on the **first language** of each target
  only; check other languages with `validate {langs: [...]}` and by eye.
- Intended exceptions go into `allow: [code]` on the element, project-wide switches into
  `checks.off` – prefer fixing. What each code measures: `docs_get {topic: "checks"}`.

## References

- `references/vitrine-technical.md` – how Vitrine works over MCP: model, elements, screenshots,
  variants, targets, ops, fonts, checks, preview, export, upload, proven techniques
- `references/design-rules.md` – story, layout, typography, colour, panorama, props
- `references/copy.md` – headline rules, formulas, weak → better, localizing copy
- `references/store-specs.md` – App Store and Google Play rules and sizes
- `references/qa.md` – the QA gate before showing anything
