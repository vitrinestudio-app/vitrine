//! Canvas geometry lives in `arrangia_core::geometry` (ADR 0004); this
//! module re-exports it and adds the tiny-skia transform of a viewport.

pub use arrangia_core::geometry::{CanvasLayout, Rect, Vec2, Viewport};

/// Canvas lu → pixel transform of a viewport.
pub fn view_transform(v: &Viewport) -> tiny_skia::Transform {
    let s = v.scale as f32;
    tiny_skia::Transform::from_row(
        s,
        0.0,
        0.0,
        s,
        (-v.origin_x * v.scale) as f32,
        (-v.origin_y * v.scale) as f32,
    )
}

/// The same viewport with every pixel dimension multiplied by `factor`
/// (previews): the visible canvas region stays (up to rounding).
pub fn scaled_viewport(v: &Viewport, factor: f64) -> Viewport {
    Viewport {
        origin_x: v.origin_x,
        origin_y: v.origin_y,
        scale: v.scale * factor,
        width_px: (f64::from(v.width_px) * factor).round().max(1.0) as u32,
        height_px: (f64::from(v.height_px) * factor).round().max(1.0) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transform_maps_origin_to_zero() {
        let v = Viewport::exact(Rect::new(1040.0, 0.0, 1000.0, 2172.7), 1320, 2868);
        let mut p = [tiny_skia::Point::from_xy(1040.0, 100.0)];
        view_transform(&v).map_points(&mut p);
        assert!(p[0].x.abs() < 1e-3);
        assert!((p[0].y - 132.0).abs() < 1e-3);
        let half = scaled_viewport(&v, 0.5);
        assert_eq!((half.width_px, half.height_px), (660, 1434));
    }
}
