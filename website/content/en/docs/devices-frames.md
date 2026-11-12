---
title: Devices & frames
description: "Built-in device frames, Apple Product Bezels you import yourself (and store in a private project for CI), your own frame packs, device swaps per target, foldables and watches."
weight: 50
icon: frame
---

A **device** element shows one of your screenshots inside a device frame. You
choose a **model** (the shape: phone with island, Android tablet, notebook …)
and a **frame** (how it is drawn). Three kinds of frames are available:

1. **Built-in frames** – drawn by Vitrine Studio itself, always available.
2. **Apple Product Bezels** – Apple's official device images, which you
   download yourself and import once per computer.
3. **Your own frame packs** – any PNG or SVG frame with a small description
   file.

## Built-in devices

| Class | Models |
|---|---|
| Phones | Phone with island, Phone with notch, Phone with home button, Android phone with punch-hole camera, Foldable phone (open / folded) |
| Tablets | Tablet without home button, Tablet with home button, Android tablet |
| Laptops | Notebook (16:10), Laptop (16:9) |
| Desktops | All-in-one desktop (16:9), Monitor (16:9) |
| Watches | Rugged watch, Watch |
| Browser | Browser window |
| Without frame | No frame (rounded screenshot) |

The built-in frames are Vitrine Studio's own artwork, drawn from measurements –
they look like the real devices but contain no vendor graphics. Two styles:

- **Detailed** – rims, glass edge, buttons and camera; the default.
- **Stylized** – flat and simple, few details.

Each model has its own colours (*Black titanium*, *Natural titanium*,
*Midnight*, *Silver*, *Sage* …) and you can type any `#RRGGBB`.

**Browser windows:** any device can get *Chrome → Browser* with a *URL* and a
*Light/dark* look – handy for web apps and Mac screenshots on a website.

## Setting up a device

Select a device and look at the inspector:

- **Model** and **Frame** (*Project* = the project default, *Stylized*,
  *Detailed*, *Apple*, or a frame pack).
- **Colour** – the swatches of the chosen frame.
- **Orientation** – *Portrait* or *Landscape*.
- **Screenshot** – the **key** of the screenshot to show (see
  [Screenshots](../screenshots/)); *Fit* decides how a screenshot of a
  different aspect fills the display.
