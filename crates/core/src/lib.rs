//! # arrangia-core
//!
//! The heart of Arrangia: the project model and everything that works on it
//! without touching pixels or the network.
//!
//! Responsibilities (see `docs/KONZEPT.md`, sections 3 and 10):
//!
//! - **Model**: projects, strips (canvases), pages, gaps, elements, styles,
//!   variants, targets. Every element hangs on a *home page* and is positioned
//!   relative to it; a page is always [`PAGE_WIDTH_LU`] logical units wide.
//! - **YAML IO**: loading and saving the project folder (`project.yaml`,
//!   `i18n/*.yaml`) with stable, diff-friendly output.
//! - **Operations API**: every change is an `Op`; `Project::apply(op)`
//!   validates, applies and returns the inverse op (undo). GUI, CLI and MCP
//!   use nothing else to modify a project.
//! - **Resolution**: `resolve(target, language)` turns the project into a
//!   concrete scene (anchors, fallbacks, device mapping, styles).
//! - **Layout**: text shaping, wrapping and auto-fit, bounding boxes.
//! - **Validation** and **catalogs** (devices, targets, gallery profiles),
//!   the latter loaded from data files in `crates/core/data/`.
//!
//! Module layout (`model`, `io`, `ops`, `catalog`, `geometry`, `resolve`,
//! `layout`, `markup`, `validate`).
//! The file format is described in `docs/adr/0002-project-file-format.md`.
//!
//! ```text
//! model/      project, strip, page, element, style, target, variant
//! io          YAML load/save, atomic writes, revision check
//! ops/        Op, apply, inverse, transactions, undo/redo history
//! geometry    page → canvas mapping, gaps, grouping, anchors, viewports
//! resolve/    anchors, page grouping, fallbacks, device_map
//! layout/     text layout and auto-fit
//! validate/   findings per target × language (references, texts, screenshots, store rules)
//! catalog/    devices, targets, gallery profiles (from data/*.yaml)
//! templates   built-in project templates (`arrangia new`, GUI)
//! ```

pub mod catalog;
pub mod geometry;
pub mod io;
pub mod layout;
pub mod markup;
pub mod model;
pub mod ops;
pub mod resolve;
pub mod templates;
pub mod validate;

pub use model::Project;

/// Width of a single page in logical units (lu).
///
/// All layout values are expressed in lu; the export resolution is derived
/// solely from the output target.
pub const PAGE_WIDTH_LU: f64 = 1000.0;

/// Identifier written as `format:` at the top of every `project.yaml`.
pub const PROJECT_FORMAT: &str = "arrangia/1";

/// Height of a page in lu for a given aspect ratio (`width : height`).
///
/// ```
/// let h = arrangia_core::page_height_lu(1320.0, 2868.0);
/// assert!((h - 2172.727).abs() < 0.001);
/// ```
pub fn page_height_lu(aspect_width: f64, aspect_height: f64) -> f64 {
    PAGE_WIDTH_LU * aspect_height / aspect_width
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_page_is_as_high_as_wide() {
        assert!((page_height_lu(1.0, 1.0) - PAGE_WIDTH_LU).abs() < f64::EPSILON);
    }

    #[test]
    fn format_identifier_is_versioned() {
        assert!(PROJECT_FORMAT.starts_with("arrangia/"));
    }
}
