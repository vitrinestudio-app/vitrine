//! Devices with imported vendor frames (ADR 0012), using a synthetic bezel
//! package (generated PNGs, never vendor artwork).

#![allow(clippy::many_single_char_names)]

use std::path::Path;
use std::sync::Arc;

use arrangia_core::frame_import::{FrameIndex, FrameStore, Package, Rules, import, synthetic};
use arrangia_core::io::project_from_yaml;
use arrangia_core::layout::FontLibrary;
use arrangia_core::model::FrameStyle;
use arrangia_core::resolve::{DiagnosticCode, FsAssets, ResolvedKind, ResolvedScene, resolve};
use arrangia_render::raster::save_png_rgb;
use arrangia_render::scene::{RenderContext, RenderOptions, Rendered};
use tiny_skia::Pixmap;

const PROJECT: &str = r##"
format: arrangia/1
name: imported
languages: {default: en, available: [en]}
strips:
- id: s
  page: {aspect: 1000:2000, count: 1}
  backgrounds: ["#FFFFFF"]
  elements:
  - id: phone
    kind: device
    page: 0
    at: {x: 500, y: 1000}
    width: 600
    model: iphone-island
    frame: imported:apple
    color: black-titanium
    screenshot: shot
    shadow: soft
targets:
- id: t
  strip: s
  size: [500, 1000]
"##;

const GREEN: [u8; 3] = [40, 200, 70];

fn setup() -> (tempfile::TempDir, Arc<FrameIndex>) {
    let dir = tempfile::tempdir().unwrap();
    let pkg_dir = dir.path().join("package");
    synthetic::write_folder(&pkg_dir, &synthetic::default_specs()).unwrap();
    let store = FrameStore::new(dir.path().join("frames"));
    let pkg = Package::open(&pkg_dir).unwrap();
    import(&store, &pkg, Rules::builtin("apple").unwrap(), true).unwrap();
    let mut shot = Pixmap::new(330, 717).unwrap();
    shot.fill(tiny_skia::Color::from_rgba8(
        GREEN[0], GREEN[1], GREEN[2], 255,
    ));
    let p = dir.path().join("project/screenshots/en/shot.png");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    shot.save_png(p).unwrap();
    (dir, Arc::new(store.index()))
}

fn render(root: &Path, frames: Option<Arc<FrameIndex>>, yaml: &str) -> (ResolvedScene, Rendered) {
    let project = project_from_yaml(yaml).unwrap();
    let ctx = RenderContext::with_fonts(root, FontLibrary::bundled()).with_frames(frames);
    let fs = FsAssets::new(root);
    let scene = resolve(&project, "t", "en", &ctx.resolve_options(&fs)).unwrap();
    let r = ctx.rasterize(&scene, 0, &RenderOptions::export());
    if let Ok(dir) = std::env::var("ARRANGIA_RENDER_DEBUG_DIR") {
        let name = if ctx.frames().is_some() {
            "imported.png"
        } else {
            "imported-fallback.png"
        };
        save_png_rgb(&r.pixmap, &Path::new(&dir).join(name), [255, 255, 255]).unwrap();
    }
    (scene, r)
}

fn rgb(pm: &Pixmap, x: f64, y: f64) -> [u8; 3] {
    let c = pm.pixel(x as u32, y as u32).unwrap().demultiply();
    [c.red(), c.green(), c.blue()]
}

fn is_green(c: [u8; 3]) -> bool {
    c.iter().zip(GREEN).all(|(a, b)| a.abs_diff(b) < 12)
}

