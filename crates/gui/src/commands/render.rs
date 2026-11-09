//! Preview rendering: output images, the canvas and the store gallery as
//! binary images, cached in the session's [`crate::preview::PreviewCache`].

use std::sync::Arc;

use arrangia_core::Project;
use arrangia_core::geometry::{Rect, Viewport};
use arrangia_core::model::{TargetKind, VariantId};
use arrangia_core::resolve::{FsAssets, ResolvedScene, resolve};
use arrangia_render::export::{gallery, scene_assets};
use arrangia_render::scene::{CanvasOptions, Quality, RenderOptions, Scale};
use serde::{Deserialize, Serialize};

use super::edit::{Gesture, ViewContext, double_option, preview_project};
use super::project::check_target;
use super::{AppState, NoArgs, lang_or_default};
use crate::error::{CmdResult, CommandError};
use crate::preview::{CacheStats, PixelFormat, PreviewImage, PreviewKey, View, scene_hash};
use crate::session::Session;

/// Default longer side of image previews (px).
pub const DEFAULT_MAX_EDGE: u32 = 800;
/// Default canvas scale (px per lu).
pub const DEFAULT_CANVAS_SCALE: f64 = 0.25;
/// Default gallery image height (px).
pub const DEFAULT_GALLERY_HEIGHT: u32 = 600;
/// Size factor of interactive (drag) previews.
pub const INTERACTIVE_FACTOR: f64 = 0.5;
/// Largest preview side accepted (px).
pub const MAX_SIDE: f64 = 8192.0;

/// Render quality of previews.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PreviewQuality {
    /// Bilinear resampling (default).
    #[default]
    Preview,
    /// Lanczos3, as in the export.
    Export,
}

