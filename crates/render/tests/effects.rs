//! Render tests for effects that depend on the drawn pixels rather than the
//! element rectangle: image shadows from the alpha channel.
//!
//! Set `VITRINE_RENDER_DEBUG_DIR` to also write the rendered images there.

#![allow(clippy::many_single_char_names)]

use std::path::Path;

use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};
use vitrine_core::io::project_from_yaml;
use vitrine_core::layout::FontLibrary;
use vitrine_core::resolve::{FsAssets, ResolveOptions, resolve};
use vitrine_render::raster::save_png_rgb;
use vitrine_render::scene::{RenderContext, RenderOptions, Rendered};

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
    if let Ok(dir) = std::env::var("VITRINE_RENDER_DEBUG_DIR") {
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
format: vitrine/1
name: fx
languages: {default: en, available: [en]}
styles:
  shadows:
    drop: {layers: [{color: "#000000CC", blur: 8}]}
canvases:
- id: s
  page: {aspect: 1:1, count: 3}
  backgrounds: ["#FFFFFF"]
  elements:
  - {id: svg, kind: image, page: 0, at: {x: 500, y: 500}, width: 400, asset: assets/ring.svg, shadow: drop}
  - {id: png, kind: image, page: 1, at: {x: 500, y: 500}, width: 400, asset: assets/ring.png, shadow: drop, rotation: 30}
  - {id: sq, kind: image, page: 2, at: {x: 500, y: 500}, width: 400, asset: assets/square.png, shadow: drop}
targets:
- {id: t, canvas: s, size: [1000, 1000]}
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

const TEXTS: &str = r##"
format: vitrine/1
name: fx-text
languages: {default: en, available: [en]}
styles:
  colors:
    ring: "#FF0000"
  text:
    big: {weight: 900, size: 400, color: "#000000"}
    outlined: {weight: 900, size: 400, color: "#000000", stroke: {color: "@ring", width: 10}}
  spans:
    plain: {stroke: {color: "#000000", width: 0}}
canvases:
- id: s
  page: {aspect: 1:1, count: 4}
  backgrounds: ["#FFFFFF"]
  elements:
  - {id: a, kind: text, page: 0, at: {x: 500, y: 500}, width: 900, text: h, style: outlined, align: center, valign: center}
  - {id: b, kind: text, page: 1, at: {x: 500, y: 500}, width: 900, text: h, style: big, align: center, valign: center}
  - {id: c, kind: text, page: 2, at: {x: 500, y: 500}, width: 900, text: h, style: big, align: center, valign: center, glow: {color: "#0000FF", radius: 20}}
  - {id: d, kind: text, page: 3, at: {x: 500, y: 500}, width: 900, text: hs, style: outlined, align: center, valign: center, rotation: 20}
targets:
- {id: t, canvas: s, size: [1000, 1000]}
"##;

fn rgb(r: &Rendered, x: u32, y: u32) -> [u8; 3] {
    let c = r.pixmap.pixel(x, y).unwrap().demultiply();
    let a = f32::from(c.alpha()) / 255.0;
    let over = |v: u8| (f32::from(v) * a + 255.0 * (1.0 - a)) as u8;
    [over(c.red()), over(c.green()), over(c.blue())]
}

fn all_pixels(r: &Rendered) -> impl Iterator<Item = (u32, u32, [u8; 3])> + '_ {
    let (w, h) = (r.pixmap.width(), r.pixmap.height());
    (0..h).flat_map(move |y| (0..w).map(move |x| (x, y, rgb(r, x, y))))
}

#[test]
fn text_outline_sits_outside_the_fill_and_glow_fades() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = project_from_yaml(TEXTS).unwrap();
    let en = p
        .strings
        .entry(vitrine_core::model::Lang::new("en"))
        .or_default();
    en.insert(vitrine_core::model::StringKey::new("h"), "H".into());
    en.insert(
        vitrine_core::model::StringKey::new("hs"),
        "H[plain]H[/]".into(),
    );
    let s = resolve(
        &p,
        "t",
        "en",
        &ResolveOptions::new(&FsAssets::new(dir.path())),
    )
    .unwrap();
    let ctx = RenderContext::with_fonts(dir.path(), FontLibrary::bundled());
    let prepared = ctx.prepare(&s);
    let r: Vec<Rendered> = (0..4)
        .map(|i| ctx.rasterize_prepared(&prepared, i, &RenderOptions::export()))
        .collect();
    for (i, x) in r.iter().enumerate() {
        debug(x, &format!("text_fx_{i}.png"));
    }
    let red = |c: [u8; 3]| c[0] > 200 && c[1] < 60 && c[2] < 60;
    let black = |c: [u8; 3]| c[0] < 40 && c[1] < 40 && c[2] < 40;
    // Outline pixels exist and lie outside the glyph fill of the plain text.
    let outline: Vec<(u32, u32)> = all_pixels(&r[0])
        .filter(|(_, _, c)| red(*c))
        .map(|(x, y, _)| (x, y))
        .collect();
    assert!(outline.len() > 3000, "{}", outline.len());
    assert!(outline.iter().all(|&(x, y)| rgb(&r[1], x, y)[0] > 200));
    // The fill keeps its shape (outside outline drawn below it).
    let fill = |x: &Rendered| all_pixels(x).filter(|(_, _, c)| black(*c)).count();
    let (fa, fb) = (fill(&r[0]) as f64, fill(&r[1]) as f64);
    assert!((fa / fb - 1.0).abs() < 0.02, "{fa} vs {fb}");

    // Glow: blue around the glyph, fading with distance. Left stem of the
    // H: scan the middle row from the left to the first black pixel.
    let y = 500;
    let edge = (0..1000).find(|&x| black(rgb(&r[2], x, y))).unwrap();
    let blue_at = |d: u32| 255 - rgb(&r[2], edge - d, y)[0];
    let (b4, b20, b45, b90) = (blue_at(4), blue_at(20), blue_at(45), blue_at(90));
    assert!(b4 > b20 && b20 > b45 && b45 > b90, "{b4} {b20} {b45} {b90}");
    assert!(b4 > 60, "{b4}");
    assert!(b90 < 3, "{b90}");
    assert!(rgb(&r[2], edge - 10, y)[2] > 240, "it is blue");

    // A span switches the outline off: the second H has none.
    let reds = all_pixels(&r[3]).filter(|(_, _, c)| red(*c));
    let (left, right) = reds.fold(
        (0, 0),
        |(l, rr), (x, _, _)| {
            if x < 545 { (l + 1, rr) } else { (l, rr + 1) }
        },
    );
    assert!(left > 1500 && right == 0, "{left} {right}");
}
