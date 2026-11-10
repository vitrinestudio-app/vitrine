//! Render tests for effects that depend on the drawn pixels rather than the
//! element rectangle: image shadows from the alpha channel.
//!
//! Set `ARRANGIA_RENDER_DEBUG_DIR` to also write the rendered images there.

#![allow(clippy::many_single_char_names)]

use std::path::Path;

use arrangia_core::io::project_from_yaml;
use arrangia_core::layout::FontLibrary;
use arrangia_core::resolve::{FsAssets, ResolveOptions, resolve};
use arrangia_render::raster::save_png_rgb;
use arrangia_render::scene::{RenderContext, RenderOptions, Rendered};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// A red ring on a transparent background (outer radius 45, inner 35 of 100).
const RING_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><circle cx="50" cy="50" r="40" fill="none" stroke="#FF0000" stroke-width="10"/></svg>"##;

fn write_assets(root: &Path) {
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::write(root.join("assets/ring.svg"), RING_SVG).unwrap();
    // The same ring as PNG with alpha.
    let mut pm = Pixmap::new(200, 200).unwrap();
    let mut paint = Paint::default();
    paint.set_color_rgba8(255, 0, 0, 255);
    paint.anti_alias = true;
    let circle = PathBuilder::from_circle(100.0, 100.0, 80.0).unwrap();
    pm.stroke_path(
        &circle,
        &paint,
        &Stroke {
            width: 20.0,
            ..Stroke::default()
        },
        Transform::identity(),
        None,
    );
    pm.save_png(root.join("assets/ring.png")).unwrap();
    // An opaque square (rect shadow path).
    let mut sq = Pixmap::new(50, 50).unwrap();
    sq.fill(Color::from_rgba8(0, 0, 255, 255));
    sq.save_png(root.join("assets/square.png")).unwrap();
}

fn debug(r: &Rendered, name: &str) {
    if let Ok(dir) = std::env::var("ARRANGIA_RENDER_DEBUG_DIR") {
        save_png_rgb(&r.pixmap, &Path::new(&dir).join(name), [255, 255, 255]).unwrap();
    }
}

/// Luma of the (straight) pixel at `(x, y)` over white.
fn luma(r: &Rendered, x: u32, y: u32) -> u8 {
    let c = r.pixmap.pixel(x, y).unwrap().demultiply();
    let a = f32::from(c.alpha()) / 255.0;
    let over = |v: u8| f32::from(v) * a + 255.0 * (1.0 - a);
    ((over(c.red()) + over(c.green()) + over(c.blue())) / 3.0) as u8
}

const IMAGES: &str = r##"
format: arrangia/1
name: fx
languages: {default: en, available: [en]}
styles:
  shadows:
    drop: {layers: [{color: "#000000CC", blur: 8}]}
strips:
- id: s
  page: {aspect: 1:1, count: 3}
  backgrounds: ["#FFFFFF"]
  elements:
  - {id: svg, kind: image, page: 0, at: {x: 500, y: 500}, width: 400, asset: assets/ring.svg, shadow: drop}
  - {id: png, kind: image, page: 1, at: {x: 500, y: 500}, width: 400, asset: assets/ring.png, shadow: drop, rotation: 30}
  - {id: sq, kind: image, page: 2, at: {x: 500, y: 500}, width: 400, asset: assets/square.png, shadow: drop}
targets:
- {id: t, strip: s, size: [1000, 1000]}
"##;

#[test]
fn image_shadow_follows_the_alpha_channel() {
    let dir = tempfile::tempdir().unwrap();
    write_assets(dir.path());
    let p = project_from_yaml(IMAGES).unwrap();
    let s = resolve(
        &p,
        "t",
        "en",
        &ResolveOptions::new(&FsAssets::new(dir.path())),
    )
    .unwrap();
    let ctx = RenderContext::with_fonts(dir.path(), FontLibrary::bundled());
    let prepared = ctx.prepare(&s);
    for (i, name) in ["svg", "png"].into_iter().enumerate() {
        let r = ctx.rasterize_prepared(&prepared, i, &RenderOptions::export());
        debug(&r, &format!("alpha_shadow_{name}.png"));
        assert!(r.issues.is_empty(), "{:?}", r.issues);
        // Ring: outer radius 180 lu, inner 140 lu around (500, 500).
        assert_eq!(luma(&r, 500, 500), 255, "{name}: shadow inside the hole");
        assert!(luma(&r, 500, 380) > 250, "{name}: shadow inside the hole");
        // Just outside the ring: shadow.
        assert!(luma(&r, 500, 684) < 240, "{name}: {}", luma(&r, 500, 684));
        // Corners of the image rect stay clean (no rectangular shadow).
        assert!(luma(&r, 305, 305) > 250, "{name}: {}", luma(&r, 305, 305));
        assert!(luma(&r, 695, 305) > 250, "{name}: {}", luma(&r, 695, 305));
    }
    // An opaque image keeps its rectangular shadow.
    let r = ctx.rasterize_prepared(&prepared, 2, &RenderOptions::export());
    debug(&r, "alpha_shadow_opaque.png");
    assert!(luma(&r, 500, 705) < 225, "{}", luma(&r, 500, 705));
    assert!(luma(&r, 500, 500) < 100);
}
