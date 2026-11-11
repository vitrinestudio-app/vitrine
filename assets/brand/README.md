# Vitrine Studio brand assets

Own work (drawn for the project on 2026-11-11), no third-party material.

| File | Use |
|---|---|
| `vitrine-studio.svg` | Logo mark, full bleed (website, favicon, docs) |
| `vitrine-studio-appicon.svg` | App icon on the macOS grid (824 px squircle on a 1024 canvas, soft shadow) – source of `crates/gui/icons/*` |
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
