//! Group `scale` (ADR 0030) renders like the same elements scaled by hand:
//! shape size, corner radius, stroke and shadow, text size and span
//! outline. Set `VITRINE_RENDER_DEBUG_DIR` to also write the images there.

#![allow(clippy::many_single_char_names)]

use std::path::Path;

use vitrine_core::io::project_from_yaml;
use vitrine_core::layout::FontLibrary;
use vitrine_core::model::{Lang, StringKey};
use vitrine_core::ops::Op;
use vitrine_core::resolve::{FsAssets, ResolveOptions, resolve};
use vitrine_render::raster::save_png_rgb;
use vitrine_render::scene::{RenderContext, RenderOptions, Rendered};

/// Page 0: a group at scale 0.5; page 1: the same content written at half
/// size. Both groups are centred on (500, 500) of their page.
const PROJECT: &str = r##"
format: vitrine/1
name: group-scale
languages: {default: en, available: [en]}
styles:
  shadows:
    drop: {layers: [{color: "#00000099", blur: 12, y: 16}]}
    drop-half: {layers: [{color: "#00000099", blur: 6, y: 8}]}
  spans:
    o: {stroke: {color: "#00AA00", width: 12}}
    o-half: {stroke: {color: "#00AA00", width: 6}}
canvases:
- id: s
  page: {aspect: 1:1, count: 2}
  backgrounds: ["#FFFFFF"]
  elements:
  - id: g
    kind: group
    page: 0
    scale: 0.5
    children:
    - {id: card, kind: shape, page: 0, at: {x: 500, y: 400}, width: 800, height: 600, shape: rect, radius: 80, fill: "#FFCC00", stroke: {color: "#0000FF", width: 24}, shadow: drop}
    - {id: label, kind: text, page: 0, at: {x: 500, y: 600}, width: 760, font_size: 120, text: k, align: center, color: "#000000"}
  - {id: card2, kind: shape, page: 1, at: {x: 500, y: 450}, width: 400, height: 300, shape: rect, radius: 40, fill: "#FFCC00", stroke: {color: "#0000FF", width: 12}, shadow: drop-half}
  - {id: label2, kind: text, page: 1, at: {x: 500, y: 550}, width: 380, font_size: 60, text: k2, align: center, color: "#000000"}
targets:
- {id: t, canvas: s, size: [1000, 1000]}
"##;

fn debug(r: &Rendered, name: &str) {
    if let Ok(dir) = std::env::var("VITRINE_RENDER_DEBUG_DIR") {
        save_png_rgb(&r.pixmap, &Path::new(&dir).join(name), [255, 255, 255]).unwrap();
    }
}

#[test]
fn scaled_group_renders_like_hand_scaled_elements() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = project_from_yaml(PROJECT).unwrap();
    // The hand-scaled copy uses the half-size span.
    let set = |key: &str, value: &str| Op::SetString {
        lang: Lang::new("en"),
        key: StringKey::new(key),
        value: Some(value.to_owned()),
    };
    p.apply_ops(vec![
        set("k", "[o]Hi[/] there"),
        set("k2", "[o-half]Hi[/] there"),
    ])
    .unwrap();
    let s = resolve(
        &p,
        "t",
        "en",
        &ResolveOptions::new(&FsAssets::new(dir.path())),
    )
    .unwrap();
    let ctx = RenderContext::with_fonts(dir.path(), FontLibrary::bundled());
    let prepared = ctx.prepare(&s);
    let a = ctx.rasterize_prepared(&prepared, 0, &RenderOptions::export());
    let b = ctx.rasterize_prepared(&prepared, 1, &RenderOptions::export());
    debug(&a, "group_scale_group.png");
    debug(&b, "group_scale_hand.png");
    assert!(a.issues.is_empty() && b.issues.is_empty());
    assert_eq!(a.pixmap.width(), b.pixmap.width());
    let mut differing = 0usize;
    let mut ink = 0usize;
    for (pa, pb) in a.pixmap.pixels().iter().zip(b.pixmap.pixels()) {
        let (pa, pb) = (pa.demultiply(), pb.demultiply());
        let d = [
            pa.red().abs_diff(pb.red()),
            pa.green().abs_diff(pb.green()),
            pa.blue().abs_diff(pb.blue()),
            pa.alpha().abs_diff(pb.alpha()),
        ];
        if d.iter().any(|&v| v > 8) {
            differing += 1;
        }
        if pa.green() > 150 && pa.red() < 100 && pa.blue() < 100 {
            ink += 1; // the green span outline is drawn
        }
    }
    let total = a.pixmap.pixels().len();
    assert!(ink > 100, "span outline missing ({ink} px)");
    assert!(differing < 50, "{differing} of {total} pixels differ");
}
