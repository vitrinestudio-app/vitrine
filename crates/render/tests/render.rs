//! Render and export tests on a small inline project.
//!
//! Set `VITRINE_RENDER_DEBUG_DIR` to also write the rendered images there
//! for visual inspection.

#![allow(clippy::many_single_char_names)]

use std::path::Path;

use tiny_skia::Pixmap;
use vitrine_core::Project;
use vitrine_core::io::project_from_yaml;
use vitrine_core::layout::FontLibrary;
use vitrine_core::model::{Lang, StringKey};
use vitrine_core::resolve::{FsAssets, ResolveOptions, ResolvedScene, resolve};
use vitrine_render::export::{ExportOptions, FileKind, IssueSource, export};
use vitrine_render::raster::save_png_rgb;
use vitrine_render::scene::{CanvasOptions, RenderContext, RenderOptions, Rendered};

const PROJECT: &str = r##"
format: vitrine/1
name: render-test
languages: {default: en, available: [en, de]}
canvases:
- id: s
  page: {aspect: 1000:2000, count: 4, ids: [hero, two]}
  backgrounds: ["#FFFFFF"]
  elements:
  - id: gapbar
    kind: shape
    page: 0
    at: {x: 1050, y: 1000}
    width: 60
    height: 1800
    shape: rect
    fill: "#FF0000"
  - id: wide
    kind: text
    page: 2
    at: {x: 500, y: 600}
    width: 3000
    text: wide
    font_size: 160
    color: "#0000FF"
    align: center
  - id: tight
    kind: text
    page: 3
    text: tight
    box: {top: 100, left: 100, right: 100, bottom: 1700}
    font_size: 120
    fit: {max_lines: 1}
  - id: phone
    kind: device
    page: 3
    at: {x: 500, y: 1200}
    width: 700
    model: iphone-island
    orientation: landscape
    screenshot: missing
    shadow: soft
- id: web
  page: {aspect: 16:10, count: 3}
  backgrounds:
  - {kind: linear-gradient, angle: 90, stops: [["#DDE6F0", 0], ["#F4F4F8", 1]]}
  elements:
  - id: browser
    kind: device
    page: 0
    at: {x: 500, y: 320}
    width: 860
    model: browser
    appearance: dark
    screenshot: shot
  - id: laptop
    kind: device
    page: 1
    at: {x: 500, y: 330}
    width: 900
    model: macbook
    screenshot: shot
    fit: contain
    rotation: 6
    opacity: 0.8
    shadow: soft
  - id: tablet
    kind: device
    page: 2
    at: {x: 500, y: 320}
    width: 520
    model: ipad-modern
    frame: stylized
    screenshot: shot
targets:
- id: pages
  canvas: s
  size: [500, 1000]
  gallery: {gap: 100}
- id: grouped
  canvas: s
  size: [1000, 1000]
  group: 2
  gallery: {gap: 100}
  langs: [en]
- id: banner
  kind: banner
  canvas: s
  size: [800, 400]
  format: jpeg
  quality: 80
  langs: [en]
- id: web
  canvas: web
  size: [1600, 1000]
  langs: [en]
"##;

fn project() -> Project {
    let mut p = project_from_yaml(PROJECT).unwrap();
    for (lang, text) in [("en", "Clipped"), ("de", "Abgeschnitten")] {
        let s = p.strings.entry(Lang::new(lang)).or_default();
        s.insert(StringKey::new("wide"), format!("{text} {text} {text}"));
        s.insert(
            StringKey::new("tight"),
            "Far too much text for one single line".into(),
        );
    }
    p
}

/// Project folder with one screenshot.
fn folder() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let mut shot = Pixmap::new(320, 200).unwrap();
    shot.fill(tiny_skia::Color::from_rgba8(40, 120, 70, 255));
    let p = dir.path().join("screenshots/en/shot.png");
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    shot.save_png(p).unwrap();
    dir
}

fn ctx(root: &Path) -> RenderContext {
    RenderContext::with_fonts(root, FontLibrary::bundled())
}

fn scene(p: &Project, root: &Path, target: &str) -> ResolvedScene {
    resolve(p, target, "en", &ResolveOptions::new(&FsAssets::new(root))).unwrap()
}