#[test]
fn screenshot_is_inside_the_mask_and_under_the_frame() {
    let (dir, index) = setup();
    let root = dir.path().join("project");
    let (scene, r) = render(&root, Some(index.clone()), PROJECT);
    assert!(
        !scene
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::ImportedFrameMissing),
        "{:?}",
        scene.diagnostics
    );
    assert!(r.issues.is_empty(), "{:?}", r.issues);
    let e = &scene.elements[0];
    let ResolvedKind::Device(dev) = &e.props else {
        panic!()
    };
    let f = dev.imported.as_ref().expect("imported frame");
    assert_eq!(dev.frame, FrameStyle::Imported("apple".into()));
    assert_eq!(f.color, "black-titanium");
    // Element height follows the bezel's aspect.
    let aspect = f64::from(f.size.h) / f64::from(f.size.w);
    assert!((e.size.y / e.size.x - aspect).abs() < 1e-9);

    // Canvas lu → px (0.5) and bezel px → output px.
    let px_per_lu = 0.5;
    let k = e.size.x * px_per_lu / f64::from(f.size.w);
    let left = (e.center.x - e.size.x / 2.0) * px_per_lu;
    let top = (e.center.y - e.size.y / 2.0) * px_per_lu;
    let at = |bx: f64, by: f64| (left + bx * k, top + by * k);
    let s = f.screen;
    let pm = &r.pixmap;

    // Screen centre: the screenshot.
    let (cx, cy) = at(
        f64::from(s.x) + f64::from(s.w) / 2.0,
        f64::from(s.y) + f64::from(s.h) / 2.0,
    );
    assert!(is_green(rgb(pm, cx, cy)));
    // The island (opaque in the bezel) covers the screenshot.
    let island_y = f64::from(s.y) + 12.0 + f64::from(s.w) * 0.05;
    let (ix, iy) = at(f64::from(s.x) + f64::from(s.w) / 2.0, island_y);
    let c = rgb(pm, ix, iy);
    assert!(!is_green(c) && c[0] < 80, "island {c:?}");
    // Bezel border: frame colour.
    let (bx, by) = at(12.0, f64::from(f.size.h) / 2.0);
    assert!(rgb(pm, bx, by)[0] < 80);
    // Outside the device: background (shadow aside), never green.
    let (ox, oy) = at(-20.0, f64::from(f.size.h) / 2.0);
    assert!(rgb(pm, ox, oy).iter().all(|&v| v > 150));
    // No screenshot pixel outside the screen rect (+1 px).
    let (sx0, sy0) = at(f64::from(s.x) - 1.0, f64::from(s.y) - 1.0);
    let (sx1, sy1) = at(f64::from(s.x + s.w) + 1.0, f64::from(s.y + s.h) + 1.0);
    for y in 0..pm.height() {
        for x in 0..pm.width() {
            let (xf, yf) = (f64::from(x), f64::from(y));
            let outside = xf < sx0 || xf > sx1 || yf < sy0 || yf > sy1;
            assert!(
                !(outside && is_green(rgb(pm, xf, yf))),
                "screenshot pixel outside the screen at {x},{y}"
            );
        }
    }
    // Screen corners are rounded: the very corner of the rect is frame.
    let (kx, ky) = at(f64::from(s.x) + 1.0, f64::from(s.y) + 1.0);
    assert!(!is_green(rgb(pm, kx, ky)));
}

#[test]
fn missing_frames_fall_back_to_parametric_with_an_info() {
    let (dir, index) = setup();
    let root = dir.path().join("project");
    // No frames installed at all.
    let (scene, r) = render(&root, None, PROJECT);
    let ResolvedKind::Device(dev) = &scene.elements[0].props else {
        panic!()
    };
    assert!(dev.imported.is_none());
    assert_eq!(dev.frame, FrameStyle::Detailed);
    let d = scene
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::ImportedFrameMissing)
        .expect("info");
    assert_eq!(d.level, arrangia_core::resolve::Level::Info);
    assert_eq!(r.issues, []);
    // A colour that is not installed: fallback too; the vendor colour name
    // is only an info.
    let yaml = PROJECT.replace("color: black-titanium", "color: cosmic-orange");
    let (scene, _) = render(&root, Some(index.clone()), &yaml);
    assert!(
        scene
            .diagnostics
            .iter()
            .all(|d| d.level == arrangia_core::resolve::Level::Info)
    );
    assert!(
        scene
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::ImportedFrameMissing)
    );
    // No colour: the first installed one; landscape uses the landscape PNG.
    let yaml = PROJECT
        .replace("    color: black-titanium\n", "")
        .replace("width: 600", "width: 900\n    orientation: landscape");
    let (scene, _) = render(&root, Some(index), &yaml);
    let e = &scene.elements[0];
    let ResolvedKind::Device(dev) = &e.props else {
        panic!()
    };
    let f = dev.imported.as_ref().unwrap();
    assert!(f.file.ends_with("landscape.png"));
    assert!(e.size.x > e.size.y);
}
