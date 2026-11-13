//! `clip_to` (ADR 0031): an image clipped to the outline of a shape
//! element, hidden or drawn, rotated, combined with the page clip.
//!
//! Set `VITRINE_RENDER_DEBUG_DIR` to also write the rendered images there.

#![allow(clippy::many_single_char_names)]

use std::path::Path;

use tiny_skia::{Color, Pixmap};
use vitrine_core::io::project_from_yaml;
use vitrine_core::layout::FontLibrary;
use vitrine_core::resolve::{FsAssets, ResolveOptions, resolve};
use vitrine_render::raster::save_png_rgb;
use vitrine_render::scene::{RenderContext, RenderOptions, Rendered};

const PROJECT: &str = r##"
format: vitrine/1
name: clip
languages: {default: en, available: [en]}
canvases:
- id: s
  page: {aspect: 1:1, count: 4}
  backgrounds: ["#FFFFFF"]
  elements:
  # 0: hidden ellipse mask
  - {id: oval, kind: shape, page: 0, at: {x: 500, y: 500}, width: 400, height: 400, shape: ellipse, visible: false}
  - {id: a, kind: image, page: 0, at: {x: 500, y: 500}, width: 600, height: 600, asset: assets/blue.png, fit: stretch, clip_to: oval}
  # 1: rotated rect mask (vertical bar after 90°)
  - {id: bar, kind: shape, page: 1, at: {x: 500, y: 500}, width: 400, height: 100, shape: rect, rotation: 90, visible: false}
  - {id: b, kind: image, page: 1, at: {x: 500, y: 500}, width: 600, height: 600, asset: assets/blue.png, fit: stretch, clip_to: bar}
  # 2: drawn rounded rect (red frame), image clipped to it on top
  - {id: card, kind: shape, page: 2, at: {x: 500, y: 500}, width: 500, height: 500, shape: rect, radius: 200, fill: "#FF0000"}
  - {id: c, kind: image, page: 2, at: {x: 500, y: 500}, width: 460, height: 460, asset: assets/blue.png, fit: stretch, clip_to: card}
  # 3: invalid reference: drawn unclipped
  - {id: d, kind: image, page: 3, at: {x: 500, y: 500}, width: 600, height: 600, asset: assets/blue.png, fit: stretch, clip_to: nope}
targets:
- {id: t, canvas: s, size: [1000, 1000]}
"##;

fn debug(r: &Rendered, name: &str) {
    if let Ok(dir) = std::env::var("VITRINE_RENDER_DEBUG_DIR") {
        save_png_rgb(&r.pixmap, &Path::new(&dir).join(name), [255, 255, 255]).unwrap();
    }
}

fn rgb(r: &Rendered, x: u32, y: u32) -> [u8; 3] {
    let c = r.pixmap.pixel(x, y).unwrap().demultiply();
    let a = f32::from(c.alpha()) / 255.0;
    let over = |v: u8| (f32::from(v) * a + 255.0 * (1.0 - a)) as u8;
    [over(c.red()), over(c.green()), over(c.blue())]
}

const BLUE: [u8; 3] = [0, 0, 255];
const WHITE: [u8; 3] = [255, 255, 255];
const RED: [u8; 3] = [255, 0, 0];

#[test]
fn images_are_clipped_to_their_shape() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("assets")).unwrap();
    let mut sq = Pixmap::new(60, 60).unwrap();
    sq.fill(Color::from_rgba8(0, 0, 255, 255));
    sq.save_png(dir.path().join("assets/blue.png")).unwrap();
    let p = project_from_yaml(PROJECT).unwrap();
    let s = resolve(
        &p,
        "t",
        "en",
        &ResolveOptions::new(&FsAssets::new(dir.path())),
    )
    .unwrap();
    let ctx = RenderContext::with_fonts(dir.path(), FontLibrary::bundled());
    let prepared = ctx.prepare(&s);
    let page = |i| {
        let r = ctx.rasterize_prepared(&prepared, i, &RenderOptions::export());
        debug(&r, &format!("clip_to_{i}.png"));
        assert!(r.issues.is_empty(), "{:?}", r.issues);
        r
    };

    // Ellipse of radius 200 around (500, 500); the hidden shape is not drawn.
    let r = page(0);
    assert_eq!(rgb(&r, 500, 500), BLUE);
    assert_eq!(rgb(&r, 500, 320), BLUE);
    assert_eq!(rgb(&r, 330, 330), WHITE, "image corner outside the oval");
    assert_eq!(rgb(&r, 250, 500), WHITE, "image edge outside the oval");

    // 400×100 bar rotated by 90°: x 450..550, y 300..700.
    let r = page(1);
    assert_eq!(rgb(&r, 500, 320), BLUE);
    assert_eq!(rgb(&r, 500, 680), BLUE);
    assert_eq!(rgb(&r, 600, 500), WHITE);
    assert_eq!(rgb(&r, 500, 750), WHITE);

    // Drawn card: its red rim shows around the clipped image, the
    // image's corners follow the card's rounded corners.
    let r = page(2);
    assert_eq!(rgb(&r, 500, 500), BLUE);
    assert_eq!(rgb(&r, 500, 260), RED, "rim of the card");
    assert_eq!(rgb(&r, 280, 280), WHITE, "rounded corner of the card");
    assert_eq!(rgb(&r, 500, 280), BLUE, "image edge inside the card");

    // Unknown shape: unclipped.
    let r = page(3);
    assert_eq!(rgb(&r, 210, 210), BLUE);
}
