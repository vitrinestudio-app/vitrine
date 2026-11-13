# QA gate – run before showing anything

Render the gallery and every page (`render_preview`), per target and language. Any "no"
means fix and re-render.

## Message
- [ ] Page 1 says what the app is for in one second (thumbnail, arm's length).
- [ ] Pages 1–3 work together as a story without the rest.
- [ ] One idea per page; no two pages make the same claim.
- [ ] Every claim is backed by visible UI on that page.
- [ ] No banned words, prices, CTAs, awards, other platforms; proof only if true (never on Play).

## Visual
- [ ] Thumbnail test: page shrunk to ~200 px wide, on #F2F2F7 and on #1C1C1E – headline
      readable, app purpose clear, page doesn't dissolve into the store background.
- [ ] Headline contrast ≥ 4.5:1 at its worst point, including the emphasis word.
- [ ] No text clipped, no word wider than ~88 % of the page, no orphan single-word line.
- [ ] Device ≥ ~65 % of page height on device pages; no empty band > ~22 %.
- [ ] Adjacent pages use different layouts; contrast page present in sets of 5+.
- [ ] Cross-page elements: each exported page still reads alone; nothing important in a gap.
- [ ] UI inside devices is real/realistic, full, clean status bar, correct frame per device.
- [ ] Consistent palette, type, margins and frame treatment across the set.

## Store
- [ ] App Store: > 50 % of pages show the app in use; iPad pages use iPad frames.
- [ ] Play: no device imagery, tagline ≤ 20 % of the image, alt text prepared.
- [ ] `validate` clean for every target × language: 0 errors, 0 warnings; every `allow`
      intended and explainable.
- [ ] Only one upload target per store slot (portrait vs. landscape tablet).

## What validate can't see – check by eye
- [ ] No prop or image over important screen content (check also below `screen_cover`).
- [ ] Motifs inside background images: square shapes out of rounded store corners, edges and
      lines continue at the same height across seams.
- [ ] Seam-crossing motifs are one element (`clip: none`), lined up in every target's gallery.
- [ ] Props consistent in light, angle and scale; no decoration that reads as a system icon.
- [ ] No same-colour-on-same-colour images; nothing overlapping a headline.
- [ ] Play variants: overrides complete (no `dropped_override_fields` left unchecked).

## Languages
- [ ] Every language filled; rendered and looked at, not just validated.
- [ ] German (and other long languages) fit with the same size or a documented override.
- [ ] Screenshot UI language matches the copy language (or fallback is intentional).

## Templates only
- [ ] Phone and tablet canvases (tablet orientation as planned), App Store and Play targets, EN (default) and DE.
- [ ] No real brand, logo, character, person or trademark anywhere.
- [ ] Placeholder copy follows the copy rules and the style's voice.
- [ ] README written, previews saved, asset provenance noted.
