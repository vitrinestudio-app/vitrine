//! Page → canvas geometry: home pages, gaps, page grouping, vertical
//! anchors and viewports (KONZEPT §3.2–3.4, ADR 0001, ADR 0004).
//!
//! Ported from the Phase 0 spike (`arrangia-render/src/geometry.rs`), free
//! of any rendering dependency so that `core::resolve`, validation, the GUI
//! overlays and the renderer share one implementation.
//!
//! * A page (or, with grouping, a column) is always [`PAGE_WIDTH_LU`] wide.
//!   Its height comes from the *target*: `k · 1000 · h / w` for an export
//!   image of `w × h` px holding `k` pages side by side.
//! * Pages are laid out left to right. Between two **export images** lies a
//!   physical gap of `gap` lu; the pages of one image are adjacent columns:
//!
//!   `page_x(i) = i · 1000 + ⌊i / k⌋ · gap`
//!
//! * Elements store positions relative to their home page. Only the home
//!   page origin moves when gap or grouping change, so content reaching into
//!   a neighbouring page keeps lining up with the store gallery.
//! * y is relative to the page top and designed for the *reference* page
//!   height (the strip's own aspect); [`CanvasLayout::anchor_y`] adapts it
//!   to the target page height.
//! * y points down; rotation in degrees, positive = clockwise.

use serde::{Deserialize, Serialize};

use crate::PAGE_WIDTH_LU;
use crate::model::AnchorY;

/// A point or offset in lu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    /// Rotate around `center` by `degrees` (positive = clockwise in a
    /// y-down coordinate system).
    #[must_use]
    pub fn rotated_around(self, center: Vec2, degrees: f64) -> Vec2 {
        if degrees == 0.0 {
            return self;
        }
        let (s, c) = degrees.to_radians().sin_cos();
        let dx = self.x - center.x;
        let dy = self.y - center.y;
        Vec2::new(center.x + dx * c - dy * s, center.y + dx * s + dy * c)
    }
}

impl std::ops::Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl std::ops::Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl std::ops::Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f64) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
}

/// Axis-aligned rectangle (lu unless stated otherwise).
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    /// Rectangle of size `w × h` centred at `c`.
    pub fn centered(c: Vec2, w: f64, h: f64) -> Self {
        Self::new(c.x - w / 2.0, c.y - h / 2.0, w, h)
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn center(&self) -> Vec2 {
        Vec2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// Do the interiors overlap (touching edges do not count)?
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    /// Is `p` inside (left/top edge inclusive, right/bottom exclusive)?
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x < self.right() && p.y >= self.y && p.y < self.bottom()
    }

    /// Smallest rectangle containing both.
    #[must_use]
    pub fn union(&self, other: &Rect) -> Rect {
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Rect::new(
            x,
            y,
            self.right().max(other.right()) - x,
            self.bottom().max(other.bottom()) - y,
        )
    }
}

/// Page height in lu for an export image of `width_px × height_px` holding
/// `group` pages side by side: `group · 1000 · h / w`.
pub fn target_page_height(width_px: u32, height_px: u32, group: u32) -> f64 {
    f64::from(group.max(1)) * PAGE_WIDTH_LU * f64::from(height_px) / f64::from(width_px)
}

/// How the pages of a strip are arranged on the canvas for one target.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanvasLayout {
    /// Number of pages of the strip.
    pub page_count: u32,
    /// Page height the layout was designed for (strip aspect), lu.
    pub reference_page_height: f64,
    /// Page (column) height for the target, lu.
    pub page_height: f64,
    /// Gap between export images in lu (`0` = no gaps).
    pub gap: f64,
    /// Pages per export image (`group: k`, 1 = one page per image).
    pub group: u32,
}

impl CanvasLayout {
    /// Layout with the reference page height, no gaps, no grouping.
    pub fn new(page_count: u32, reference_page_height: f64) -> Self {
        Self {
            page_count,
            reference_page_height,
            page_height: reference_page_height,
            gap: 0.0,
            group: 1,
        }
    }

    /// Page height and grouping from a target pixel size.
    #[must_use]
    pub fn for_target(mut self, width_px: u32, height_px: u32, group: u32) -> Self {
        self.group = group.max(1);
        self.page_height = target_page_height(width_px, height_px, self.group);
        self
    }

    #[must_use]
    pub fn with_gap(mut self, gap: f64) -> Self {
        self.gap = gap;
        self
    }

    fn k(&self) -> i64 {
        i64::from(self.group.max(1))
    }

    /// Canvas x of the left edge of page `page` (also defined for indices
    /// outside the strip, for elements that reference missing pages).
    pub fn page_x(&self, page: i64) -> f64 {
        page as f64 * PAGE_WIDTH_LU + page.div_euclid(self.k()) as f64 * self.gap
    }

