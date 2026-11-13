//! End-to-end run of the render pipeline on the core sample project
//! (`crates/core/tests/data/sample`): previews, canvas views and a full
//! export, with timings.
//!
//! ```sh
//! cargo run -p vitrine-render --release --example sample -- <out-dir> [--runs N]
//! ```
//!
//! The sample project is copied to `<out-dir>/project` and given
//! screenshots (phone screens of the example project in
//! `tests/fixtures/example/screenshots/` plus generated tablet, wide "web"
//! and landscape screens), an SVG badge, a `web` target
//! and a `devices` canvas exercising more frames (landscape phone and tablet,
//! desktop with browser chrome, frameless screenshot, a missing screenshot
//! placeholder). Writes `<out-dir>/previews/*`, `<out-dir>/canvas/*` and
//! exports to `<out-dir>/project/export/`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use vitrine_core::Project;
use vitrine_core::resolve::{FsAssets, ResolveOptions, resolve};
use vitrine_render::export::{ExportOptions, export};
use vitrine_render::raster::{encode_png_rgb_with, save_png_rgb};
use vitrine_render::scene::{CanvasOptions, RenderContext, RenderOptions};

/// Extra canvas and targets appended to the copied sample project.
const DEVICES_CANVAS: &str = r##"- id: devices
  page: {aspect: 16:10, count: 3}
  backgrounds:
  - kind: linear-gradient
    angle: 180
    stops: [["#EEF1F5", 0], ["#D5DCE6", 1]]
  elements:
  - id: phone-landscape
    kind: device
    page: 0
    at: {x: 420, y: 330}
    width: 680
    rotation: -6
    model: iphone-island
    orientation: landscape
    color: natural-titanium
    screenshot: wide
    shadow: soft
  - id: phone-missing
    kind: device
    page: 0
    at: {x: 870, y: 360}
    width: 210
    model: android-punchhole
    frame: stylized
    color: porcelain
    screenshot: settings
    shadow: soft
  - id: imac-chrome
    kind: device
    page: 1
    at: {x: 500, y: 300}
    width: 600
    model: imac
    color: pastel-blue
    chrome: browser
    url: https://grove.eco/harvest
    screenshot: site
    shadow: soft
  - id: frameless
    kind: device
    page: 2
    at: {x: 190, y: 330}
    width: 230
    model: none
    screenshot: home
    shadow: soft
  - id: tablet-landscape
    kind: device
    page: 2
    at: {x: 640, y: 330}
    width: 600
    model: ipad-modern
    orientation: landscape
    frame: stylized
    color: midnight
    screenshot: wide
    shadow: soft
"##;

const EXTRA_TARGETS: &str = r"- id: web
  kind: banner
  canvas: web
  size: [2880, 900]
  region: {pages: [0, 1]}
  fit: cover
  gaps: none
- id: devices
  kind: banner
  canvas: devices
  size: [4320, 900]
  region: {pages: [0, 2]}
  fit: cover
  gaps: none
";

const BADGE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="220" height="220" viewBox="0 0 220 220">
  <circle cx="110" cy="110" r="104" fill="#2F6B3A"/>
  <path d="M110 40 L128 92 L182 92 L138 124 L154 176 L110 144 L66 176 L82 124 L38 92 L92 92 Z" fill="#E8A33D"/>
</svg>"##;

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let target = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &target)?;
        } else {
            std::fs::copy(e.path(), target)?;
        }
    }
    Ok(())
}

fn setup(out: &Path) -> anyhow::Result<PathBuf> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let project = out.join("project");
    if project.exists() {
        std::fs::remove_dir_all(&project)?;
    }
    copy_dir(&repo.join("crates/core/tests/data/sample"), &project)?;
    let screens = repo.join("tests/fixtures/example/screenshots");
    let shots = [
        ("en/ios-phone/home.png", "en/ios-phone/home.png"),
        ("en/home.png", "en/ios-phone/home.png"),
        ("de/ios-phone/home.png", "de/ios-phone/harvest.png"),
    ];
    for (to, from) in shots {
        let to = project.join("screenshots").join(to);
        std::fs::create_dir_all(to.parent().expect("parent"))?;
        std::fs::copy(screens.join(from), to)?;
    }
    for (name, w, h, accent) in [
        ("site", 1440, 900, [0x2F, 0x6B, 0x3A]),
        ("dashboard", 1440, 900, [0x3D, 0x5A, 0xA8]),
        ("wide", 2556, 1179, [0xE8, 0xA3, 0x3D]),
        ("planner", 2752, 2064, [0x5B, 0x8C, 0x5A]),
    ] {
        let to = project.join(format!("screenshots/en/{name}.png"));
        save_png_rgb(&fake_screen(w, h, accent), &to, [255, 255, 255])?;
    }
    let yaml = project.join("project.yaml");
    let text = std::fs::read_to_string(&yaml)?
        .replace("\ntargets:\n", &format!("\n{DEVICES_CANVAS}targets:\n"))
        + EXTRA_TARGETS;
    std::fs::write(&yaml, text)?;
    std::fs::create_dir_all(project.join("assets"))?;
    std::fs::write(project.join("assets/badge-new.svg"), BADGE)?;
    Ok(project)
}

