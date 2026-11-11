//! # vitrine-render
//!
//! One renderer for everything: GUI preview, export and MCP `render_preview`
//! all come from this pipeline, so line breaks and positions in the preview
//! match the export exactly (`docs/KONZEPT.md`, section 6).
//!
//! Responsibilities:
//!
//! - **Rasterization** (`scene`) of a resolved scene into an RGBA pixmap of
//!   one output image or the whole canvas (tiny-skia), with a shared,
//!   thread-safe asset cache and text layouter pool (`RenderContext`).
//! - **Device frames** drawn parametrically from catalog data in the styles
//!   `stylized` and `detailed`, plus imported and custom frame packs.
//! - **Text rendering**: glyph outlines of the `TextLayout` computed by
//!   `vitrine_core::layout`, drawn as paths (`text`).
//! - **Export**: cutting pages (omitting gap content), banners, gallery
//!   previews, removing alpha, encoding PNG/JPEG.
//!
//! Libraries and pipeline: `docs/adr/0001-render-pipeline.md`,
//! `docs/adr/0006-rasterize-and-export.md`.

pub mod blur;
pub mod export;
pub mod frames;
pub mod geometry;
pub mod paint;
pub mod raster;
pub mod scene;
pub mod text;

pub use export::{ExportOptions, ExportReport, export};
pub use scene::{
    CanvasOptions, PreparedScene, Quality, RenderContext, RenderIssue, RenderOptions, Rendered,
    Scale, rasterize, render_canvas,
};

/// Page width in lu as defined by `vitrine-core`.
pub fn core_page_width_lu() -> f64 {
    vitrine_core::PAGE_WIDTH_LU
}

/// Scale factor from logical units to pixels for a target page width in px.
///
/// ```
/// assert!((vitrine_render::px_per_lu(1320) - 1.32).abs() < 1e-9);
/// ```
pub fn px_per_lu(page_width_px: u32) -> f64 {
    f64::from(page_width_px) / vitrine_core::PAGE_WIDTH_LU
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousand_px_page_is_one_px_per_lu() {
        assert!((px_per_lu(1000) - 1.0).abs() < f64::EPSILON);
        assert!((core_page_width_lu() - 1000.0).abs() < f64::EPSILON);
    }
}