    /// Canvas rectangle of a page (= its column when grouped).
    pub fn page_rect(&self, page: i64) -> Rect {
        Rect::new(self.page_x(page), 0.0, PAGE_WIDTH_LU, self.page_height)
    }

    /// Export image holding `page` and the column within it.
    pub fn image_and_column(&self, page: i64) -> (i64, i64) {
        (page.div_euclid(self.k()), page.rem_euclid(self.k()))
    }

    /// Number of export images (page groups); the last one may be partial.
    pub fn image_count(&self) -> u32 {
        self.page_count.div_ceil(self.group.max(1))
    }

    /// Pages contained in export image `image` (fewer than `group` for a
    /// partial last image).
    pub fn image_pages(&self, image: u32) -> std::ops::Range<u32> {
        let k = self.group.max(1);
        let start = image * k;
        start..(start + k).min(self.page_count)
    }

    /// Canvas rectangle of export image `image`: always `group` columns
    /// wide, so every image has the target's aspect (a partial last image
    /// has empty columns).
    pub fn image_rect(&self, image: u32) -> Rect {
        let first = i64::from(image * self.group.max(1));
        Rect::new(
            self.page_x(first),
            0.0,
            f64::from(self.group.max(1)) * PAGE_WIDTH_LU,
            self.page_height,
        )
    }

    /// Total canvas width including gaps (up to the right edge of the last
    /// export image).
    pub fn canvas_width(&self) -> f64 {
        match self.image_count() {
            0 => 0.0,
            n => self.image_rect(n - 1).right(),
        }
    }

    /// The whole canvas (pages and gaps).
    pub fn canvas_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.canvas_width(), self.page_height)
    }

    /// Gap rectangles between export images. Content there is in no page
    /// export (but shows in banners with `gaps: as_store`).
    pub fn gap_rects(&self) -> Vec<Rect> {
        if self.gap <= 0.0 {
            return Vec::new();
        }
        (0..self.image_count().saturating_sub(1))
            .map(|i| Rect::new(self.image_rect(i).right(), 0.0, self.gap, self.page_height))
            .collect()
    }

    /// Target page height minus reference page height.
    pub fn delta_height(&self) -> f64 {
        self.page_height - self.reference_page_height
    }

    /// Vertical scale of `stretch` anchored elements.
    pub fn stretch_factor(&self) -> f64 {
        if self.reference_page_height > 0.0 {
            self.page_height / self.reference_page_height
        } else {
            1.0
        }
    }

    /// Adapt a y coordinate (page-relative, designed for the reference page
    /// height) to the target page height.
    pub fn anchor_y(&self, y: f64, anchor: AnchorY) -> f64 {
        match anchor {
            AnchorY::Top => y,
            AnchorY::Bottom => y + self.delta_height(),
            AnchorY::Center => y + self.delta_height() / 2.0,
            AnchorY::Stretch => y * self.stretch_factor(),
        }
    }

    /// Inverse of [`CanvasLayout::anchor_y`].
    pub fn unanchor_y(&self, y: f64, anchor: AnchorY) -> f64 {
        match anchor {
            AnchorY::Top => y,
            AnchorY::Bottom => y - self.delta_height(),
            AnchorY::Center => y - self.delta_height() / 2.0,
            AnchorY::Stretch => y / self.stretch_factor(),
        }
    }

    /// Home-page-relative position → canvas.
    pub fn to_canvas(&self, home_page: i64, p: Vec2, anchor: AnchorY) -> Vec2 {
        Vec2::new(self.page_x(home_page) + p.x, self.anchor_y(p.y, anchor))
    }

    /// Canvas → home-page-relative position (inverse of [`Self::to_canvas`]),
    /// e.g. for dragging in the GUI.
    pub fn from_canvas(&self, home_page: i64, p: Vec2, anchor: AnchorY) -> Vec2 {
        Vec2::new(p.x - self.page_x(home_page), self.unanchor_y(p.y, anchor))
    }
}

/// Maps a canvas region in lu onto a pixel raster.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    /// Canvas x (lu) at pixel column 0.
    pub origin_x: f64,
    /// Canvas y (lu) at pixel row 0.
    pub origin_y: f64,
    /// Pixels per lu.
    pub scale: f64,
    pub width_px: u32,
    pub height_px: u32,
}

impl Viewport {
    /// Shows exactly `region` in `width_px` px (height follows).
    pub fn fit_width(region: Rect, width_px: u32) -> Self {
        let scale = f64::from(width_px) / region.w;
        Self {
            origin_x: region.x,
            origin_y: region.y,
            scale,
            width_px,
            height_px: (region.h * scale).round().max(1.0) as u32,
        }
    }