impl PreviewQuality {
    fn render(self) -> Quality {
        match self {
            Self::Preview => Quality::Preview,
            Self::Export => Quality::Export,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct RenderImageArgs {
    /// Default: the first target.
    #[serde(default)]
    pub target: Option<String>,
    /// Default: the project's default language.
    #[serde(default)]
    pub lang: Option<String>,
    /// Missing = the target's own variant, `null` = the base elements, a
    /// name = that variant of the target's strip (variant view).
    #[serde(default, deserialize_with = "double_option")]
    pub variant: Option<Option<String>>,
    /// Output image index, 0-based (a page, a page group or the banner).
    #[serde(default)]
    pub image: u32,
    /// Size: at most one of `max_edge` (default 800), `width`, `scale`
    /// (factor of the target size).
    #[serde(default)]
    pub max_edge: Option<u32>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub quality: PreviewQuality,
    /// Coarse preview while dragging: half the size.
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub format: PixelFormat,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct RenderCanvasArgs {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    /// Missing = the target's own variant, `null` = the base elements, a
    /// name = that variant of the target's strip (variant view).
    #[serde(default, deserialize_with = "double_option")]
    pub variant: Option<Option<String>>,
    /// Only this canvas region (lu; e.g. a gap or a tile). Default: the
    /// whole canvas.
    #[serde(default)]
    pub region: Option<Rect>,
    /// Pixels per lu (default 0.25); or `max_edge` for the longer side.
    #[serde(default)]
    pub scale: Option<f64>,
    #[serde(default)]
    pub max_edge: Option<u32>,
    /// Page boundaries, hatched gaps, page labels (default true).
    #[serde(default = "yes")]
    pub overlay: bool,
    #[serde(default)]
    pub quality: PreviewQuality,
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub format: PixelFormat,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct RenderGalleryArgs {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    /// Missing = the target's own variant, `null` = the base elements, a
    /// name = that variant of the target's strip (variant view).
    #[serde(default, deserialize_with = "double_option")]
    pub variant: Option<Option<String>>,
    /// Image height in px (default 600, never above the target height).
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub interactive: bool,
    #[serde(default)]
    pub format: PixelFormat,
}

fn yes() -> bool {
    true
}

/// `project` with `target` showing `variant` (see [`ViewContext`]).
#[allow(clippy::option_option)]
fn with_variant(
    mut project: Project,
    target: &str,
    variant: Option<&Option<String>>,
) -> CmdResult<Project> {
    let Some(variant) = variant else {
        return Ok(project);
    };
    let variant = match variant {
        None => None,
        Some(v) => {
            let id = VariantId::parse(v).map_err(CommandError::invalid)?;
            let strip = project
                .target(target)
                .and_then(|t| project.strip(t.strip.as_str()));
            if !strip.is_some_and(|s| s.variants.contains_key(&id)) {
                return Err(CommandError::not_found(format!(
                    "unknown variant `{v}` for target `{target}`"
                )));
            }
            Some(id)
        }
    };
    if let Some(t) = project.targets.iter_mut().find(|t| t.id.as_str() == target) {
        t.variant = variant;
    }
    Ok(project)
}

/// Resolved scene of `project` (default: the session's current project)
/// for the view; rendering happens on this snapshot, outside the lock.
#[allow(clippy::option_option)]
fn scene(
    session: &Session,
    project: Option<&Project>,
    target: Option<String>,
    lang: Option<String>,
    variant: Option<&Option<String>>,
) -> CmdResult<(String, String, ResolvedScene)> {
    let project = match project {
        Some(p) => p.clone(),
        None => session.snapshot(),
    };
    let target = match target {
        Some(t) => {
            check_target(&project, &t)?;
            t
        }
        None => project
            .targets
            .first()
            .map(|t| t.id.to_string())
            .ok_or_else(|| CommandError::not_found("the project has no targets"))?,
    };
    let project = with_variant(project, &target, variant)?;
    let lang = lang_or_default(session, lang);
    let fs = FsAssets::new(session.dir());
    let render = session.render_context();
    let scene = resolve(&project, &target, &lang, &render.resolve_options(&fs))?;
    Ok((target, lang, scene))
}

fn positive(v: f64, what: &str) -> CmdResult<f64> {
    if v.is_finite() && v > 0.0 {
        Ok(v)
    } else {
        Err(CommandError::invalid(format!(
            "{what} must be a positive number"
        )))
    }
}

/// Cached render: looks up `key`, else renders with `f` and stores.
fn cached(
    session: &Session,
    key: PreviewKey,
    scene: &ResolvedScene,
    f: impl FnOnce() -> CmdResult<PreviewImage>,
) -> CmdResult<Arc<PreviewImage>> {
    if let Some(hit) = session.preview().get(&key) {
        return Ok(hit);
    }
    let image = Arc::new(f()?);
    session
        .preview()
        .insert(key, image.clone(), scene_assets(scene));
    Ok(image)
}

fn too_large(w: f64, h: f64) -> CmdResult<()> {
    if w.max(h) > MAX_SIDE {
        return Err(CommandError::invalid(format!(
            "preview too large ({w:.0}×{h:.0} px, max {MAX_SIDE} px per side)"
        )));
    }
    Ok(())
}

/// One output image (page, page group or banner).
pub fn render_image(app: &AppState, args: RenderImageArgs) -> CmdResult<Arc<PreviewImage>> {
    let session = app.session()?;
    let (target, lang, scene) = scene(
        &session,
        None,
        args.target.clone(),
        args.lang.clone(),
        args.variant.as_ref(),
    )?;
    image_of(&session, &scene, &target, &lang, &args, true)
}

/// Renders output image `args.image` of `scene` (cached unless `cache` is
/// false).
fn image_of(
    session: &Session,
    scene: &ResolvedScene,
    target: &str,
    lang: &str,
    args: &RenderImageArgs,
    cache: bool,
) -> CmdResult<Arc<PreviewImage>> {
    let Some(img) = scene.frame.images.get(args.image as usize) else {
        return Err(CommandError::not_found(format!(
            "image {} is out of range (target `{target}` has {} output images)",
            args.image,
            scene.frame.images.len()
        )));
    };
    let vp = img.viewport;
    let (w, h) = (f64::from(vp.width_px), f64::from(vp.height_px));
    let mut factor = match (args.width, args.max_edge, args.scale) {
        (Some(px), None, None) => f64::from(px) / w,
        (None, Some(px), None) => f64::from(px) / w.max(h),
        (None, None, Some(s)) => positive(s, "scale")?,
        (None, None, None) => f64::from(DEFAULT_MAX_EDGE) / w.max(h),
        _ => {
            return Err(CommandError::invalid(
                "give at most one of `width`, `max_edge`, `scale`",
            ));
        }
    };
    if args.interactive {
        factor *= INTERACTIVE_FACTOR;
    }
    positive(factor, "size")?;
    too_large(w * factor, h * factor)?;
    let ctx = session.render_context();
    let render = || {
        let prepared = ctx.prepare(scene);
        let r = ctx.rasterize_prepared(
            &prepared,
            args.image as usize,
            &RenderOptions {
                scale: Scale::Factor(factor),
                quality: args.quality.render(),
            },
        );
        PreviewImage::encode(&r.pixmap, Some(&r.viewport), args.format)
    };
    if !cache {
        return render().map(Arc::new);
    }
    let key = PreviewKey {
        target: target.to_owned(),
        lang: lang.to_owned(),
        view: View::Image(args.image),
        size: format!("f{factor:.6}"),
        quality: format!("{:?}", args.quality),
        format: args.format,
        scene_hash: scene_hash(scene, &ctx),
    };
    cached(session, key, scene, render)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct RenderPreviewEditArgs {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub variant: Option<Option<String>>,
    /// The gesture in progress (as the matching `edit_*` command takes it,
    /// with `kind`: `move`, `move_by`, `resize`, `rotate`).
    pub edit: Gesture,
    /// Output image index and size as in `render_image`.
    #[serde(default)]
    pub image: u32,
    #[serde(default)]
    pub max_edge: Option<u32>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub scale: Option<f64>,
    /// Default true: half size, as while dragging.
    #[serde(default = "yes")]
    pub interactive: bool,
    #[serde(default)]
    pub format: PixelFormat,
}

/// Live drag feedback: output image `image` as it would look after the
/// gesture, rendered from a scratch copy of the project (nothing is saved
/// or cached; the scratch copy is shared by the images of one frame).
pub fn render_preview_edit(
    app: &AppState,
    args: RenderPreviewEditArgs,
) -> CmdResult<Arc<PreviewImage>> {
    let session = app.session()?;
    let view = ViewContext {
        target: args.target.clone(),
        lang: args.lang.clone(),
        variant: args.variant.clone(),
    };
    let project = preview_project(&session, &view, &args.edit)?;
    let (target, lang, scene) = scene(
        &session,
        Some(&project),
        args.target.clone(),
        args.lang.clone(),
        args.variant.as_ref(),
    )?;
    let image_args = RenderImageArgs {
        target: None,
        lang: None,
        variant: None,
        image: args.image,
        max_edge: args.max_edge,
        width: args.width,
        scale: args.scale,
        quality: PreviewQuality::Preview,
        interactive: args.interactive,
        format: args.format,
    };
    image_of(&session, &scene, &target, &lang, &image_args, false)
}

/// The whole canvas (pages and gaps) or a `region` of it, optionally with
/// the debug overlay (whole canvas only).
pub fn render_canvas(app: &AppState, args: RenderCanvasArgs) -> CmdResult<Arc<PreviewImage>> {
    let session = app.session()?;
    let (target, lang, scene) = scene(
        &session,
        None,
        args.target,
        args.lang,
        args.variant.as_ref(),
    )?;
    let canvas = match args.region {
        Some(r) if r.w > 0.0 && r.h > 0.0 && r.x.is_finite() && r.y.is_finite() => r,
        Some(_) => return Err(CommandError::invalid("`region` must have a positive size")),
        None => scene.frame.canvas,
    };
    let mut scale = match (args.scale, args.max_edge) {
        (Some(s), None) => positive(s, "scale")?,
        (None, Some(m)) => f64::from(m) / canvas.w.max(canvas.h),
        (None, None) => DEFAULT_CANVAS_SCALE,
        _ => {
            return Err(CommandError::invalid(
                "give at most one of `scale`, `max_edge`",
            ));
        }
    };
    if args.interactive {
        scale *= INTERACTIVE_FACTOR;
    }
    too_large(canvas.w * scale, canvas.h * scale)?;
    let ctx = session.render_context();
    let key = PreviewKey {
        target,
        lang,
        view: View::Canvas {
            overlay: args.overlay,
            region: args
                .region
                .map(|r| format!("{},{},{},{}", r.x, r.y, r.w, r.h)),
        },
        size: format!("s{scale:.6}"),
        quality: format!("{:?}", args.quality),
        format: args.format,
        scene_hash: scene_hash(&scene, &ctx),
    };
    cached(&session, key, &scene, || {
        let prepared = ctx.prepare(&scene);
        let r = if args.region.is_some() {
            let width = (canvas.w * scale).round().max(1.0) as u32;
            ctx.render_view(
                &prepared,
                &Viewport::fit_width(canvas, width),
                args.quality.render(),
            )
        } else {
            ctx.render_canvas(
                &prepared,
                &CanvasOptions {
                    scale,
                    quality: args.quality.render(),
                    overlay: args.overlay,
                },
            )
        };
        PreviewImage::encode(&r.pixmap, Some(&r.viewport), args.format)
    })
}

/// The store gallery preview (output images side by side with the store gaps).
pub fn render_gallery(app: &AppState, args: RenderGalleryArgs) -> CmdResult<Arc<PreviewImage>> {
    let session = app.session()?;
    let (target, lang, scene) = scene(
        &session,
        None,
        args.target,
        args.lang,
        args.variant.as_ref(),
    )?;
    if scene.frame.kind == TargetKind::Banner {
        return Err(CommandError::invalid(format!(
            "target `{target}` has no store gallery (banner targets have none)"
        )));
    }
    let mut height = args.height.unwrap_or(DEFAULT_GALLERY_HEIGHT).max(1);
    if args.interactive {
        height = ((f64::from(height) * INTERACTIVE_FACTOR).round() as u32).max(1);
    }
    too_large(0.0, f64::from(height))?;
    let ctx = session.render_context();
    let key = PreviewKey {
        target,
        lang,
        view: View::Gallery,
        size: format!("h{height}"),
        quality: "export".into(),
        format: args.format,
        scene_hash: scene_hash(&scene, &ctx),
    };
    cached(&session, key, &scene, || {
        let prepared = ctx.prepare(&scene);
        let (pixmap, _) = gallery(&ctx, &prepared, height);
        too_large(f64::from(pixmap.width()), 0.0)?;
        PreviewImage::encode(&pixmap, None, args.format)
    })
}

/// Preview cache statistics of the open project.
pub fn preview_stats(app: &AppState, _: NoArgs) -> CmdResult<CacheStats> {
    Ok(app.session()?.preview().stats())
}
