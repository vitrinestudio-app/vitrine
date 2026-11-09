//! Page → canvas mapping: home pages, gaps, page grouping and vertical anchors.
//!
//! This is the core of the canvas model (`docs/KONZEPT.md` §2, §3.2–3.4):
//!
//! * A page is always [`PAGE_W`] = 1000 lu wide. Its height depends on the
//!   *target* (`page_height`), not on the canvas design height.
//! * Pages are laid out left to right. Between two **export images** there is
//!   a physical gap of `gap` lu (store gallery gap). With page grouping
//!   (`group: k`) the k pages of one export image are adjacent columns without
//!   gaps; gaps only appear between groups:
//!
//!   `page_x(i) = i · 1000 + ⌊i / k⌋ · gap`
//!
//! * Elements store their position relative to their home page's top-left
//!   corner. `canvas = (page_x(home) + x, anchor_y(y))`. Because only the
//!   home page origin moves when the gap or grouping changes, an element that
//!   reaches into the neighbouring page keeps lining up with the store
//!   gallery; the part that falls into a gap is simply not covered by any
//!   export image.
//! * Coordinates are y-down; rotation is in degrees, **positive = clockwise**
//!   (CSS convention), so `-8°` tilts an element counter-clockwise.

#![allow(
    clippy::cast_possible_wrap,
    reason = "page indices are tiny; usize -> i64 cannot wrap in practice"
)]

/// Width of one page in logical units (lu).
pub const PAGE_W: f64 = 1000.0;

/// Vertical anchor of an element (`anchor_y`, `docs/KONZEPT.md` §3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnchorY {
    /// Distance to the top edge is preserved (headlines).
    #[default]
    Top,
    /// Distance to the bottom edge is preserved (devices running out of the image).
    Bottom,
    /// Offset from the vertical centre is preserved.
    Center,
    /// Position (and height) scale with the page height (backgrounds).
    Stretch,
}

/// Axis-aligned rectangle in lu (or px, depending on context).
#[derive(Debug, Clone, Copy, PartialEq)]
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

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }
}

/// How the pages of a strip are arranged on the canvas for one output target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripLayout {
    /// Number of pages of the strip.
    pub page_count: usize,
    /// Page height the layout was designed for (from the canvas aspect), in lu.
    pub design_page_height: f64,
    /// Page (column) height for the current target, in lu.
    pub page_height: f64,
    /// Gap between export images in lu (`0` = no gaps).
    pub gap: f64,
    /// Pages per export image (`group: k`, `1` = one page per image).
    pub group: usize,
}

impl StripLayout {
    /// Layout for a target with the same aspect as the canvas design.
    pub fn new(page_count: usize, design_page_height: f64, gap: f64) -> Self {
        Self {
            page_count,
            design_page_height,
            page_height: design_page_height,
            gap,
            group: 1,
        }
    }

    /// Derives the page height from a target pixel size.
    ///
    /// With `group = k` an export image of `w × h` px contains k columns of
    /// 1000 lu, so `page_height = k · 1000 · h / w`.
    #[must_use]
    pub fn for_target(mut self, width_px: u32, height_px: u32, group: usize) -> Self {
        let group = group.max(1);
        self.group = group;
        self.page_height = group as f64 * PAGE_W * f64::from(height_px) / f64::from(width_px);
        self
    }

    #[must_use]
    pub fn with_gap(mut self, gap: f64) -> Self {
        self.gap = gap;
        self
    }

    /// Canvas x of the left edge of page `page` (may be negative or beyond the
    /// last page for elements that reference out-of-range pages).
    pub fn page_x(&self, page: i64) -> f64 {
        let group = self.group.max(1) as i64;
        page as f64 * PAGE_W + page.div_euclid(group) as f64 * self.gap
    }

    /// Rectangle of a page on the canvas.
    pub fn page_rect(&self, page: usize) -> Rect {
        Rect::new(self.page_x(page as i64), 0.0, PAGE_W, self.page_height)
    }

    /// Total canvas width including gaps.
    pub fn canvas_width(&self) -> f64 {
        if self.page_count == 0 {
            return 0.0;
        }
        self.page_x(self.page_count as i64 - 1) + PAGE_W
    }

    /// Whole canvas rectangle (pages and gaps).
    pub fn canvas_rect(&self) -> Rect {
        Rect::new(0.0, 0.0, self.canvas_width(), self.page_height)
    }