fn debug(r: &Rendered, name: &str) {
    if let Ok(dir) = std::env::var("VITRINE_RENDER_DEBUG_DIR") {
        save_png_rgb(&r.pixmap, &Path::new(&dir).join(name), [255, 255, 255]).unwrap();
    }
}

fn count(pm: &Pixmap, x0: u32, x1: u32, pred: impl Fn([u8; 4]) -> bool) -> usize {
    let mut n = 0;
    for y in 0..pm.height() {
        for x in x0..x1.min(pm.width()) {
            let c = pm.pixel(x, y).unwrap().demultiply();
            if pred([c.red(), c.green(), c.blue(), c.alpha()]) {
                n += 1;
            }
        }
    }
    n
}

fn red(c: [u8; 4]) -> bool {
    c[0] > 200 && c[1] < 60 && c[2] < 60
}

fn blue(c: [u8; 4]) -> bool {
    c[2] > 150 && c[0] < 100 && c[1] < 100
}

#[test]
fn gap_content_is_omitted_from_page_exports() {
    let dir = folder();
    let p = project();
    let s = scene(&p, dir.path(), "pages");
    assert_eq!(s.frame.gaps.len(), 3);
    let ctx = ctx(dir.path());
    let prepared = ctx.prepare(&s);
    for i in 0..s.frame.images.len() {
        let r = ctx.rasterize_prepared(&prepared, i, &RenderOptions::export());
        assert_eq!((r.pixmap.width(), r.pixmap.height()), (500, 1000));
        assert_eq!(
            count(&r.pixmap, 0, 500, red),
            0,
            "image {i} shows gap content"
        );
        debug(&r, &format!("pages_{i}.png"));
    }
    // The canvas view shows it.
    let c = ctx.render_canvas(
        &prepared,
        &CanvasOptions {
            scale: 0.5,
            overlay: false,
            ..CanvasOptions::default()
        },
    );
    debug(&c, "pages_canvas.png");
    assert!(count(&c.pixmap, 500, 550, red) > 1000);
}

#[test]
fn text_is_clipped_to_its_page() {
    let dir = folder();
    let p = project();
    let s = scene(&p, dir.path(), "pages");
    let ctx = ctx(dir.path());
    let c = ctx.render_canvas(
        &ctx.prepare(&s),
        &CanvasOptions {
            scale: 0.5,
            overlay: false,
            ..CanvasOptions::default()
        },
    );
    // Page 2 (home page of `wide`) spans canvas x 2200..3200 lu → px 1100..1600.
    let page = s.frame.pages[2].rect;
    let (x0, x1) = ((page.x * 0.5) as u32, (page.right() * 0.5) as u32);
    assert!(count(&c.pixmap, x0, x1, blue) > 500);
    assert_eq!(count(&c.pixmap, 0, x0, blue), 0);
    assert_eq!(count(&c.pixmap, x1, c.pixmap.width(), blue), 0);
}

#[test]
fn grouped_and_banner_sizes_and_names() {
    let dir = folder();
    let p = project();
    let out = dir.path().join("out");
    let report = export(
        &p,
        &ctx(dir.path()),
        &ExportOptions {
            out_dir: Some(out.clone()),
            targets: vec!["grouped".into(), "banner".into()],
            ..ExportOptions::default()
        },
    );
    assert!(report.errors.is_empty(), "{:?}", report.errors);
    let names: Vec<(&str, u32, u32)> = report
        .files
        .iter()
        .map(|f| (f.path.as_str(), f.width, f.height))
        .collect();
    assert_eq!(
        names,
        [
            ("banner/en/banner.jpg", 800, 400),
            ("grouped/en/01_hero+two.png", 1000, 1000),
            ("grouped/en/02_page-03+page-04.png", 1000, 1000),
            ("grouped/en/_gallery.png", 2300, 1200),
        ]
    );
    let jpg = image::open(out.join("banner/en/banner.jpg")).unwrap();
    assert_eq!(jpg.color(), image::ColorType::Rgb8);
    assert!(report.files.iter().any(|f| f.kind == FileKind::Gallery));
}

