//! Preview images of templates (ADR 0023): one PNG per canvas, rendered from
//! the package itself (its placeholders or sample screenshots, assets and
//! fonts; parametric device frames only, never imported vendor bezels). The
//! app renders them when saving a template; the gallery server renders them
//! again from the uploaded package (trusted previews), plus one sprite per
//! canvas for the website gallery ([`sprites`]: all pages side by side
//! without gaps, so CSS can show any page with `background-position`).

use std::path::{Path, PathBuf};

use anyhow::Context;
use tiny_skia::{Pixmap, PixmapPaint, Transform};
use vitrine_core::geometry::Viewport;
use vitrine_core::model::{Project, Target, TargetKind};
use vitrine_core::resolve::{FsAssets, resolve};
use vitrine_core::template::Package;

use crate::export::gallery;
use crate::raster::encode_png_rgb;
use crate::scene::{Quality, RenderContext, RenderOptions, Scale};

/// Default height of a gallery preview (px).
pub const PREVIEW_HEIGHT: u32 = 480;

/// A temporary folder removed on drop.
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let dir =
            std::env::temp_dir().join(format!("vitrine-template-{}-{nanos}", std::process::id()));
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The target that represents a canvas in previews: the first page target
/// showing it, else its first banner.
fn preview_target<'p>(project: &'p Project, canvas: &str) -> Option<&'p Target> {
    let targets: Vec<&Target> = project
        .targets
        .iter()
        .filter(|t| t.canvas.as_str() == canvas)
        .collect();
    targets
        .iter()
        .find(|t| t.kind != TargetKind::Banner)
        .or_else(|| targets.first())
        .copied()
}

/// Preview of one canvas of the project in `root`: the store gallery of the
/// first page target showing it, else its first banner; `None` when no
/// target shows the canvas.
pub fn render_canvas_preview(
    ctx: &RenderContext,
    root: &Path,
    project: &Project,
    canvas: &str,
    height: u32,
) -> anyhow::Result<Option<Vec<u8>>> {
    let Some(target) = preview_target(project, canvas) else {
        return Ok(None);
    };
    let lang = project.languages.default.to_string();
    let fs = FsAssets::new(root);
    let scene = resolve(
        project,
        target.id.as_str(),
        &lang,
        &ctx.resolve_options(&fs),
    )
    .with_context(|| format!("canvas `{canvas}`"))?;
    let prepared = ctx.prepare(&scene);
    let pixmap = if scene.frame.kind == TargetKind::Banner {
        ctx.rasterize_prepared(
            &prepared,
            0,
            &RenderOptions {
                scale: Scale::MaxEdge(height * 2),
                quality: Quality::Preview,
            },
        )
        .pixmap
    } else {
        gallery(ctx, &prepared, height).0
    };
    Ok(Some(encode_png_rgb(&pixmap, [255, 255, 255])?))
}

/// Renders a preview per canvas of `pkg` and stores them as
/// `previews/<canvas>.png` (manifest entries follow). Existing previews
/// are replaced.
pub fn add_previews(pkg: &mut Package, height: u32) -> anyhow::Result<()> {
    let tmp = TempDir::new();
    pkg.write_dir(&tmp.0)?;
    let project = Project::load(&tmp.0)?;
    let (ctx, _) = RenderContext::new(&tmp.0);
    let mut previews = Vec::new();
    for c in &project.canvases {
        if let Some(png) = render_canvas_preview(&ctx, &tmp.0, &project, c.id.as_str(), height)? {
            previews.push((c.id.to_string(), png));
        }
    }
    drop(tmp);
    let old: Vec<String> = pkg
        .manifest
        .canvases
        .iter_mut()
        .filter_map(|c| c.preview.take())
        .collect();
    for p in old {
        pkg.remove(&p);
    }
    for (canvas, png) in previews {
        let path = format!("previews/{canvas}.png");
        pkg.put(&path, png)?;
        if let Some(c) = pkg.manifest.canvases.iter_mut().find(|c| c.id == canvas) {
            c.preview = Some(path);
        }
    }
    pkg.check()?;
    Ok(())
}

/// Default page height of a gallery sprite (px); the 2× variant doubles it.
pub const SPRITE_PAGE_HEIGHT: u32 = 600;
/// Widest 1× sprite (px): the 2× variant stays within WebP's 16383 px;
/// pages shrink to fit.
pub const SPRITE_MAX_WIDTH: u32 = 8000;

/// All pages of a canvas side by side, without gaps (gallery sprite).
#[derive(Debug, Clone)]
pub struct Sprite {
    /// Opaque RGBA (transparent areas on white).
    pub pixmap: Pixmap,
    pub pages: u32,
    pub page_width: u32,
    pub page_height: u32,
}