    /// An export image of exactly `width_px × height_px` showing `region`
    /// (which has the target aspect); the horizontal scale is authoritative.
    pub fn exact(region: Rect, width_px: u32, height_px: u32) -> Self {
        Self {
            origin_x: region.x,
            origin_y: region.y,
            scale: f64::from(width_px) / region.w,
            width_px,
            height_px,
        }
    }

    /// `fit: cover`: the image is completely covered by `region`, the excess
    /// is cropped. `focus` (fractions) selects which part of the excess is
    /// kept: `(0.5, 0.0)` = horizontally centred, top kept.
    pub fn cover(region: Rect, width_px: u32, height_px: u32, focus: Vec2) -> Self {
        let scale = (f64::from(width_px) / region.w).max(f64::from(height_px) / region.h);
        Self::with_scale(region, width_px, height_px, scale, focus)
    }

    /// `fit: contain`: all of `region` is visible, centred; the image area
    /// outside it shows the extended canvas background.
    pub fn contain(region: Rect, width_px: u32, height_px: u32) -> Self {
        let scale = (f64::from(width_px) / region.w).min(f64::from(height_px) / region.h);
        Self::with_scale(region, width_px, height_px, scale, Vec2::new(0.5, 0.5))
    }

    fn with_scale(region: Rect, width_px: u32, height_px: u32, scale: f64, focus: Vec2) -> Self {
        let vis_w = f64::from(width_px) / scale;
        let vis_h = f64::from(height_px) / scale;
        Self {
            origin_x: region.x + (region.w - vis_w) * focus.x,
            origin_y: region.y + (region.h - vis_h) * focus.y,
            scale,
            width_px,
            height_px,
        }
    }

    /// The canvas rectangle (lu) visible in this viewport.
    pub fn visible(&self) -> Rect {
        Rect::new(
            self.origin_x,
            self.origin_y,
            f64::from(self.width_px) / self.scale,
            f64::from(self.height_px) / self.scale,
        )
    }