- **Shadow** – a shadow preset (*soft*, *hard* or one of your project's).
- Size: set the **width**; the height follows the device.

The project's default frame is in **Project settings → General → Frame
style**; new projects start with the default from the app settings (*Frame
style for new projects*).

## Device swap per target

One canvas, different devices per store: a target's **Device swap**
(*Canvases & targets* → target) replaces models for that target only. The
Google Play target of the Phone template swaps the iPhone for the Android
phone; an iPad target can swap it for a tablet. Variant, screenshots and
layout stay the same.

## Apple Product Bezels

Apple publishes official device images ("Product Bezels") in its
[Design Resources](https://developer.apple.com/design/resources/). Apple's
licence doesn't allow Vitrine Studio to ship them, so you download them
yourself and import them once – they then work in every project on that
computer.

1. Download the bezel package for your devices (e.g. *Bezel-iPhone-18.dmg*)
   from developer.apple.com/design/resources (Help menu → *Download Apple
   Product Bezels* opens the page).
2. **Open the DMG in Finder and accept the licence.** The disk image contains a
   licence agreement; importing the `.dmg` file directly fails. After
   accepting, the volume is mounted (e.g. `/Volumes/Bezel-iPhone-18`).
3. **App settings → Frame packages → Manage…** → **Choose folder…** and pick
   the mounted volume (a `.zip` or an extracted folder works too:
   **Choose .dmg or .zip…**, or drop it onto the window).
4. Check the list – device, colour and orientation are recognised from the
   file names, the screen area is detected – and click **Import**.

{{< placeholder "The Frame packages dialog after analysing an Apple bezel package." >}}

To use them:

- For the whole project: **Project settings → General → Frame style → Apple
  (imported)**.
- For one device: inspector → **Frame → Apple**.

With an Apple frame, the colour swatches show the colours of the package (e.g.
Black, Burgundy, Glacier, Silver for iPhone 18). If several sizes are
installed (iPhone 18 Pro and Pro Max), **Bezel → Automatic** picks the one
matching the target's image size; you can pin one per device.

Good to know:

- Bezels are stored in the app data folder; the project only says
  `frame: imported:apple`. On a computer without the bezels (a colleague's,
  CI) the built-in detailed frame is drawn and validation notes
  `imported_frame_missing` – unless you
  [store the used bezels in the project](#apple-bezels-in-ci).
- A colour that isn't installed there falls back to an installed one
  (`imported_color_fallback`).
- The import report separates files **skipped on purpose** (e.g. marketing
  composites in the iPhone Duo package), files you chose to **ignore**, and
  **unmatched** files you can assign by hand.
- Apple's terms allow the bezels for mock-ups of apps on Apple platforms –
  read the licence file in the package.

On the command line:

```sh
vitrine frames import /Volumes/Bezel-iPhone-18 --dry-run   # show what would be imported
vitrine frames import /Volumes/Bezel-iPhone-18
vitrine frames list
vitrine frames remove --bezel "iPhone 18 Pro"              # or --model, --color, --all
```

## Apple bezels in CI: store them in the project {#apple-bezels-in-ci}

A build server has no imported bezels, so `vitrine export` there would draw
the built-in frames. To get the same images as on your Mac, store the bezels
your project uses **in the project**:

- **Project settings → Frames → Store used device frames in the project**, or
  `vitrine frames embed`.
- Only the bezels your targets actually draw are copied (model × colour ×
  orientation × size), as imported, into `frames/imported/apple/` – usually a
  few MB. **All sizes of the used models** also keeps e.g. the iPhone Pro next
  to the Pro Max.
- The folder carries a `README.md` with the licence notice and, when the
  package was imported with it, Apple's licence file
  (`apple/licenses/<package>/License.rtf`).
- Frames are then looked up in the project first, then in this computer's
  frame store, then the built-in frame is drawn. The inspector says where a
  bezel comes from (*stored in the project* / *this computer's frame store*).
- When devices, colours or targets change, the tab shows what an update would
  do (new, re-imported, no longer used) – click **Update stored frames** or
  run `vitrine frames embed` again. Validation notes it as well:
  `imported_frame_not_stored` (a target draws bezels only this computer has),
  `stored_frames_stale` (unused or outdated copies) and
  `imported_frames_stored` (licence reminder).
- **Remove from project** / `vitrine frames unembed` deletes the folder.

```sh
vitrine frames embed --dry-run      # what would be stored, with sizes
vitrine frames embed                # store or update (--all-sizes, --used-only)
vitrine frames unembed              # remove frames/imported/
```

{{< callout warn >}}
**Licence: private repositories only.** Apple's licence allows the bezels in
mock-ups but not redistributing them. Stored frames are fine in a private
repository; never publish the project with them – no public repository, no
shared archive. Templates and gallery uploads never contain them. For a public
repository add `frames/imported/` to `.gitignore` (CI then draws the built-in
frames) or remove the stored frames.
{{< /callout >}}

## Your own frame packs

Any device frame you are allowed to use – your own drawing, a CC0 kit, a design
tool export – can become a **frame pack**: a folder with PNG or SVG frames and a
`frame.yaml`.

```yaml
format: vitrine-frame/1
name: Clay phone
author: Jane Doe
license: CC0-1.0
models: [iphone-island, android-punchhole]   # omit = fits any device
frames:
  - file: clay-portrait.png
    color: Sand
    orientation: portrait
  - file: clay-portrait-night.png
    color: Night
```

- **Screen area:** detected automatically from the transparent area around the
  image centre (like Apple's bezels); or give `screen: {x, y, w, h}` with a
  corner `radius`; or a `mask` image. A frame with a painted (opaque) screen
  needs `cut_screen: true`.
- **Several sizes:** add frames with different `bezel` names; they are picked
  like Apple's (by target size, then screenshot size).
- **A pack without `models`** fits any device – useful for devices the catalog
  doesn't have; the element's height follows the frame.
- Unknown fields are errors, so typos are reported.

Import it in **App settings → Frame packages** (folder or `.zip`), choosing
**Import into** *This computer (all projects)* or *This project (frames/
folder, travels with it)*. Or on the command line:

```sh
vitrine frames import clay-pack/ --to project     # or --to app (default); --id to rename
vitrine frames list --packs
vitrine frames remove --pack clay-pack --from project
```

Devices then use **Frame → Pack: Clay phone** (`frame: pack:clay-pack`). A
project pack hides an app-wide pack with the same id. Copying a pack folder
into the project's `frames/` folder by hand works too; after changing files in
it, re-open the project or import it again.

The complete field reference is in the repository:
[`docs/guide/frame-packs.md`](https://github.com/vitrinestudio-app/vitrine/blob/main/docs/guide/frame-packs.md).

{{< callout warn >}}
Check the terms of vendor kits before you use them in store listings. Apple's
bezels are for mock-ups of Apple-platform apps and may not be redistributed;
Google's Device Art Generator output may not be used in Play listings. Keep
vendor packs app-wide; Apple bezels go into a project only as
[stored frames](#apple-bezels-in-ci) of a private repository.
{{< /callout >}}

## Foldables: iPhone Duo

Two models, one per state: *Foldable phone (open)* (`iphone-duo`, inner
display, landscape by nature) and *Foldable phone (folded)*
(`iphone-duo-folded`, outer display). The inspector's **Fold** (*Open* /
*Folded*) switches between them – also after a device swap. The presets
`ios-iphone-duo-inner` and `ios-iphone-duo-outer` export the matching sizes;
Apple accepts them only through the App Asset Library (see
[Store upload](../store-upload/#app-store-upload-method)).

## Watches and Wear OS

The App Store and Google Play want **bare app screenshots** for watches – no
frame, no text, no background. Watch presets (`watch-ultra`, `watch-series`, …)
and `play-wear` are therefore **screenshot only**: each page exports the
screenshot of its largest device alone, filling the image. You can still design
a marketing layout with the watch frames on another canvas, e.g. for your
website. See [Targets & store formats](../targets/#screenshot-only-targets).
