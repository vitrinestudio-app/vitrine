---
title: Troubleshooting & FAQ
description: "Common validation findings and how to fix them, frequent questions, and current limits."
weight: 160
icon: help
---

## Validation findings

The validation badge (app) and `vitrine validate` (CLI) report findings per
target and language as **error**, **warning** or **info**. Errors make
`vitrine validate` exit with 1; `--strict` fails on warnings too. The most
common ones:

| Code | Level | Meaning and fix |
|---|---|---|
| `screenshot_missing` | error | no screenshot with this key in the language or its fallbacks – a placeholder is shown. Add the file, or use *Preview mode* / `--preview` while designing. |
| `screenshot_fallback` | info | the screenshot comes from a fallback language – fine. |
| `screenshot_aspect` | warning | the capture's aspect differs from the device display – wrong device or wrong screenshot variant folder? |
| `screenshot_upscale` | warning | the capture is enlarged more than 15 % and may look soft – use a capture from a larger device. |
| `text_overflow` | error | the text doesn't fit even at its minimum size – see [Fitting long texts](../languages/#fitting-long-texts). |
| `emergency_break` | warning | a word was broken without a hyphen – add a soft hyphen `\-`. |
| `hyphenated` | info | the text was hyphenated automatically – check the render. |
| `text_fallback` | warning / info | text from the default language ("not translated", warning) or from another fallback (info). |
| `text_missing` | error | the key exists in no language – nothing is drawn. |
| `text_in_gap`, `text_clipped` | warning | text reaches into a store gap or past its page and is cut off. |
| `text_outside_safe_area` | warning | text outside the target's safe area. |
| `low_contrast` | warning | the text's contrast against what is actually drawn behind it (colour, gradient, photo, pattern) is below WCAG AA – 4.5:1, large text (from 80 lu, bold from 62 lu) 3:1. The message names the ratio and where; change the colour, add a scrim, shadow or outline. |
| `text_near_device` | warning / info | glyphs hidden behind or running over a device frame (warning), or closer than 16 lu to it (info). Text lying completely on a device, or on a card over it, is fine. |
| `screen_header_covered`, `screen_header_cut` | info | an element covers the top bar (status bar, app title) of a device's screen, or the page edge cuts through it so half a bar shows. |
| `seam_overlap` | info | an element reaching across a page border is drawn over an element of the next page – check the gallery. |
| `seam_split` | info | one shape built as two halves at a page border: the store gap pulls them apart. Use **one** element with *Clip: none* across the border – it stays continuous behind the gap in every target. |
| `element_in_corner` | info | an element reaches into an image corner the store rounds off. |
| `device_small` | info | the largest device of a page fills less than half of it. |
| `page_not_covered` | info | a picture or panel covers almost the whole page but leaves a thin strip (typical after a variant changed the page height). |
| `page_blends_with_store` | info | the edges of the images are hardly distinguishable from the store's light or dark background. |
| `missing_glyphs` | warning | characters no font can draw – add a [fallback font](../styles-fonts/#other-scripts-fallback-fonts). |
| `variant_suggested` | info | the target's page aspect differs a lot from the canvas – consider a variant. |
| `store_size`, `store_count`, `store_format` | error | the size, number of images or format is not accepted by the store slot (too few images is a warning). |
| `imported_frame_missing` | info | Apple bezels or a frame pack aren't installed here – the built-in frame is drawn. |
| `imported_frame_not_stored` | info | a target draws Apple bezels only this computer has – CI would draw the built-in frame; [store them in the project](../devices-frames/#apple-bezels-in-ci). |
| `stored_frames_stale` | info | bezels stored in the project are no longer used, or this computer has a newer import – update the stored frames. |
| `imported_frames_stored` | info | the project stores Apple bezels – keep the repository private. |
| `color_profile_unsupported` | info | an image's colour profile can't be used (CMYK, HDR) – it is drawn as sRGB. |
| `background_hidden` | warning | a canvas background layer lies under an opaque one and never shows – canvas backgrounds are layers, not one per page (see below). |

In the app, **Show on the canvas** jumps to the element.

The design checks (`low_contrast` to `page_blends_with_store`) render the
images; `vitrine validate --no-pixel-checks` skips them. When a design means
it – giant clipped type, a label pointing at a device – list the code in the
element's `allow` (`allow: [text_clipped, low_contrast]`, also on a group).
Project-wide thresholds and codes switched off go into `project.yaml`:

```yaml
checks:
  device_distance: 16      # lu between glyphs and a device frame
  min_contrast: 4.5
  min_contrast_large: 3
  off: [page_blends_with_store]
```

## Questions

**Where is the Save button?**
There is none – every change is saved immediately, as one undo step.

**Can I edit the project in a text editor or with Git while the app is open?**
Yes. Changes on disk are picked up live. If you changed the same thing in the
app at the same moment, the app reloads the file and tells you that your last
edit was not saved.

**My French headline is tiny.**
Auto-fit shrank it to fit. French often needs noticeably more room than
English – "Edit photos in seconds" becomes "Retouchez vos photos en quelques
secondes". Give French a [language override](../languages/#language-overrides)
with a wider box or a higher minimum size, add a soft hyphen, or switch on
hyphenation.

**I gave the canvas one background colour per page, but every page has the same colour.**
The canvas backgrounds are **layers** over the whole canvas, bottom to top –
the top opaque one covers the others. For a colour (or photo) per page add a
*Background* element on each page; it fills its page at every target's
height. Elements paint in the order of their layer (`z`) and the layers list,
so keep page backgrounds at the bottom (a low `z`).

**My background element disappeared behind a photo.**
Background elements are ordinary layers: an opaque image above them in the
layers list (or with a higher `z`) covers them. Move the background down or
give the image transparent areas.

**The phone sits too high on Google Play.**
Google Play's 1:2 page is shorter than the iPhone page. Set the phone's
*Anchor* to *Bottom*, or move it in a variant for the Play target.

**How do I get real Apple device frames?**
Download the Product Bezels from Apple and import them once – see
[Devices & frames](../devices-frames/#apple-product-bezels). Importing the
`.dmg` directly fails: open it in Finder, accept the licence, import the
mounted volume.

**A colleague sees different frames.**
Apple bezels and app-wide frame packs are stored per computer. Without them,
the built-in frame is drawn. Put frame packs you may share into the project's
`frames/` folder; Apple's bezels must be imported by each person.

**The export folder has old files.**
Re-exporting removes images Vitrine Studio wrote earlier and no longer
produces, but never other files. Delete foreign files yourself.

**Upload says "no editable App Store version".**
Create a new version in App Store Connect, then plan again. See
[Store upload](../store-upload/#troubleshooting) for more messages.

**Can I use Vitrine Studio without the app, e.g. in CI?**
Yes – the [CLI](../cli/) validates, exports, imports and uploads; use the
standalone CLI archive on build machines and `VITRINE_APP_DIR` for a separate
settings folder. See [Automation](../automation/).

**Does Vitrine Studio send data anywhere?**
Only when you ask it to: store uploads and connection tests go to Apple or
Google, the template gallery (in the app or with `vitrine gallery`) to
`api.vitrinestudio.app`, and the daily update
check asks GitHub whether a new version exists (switch it off in *App
settings → Updates*). No statistics, no account.

**Can an AI assistant break my project?**
Every change is one undo step labelled with the assistant's name, and you see
it live. Store uploads need your click. See [AI assistants](../ai-mcp/).

## Current limits

- Right-to-left layouts (Arabic, Hebrew) are not mirrored yet.
- Text inside SVG images is not drawn yet.
- Exports are sRGB only (no Display P3 output).
- Gallery gaps of the native store apps are estimates; the store websites were
  measured.
- No Swedish hyphenation patterns.
- Frame packs in `frames/` are not recompiled while the project is open –
  re-open it after changing them.

## Getting help

Report bugs and ideas on
[GitHub](https://github.com/vitrinestudio-app/vitrine/issues). Include the
output of `vitrine validate --format json` if it's about a project, and never
include store keys.
