//! `vitrine render`: one preview/debug PNG (an output image, the whole
//! canvas with overlay, or the store gallery preview).

use std::path::PathBuf;

use anyhow::{Context, bail};
use clap::Args;
use vitrine_core::model::TargetKind;
use vitrine_core::resolve::{Frame, FsAssets, Level, ResolvedScene, resolve};
use vitrine_render::export::gallery;
use vitrine_render::raster::save_png_rgb;
use vitrine_render::scene::{CanvasOptions, Quality, RenderIssue, RenderOptions, Scale};

use crate::{EXIT_OK, Global, project};

/// Default gallery preview height (px), as in the export.
const GALLERY_HEIGHT: u32 = 1200;
/// Default canvas scale (px per lu).
const CANVAS_SCALE: f64 = 0.25;

#[derive(Debug, Args)]
pub struct RenderArgs {
    /// Target id (default: the first target of the project).
    #[arg(long)]
    pub target: Option<String>,
    /// Language (default: the project's default language).
    #[arg(long)]
    pub lang: Option<String>,
    #[command(flatten)]
    pub what: What,
    #[command(flatten)]
    pub size: Size,
    /// Output PNG (default: `<project>/.cache/render/<target>_<lang>_<what>.png`).
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
}

/// What to render (default: `--image 1`).
#[derive(Debug, Args)]
#[group(multiple = false)]
pub struct What {
    /// The output image containing page N (1-based, as in export file names).
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub page: Option<u32>,
    /// Output image N (1-based: a page, a page group or the banner).
    #[arg(long, value_name = "N", value_parser = clap::value_parser!(u32).range(1..))]
    pub image: Option<u32>,
    /// The whole canvas with page boundaries and hatched gaps.
    #[arg(long)]
    pub canvas: bool,
    /// The store gallery preview (images side by side with the store gaps).
    #[arg(long)]
    pub gallery: bool,
}

/// Output size (default: target size for images, 0.25 px/lu for the canvas,
/// 1200 px high images for the gallery).
#[derive(Debug, Args)]
#[group(multiple = false)]
pub struct Size {
    /// Width in px (images, canvas).
    #[arg(long, value_name = "PX", value_parser = clap::value_parser!(u32).range(1..))]
    pub width: Option<u32>,
    /// Longer side in px (images, canvas); image height for the gallery.
    #[arg(long, value_name = "PX", value_parser = clap::value_parser!(u32).range(1..))]
    pub max_edge: Option<u32>,
    /// Factor of the target size (images, gallery) or px per lu (canvas).
    #[arg(long, value_name = "F")]
    pub scale: Option<f64>,
}

pub fn run(args: &RenderArgs, global: &Global) -> anyhow::Result<u8> {
    let root = &global.project;
    let project = project::load(root)?;
    let target = match &args.target {
        Some(t) => {
            project::check_targets(&project, std::slice::from_ref(t))?;
            t.clone()
        }
        None => project
            .targets
            .first()
            .map(|t| t.id.to_string())
            .context("the project has no targets")?,
    };
    let lang = args
        .lang
        .clone()
        .unwrap_or_else(|| project.languages.default.to_string());
    if let Some(s) = args.size.scale
        && !(s.is_finite() && s > 0.0)
    {
        bail!("--scale must be a positive number");
    }

    let (ctx, fonts) = crate::frames::render_context(root, &project, global);
    if !global.quiet {
        project::report_font_errors(&fonts.errors);
    }
    let fs = FsAssets::new(root);
    let scene = resolve(&project, &target, &lang, &ctx.resolve_options(&fs))?;
    let prepared = ctx.prepare(&scene);
    let frame = &scene.frame;

    let (pixmap, issues, label) = if args.what.canvas {
        let canvas = frame.canvas;
        let scale = match (args.size.width, args.size.max_edge, args.size.scale) {
            (Some(w), _, _) => f64::from(w) / canvas.w,
            (_, Some(m), _) => f64::from(m) / canvas.w.max(canvas.h),
            (_, _, Some(s)) => s,
            _ => CANVAS_SCALE,
        };
        let r = ctx.render_canvas(
            &prepared,
            &CanvasOptions {
                scale,
                quality: Quality::Preview,
                overlay: true,
            },
        );
        (r.pixmap, r.issues, "canvas".to_owned())
    } else if args.what.gallery {
        if frame.kind == TargetKind::Banner {
            bail!("target `{target}` has no store gallery (banner targets have none)");
        }
        if args.size.width.is_some() {
            bail!("--width is not supported with --gallery (use --max-edge or --scale)");
        }
        let height = match (args.size.max_edge, args.size.scale) {
            (Some(m), _) => m,
            (_, Some(s)) => (f64::from(frame.size.h) * s).round().max(1.0) as u32,
            _ => GALLERY_HEIGHT,
        };
        let (pixmap, issues) = gallery(&ctx, &prepared, height);
        (pixmap, issues, "gallery".to_owned())
    } else {
        let index = image_index(&args.what, frame, &target)?;
        let opts = match (args.size.width, args.size.max_edge, args.size.scale) {
            (Some(w), _, _) => RenderOptions::preview(w),
            (_, Some(m), _) => RenderOptions {
                scale: Scale::MaxEdge(m),
                quality: Quality::Preview,
            },
            (_, _, Some(s)) => RenderOptions {
                scale: Scale::Factor(s),
                quality: Quality::Preview,
            },
            _ => RenderOptions::export(),
        };
        let r = ctx.rasterize_prepared(&prepared, index, &opts);
        (r.pixmap, r.issues, format!("{:02}", index + 1))
    };

    if !global.quiet {
        print_issues(&scene, prepared.issues.iter().chain(&issues));
    }

    let out = args.output.clone().unwrap_or_else(|| {
        root.join(".cache/render")
            .join(format!("{target}_{lang}_{label}.png"))
    });
    if let Some(parent) = out.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot create `{}`", parent.display()))?;
    }
    save_png_rgb(&pixmap, &out, [255, 255, 255])
        .with_context(|| format!("cannot write `{}`", out.display()))?;
    if global.verbose {
        eprintln!("{}x{} px", pixmap.width(), pixmap.height());
    }
    println!("{}", out.display());
    Ok(EXIT_OK)
}

/// 0-based output image for `--page N` / `--image N` (default image 1).
fn image_index(what: &What, frame: &Frame, target: &str) -> anyhow::Result<usize> {
    if let Some(page) = what.page {
        let p = frame.pages.get(page as usize - 1).with_context(|| {
            format!(
                "page {page} is out of range (the canvas has {} pages)",
                frame.pages.len()
            )
        })?;
        let image = p.image.with_context(|| {
            format!("target `{target}` is a banner without page images; use --image 1")
        })?;
        return Ok(image as usize);
    }
    let image = what.image.unwrap_or(1) as usize;
    if image > frame.images.len() {
        bail!(
            "image {image} is out of range (target `{target}` has {} output images)",
            frame.images.len()
        );
    }
    Ok(image - 1)
}

/// Resolve diagnostics and render issues on stderr.
fn print_issues<'a>(scene: &ResolvedScene, issues: impl Iterator<Item = &'a RenderIssue>) {
    let style = crate::term::Style::stderr();
    for d in &scene.diagnostics {
        let level = match d.level {
            Level::Info => style.info("info:"),
            Level::Warn => style.warn("warning:"),
        };
        eprintln!("{level} {}", d.message);
    }
    for i in issues {
        eprintln!("{} {i}", style.warn("warning:"));
    }
}
