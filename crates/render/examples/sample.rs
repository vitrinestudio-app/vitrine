//! End-to-end run of the render pipeline on the core sample project
//! (`crates/core/tests/data/sample`): previews, canvas views and a full
//! export, with timings.
//!
//! ```sh
//! cargo run -p arrangia-render --release --example sample -- <out-dir> [--runs N]
//! ```
//!
//! The sample project is copied to `<out-dir>/project` and given
//! screenshots (the Phase 0 spike test screens in `assets/spike/`) and an
//! SVG badge. Writes `<out-dir>/previews/*`, `<out-dir>/canvas/*` and
//! exports to `<out-dir>/project/export/`.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use arrangia_core::Project;
use arrangia_core::resolve::{FsAssets, ResolveOptions, resolve};
use arrangia_render::export::{ExportOptions, export};
use arrangia_render::raster::{encode_png_rgb_with, save_png_rgb};
use arrangia_render::scene::{CanvasOptions, RenderContext, RenderOptions};

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
    let spike = repo.join("assets/spike");
    let shots = [
        ("en/ios-phone/home.png", "phone-home.png"),
        ("en/home.png", "phone-home.png"),
        ("de/ios-phone/home.png", "phone-harvest.png"),
        ("en/dashboard.png", "tablet-planner.png"),
        ("en/planner.png", "tablet-planner.png"),
    ];
    for (to, from) in shots {
        let to = project.join("screenshots").join(to);
        std::fs::create_dir_all(to.parent().expect("parent"))?;
        std::fs::copy(spike.join(from), to)?;
    }
    std::fs::create_dir_all(project.join("assets"))?;
    std::fs::write(project.join("assets/badge-new.svg"), BADGE)?;
    Ok(project)
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

fn serde_issue(s: &arrangia_render::export::IssueSource) -> String {
    match s {
        arrangia_render::export::IssueSource::Resolve(d) => format!("[resolve] {}", d.message),
        arrangia_render::export::IssueSource::Render(r) => format!("[render] {r}"),
    }
}
