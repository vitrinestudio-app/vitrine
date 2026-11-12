//! Frame packs (ADR 0024) with SVG support: the resvg rasterizer for
//! `core::frame_import::pack` and the combined frame index (imported vendor
//! frames + app-wide packs + the project's packs) every front end uses.

use std::path::Path;
use std::sync::Arc;

use resvg::usvg;
use tiny_skia::{Pixmap, Transform};
use vitrine_core::frame_import::embed;
use vitrine_core::frame_import::pack::{self, PackInfo, SvgRasterizer};
use vitrine_core::frame_import::{FrameIndex, RgbaImage};

/// Rasterizes SVG frames with resvg (no system fonts, like SVG assets).
#[derive(Debug, Clone, Copy, Default)]
pub struct ResvgRasterizer;

impl SvgRasterizer for ResvgRasterizer {
    fn rasterize(&self, svg: &[u8], long_side: u32) -> Result<(RgbaImage, f64), String> {
        let tree = usvg::Tree::from_data(svg, &usvg::Options::default())
            .map_err(|e| format!("SVG: {e}"))?;
        let size = tree.size();
        let (sw, sh) = (f64::from(size.width()), f64::from(size.height()));
        if sw <= 0.0 || sh <= 0.0 {
            return Err("SVG has no size".into());
        }
        let k = f64::from(long_side) / sw.max(sh);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "≤ 8192"
        )]
        let (w, h) = (
            ((sw * k).round() as u32).max(1),
            ((sh * k).round() as u32).max(1),
        );
        let mut pm = Pixmap::new(w, h).ok_or("SVG too large")?;
        #[allow(clippy::cast_possible_truncation, reason = "scale factor")]
        resvg::render(
            &tree,
            Transform::from_scale(k as f32, k as f32),
            &mut pm.as_mut(),
        );
        let data = pm
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        Ok((
            RgbaImage {
                width: w,
                height: h,
                data,
            },
            k,
        ))
    }
}

/// All packs (app-wide below `frames_root/packs`, the project's in
/// `project/frames`), compiled where they changed.
pub fn load_packs(frames_root: Option<&Path>, project: Option<&Path>) -> Vec<PackInfo> {
    pack::load_packs(frames_root, project, Some(&ResvgRasterizer))
}

/// The frame index for rendering and validation: installed vendor frames
/// below `frames_root`, the project's stored copies of them
/// (`frames/imported/`, ADR 0027; they win), plus all packs (`None` when
/// there is nothing).
pub fn frame_index(frames_root: Option<&Path>, project: Option<&Path>) -> Option<Arc<FrameIndex>> {
    let mut index = frames_root.map(FrameIndex::load).unwrap_or_default();
    if let Some(project) = project {
        embed::add_project_copies(&mut index, project);
    }
    let packs = load_packs(frames_root, project);
    pack::add_packs(&mut index, &packs);
    (!index.is_empty()).then(|| Arc::new(index))
}