    /// Number of export images (page groups).
    pub fn image_count(&self) -> usize {
        self.page_count.div_ceil(self.group.max(1))
    }

    /// Pages contained in export image `image`.
    pub fn image_pages(&self, image: usize) -> std::ops::Range<usize> {
        let start = image * self.group;
        start..(start + self.group).min(self.page_count)
    }

    /// Canvas rectangle covered by export image `image` (its columns, no gaps).
    pub fn image_rect(&self, image: usize) -> Rect {
        let pages = self.image_pages(image);
        let x = self.page_x(pages.start as i64);
        Rect::new(x, 0.0, pages.len() as f64 * PAGE_W, self.page_height)
    }

    /// Gap rectangles between export images (content there is never exported
    /// as a page, but shows in banners with `gaps: as_store`).
    pub fn gap_rects(&self) -> Vec<Rect> {
        if self.gap <= 0.0 {
            return Vec::new();
        }
        (0..self.image_count().saturating_sub(1))
            .map(|i| {
                let r = self.image_rect(i);
                Rect::new(r.right(), 0.0, self.gap, self.page_height)
            })
            .collect()
    }

    /// Height difference between the target page and the design page.
    pub fn delta_height(&self) -> f64 {
        self.page_height - self.design_page_height
    }

    /// Adjusts a y coordinate (relative to the page top, designed for
    /// `design_page_height`) to the current page height.
    pub fn anchor_y(&self, y: f64, anchor: AnchorY) -> f64 {
        match anchor {
            AnchorY::Top => y,
            AnchorY::Bottom => y + self.delta_height(),
            AnchorY::Center => y + self.delta_height() / 2.0,
            AnchorY::Stretch => y * self.stretch_factor(),
        }
    }

    /// Vertical scale for `stretch` anchored elements.
    pub fn stretch_factor(&self) -> f64 {
        if self.design_page_height > 0.0 {
            self.page_height / self.design_page_height
        } else {
            1.0
        }
    }

    /// Maps a home-page-relative position to canvas coordinates.
    pub fn to_canvas(&self, home_page: i64, x: f64, y: f64, anchor: AnchorY) -> (f64, f64) {
        (self.page_x(home_page) + x, self.anchor_y(y, anchor))
    }
}

