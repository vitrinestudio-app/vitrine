# Vitrine Studio brand assets

Own work (drawn for the project on 2026-11-11), no third-party material.

## Licence: not GPL

The name "Vitrine Studio", the logo and the app icons are **© 2026
Looking4Cache UG (haftungsbeschränkt), all rights reserved**, and are *not*
covered by the GPL licence of the source code (ADR 0022). This covers the
files in this folder, the app icons generated from them
(`crates/gui/icons/*`) and `crates/gui/ui/img/logo.svg`.

- You may redistribute **unmodified** copies of Vitrine Studio (source or
  binaries built from the official sources) with the name and logo, and
  use them to refer to the project (articles, reviews, links).
- **Forks and modified versions** must not use the name "Vitrine Studio" or
  the logo as their own name or app icon. Replace the files listed above
  and change `productName`/`identifier` in `crates/gui/tauri.conf.json`.
- Other uses (merchandise, a commercial product's branding, …): ask the
  owner first.

## Files

| File | Use |
|---|---|
| `vitrine-studio.svg` | Logo mark, full bleed (website, favicon, docs) |
| `vitrine-studio-appicon.svg` | App icon on the macOS grid (824 px squircle on a 1024 canvas, soft shadow) – source of `crates/gui/icons/*` |
| `vitrine-studio-nobackground.svg` / `.png` | Mark without the white tile (three pages only, transparent background) – website, docs, dark backgrounds |
| `vitrine-studio.png` | 1024 px PNG of the full-bleed mark |
| `vitrine-studio-mono.svg` | One-colour mark (menu bar / template images, small sizes) |

The motif is the core idea of the app: three store pages with gaps, and one
background (gradient + wave) running continuously across them.

## Regenerating the app icons

1. Render `vitrine-studio-appicon.svg` to a 1024×1024 PNG with a full SVG
   renderer (resvg – the same library the app uses; ImageMagick's built-in
   renderer drops gradients and clip paths).
2. `cargo tauri icon <that.png> -o <temp dir>` and copy `32x32.png`,
   `128x128.png`, `128x128@2x.png`, `icon.png`, `icon.icns`, `icon.ico` into
   `crates/gui/icons/` (the files listed in `crates/gui/tauri.conf.json`).
