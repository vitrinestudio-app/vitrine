//! Target frame: page height, gaps, output images.

use super::Diags;
use super::scene::{DiagnosticCode, Frame, GalleryInfo, OutputImage, PageFrame};
use crate::catalog::Catalog;
use crate::geometry::{CanvasLayout, Rect, Vec2, Viewport};
use crate::model::{BannerFit, Gaps, PixelSize, Region, Strip, Target, TargetKind};

/// Focus of `fit: cover` banners: horizontally centred, top kept (the
/// headlines live there; excess height is cropped from the bottom).
pub const BANNER_COVER_FOCUS: Vec2 = Vec2::new(0.5, 0.0);

pub(super) fn frame(target: &Target, strip: &Strip, catalog: &Catalog, d: &mut Diags) -> Frame {
    let count = strip.page.count;
    let reference = strip.page.aspect.page_height();
    match target.kind {
        TargetKind::Pages => pages_frame(target, strip, catalog, count, reference, d),
        TargetKind::Banner => banner_frame(target, strip, catalog, count, reference, d),
    }
}

fn pages_frame(
    target: &Target,
    strip: &Strip,
    catalog: &Catalog,
    count: u32,
    reference: f64,
    d: &mut Diags,
) -> Frame {
    let group = target.group_size();
    let gallery = gallery(target, catalog, target.size, group, d);
    let layout = CanvasLayout::new(count, reference)
        .for_target(target.size.w, target.size.h, group)
        .with_gap(gallery.as_ref().map_or(0.0, |g| g.gap));
    if !count.is_multiple_of(group) {
        d.info(
            DiagnosticCode::PartialImage,
            format!(
                "{count} pages with `group: {group}`: the last image has {} empty column(s)",
                group - count % group
            ),
        );
    }
    let images = (0..layout.image_count())
        .map(|i| {
            let source = layout.image_rect(i);
            let pages: Vec<u32> = layout.image_pages(i).collect();
            OutputImage {
                index: i,
                columns: pages
                    .iter()
                    .map(|&p| layout.page_rect(i64::from(p)))
                    .collect(),
                pages,
                source,
                viewport: Viewport::exact(source, target.size.w, target.size.h),
            }
        })
        .collect();
    let pages = (0..count)
        .map(|p| PageFrame {
            index: p,
            id: strip.page.page_id(p),
            rect: layout.page_rect(i64::from(p)),
            image: Some(p / group),
            column: p % group,
        })
        .collect();
    Frame {
        kind: TargetKind::Pages,
        size: target.size,
        layout,
        canvas: layout.canvas_rect(),
        pages,
        images,
        gaps: layout.gap_rects(),
        gallery,
    }
}

fn banner_frame(
    target: &Target,
    strip: &Strip,
    catalog: &Catalog,
    count: u32,
    reference: f64,
    d: &mut Diags,
) -> Frame {
    // Gap of the store gallery for pages in the strip's own aspect.
    let gallery = match target.gaps.unwrap_or(Gaps::None) {
        Gaps::None => None,
        Gaps::AsStore => {
            let page_px = PixelSize::new(strip.page.aspect.w, strip.page.aspect.h);
            let g = gallery(target, catalog, page_px, 1, d);
            if g.is_none() {
                d.warn(
                    DiagnosticCode::NoGallery,
                    "`gaps: as_store` without `gallery`: no gaps".into(),
                );
            }
            g
        }
    };
    let layout =
        CanvasLayout::new(count, reference).with_gap(gallery.as_ref().map_or(0.0, |g| g.gap));
    let last = count.saturating_sub(1);
    let region = match target.region {
        None => layout.canvas_rect(),
        Some(Region::Pages([a, b])) => {
            if b > last {
                d.warn(
                    DiagnosticCode::RegionOutOfRange,
                    format!("region pages [{a}, {b}] exceed the strip ({count} pages)"),
                );
            }
            let (a, b) = (a.min(last), b.min(last));
            let x = layout.page_x(i64::from(a));
            Rect::new(
                x,
                0.0,
                layout.page_rect(i64::from(b)).right() - x,
                layout.page_height,
            )
        }
        Some(Region::Rect(r)) => {
            if r.page > last {
                d.warn(
                    DiagnosticCode::RegionOutOfRange,
                    format!(
                        "region page {} is outside the strip ({count} pages)",
                        r.page
                    ),
                );
            }
            Rect::new(
                layout.page_x(i64::from(r.page)) + r.x,
                r.y,
                r.width,
                r.height,
            )
        }
    };
    let viewport = match target.fit.unwrap_or(BannerFit::Cover) {
        BannerFit::Cover => {
            Viewport::cover(region, target.size.w, target.size.h, BANNER_COVER_FOCUS)
        }
        BannerFit::Contain => Viewport::contain(region, target.size.w, target.size.h),
    };
    let visible = viewport.visible();
    let shown: Vec<u32> = (0..count)
        .filter(|&p| layout.page_rect(i64::from(p)).intersects(&visible))
        .collect();
    let image = OutputImage {
        index: 0,
        columns: shown
            .iter()
            .map(|&p| layout.page_rect(i64::from(p)))
            .collect(),
        pages: shown,
        source: visible,
        viewport,
    };
    let pages = (0..count)
        .map(|p| PageFrame {
            index: p,
            id: strip.page.page_id(p),
            rect: layout.page_rect(i64::from(p)),
            image: None,
            column: 0,
        })
        .collect();
    Frame {
        kind: TargetKind::Banner,
        size: target.size,
        layout,
        canvas: layout.canvas_rect(),
        pages,
        images: vec![image],
        gaps: layout.gap_rects(),
        gallery,
    }
}

/// Gap and corner radius of the target's gallery in canvas lu for images of
/// `image` px holding `group` pages. Unknown presets fall back to the
/// overrides (width basis).
fn gallery(
    target: &Target,
    catalog: &Catalog,
    image: PixelSize,
    group: u32,
    d: &mut Diags,
) -> Option<GalleryInfo> {
    let gref = target.gallery.as_ref()?;
    let profile = gref.preset.as_deref().and_then(|p| {
        let found = catalog.gallery(p);
        if found.is_none() {
            d.warn(
                DiagnosticCode::UnknownGallery,
                format!("unknown gallery preset `{p}`"),
            );
        }
        found
    });
    let (gap, radius) = gref.resolve(profile);
    let factor = profile.map_or(f64::from(group.max(1)), |p| {
        p.page_lu_per_unit(image, group)
    });
    Some(GalleryInfo {
        preset: gref.preset.clone(),
        gap: gap * factor,
        radius: Vec2::new(radius.x * factor, radius.y * factor),
        background: profile.map(|p| p.background),
    })
}
