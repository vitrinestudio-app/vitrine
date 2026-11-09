//! Rasterisation of a resolved scene (KONZEPT §6.1): `core::resolve` →
//! `core::layout` → pixels. One code path for GUI previews, MCP previews
//! and export.
//!
//! ```text
//! RenderContext (assets, fonts, text layouters; Send + Sync, shared)
//!   ├─ prepare(scene)               → PreparedScene (text layouts, issues)
//!   ├─ rasterize(scene, image, opt) → one output image (page / group / banner)
//!   └─ render_canvas(scene, opt)    → the whole canvas (debug / overview)
//! ```
//!
//! Rendering is deterministic: the same scene, assets and options give the
//! same pixels (no clocks, no randomness, no thread-dependent order).

pub mod assets;
mod draw;
mod overlay;

use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use arrangia_core::geometry::Viewport;
use arrangia_core::layout::{
    self, FontDirReport, FontLibrary, LayoutReport, PlacedText, TextDiagnostic, TextLayout,
    TextLayouter,
};
use arrangia_core::model::ElementId;
use arrangia_core::resolve::{ResolvedKind, ResolvedScene};
use serde::Serialize;
use tiny_skia::Pixmap;

pub use assets::{AssetCache, AssetError, Decoded, Resample};

use crate::geometry::scaled_viewport;
use crate::raster::Filter;

/// Rendering quality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// Cheaper resampling (bilinear convolution); for GUI/MCP previews.
    Preview,
    /// Lanczos3 resampling of screenshots and images.
    Export,
}

impl Quality {
    fn filter(self) -> Filter {
        match self {
            Self::Preview => Filter::Bilinear,
            Self::Export => Filter::Lanczos3,
        }
    }
}

/// Output size of [`RenderContext::rasterize`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scale {
    /// The target's pixel size.
    Target,
    /// Target size times a factor.
    Factor(f64),
    /// The longer side gets this many pixels.
    MaxEdge(u32),
    /// This many pixels wide.
    Width(u32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderOptions {
    pub scale: Scale,
    pub quality: Quality,
}

impl RenderOptions {
    /// Target size, export quality.
    pub fn export() -> Self {
        Self {
            scale: Scale::Target,
            quality: Quality::Export,
        }
    }

    /// `width` px wide, preview quality.
    pub fn preview(width: u32) -> Self {
        Self {
            scale: Scale::Width(width),
            quality: Quality::Preview,
        }
    }

    fn factor(&self, vp: &Viewport) -> f64 {
        match self.scale {
            Scale::Target => 1.0,
            Scale::Factor(f) => f,
            Scale::MaxEdge(m) => f64::from(m) / f64::from(vp.width_px.max(vp.height_px)),
            Scale::Width(w) => f64::from(w) / f64::from(vp.width_px),
        }
    }
}

/// Options of [`RenderContext::render_canvas`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasOptions {
    /// Pixels per lu.
    pub scale: f64,
    pub quality: Quality,
    /// Draw page boundaries, hatched gaps and page labels on top.
    pub overlay: bool,
}

impl Default for CanvasOptions {
    fn default() -> Self {
        Self {
            scale: 0.25,
            quality: Quality::Preview,
            overlay: true,
        }
    }
}

/// Something that went wrong or needs attention while rendering.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RenderIssue {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementId>,
    /// Home page of the element.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
    #[serde(flatten)]
    pub kind: IssueKind,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "issue", rename_all = "snake_case")]
pub enum IssueKind {
    /// From the text layout (overflow, emergency break, unknown span …).
    Text { diagnostic: TextDiagnostic },
    /// An asset could not be read or decoded; a placeholder was drawn.
    Asset { message: String },
}

impl RenderIssue {
    /// Text does not fit its box.
    pub fn is_overflow(&self) -> bool {
        matches!(&self.kind, IssueKind::Text { diagnostic } if diagnostic.is_overflow())
    }
}

impl std::fmt::Display for RenderIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(e) = &self.element {
            write!(f, "`{e}`: ")?;
        }
        match &self.kind {
            IssueKind::Text { diagnostic } => write!(f, "{diagnostic}"),
            IssueKind::Asset { message } => f.write_str(message),
        }
    }
}

/// A rendered image and what happened while rendering it.
#[derive(Debug)]
pub struct Rendered {
    pub pixmap: Pixmap,
    /// The canvas → pixel mapping that was used.
    pub viewport: Viewport,
    /// Asset problems of this render (text issues are in [`PreparedScene`]).
    pub issues: Vec<RenderIssue>,
}

/// A scene with its texts laid out; rasterise any number of its images
/// (in parallel) without laying out again.
#[derive(Debug)]
pub struct PreparedScene<'s> {
    pub scene: &'s ResolvedScene,
    /// Index-aligned with `scene.elements` (`Some` for texts with a string).
    texts: Vec<Option<PlacedText>>,
    /// Text layout diagnostics of all texts.
    pub issues: Vec<RenderIssue>,
}

impl PreparedScene<'_> {
    /// Placed layout of the text element at `index` in `scene.elements`.
    pub fn text(&self, index: usize) -> Option<&PlacedText> {
        self.texts.get(index).and_then(Option::as_ref)
    }

    /// Layout of the text element at `index` in `scene.elements`.
    pub fn text_layout(&self, index: usize) -> Option<&TextLayout> {
        self.text(index).map(|p| &p.layout)
    }

    /// Layout reports of all texts, boxes in canvas lu.
    pub fn layout_reports(&self) -> Vec<(ElementId, LayoutReport)> {
        self.scene
            .elements
            .iter()
            .zip(&self.texts)
            .filter_map(|(e, p)| {
                let p = p.as_ref()?;
                Some((
                    e.id.clone(),
                    p.layout
                        .report()
                        .translated(p.origin.x as f32, p.origin.y as f32),
                ))
            })
            .collect()
    }
}

