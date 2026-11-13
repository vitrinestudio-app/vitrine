---
name: vitrine-store-screenshots
description: Design and build App Store and Google Play screenshot sets (and gallery templates) in Vitrine Studio via its MCP server — story, copy, layout, localization, store compliance and the technical side of Vitrine (canvases, variants, targets, apply_ops, validate, export). Use for app store screenshots, ASO marketing images, Play Store graphics, screenshot templates or panorama sets.
---

# Store screenshots with Vitrine Studio

You design store screenshot sets in **Vitrine Studio** through its MCP server (`vitrine-studio`).
Vitrine works on **one continuous canvas per device class**: pages sit side by side with the real
store gallery gaps between them, so a device or background can cross a page boundary and still
line up in the store. Texts and screenshots are resolved per language with fallback chains;
variants adapt a canvas to other targets (aspect ratios, landscape sets from grouped pages).

Two kinds of jobs use this skill:

- **A set for a real app** – the user's own screenshots, benefits and languages.
- **A gallery template** – a reusable design with placeholder app, placeholder copy and assets
  that users swap out. Read `references/template-authoring.md` before building one.

## Core principle

Screenshots are **ads, not documentation**. Each page sells exactly one idea – an outcome, a
moment, or a pain that goes away – and the real app UI proves it. The first three pages carry
the decision: they show in search results and are often all a visitor sees.

## Workflow

1. **Get oriented in Vitrine.** Read `references/vitrine-technical.md` (model, elements,
   variants, ops, checks, export) and `docs_get` (`elements`, `ops`, `scopes`, `markup`) before
   the first edit; `docs_get` wins on conflicts. Check that the current build runs
   (`apply_ops` knows `file`). Open or create the project; check canvases, variants, targets
   and languages. Never edit project YAML by hand while the app has the project open – go
   through the tools so changes are live and undoable.
2. **Find the benefits (real apps).** From the user, the code or the store listing: who is it for,
   why do they install, what is different. Write 3–5 benefits, each verb-first and specific.
   Get them confirmed before designing.
3. **Choose a set structure** (see `references/design-rules.md` §Story): feature-led,
   journey, problem → solution, or proof-first. Plan page jobs: 1 hero · 2 differentiator ·
   3 proof/ecosystem · 4+ features · last = breadth (feature wall or screen mosaic).
4. **Write copy first.** Three options per page (paint a moment / state an outcome / kill a
   pain), pick one, check 3–5 words per line, max two lines. Rules and patterns:
   `references/copy.md`.
5. **Set the visual system once:** background roles, palette from the app icon (one accent),
   max two font families from the bundled set, headline size ≈ 7–10 % of canvas width,
   margins, device scale, frame style. Then lay out pages and **vary layouts** – never the same
   composition twice in a row; break the phone parade every 2–3 pages.
6. **Use the continuous canvas deliberately:** at most one cross-page moment in a 5–7 page set,
   two in 8–10. Only backgrounds, paths, decoration or 10–30 % of a non-critical device may
   cross a gap – never text, faces, ratings or the UI the headline depends on.
7. **Localize.** Fill every language; design for the longest (German headlines run 60–80 %
   longer than English; plan headroom, allow hyphenation, use per-language overrides only
   where needed). Make sure UI language in the screenshot matches the copy language.
8. **Store variants.** App Store: majority of pages must show the app in use. Google Play:
   none of the banned words; Google's guideline discourages device imagery and caps taglines
   at 20 % of the image, but most top apps use framed, captioned sets anyway – by default use
   the same framed design for both stores and offer a frameless Play variant only when the
   user asks. Details: `references/store-specs.md`.
9. **Build and check.** Generate ops per canvas (base + variant overrides) with a script,
   write them into `<project>/.cache/ops/` and apply with `apply_ops {file}`; single fields
   in variants via `element_update {scope: "variant"}` (merges). Then `validate` every
   target × language to 0 errors / 0 warnings, render the gallery (`render_preview`, light and
   dark store background) and **look at it** – validate can't see props over UI, motifs inside
   background images, seam alignment of baked art or taste. Run the QA gate in
   `references/qa.md`, including the thumbnail test at ~200 px width. Fix before presenting.
10. **Hand over.** Show the gallery preview, list open points (missing screenshots, claims to
    verify). Store upload always goes plan → explicit user confirmation → execute; never
    execute an upload on your own.

## Hard rules

- Real UI with realistic data; no empty states, loading screens, settings or login as content.
- No prices, discounts, URLs, ©, "#1/best/top/new/free", award or Editor's-Choice badges,
  calls to action, or other platforms' names, devices or store badges.
- Proof (ratings, user counts, press) only when true and permitted – and never on Play.
- Text contrast ≥ 4.5:1 against the worst pixel underneath, in every language.
- Imported Apple bezels stay upright and unmodified; tilt and crop only parametric frames.
- Don't present unverifiable conversion statistics as fact. Apple's own published tests show
  single-digit effects; good fundamentals beat tricks.

## Technical essentials (details in `references/vitrine-technical.md`)

- Pages are 1000 lu wide on one continuous canvas; the store gap is cut out of anything that
  crosses a seam. Cross a seam with **one** element and `clip: none`, never with twin halves;
  panoramas as canvas `backgrounds` image or one wide element – no per-page slices.
- `at.y` is always in canvas page height; other page heights are handled by `anchor_y`.
- Play targets get their own variant (lower 9:16 pages) and a `device_map` to Android frames.
  One upload target per store slot (portrait and landscape tablet share a slot).
- `set_variant_override` replaces an element's whole override (check
  `dropped_override_fields`); `element_update {scope: "variant"}` merges.
- Groups take `scale` (one override per card instead of one per child); images take
  `clip_to` a shape; spans take `background` + `padding` + `radius` for pills and markers.
- Intended exceptions to checks go into `allow: [code]` on the element, thresholds into
  `set_checks` – prefer fixing over allowing. What each code measures: `docs_get {topic: "checks"}`.

## References

- `references/design-rules.md` – story, layout, typography, colour, panorama, numbers
- `references/copy.md` – headline rules, formulas, weak → better, localization of copy
- `references/store-specs.md` – App Store and Google Play rules and sizes
- `references/template-authoring.md` – building gallery templates (placeholders, variants, assets)
- `references/qa.md` – the QA gate before showing anything
- `references/vitrine-technical.md` – how Vitrine works over MCP: model, elements, variants, targets, ops, checks, preview/export, templates, proven techniques
