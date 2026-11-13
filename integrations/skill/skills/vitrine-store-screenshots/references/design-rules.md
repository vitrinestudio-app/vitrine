# Design rules

Numbers are calibrated to page width **W**. In Vitrine every page is 1000 lu wide (height
follows the aspect), so W = 1000 lu on every canvas; headline ≈ 70–90 lu.

## Story

How the set is seen: search shows the first 1–3 pages (orientation-dependent) when no preview
video exists; most visitors never reach page 5. So pages 1–3 must work as a group without the
gallery being opened.

Pick one structure:

| Structure | Use when | Sequence |
|---|---|---|
| Feature-led | Value = several distinct features | feature, feature, feature |
| Journey | The aha only shows in the flow | open → do → done |
| Problem → solution | Pain is obvious and relatable | problem, fix, result |
| Proof-first | Real, strong ratings/numbers (App Store only) | hook, proof, feature |

Default arc for up to 10 pages (skip slots that don't fit):

1. **Hero** – the main outcome, with the best real screen.
2. **Differentiator** – why this app and not the others.
3. **Proof or ecosystem** – widgets, watch, sync; or a true trust signal.
4+. **Features** – one per page, most important first.
Second-to-last: **identity** ("Made for people who …").
Last: **breadth** – feature wall (big type list) or a mosaic of small screens. Never waste the
last page on another single feature.

Every page answers one buying question. If two pages make the same claim, merge them. Avoid
"Step 2" unless step 1 is guaranteed visible.

## Layout

- Headline zone: top ~25–35 % of the page. Captions at the top are the norm.
- Device on a phone page: ~65–80 % of page height, anchored to an edge (bottom bleed, top
  bleed or flush). Never float a phone mid-page with empty bands above and below.
- No empty band larger than ~22 % of page height.
- Max three layers: background → device/illustration → text + one accent.
- Density inside the device, calm outside: one headline, one optional subline, one accent.
- Crop toward the decision-relevant part of the UI when the full screen gets illegible;
  an enlarged "breakout" card from the UI (soft shadow, no rotation) is allowed – 1, max 2.
- Layout palette to rotate through: device centred bottom · device top (text below) · device
  left/right with text beside or above · two devices (back dimmed/smaller, front prominent) ·
  no device (big type, illustration, mosaic). Never repeat the same layout on adjacent pages.
- 1–2 contrast pages (inverted background) in sets of 5+.
- Frames: bezelless, floating with soft shadow, or full bezel centred. Mix at most two
  treatments per set. Tint shadows with the background colour; stack a soft long shadow
  and a tight contact shadow.

## Typography

| Element | Size | Weight | Leading |
|---|---|---|---|
| Eyebrow label | W × 0.028 | 600 | normal |
| Headline | W × 0.07–0.09 | 700–900 | 0.95–1.05 |
| Hero headline | W × 0.09–0.10 | 700–900 | 0.92–1.0 |
| Subline (optional) | W × 0.035–0.045 | 400–500 | 1.2 |

- Characters per line ≈ W ÷ font size × 0.45. Max two lines (three only if each is short).
- Tracking slightly negative for display sizes (−0.01 to −0.03 em).
- Max three text styles, max two families. One emphasis word per headline (accent colour,
  italic, script, heavier weight) – the emphasis colour must pass contrast on its own.
- Thin weights fail at thumbnail size and in dark mode.
- Vitrine bundled fonts: Inter, Inter Display, Montserrat, Nunito, Oswald (sans); Bebas Neue
  (caps-only display); Playfair Display, Lora (serif); Caveat, Dancing Script (script
  accents); JetBrains Mono (mono). Script fonts only for single emphasis words. Bold Inter
  headlines: use Inter Display (Inter itself has only 400 and 600; see
  `vitrine-technical.md` §3).

## Colour and background

- The background is a designed surface: colour field, warm off-white, deep navy/slate, or a
  gradient with ≥3 stops and a real hue shift. Never default white (merges with the light
  store) or pure black (merges with the dark store); validate reports
  `page_blends_with_store`.
- Palette from the app icon. Assign roles: surface, primary text, secondary text, accent,
  contrast surface. Fewer colours read as more polished.
- Contrast ≥ 4.5:1 against the worst pixel under the text (lightest gradient stop, photo
  highlights). Otherwise: darker colour, scrim, or move the text.
- No pure white text on a saturated mid-tone without a scrim.

## Continuous canvas and panorama

- Panorama is a style choice, not a proven conversion lever. Good for mood/brand apps; for
  feature-heavy apps prefer self-contained pages with one bridging moment.
- Every exported page must still read as a complete ad on its own.
- Allowed across a gap: backgrounds, gradients, paths/routes, waves, decoration, a shared
  object, 10–30 % of a non-critical device. Above 40 % only for backgrounds and abstract shapes.
- Never across a gap: text, app name, ratings, faces/eyes, chart numbers, the UI the headline
  depends on.
- Put the seam through negative space or a simple shape. Build a crossing motif as **one**
  element with `clip: none` (Vitrine cuts each store's real gap); never two halves.
- Per-page background art must be computed in global coordinates including each target's
  gap, otherwise lines jump at the seams.
- Respect the rounded gallery corners (iPhone ≈ 125 lu, `layout_report.gallery.radius_lu`): nothing square or important there;
  inner frames get radius = store radius − inset.

## Props and decoration

- One consistent prop set per screenshot set: same light direction, camera angle and material;
  true-to-scale relative to the device; touching frames, never covering the UI.
- Decoration must not look like a UI system icon (image placeholder, warning triangle,
  spinner, broken image) – viewers read it as a rendering error.

## Decoration density

Pick a density and stay consistent: minimal (0 floating elements), editorial (2–5 per page),
maximalist (8–14, some bleeding off the edge). Wrong density reads as a style misfire.
Illustrations need volume: two tones per shape, a cast shadow, a texture or highlight –
flat single-fill shapes look like clip art.

## Category conventions (starting points, not laws)

- Finance: trust and clarity before expression; calm palettes, big numbers.
- Productivity: show the workflow fast; clean, editorial.
- Health/wellness: soft, warm, slow; generous space.
- Fitness/sport: dark, high-energy, one volt accent, big stats.
- Games: atmosphere and characters may lead; gameplay must be visible.
- Kids/family: primaries, rounded type, friendly shapes – but no tiny details.
Study 3–5 direct competitors, decide which conventions to keep and which to break.