/// A plain generated "app/web page" screen: header bar, hero block, cards.
fn fake_screen(w: u32, h: u32, accent: [u8; 3]) -> tiny_skia::Pixmap {
    use tiny_skia::{Color, Paint, Rect, Transform};
    let mut pm = tiny_skia::Pixmap::new(w, h).expect("screen size");
    pm.fill(Color::from_rgba8(0xF7, 0xF8, 0xF6, 255));
    let (wf, hf) = (w as f32, h as f32);
    let mut rect = |x: f32, y: f32, rw: f32, rh: f32, c: [u8; 3]| {
        let mut paint = Paint::default();
        paint.set_color_rgba8(c[0], c[1], c[2], 255);
        if let Some(r) = Rect::from_xywh(x * wf, y * hf, rw * wf, rh * hf) {
            pm.fill_rect(r, &paint, Transform::identity(), None);
        }
    };
    rect(0.0, 0.0, 1.0, 0.09, [0xFF, 0xFF, 0xFF]);
    rect(0.04, 0.03, 0.12, 0.03, accent);
    rect(0.05, 0.15, 0.9, 0.36, accent);
    rect(0.09, 0.24, 0.4, 0.05, [0xFF, 0xFF, 0xFF]);
    rect(0.09, 0.32, 0.28, 0.03, [0xEE, 0xF2, 0xEC]);
    for i in 0..3 {
        let x = 0.05 + i as f32 * 0.31;
        rect(x, 0.57, 0.28, 0.36, [0xFF, 0xFF, 0xFF]);
        rect(x + 0.02, 0.6, 0.24, 0.16, [0xDD, 0xE8, 0xD6]);
        rect(x + 0.02, 0.8, 0.16, 0.025, [0x9A, 0xA2, 0x9A]);
    }
    pm
}

fn ms(d: Duration) -> f64 {
    d.as_secs_f64() * 1000.0
}

fn median(mut v: Vec<Duration>) -> f64 {
    v.sort();
    ms(v[v.len() / 2])
}

#[allow(clippy::too_many_lines, reason = "linear demo driver")]
fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(
        args.get(1)
            .cloned()
            .unwrap_or_else(|| "target/sample".into()),
    );
    let runs: usize = args
        .iter()
        .position(|a| a == "--runs")
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(9);
    let root = setup(&out)?;
    let project = Project::load(&root)?;
    let (ctx, _) = RenderContext::new(&root);
    let fs = FsAssets::new(&root);

    // Previews (400 px wide) and canvas views, per target, `en`.
    for target in &project.targets {
        let id = target.id.as_str();
        let scene = resolve(&project, id, "en", &ResolveOptions::new(&fs))?;
        let t = Instant::now();
        let prepared = ctx.prepare(&scene);
        let t_prepare = t.elapsed();
        let mut cold = Vec::new();
        let mut warm = Vec::new();
        for i in 0..scene.frame.images.len() {
            let t = Instant::now();
            let r = ctx.rasterize_prepared(&prepared, i, &RenderOptions::preview(400));
            cold.push(t.elapsed());
            for _ in 0..runs {
                let t = Instant::now();
                let _ = ctx.rasterize_prepared(&prepared, i, &RenderOptions::preview(400));
                warm.push(t.elapsed());
            }
            save_png_rgb(
                &r.pixmap,
                &out.join(format!("previews/{id}_en_{:02}.png", i + 1)),
                [255, 255, 255],
            )?;
        }
        let canvas = ctx.render_canvas(&prepared, &CanvasOptions::default());
        save_png_rgb(
            &canvas.pixmap,
            &out.join(format!("canvas/{id}_en.png")),
            [255, 255, 255],
        )?;
        let t = Instant::now();
        let full = ctx.rasterize_prepared(&prepared, 0, &RenderOptions::export());
        let t_full = t.elapsed();
        let mut enc = String::new();
        for (name, c) in [
            ("fast", png::Compression::Fast),
            ("balanced", png::Compression::Balanced),
            ("high", png::Compression::High),
        ] {
            let t = Instant::now();
            let bytes = encode_png_rgb_with(&full.pixmap, [255, 255, 255], c)?;
            let _ = write!(
                enc,
                " {name} {:.0} ms/{} kB",
                ms(t.elapsed()),
                bytes.len() / 1024
            );
        }
        println!(
            "{id:24} layout {:6.2} ms | preview 400 px: first {:6.1} ms, warm median {:5.1} ms | full {}x{}: {:6.1} ms",
            ms(t_prepare),
            ms(cold[0]),
            median(warm),
            full.pixmap.width(),
            full.pixmap.height(),
            ms(t_full),
        );
        println!("{:24} png:{enc}", "");
    }

    // Full export, then an incremental run.
    for (label, incremental) in [("export (clean)", false), ("export (incremental)", true)] {
        let report = export(
            &project,
            &ctx,
            &ExportOptions {
                incremental,
                ..ExportOptions::default()
            },
        );
        let written = report.written().count();
        let render: f64 = report.files.iter().map(|f| f.render_ms).sum();
        let encode: f64 = report.files.iter().map(|f| f.encode_ms).sum();
        println!(
            "{label}: {} files ({written} written) in {:.0} ms wall (resolve+layout {:.0} ms; CPU sum render {:.0} ms, encode {:.0} ms); {} issues, {} errors",
            report.files.len(),
            report.total_ms,
            report.resolve_ms,
            render,
            encode,
            report.issues.len(),
            report.errors.len(),
        );
        for e in &report.errors {
            println!("  error: {e}");
        }
    }
    let report = export(&project, &ctx, &ExportOptions::default());
    for f in &report.files {
        println!(
            "  {:48} {:5}x{:<5} {:8} B",
            f.path, f.width, f.height, f.bytes
        );
    }
    for i in &report.issues {
        println!("  {} {}: {}", i.target, i.lang, serde_issue(&i.source));
    }
    Ok(())
}

fn serde_issue(s: &vitrine_render::export::IssueSource) -> String {
    match s {
        vitrine_render::export::IssueSource::Resolve(d) => format!("[resolve] {}", d.message),
        vitrine_render::export::IssueSource::Render(r) => format!("[render] {r}"),
    }
}
