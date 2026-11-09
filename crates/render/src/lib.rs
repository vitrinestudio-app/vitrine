//! # arrangia-render
//!
//! One renderer for everything: GUI preview, export and MCP `render_preview`
//! all come from this pipeline, so line breaks and positions in the preview
//! match the export exactly (`docs/KONZEPT.md`, section 6).
//!
//! Responsibilities:
//!
//! - **Rasterization** of a resolved scene into an RGBA pixmap of the whole
//!   strip or a single page (planned: `tiny-skia`).
//! - **Device frames** drawn parametrically from catalog data in the styles
//!   `stylized` and `detailed`, plus imported and custom frame packs.
//! - **Text rendering** from the layout computed in `arrangia-core`.
//! - **Export**: cutting pages (omitting gap content), banners, gallery
//!   previews, removing alpha, encoding PNG/JPEG.
//!
//! The concrete libraries are decided by the Phase 0 spike (see
//! `docs/adr/0001-render-pipeline.md`).

pub mod blur;
pub mod frames;
pub mod geometry;
pub mod paint;
pub mod raster;
pub mod text;

/// Page width in lu as defined by `arrangia-core`.
pub fn core_page_width_lu() -> f64 {
    arrangia_core::PAGE_WIDTH_LU
}

/// Scale factor from logical units to pixels for a target page width in px.
///
/// ```
/// assert!((arrangia_render::px_per_lu(1320) - 1.32).abs() < 1e-9);
/// ```
pub fn px_per_lu(page_width_px: u32) -> f64 {
    f64::from(page_width_px) / arrangia_core::PAGE_WIDTH_LU
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