/// The sprites of one canvas: the pages of its preview target in the
/// default language, side by side. The first sprite has pages
/// `page_height` px high (less when it would be wider than
/// [`SPRITE_MAX_WIDTH`]); each further `scales` entry renders exactly that
/// multiple of the first's page size (`[1, 2]` = 1× and 2×). `None` when
/// no target shows the canvas.
pub fn render_canvas_sprites(
    ctx: &RenderContext,
    root: &Path,
    project: &Project,
    canvas: &str,
    page_height: u32,
    scales: &[u32],
) -> anyhow::Result<Option<Vec<Sprite>>> {
    let Some(target) = preview_target(project, canvas) else {
        return Ok(None);
    };
    let lang = project.languages.default.to_string();
    let fs = FsAssets::new(root);
    let scene = resolve(
        project,
        target.id.as_str(),
        &lang,
        &ctx.resolve_options(&fs),
    )
    .with_context(|| format!("canvas `{canvas}`"))?;
    let pages = &scene.frame.pages;
    let Some(first) = pages.first() else {
        return Ok(None);
    };
    let n = u32::try_from(pages.len()).unwrap_or(u32::MAX).max(1);
    let aspect = first.rect.w / first.rect.h.max(1e-9);
    let mut height = page_height.max(1);
    let mut width = (f64::from(height) * aspect).round().max(1.0) as u32;
    if width.saturating_mul(n) > SPRITE_MAX_WIDTH {
        width = (SPRITE_MAX_WIDTH / n).max(1);
        height = (f64::from(width) / aspect).round().max(1.0) as u32;
    }
    let prepared = ctx.prepare(&scene);
    let mut out = Vec::new();
    for &scale in scales {
        let (w, h) = (width * scale.max(1), height * scale.max(1));
        let mut pm = Pixmap::new(w * n, h).context("sprite size")?;
        pm.fill(tiny_skia::Color::WHITE);
        for (i, page) in pages.iter().enumerate() {
            let view = Viewport::exact(page.rect, w, h);
            let rendered = ctx.render_view(&prepared, &view, Quality::Export);
            let left = i32::try_from(u32::try_from(i).unwrap_or(0) * w).unwrap_or(i32::MAX);
            pm.draw_pixmap(
                left,
                0,
                rendered.pixmap.as_ref(),
                &PixmapPaint::default(),
                Transform::identity(),
                None,
            );
        }
        out.push(Sprite {
            pixmap: pm,
            pages: n,
            page_width: w,
            page_height: h,
        });
    }
    Ok(Some(out))
}

/// Sprites of every canvas of `pkg` ([`render_canvas_sprites`]):
/// `(canvas, sprites per scale)` in project order; canvases without a
/// target are left out.
pub fn sprites(
    pkg: &Package,
    page_height: u32,
    scales: &[u32],
) -> anyhow::Result<Vec<(String, Vec<Sprite>)>> {
    let tmp = TempDir::new();
    pkg.write_dir(&tmp.0)?;
    let project = Project::load(&tmp.0)?;
    let (ctx, _) = RenderContext::new(&tmp.0);
    let mut out = Vec::new();
    for c in &project.canvases {
        if let Some(list) =
            render_canvas_sprites(&ctx, &tmp.0, &project, c.id.as_str(), page_height, scales)?
        {
            out.push((c.id.to_string(), list));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_sprite_has_all_pages_side_by_side() {
        let pkg = vitrine_core::template::builtin::package("phone").unwrap();
        let list = sprites(&pkg, 100, &[1, 2]).unwrap();
        assert_eq!(list.len(), 1);
        let (canvas, s) = &list[0];
        assert_eq!(canvas, "phone");
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].pages, 3);
        assert_eq!(s[0].page_height, 100);
        assert_eq!(s[0].pixmap.width(), 3 * s[0].page_width);
        assert_eq!(s[0].pixmap.height(), 100);
        // 1320:2868 pages.
        assert!((f64::from(s[0].page_width) - 100.0 * 1320.0 / 2868.0).abs() < 1.5);
        // The 2x sprite is exactly twice the size.
        assert_eq!(s[1].page_height, 200);
        assert_eq!(s[1].page_width, 2 * s[0].page_width);
        assert_eq!(s[1].pixmap.width(), 2 * s[0].pixmap.width());
        // Not blank.
        let px = s[0].pixmap.pixels();
        assert!(px.iter().any(|p| p.red() != 255 || p.green() != 255));
    }

    #[test]
    fn builtin_previews_render() {
        let mut pkg = vitrine_core::template::builtin::package("minimal").unwrap();
        add_previews(&mut pkg, 120).unwrap();
        let png = pkg.preview("main").unwrap();
        let size = imagesize_png(png);
        assert!(size.1 >= 120 && size.0 > size.1 / 3, "{size:?}");
    }

    fn imagesize_png(png: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(png).unwrap();
        (img.width(), img.height())
    }
}