#[test]
fn export_writes_opaque_png_and_is_incremental() {
    let dir = folder();
    let p = project();
    let out = dir.path().join("out");
    let opts = ExportOptions {
        out_dir: Some(out.clone()),
        targets: vec!["pages".into()],
        langs: vec!["de".into()],
        gallery: false,
        ..ExportOptions::default()
    };
    let ctx = ctx(dir.path());
    let first = export(&p, &ctx, &opts);
    assert!(first.errors.is_empty(), "{:?}", first.errors);
    assert_eq!(first.written().count(), 4);
    let file = out.join("pages/de/01_hero.png");
    let reader = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&file).unwrap()))
        .read_info()
        .unwrap();
    assert_eq!(reader.info().color_type, png::ColorType::Rgb);
    assert!(reader.info().srgb.is_some());
    // Nothing changed: nothing is rendered again.
    let second = export(&p, &ctx, &opts);
    assert_eq!(second.files.len(), 4);
    assert_eq!(second.written().count(), 0);
    // A changed text re-renders.
    let mut p2 = p.clone();
    p2.strings
        .get_mut(&Lang::new("de"))
        .unwrap()
        .insert(StringKey::new("wide"), "Neu".into());
    assert_eq!(export(&p2, &ctx, &opts).written().count(), 4);
    // Overflow of `tight` is reported (max_lines: 1).
    assert!(first.issues.iter().any(|i| matches!(
        &i.source,
        IssueSource::Render(r) if r.is_overflow() && r.element.as_ref().is_some_and(|e| e.as_str() == "tight")
    )));
    // Missing screenshot: placeholder, resolve warning.
    assert!(first.issues.iter().any(|i| matches!(
        &i.source,
        IssueSource::Resolve(d) if d.message.contains("missing")
    )));
}

#[test]
fn export_reports_progress_and_can_be_cancelled() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    let dir = folder();
    let p = project();
    let ctx = ctx(dir.path());
    let calls = Arc::new(AtomicUsize::new(0));
    let seen = calls.clone();
    let opts = ExportOptions {
        out_dir: Some(dir.path().join("out")),
        targets: vec!["pages".into()],
        langs: vec!["de".into()],
        gallery: false,
        hooks: vitrine_render::export::ExportHooks {
            progress: Some(Arc::new(move |_, done, total| {
                assert!(done <= total);
                seen.fetch_add(1, Ordering::Relaxed);
            })),
            cancel: None,
        },
        ..ExportOptions::default()
    };
    let report = export(&p, &ctx, &opts);
    assert!(!report.cancelled);
    assert_eq!(calls.load(Ordering::Relaxed), report.files.len());

    let cancel = Arc::new(AtomicBool::new(true));
    let cancelled = export(
        &p,
        &ctx,
        &ExportOptions {
            out_dir: Some(dir.path().join("out2")),
            hooks: vitrine_render::export::ExportHooks {
                progress: None,
                cancel: Some(cancel),
            },
            ..opts
        },
    );
    assert!(cancelled.cancelled);
    assert!(cancelled.files.is_empty());
}

#[test]
fn rendering_is_deterministic_and_previews_scale() {
    let dir = folder();
    let p = project();
    let ctx = ctx(dir.path());
    for target in ["pages", "web"] {
        let s = scene(&p, dir.path(), target);
        let prepared = ctx.prepare(&s);
        for i in 0..s.frame.images.len() {
            let a = ctx.rasterize_prepared(&prepared, i, &RenderOptions::export());
            let b = ctx.rasterize_prepared(&prepared, i, &RenderOptions::export());
            assert!(a.pixmap.data() == b.pixmap.data(), "{target} {i}");
            assert!(a.issues.is_empty(), "{:?}", a.issues);
            debug(&a, &format!("{target}_{i}.png"));
            let small = ctx.rasterize_prepared(&prepared, i, &RenderOptions::preview(200));
            assert_eq!(small.pixmap.width(), 200);
        }
    }
}

#[test]
fn devices_draw_frames_and_screens() {
    let dir = folder();
    let p = project();
    let s = scene(&p, dir.path(), "web");
    let ctx = ctx(dir.path());
    let r = ctx.rasterize(&s, 0, &RenderOptions::export());
    // The browser window shows the green screenshot in its content area.
    let green = |c: [u8; 4]| c[1] > 100 && c[0] < 60 && c[2] < 90;
    assert!(count(&r.pixmap, 0, 1600, green) > 300_000);
    // Dark chrome bar above it.
    let c = r.pixmap.pixel(800, 120).unwrap();
    assert!(c.red() < 80, "{c:?}");
}
