//! Preview images of templates (ADR 0023): one PNG per canvas, rendered from
//! the package itself (its placeholders, assets and fonts; parametric
//! device frames only, never imported vendor bezels). The app renders them
//! when saving a template; the gallery server renders them again from the
//! uploaded package (trusted previews).

use std::path::{Path, PathBuf};

use anyhow::Context;
use vitrine_core::model::{Project, TargetKind};
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
    let targets: Vec<_> = project
        .targets
        .iter()
        .filter(|t| t.canvas.as_str() == canvas)
        .collect();
    let Some(target) = targets
        .iter()
        .find(|t| t.kind != TargetKind::Banner)
        .or_else(|| targets.first())
    else {
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

#[cfg(test)]
mod tests {
    use super::*;

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
