# Frame packs – format reference (`vitrine-frame/1`)

An own frame pack is a folder with PNG and/or SVG device frames and a small
`frame.yaml` (ADR 0024, KONZEPT §4.2 item 3). Vitrine Studio ships no vendor
images; packs are drawn by you, exported from a design tool, or built from
vendor material whose licence allows it (see
`docs/research/2026-11-11-vendor-frames.md`).

## Where packs live

| Place | Folder | Who sees it |
|---|---|---|
| Project | `<project>/frames/<id>/` | everyone who has the project (Git) |
| App-wide | `<app data>/frames/packs/<id>/` | every project on this computer |

A project pack hides an app pack with the same id. The folder name is the
pack id (letters, digits, `-`, `_`, `.`). Devices use a pack with
`frame: pack:<id>` (element or project default `styles.frame`), the colour
with `color: <colour id>`, a size with `bezel: <name>`.

Compiled files (bezel PNG + screen mask per frame) are cached and rebuilt
when the pack changes: `<project>/.cache/frames/<id>/` (ignored by Git) or
`<app data>/frames/packs/<id>/.compiled/`.

Import: GUI app settings → Frame packages → choose or drop the folder /
`.zip` (target: this computer or this project); CLI `vitrine frames import
<folder|zip> [--to app|project] [--id ID] [--dry-run]`. List: `vitrine
frames list --packs`; remove: `vitrine frames remove --pack <id> [--from
app|project]`. Copying a folder into `frames/` by hand works too.

## `frame.yaml`

```yaml
format: vitrine-frame/1          # required
name: Clay phone                 # required, shown in the GUI
author: Jane Doe                 # optional
license: CC0-1.0                 # optional: licence of the images (free text / SPDX)
source: https://example.com/kit  # optional: where the images come from
attribution: "Frames by Jane"    # optional: credit line the licence asks for
models: [iphone-island, android-punchhole]
                                 # catalog models the frames stand for
                                 # (`vitrine` catalog / catalog_devices);
                                 # omit = any device with `frame: pack:<id>`
frames:                          # required, at least one
  - file: clay-portrait.png      # PNG or SVG, relative to the pack folder
    color: Sand                  # colour name; id = slug (`sand`); default `Default`
    orientation: portrait        # default: from the image (wider than high = landscape)
    bezel: Clay 6.3"             # size/variant name; default: the pack name
    models: [iphone-island]      # overrides the pack's `models` for this frame
    screen: {x: 72, y: 69, w: 1206, h: 2622}
                                 # screen rectangle in image px (SVG: user units);
                                 # default: detected (transparent area around the centre)
    radius: 160                  # screen corner radius with an explicit `screen`
    mask: clay-mask.png          # optional mask image (same size; white/opaque = screen)
    cut_screen: true             # make the screen area transparent (painted screens);
                                 # needs `screen` or `mask`
    raster: 3000                 # SVG only: long side in px (default 3000)
```

Unknown fields are errors (typos are reported, not ignored).

### Screen

- **Detected (default):** the screen must be transparent (alpha < 50 %)
  around the image centre and enclosed by the frame. Opaque parts inside
  (island, notch, camera) are drawn over the screenshot. Same detection as
  the Apple bezels (flood fill, ADR 0012).
- **`screen` + `radius`:** an explicit rounded rectangle. With a painted
  (opaque) screen add `cut_screen: true`.
- **`mask`:** any screen shape (white or opaque = screen); `radius` is then
  only informational.

### Several sizes, colours, orientations

Each `frames` entry is one image for one colour × orientation × `bezel`.
Several `bezel` names for the same model, colour and orientation are picked
like Apple's bezels: `bezel:` on the element, else the frame whose screen
has the target's page size, then the screenshot's size, then the closest
aspect, else the largest. A device in landscape needs a landscape frame;
otherwise the built-in frame is drawn (info `imported_frame_missing`).

### New devices and sizes

A pack without `models` fits any device element: the element's height
follows the frame's aspect, the screenshot fills the screen (`fit` cover by
default). Pick the closest catalog model for the device (it still decides
the default screenshot variant and the parametric fallback).

## Example

The synthetic test pack (`cargo run -p vitrine-core --example
synthetic_frame_pack -- <dir | file.zip>`) has PNG frames with transparent
screens in two colours and both orientations, a painted-screen PNG with
`screen` + `cut_screen`, and an SVG frame.

## Licence

Only use images you may use: your own, CC0/CC BY kits (credit them with
`attribution`), or vendor art whose terms allow store marketing. Vendor
terms usually forbid redistribution: keep such packs app-wide (not in a
shared project) and never publish them.