    /// Canvas lu → pixel.
    pub fn to_px(&self, p: Vec2) -> Vec2 {
        Vec2::new(
            (p.x - self.origin_x) * self.scale,
            (p.y - self.origin_y) * self.scale,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const H: f64 = 1000.0 * 2868.0 / 1320.0;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn page_x_without_and_with_gap() {
        let l = CanvasLayout::new(4, H);
        assert!(close(l.page_x(1), 1000.0));
        assert!(close(l.page_x(3), 3000.0));
        let l = l.with_gap(40.0);
        assert!(close(l.page_x(1), 1040.0));
        assert!(close(l.page_x(3), 3120.0));
        assert!(close(l.canvas_width(), 4120.0));
        assert!(close(l.page_x(-1), -1040.0));
    }

    #[test]
    fn concept_example_320px_pages_20px_gap() {
        // KONZEPT §3.3: 320 px pages, 20 px gap → page 2 starts at x 341 (1-based).
        let l = CanvasLayout::new(3, H).with_gap(20.0 / 0.32);
        assert!(close(l.page_x(1) * 0.32, 340.0));
        assert!(close(l.page_x(2) * 0.32, 680.0));
    }

    #[test]
    fn gap_content_is_not_covered_by_any_image() {
        let l = CanvasLayout::new(4, H).with_gap(40.0);
        let gaps = l.gap_rects();
        assert_eq!(gaps.len(), 3);
        assert!(close(gaps[0].x, 1000.0) && close(gaps[0].w, 40.0));
        let p = Vec2::new(l.page_rect(0).right() + 10.0, 100.0);
        assert!(!(0..l.image_count()).any(|i| l.image_rect(i).contains(p)));
        for g in &gaps {
            for i in 0..l.image_count() {
                assert!(!g.intersects(&l.image_rect(i)));
            }
        }
    }

    #[test]
    fn grouping_maps_pages_to_columns() {
        let l = CanvasLayout::new(4, H)
            .with_gap(60.0)
            .for_target(2752, 2064, 2);
        assert!(close(l.page_height, 1500.0));
        assert_eq!(l.image_count(), 2);
        assert_eq!(l.image_pages(1), 2..4);
        assert!(close(l.page_x(2), 2060.0));
        assert!(close(l.image_rect(0).w, 2000.0));
        let p = l.to_canvas(1, Vec2::new(80.0, 0.0), AnchorY::Top);
        assert!(close(p.x - l.image_rect(0).x, 1080.0));
        assert_eq!(l.gap_rects().len(), 1);
    }

    #[test]
    fn partial_last_image_keeps_target_aspect() {
        let l = CanvasLayout::new(5, H)
            .with_gap(60.0)
            .for_target(2752, 2064, 2);
        assert_eq!(l.image_count(), 3);
        assert_eq!(l.image_pages(2), 4..5);
        assert!(close(l.image_rect(2).w, 2000.0));
        assert!(close(l.canvas_width(), 6000.0 + 120.0));
    }

    #[test]
    fn anchors_adapt_to_page_height() {
        let l = CanvasLayout::new(4, H).for_target(1080, 2160, 1);
        assert!(close(l.page_height, 2000.0));
        assert!(close(l.anchor_y(140.0, AnchorY::Top), 140.0));
        assert!(close(l.anchor_y(H - 300.0, AnchorY::Bottom), 1700.0));
        assert!(close(l.anchor_y(H / 2.0 + 50.0, AnchorY::Center), 1050.0));
        assert!(close(l.anchor_y(H, AnchorY::Stretch), 2000.0));
    }

    #[test]
    fn viewports() {
        let region = Rect::new(0.0, 0.0, 4000.0, H);
        let v = Viewport::cover(region, 2400, 1000, Vec2::new(0.5, 0.0));
        assert!(close(v.scale, 0.6));
        assert!(close(v.visible().w, 4000.0));
        assert!(close(v.visible().y, 0.0));
        let v = Viewport::contain(Rect::new(0.0, 0.0, 1000.0, 1000.0), 2000, 1000);
        assert!(close(v.scale, 1.0));
        assert!(close(v.visible().x, -500.0));
        assert!(close(v.to_px(Vec2::new(0.0, 0.0)).x, 500.0));
    }

    #[test]
    fn rotation_is_clockwise_in_y_down() {
        let p = Vec2::new(1.0, 0.0).rotated_around(Vec2::ZERO, 90.0);
        assert!(close(p.x, 0.0) && close(p.y, 1.0));
    }

    fn anchor() -> impl Strategy<Value = AnchorY> {
        prop_oneof![
            Just(AnchorY::Top),
            Just(AnchorY::Bottom),
            Just(AnchorY::Center),
            Just(AnchorY::Stretch)
        ]
    }

    fn layout() -> impl Strategy<Value = CanvasLayout> {
        (
            1u32..12,
            500.0..4000.0f64,
            0.0..200.0f64,
            1u32..4,
            100u32..4000,
            100u32..4000,
        )
            .prop_map(|(n, h0, gap, k, w, h)| {
                CanvasLayout::new(n, h0).with_gap(gap).for_target(w, h, k)
            })
    }

    proptest! {
        #[test]
        fn anchors_round_trip(l in layout(), page in -2i64..14, x in -3000.0..3000.0f64,
                              y in -3000.0..5000.0f64, a in anchor()) {
            let p = Vec2::new(x, y);
            let back = l.from_canvas(page, l.to_canvas(page, p, a), a);
            prop_assert!((back.x - x).abs() < 1e-6 && (back.y - y).abs() < 1e-6);
        }

        #[test]
        fn home_page_point_is_gap_independent(l in layout(), g2 in 0.0..200.0f64,
                                              page in 0i64..12, x in -1500.0..2500.0f64,
                                              y in 0.0..3000.0f64, a in anchor()) {
            // Relative to the image holding its home page, a point lands at
            // column · 1000 + x whatever the gap; y does not depend on it.
            let l2 = l.with_gap(g2);
            let p = Vec2::new(x, y);
            let (img, col) = l.image_and_column(page);
            for l in [l, l2] {
                let c = l.to_canvas(page, p, a);
                let image_x = l.page_x(img * i64::from(l.group));
                prop_assert!((c.x - image_x - (col as f64 * 1000.0 + x)).abs() < 1e-6);
                prop_assert!((c.y - l.anchor_y(y, a)).abs() < 1e-9);
            }
            let (c1, c2) = (l.to_canvas(page, p, a), l2.to_canvas(page, p, a));
            prop_assert!((c1.y - c2.y).abs() < 1e-9);
        }

        #[test]
        fn grouping_places_page_in_column(l in layout()) {
            for page in 0..l.page_count {
                let k = l.group;
                let img = l.image_rect(page / k);
                let r = l.page_rect(i64::from(page));
                prop_assert!(l.image_pages(page / k).contains(&page));
                prop_assert!((r.x - img.x - f64::from(page % k) * 1000.0).abs() < 1e-6);
                prop_assert!(r.right() <= img.right() + 1e-6);
                prop_assert!((r.h - img.h).abs() < 1e-9);
                for gap in l.gap_rects() {
                    prop_assert!(gap.x >= r.right() - 1e-6 || gap.right() <= r.x + 1e-6);
                }
            }
        }
    }
}