/// Maps a canvas region in lu onto a pixel raster.
#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Viewport that shows exactly `region` in `width_px` px (height follows).
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

    /// Viewport for an export image of exactly `width_px × height_px` showing `region`.
    ///
    /// The region is expected to have the target aspect (page exports). The
    /// horizontal scale is authoritative.
    pub fn exact(region: Rect, width_px: u32, height_px: u32) -> Self {
        Self {
            origin_x: region.x,
            origin_y: region.y,
            scale: f64::from(width_px) / region.w,
            width_px,
            height_px,
        }
    }

    /// `fit: cover`: scale so that the target is completely covered by the
    /// region, cropping the excess. `focus` (0..1, 0..1) selects which part of
    /// the excess is kept (0.5 = centred).
    pub fn cover(region: Rect, width_px: u32, height_px: u32, focus: (f64, f64)) -> Self {
        let scale = (f64::from(width_px) / region.w).max(f64::from(height_px) / region.h);
        let vis_w = f64::from(width_px) / scale;
        let vis_h = f64::from(height_px) / scale;
        Self {
            origin_x: region.x + (region.w - vis_w) * focus.0,
            origin_y: region.y + (region.h - vis_h) * focus.1,
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

    /// Canvas lu → pixel transform.
    pub fn transform(&self) -> tiny_skia::Transform {
        let s = self.scale as f32;
        tiny_skia::Transform::from_row(
            s,
            0.0,
            0.0,
            s,
            (-self.origin_x * self.scale) as f32,
            (-self.origin_y * self.scale) as f32,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: f64 = 1000.0 * 2868.0 / 1320.0;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-6
    }

    #[test]
    fn page_x_without_and_with_gap() {
        let l = StripLayout::new(4, H, 0.0);
        assert!(close(l.page_x(0), 0.0));
        assert!(close(l.page_x(1), 1000.0));
        assert!(close(l.page_x(3), 3000.0));
        let l = l.with_gap(40.0);
        assert!(close(l.page_x(1), 1040.0));
        assert!(close(l.page_x(3), 3120.0));
        assert!(close(l.canvas_width(), 4120.0));
    }

    #[test]
    fn concept_example_320px_pages_20px_gap() {
        // KONZEPT §3.3: 320 px pages, 20 px gap → page 2 starts at x 341 (1-based)
        let l = StripLayout::new(3, H, 20.0 / 0.32);
        let s = 0.32; // px per lu
        assert!(close(l.page_x(1) * s, 340.0));
        assert!(close(l.page_x(2) * s, 680.0));
    }

    #[test]
    fn home_page_position_moves_with_gap() {
        // Phone centre on the right edge of page 0 stays on that edge.
        let l = StripLayout::new(4, H, 40.0);
        let (x, _) = l.to_canvas(0, 1000.0, 1700.0, AnchorY::Bottom);
        assert!(close(x, l.page_rect(0).right()));
        // An element on page 2 at x=0 sits on page 2's left edge, after two gaps.
        let (x, _) = l.to_canvas(2, 0.0, 0.0, AnchorY::Top);
        assert!(close(x, 2080.0));
    }

    #[test]
    fn gap_content_is_not_covered_by_any_image() {
        let l = StripLayout::new(4, H, 40.0);
        let gaps = l.gap_rects();
        assert_eq!(gaps.len(), 3);
        assert!(close(gaps[0].x, 1000.0) && close(gaps[0].w, 40.0));
        for g in &gaps {
            for i in 0..l.image_count() {
                let r = l.image_rect(i);
                // touching edges are fine, overlap is not
                assert!(!(g.x < r.right() - 1e-9 && r.x < g.right() - 1e-9));
            }
        }
        // A point 10 lu right of page 0 lies in the first gap.
        let p = l.page_rect(0).right() + 10.0;
        assert!(!(0..4).any(|i| {
            let r = l.image_rect(i);
            p >= r.x && p < r.right()
        }));
        // Without gap the same point is on page 1 at x = 10.
        let l0 = l.with_gap(0.0);
        let r1 = l0.image_rect(1);
        assert!(close(p - r1.x, 10.0));
    }

    #[test]
    fn grouping_maps_pages_to_columns() {
        let l = StripLayout::new(4, H, 60.0).for_target(2752, 2064, 2);
        assert!(close(l.page_height, 1500.0));
        assert_eq!(l.image_count(), 2);
        assert_eq!(l.image_pages(0), 0..2);
        assert_eq!(l.image_pages(1), 2..4);
        // Columns of one image are adjacent, gap only between images.
        assert!(close(l.page_x(1), 1000.0));
        assert!(close(l.page_x(2), 2060.0));
        assert!(close(l.page_x(3), 3060.0));
        let img0 = l.image_rect(0);
        assert!(close(img0.w, 2000.0));
        // Home page 1 = right column of image 0; margins relative to column.
        let (x, _) = l.to_canvas(1, 80.0, 0.0, AnchorY::Top);
        assert!(close(x - img0.x, 1080.0));
        assert_eq!(l.gap_rects().len(), 1);
        assert!(close(l.gap_rects()[0].x, 2000.0));
    }

    #[test]
    fn anchors_adapt_to_page_height() {
        let l = StripLayout::new(4, H, 0.0).for_target(1080, 2160, 1);
        assert!(close(l.page_height, 2000.0));
        let d = 2000.0 - H;
        assert!(close(l.anchor_y(140.0, AnchorY::Top), 140.0));
        assert!(close(
            l.anchor_y(H - 300.0, AnchorY::Bottom),
            2000.0 - 300.0
        ));
        assert!(close(
            l.anchor_y(H / 2.0 + 50.0, AnchorY::Center),
            1000.0 + 50.0
        ));
        assert!(close(l.anchor_y(H, AnchorY::Stretch), 2000.0));
        assert!(close(l.anchor_y(H / 2.0, AnchorY::Stretch), 1000.0));
        assert!(close(l.delta_height(), d));
    }

    #[test]
    fn cover_viewport_crops_excess() {
        let region = Rect::new(0.0, 0.0, 4000.0, H);
        let v = Viewport::cover(region, 2400, 1000, (0.5, 0.0));
        assert!(close(v.scale, 0.6));
        let vis = v.visible();
        assert!(close(vis.w, 4000.0));
        assert!(close(vis.y, 0.0));
        assert!(vis.h < H);
    }
}