/// Shared rendering state: asset cache, fonts, text layouters. `Send +
/// Sync`; use one per project and share it between threads.
pub struct RenderContext {
    assets: AssetCache,
    fonts: FontLibrary,
    layouters: Mutex<Vec<TextLayouter>>,
}

impl std::fmt::Debug for RenderContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderContext")
            .field("assets", &self.assets)
            .finish_non_exhaustive()
    }
}

impl RenderContext {
    /// Context for the project in `root`: bundled fonts plus `root/fonts/`.
    pub fn new(root: impl Into<PathBuf>) -> (Self, Option<FontDirReport>) {
        let root = root.into();
        let mut fonts = FontLibrary::bundled();
        let dir = root.join("fonts");
        let report = dir.is_dir().then(|| fonts.load_dir(&dir));
        (Self::with_fonts(root, fonts), report)
    }

    /// Context with an explicit font library (tests: bundled fonts only).
    pub fn with_fonts(root: impl Into<PathBuf>, fonts: FontLibrary) -> Self {
        Self {
            assets: AssetCache::new(root),
            fonts,
            layouters: Mutex::new(Vec::new()),
        }
    }

    pub fn root(&self) -> &Path {
        self.assets.root()
    }

    pub fn assets(&self) -> &AssetCache {
        &self.assets
    }

    pub fn fonts(&self) -> &FontLibrary {
        &self.fonts
    }

    /// Runs `f` with a text layouter from the pool.
    pub fn with_layouter<R>(&self, f: impl FnOnce(&mut TextLayouter) -> R) -> R {
        let pooled = self
            .layouters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .pop();
        let mut l = pooled.unwrap_or_else(|| TextLayouter::new(self.fonts.clone()));
        let r = f(&mut l);
        self.layouters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(l);
        r
    }

    /// Lays out all texts of `scene` (via `core::layout::layout_text`, so
    /// rendering and validation see the same lines).
    pub fn prepare<'s>(&self, scene: &'s ResolvedScene) -> PreparedScene<'s> {
        let spans = layout::span_styles(&scene.spans);
        let mut issues = Vec::new();
        let texts = self.with_layouter(|l| {
            scene
                .elements
                .iter()
                .map(|e| {
                    let ResolvedKind::Text(t) = &e.props else {
                        return None;
                    };
                    let placed = layout::layout_text(l, t, &spans)?;
                    issues.extend(placed.layout.diagnostics.iter().map(|d| RenderIssue {
                        element: Some(e.id.clone()),
                        page: Some(e.page),
                        kind: IssueKind::Text {
                            diagnostic: d.clone(),
                        },
                    }));
                    Some(placed)
                })
                .collect()
        });
        PreparedScene {
            scene,
            texts,
            issues,
        }
    }

    /// Output image `image` of the scene (prepare + rasterise).
    ///
    /// # Panics
    /// If `image` is out of range.
    pub fn rasterize(&self, scene: &ResolvedScene, image: usize, opts: &RenderOptions) -> Rendered {
        self.rasterize_prepared(&self.prepare(scene), image, opts)
    }

    /// Output image `image` of a prepared scene. Page images show exactly
    /// their source rect, so content in the gaps is omitted.
    ///
    /// # Panics
    /// If `image` is out of range.
    pub fn rasterize_prepared(
        &self,
        prepared: &PreparedScene<'_>,
        image: usize,
        opts: &RenderOptions,
    ) -> Rendered {
        let vp = prepared.scene.frame.images[image].viewport;
        let view = scaled_viewport(&vp, opts.factor(&vp));
        self.render_view(prepared, &view, opts.quality)
    }

    /// Any canvas region (GUI: zoomed views, drag previews).
    pub fn render_view(
        &self,
        prepared: &PreparedScene<'_>,
        view: &Viewport,
        quality: Quality,
    ) -> Rendered {
        draw::paint(self, prepared, view, quality)
    }

    /// The whole canvas (pages and gaps) at `opts.scale` px per lu,
    /// optionally with page boundaries and hatched gaps.
    pub fn render_canvas(&self, prepared: &PreparedScene<'_>, opts: &CanvasOptions) -> Rendered {
        let canvas = prepared.scene.frame.canvas;
        let width = (canvas.w * opts.scale).round().max(1.0) as u32;
        let view = Viewport::fit_width(canvas, width);
        let mut out = self.render_view(prepared, &view, opts.quality);
        if opts.overlay {
            self.with_layouter(|l| {
                overlay::debug_overlay(&mut out.pixmap, prepared.scene, &view, l);
            });
        }
        out
    }
}

/// Convenience: [`RenderContext::rasterize`].
pub fn rasterize(
    ctx: &RenderContext,
    scene: &ResolvedScene,
    image: usize,
    opts: &RenderOptions,
) -> Rendered {
    ctx.rasterize(scene, image, opts)
}

/// Convenience: the whole canvas with overlay at `scale` px per lu.
pub fn render_canvas(ctx: &RenderContext, scene: &ResolvedScene, scale: f64) -> Rendered {
    ctx.render_canvas(
        &ctx.prepare(scene),
        &CanvasOptions {
            scale,
            ..CanvasOptions::default()
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn context_is_send_and_sync() {
        assert_send_sync::<RenderContext>();
    }
}
