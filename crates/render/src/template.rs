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
use vitrine_core::template::extract::SaveReport;

use crate::export::gallery;
use crate::raster::{SourceImage, encode_png_rgb};
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

/// JPEG quality of sample screenshots in saved templates (owner decision
/// 2026-11-12): packages shrink by roughly three quarters, UI screenshots
/// stay readable as samples.
pub const SAMPLE_JPEG_QUALITY: u8 = 60;

/// Re-encodes the sample screenshots of a freshly extracted package as
/// sRGB JPEG ([`SAMPLE_JPEG_QUALITY`]): `screenshots/en/home.png` becomes
/// `screenshots/en/home.jpg`, which the screenshot lookup finds in the same
/// place. Only `manifest.samples` are touched – never assets, fonts,
/// placeholders or previews. A sample keeps its file when it has
/// transparent pixels (reported in `samples_png`; JPEG has no alpha and
/// the transparency may be intended), when the JPEG would not be smaller,
/// or when it cannot be decoded. Colour profiles are converted to sRGB on
/// decode (ADR 0021); the JPEG is untagged sRGB. Updates
/// `report.samples` / `samples_bytes` and checks the package again.
pub fn compress_samples(pkg: &mut Package, report: &mut SaveReport) -> anyhow::Result<()> {
    if pkg.manifest.samples.is_empty() {
        return Ok(());
    }
    let paths = std::mem::take(&mut pkg.manifest.samples);
    // Decoding and encoding dominate: one thread per core.
    let encoded: Vec<anyhow::Result<Option<Vec<u8>>>> = {
        let inputs: Vec<&[u8]> = paths
            .iter()
            .map(|p| pkg.file(p).unwrap_or_default())
            .collect();
        let threads = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
        let chunk = inputs.len().div_ceil(threads).max(1);
        std::thread::scope(|scope| {
            let handles: Vec<_> = inputs
                .chunks(chunk)
                .map(|c| {
                    let h =
                        scope.spawn(move || c.iter().map(|d| sample_jpeg(d)).collect::<Vec<_>>());
                    (c.len(), h)
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|(n, h)| {
                    h.join().unwrap_or_else(|_| {
                        (0..n)
                            .map(|_| Err(anyhow::anyhow!("encoder panicked")))
                            .collect()
                    })
                })
                .collect()
        })
    };
    let mut samples = Vec::with_capacity(paths.len());
    let mut bytes = 0u64;
    for (path, result) in paths.into_iter().zip(encoded) {
        let original = pkg.file(&path).map_or(0, <[u8]>::len);
        let stem = path.rsplit_once('.').map_or(path.as_str(), |(s, _)| s);
        let dest = format!("{stem}.jpg");
        let free = dest == path || pkg.file(&dest).is_none();
        match result {
            Ok(Some(jpeg)) if jpeg.len() < original && free => {
                bytes += jpeg.len() as u64;
                pkg.remove(&path);
                pkg.put(&dest, jpeg)?;
                samples.push(dest);
            }
            Ok(None) => {
                bytes += original as u64;
                report.samples_png.push(path.clone());
                samples.push(path);
            }
            _ => {
                bytes += original as u64;
                samples.push(path);
            }
        }
    }
    report.samples.clone_from(&samples);
    report.samples_bytes = bytes;
    pkg.manifest.samples = samples;
    pkg.check()?;
    Ok(())
}

/// A sample as sRGB JPEG, or `None` when it has transparent pixels.
fn sample_jpeg(data: &[u8]) -> anyhow::Result<Option<Vec<u8>>> {
    let img = SourceImage::decode(data)?;
    if img.rgba.as_chunks::<4>().0.iter().any(|px| px[3] < 255) {
        return Ok(None);
    }
    let rgb: Vec<u8> = img
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|px| [px[0], px[1], px[2]])
        .collect();
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, SAMPLE_JPEG_QUALITY).encode(
        &rgb,
        img.width,
        img.height,
        image::ExtendedColorType::Rgb8,
    )?;
    Ok(Some(out))
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

    fn example_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example")
    }

    fn png_rgba(w: u32, h: u32, alpha: u8) -> Vec<u8> {
        let mut img = image::RgbaImage::new(w, h);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x * 7) as u8, (y * 3) as u8, 128, alpha]);
        }
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn samples_become_jpeg_and_new_projects_find_them() {
        use vitrine_core::resolve::{FsAssets, lang_chain, lookup_screenshot};
        use vitrine_core::samples::SampleMarks;
        use vitrine_core::template::extract::{SaveOptions, extract};
        use vitrine_core::template::instantiate::{create_project, parse_langs};

        let root = example_dir();
        let project = Project::load(&root).unwrap();
        let opts = SaveOptions {
            name: "Samples".into(),
            canvases: Some(vec!["phone".into()]),
            sample_screenshots: true,
            ..SaveOptions::default()
        };
        let (mut pkg, mut report) = extract(&project, &root, &opts).unwrap();
        let before = pkg.clone();
        let pngs = before.manifest.samples.clone();
        assert!(pngs.iter().all(|p| p.ends_with("png")), "{pngs:?}");
        compress_samples(&mut pkg, &mut report).unwrap();

        // Every sample is a JPEG in the same place (the PNG gone) – or,
        // where the JPEG would not be smaller (the fixture's flat fake UI),
        // the unchanged PNG.
        assert_eq!(pkg.manifest.samples.len(), pngs.len());
        assert_eq!(report.samples, pkg.manifest.samples);
        let mut jpegs = 0;
        for (png, jpg) in pngs.iter().zip(&pkg.manifest.samples) {
            if jpg == png {
                assert_eq!(pkg.file(png), before.file(png), "{png}");
                continue;
            }
            jpegs += 1;
            assert_eq!(jpg, &png.replace(".png", ".jpg"));
            assert!(pkg.file(png).is_none(), "{png}");
            let data = pkg.file(jpg).unwrap();
            assert!(data.starts_with(&[0xFF, 0xD8, 0xFF]), "{jpg}");
            let (a, b) = (
                image::load_from_memory(before.file(png).unwrap()).unwrap(),
                image::load_from_memory(data).unwrap(),
            );
            assert_eq!((a.width(), a.height()), (b.width(), b.height()));
        }
        assert!(jpegs > 0, "{:?}", pkg.manifest.samples);
        assert!(report.samples_bytes < report.samples_bytes_original);
        assert_eq!(report.samples_png.len(), 0);
        // Nothing else changed: assets, fonts, placeholders, texts.
        for path in before.paths().filter(|p| !pngs.iter().any(|s| s == p)) {
            assert_eq!(pkg.file(path), before.file(path), "{path}");
        }

        // A new project gets the JPEGs, marked with the hash of the bytes
        // written, and the screenshot lookup finds them.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("p");
        let langs = parse_langs(&["en".into()]).unwrap();
        let created = create_project(&pkg, &dir, "P", &langs).unwrap();
        assert_ne!(created.samples.len(), 0);
        let marks = SampleMarks::load(&dir);
        for s in &created.samples {
            assert!(marks.is_sample(&dir, s), "{s}");
        }
        let jpg = created
            .samples
            .iter()
            .find(|s| {
                s.starts_with("screenshots/en/ios-phone/")
                    && Path::new(s.as_str())
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("jpg"))
            })
            .expect("a JPEG sample for ios-phone");
        let key = jpg
            .trim_start_matches("screenshots/en/ios-phone/")
            .trim_end_matches(".jpg");
        let fs = FsAssets::new(&dir);
        let chain = lang_chain(&created.project, "en");
        let shot = lookup_screenshot(&fs, &chain, Some("ios-phone"), key);
        assert_eq!(shot.path.as_deref(), Some(jpg.as_str()));
        // Previews render from the JPEG samples.
        add_previews(&mut pkg, 120).unwrap();
        assert!(pkg.preview("phone").is_some());
    }

    #[test]
    fn transparent_samples_stay_png() {
        assert!(sample_jpeg(&png_rgba(40, 30, 200)).unwrap().is_none());
        let jpeg = sample_jpeg(&png_rgba(40, 30, 255)).unwrap().unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8, 0xFF]));
        assert!(sample_jpeg(b"not an image").is_err());
    }

    fn imagesize_png(png: &[u8]) -> (u32, u32) {
        let img = image::load_from_memory(png).unwrap();
        (img.width(), img.height())
    }
}
