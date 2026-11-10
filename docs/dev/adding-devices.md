# Adding built-in devices (parametric frames)

How to add a new device model to Arrangia's built-in frames, and the prompt to
hand to Claude Code for it.

## How built-in frames work
- **Data:** every model is an entry in `crates/core/data/devices.yaml` (embedded
  at build time, typed in `crates/core/src/catalog/devices.rs`). The header of
  that file documents every field: `class`, `unit`, `natural` orientation,
  `orientations`, `body {w,h,radius}`, `rim`, `display {x,y,w,h,radius}`,
  `cutouts` (island / notch / punch_hole over the screen; home_button / camera /
  speaker in the bezel), `buttons {edge, at, length, depth}`, `laptop` (hinge +
  base), `stand` (neck + foot), `browser`, `colors` (names from the shared
  palette), `default_color`, `screenshot {aspect|free, variant, sizes, targets}`.
- **Drawing:** `crates/render/src/frames/` turns that data into vector paths
  (tiny-skia) at render time, in two styles: `stylized` (flat) and `detailed`
  (rim gradients, gloss, lens details). Main entry points:
  `device_geometry(dev, size)` and `draw_device(pixmap, dev, size, screen, transform)`.
  Per class: `handheld.rs` (phones/tablets), `computer.rs` (laptops/desktops),
  `browser.rs` (browser window + `chrome: browser`), shared helpers in
  `shapes.rs`, `screen.rs`, `palette.rs`, `layout.rs`.
- Only ratios matter (numbers are in a model unit such as mm); the renderer
  scales the outer width to the element's `width` in lu.
- Landscape = the portrait-natural geometry rotated 90° counter-clockwise;
  the screenshot itself is never rotated.

## Two kinds of additions
1. **Data only** – a new model of an existing class whose shape the drawing
   code already supports (e.g. another phone with an island, a tablet with
   different bezels, a laptop with other proportions). Only `devices.yaml`
   (+ colours, + `targets.yaml` cross-references) changes.
2. **New shape/class** – something the drawing code can't express yet (e.g. a
   foldable / "iPhone Duo" with two screens, a watch with a crown, a TV, a
   phone with a camera bar in the screen). Needs catalog schema additions
   (`catalog/devices.rs`, validation in `Catalog::check()`) and drawing code in
   `frames/`, plus tests.

## Rules (non-negotiable)
- **Own artwork only.** Generic shapes that resemble a device class. Never trace,
  copy or embed vendor images (Apple Design Resources, Samsung/Google press
  images, …), no logos, no trademarked model names as ids or labels
  (ids like `iphone-island` describe the class; labels like "Phone with island").
  Real Apple bezels are a separate, user-imported source (ADR 0012).
- Proportions may be taken from public spec sheets (outer dimensions in mm,
  display diagonal/resolution); corner radii and bezels estimated.
- Both styles must look good; `detailed` must not look like a photo copy.
- Store/screen sizes referenced in `screenshot.sizes` / `targets` must match the
  display aspect within 2 % (enforced by `Catalog::check()`).

## Verification
- `cargo test --all` (catalog checks, frame geometry invariants for every model ×
  orientation, smoke tests drawing every model in both styles).
- Contact sheets: `cargo run -p arrangia-render --release --example frames`
  → `docs/spike/frames_catalog_{stylized,detailed}.png`; close-ups:
  `… --example frames -- --closeup <dir>`. Look at them (downscaled) and check:
  screen corners concentric with the body, cutouts placed right in portrait and
  landscape, buttons on the right edges, colours in both styles, no seams
  between body and screen, shadow silhouette matches.
- Optionally render the example project with the new model via a target
  `device_map` and check it in the gallery preview.

## Prompt template for Claude Code

Copy, fill in the angle brackets, paste into Claude Code in this repository:

```
Add a new built-in parametric device model to Arrangia.

Device: <short description, e.g. "foldable phone with two screens, book-style, portrait natural">
Proposed id / label: <e.g. phone-fold / "Foldable phone">
Class: <phone | tablet | laptop | desktop | browser | new class: …>
Reference proportions (public specs, no vendor images): <outer w × h × radius in mm, display size/resolution, notable features: island/notch/punch hole/home button/hinge/stand/crown …>
Screenshot sizes it should accept: <e.g. 1080×2400; or store target ids from crates/core/data/targets.yaml>
Colour variants: <existing palette names from devices.yaml, or new ones with body/edge/rim/glass colours>
Orientations: <portrait, landscape>

Read first: CLAUDE.md, docs/plan/STATUS.md, docs/dev/adding-devices.md,
crates/core/data/devices.yaml (header documents all fields),
crates/core/src/catalog/devices.rs, crates/render/src/frames/ and ADR 0001/0004/0012.

Requirements:
- Own generic artwork only (see "Rules" in docs/dev/adding-devices.md): no vendor
  images, no logos, no trademarked names in id/label.
- If the existing drawing code can express the shape, change data only
  (devices.yaml, colours, targets.yaml cross-references). Otherwise extend the
  catalog schema (+ Catalog::check()) and frames/ drawing for both styles
  (stylized + detailed), portrait and landscape.
- Keep Catalog::check() green (display aspect vs. screenshot sizes within 2 %).
- Tests: geometry invariants and both-style smoke tests must cover the new model
  (they iterate the catalog – make sure any new class is included).
- Regenerate the contact sheets (cargo run -p arrangia-render --release --example frames),
  look at the result and a close-up critically, iterate until it looks good.
- Add user-facing notes to docs/guide/NOTES.md; update this document if the
  schema/drawing structure changed.
- Quality gate before each commit: cargo fmt --all; cargo clippy --all-targets
  --all-features -- -D warnings; cargo test --all; cargo build -p arrangia-gui.
- Show me the contact-sheet crop of the new device before finishing.
```
