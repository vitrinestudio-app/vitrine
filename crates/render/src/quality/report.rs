//! Geometry report of a resolved scene for assistants and the GUI
//! (`layout_report`): gallery gap and corner radius, seams, every device's
//! frame and screen in page coordinates with what covers the screen, and
//! pixel-accurate overlaps of texts (and of elements crossing a seam).

// Pixel and geometry code with short names: e(lement), d(evice), t(ext),
// w/h, x/y, k (pixel), c (cell / corner).
#![allow(clippy::many_single_char_names)]

use serde::Serialize;
use vitrine_core::geometry::{Rect, Vec2};
use vitrine_core::model::ElementId;
use vitrine_core::resolve::{ResolvedKind, ResolvedScene};

use super::probe::{Probe, bounds_of, intersection, rotated_quad, view_for};
use super::{Checks, header_band, quad_mask, screen_rect};
use crate::scene::RenderContext;

/// Four corners (top-left, top-right, bottom-right, bottom-left of the
/// unrotated shape), lu relative to the element's home page.
pub type Quad = [Vec2; 4];

#[derive(Debug, Clone, Serialize)]
pub struct SceneReport {
    /// Store gallery of the target (`None` = no gallery profile).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gallery: Option<GalleryReport>,
    /// Borders between export images where the store shows a gap.
    pub seams: Vec<SeamReport>,
    pub devices: Vec<DeviceReport>,
    /// Pixel overlaps: every text with other visible elements (page
    /// backgrounds left out), and elements reaching across a seam with
    /// elements of the other page (`seam`).
    pub overlaps: Vec<OverlapReport>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GalleryReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
    /// Gap between two export images, canvas lu and export px.
    pub gap_lu: f64,
    pub gap_px: f64,
    /// Corner radius the store draws, lu (`x` along the width).
    pub radius_lu: Vec2,
    /// `no` (estimate, not yet measured), `web_css` or `fixed`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub measured: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SeamReport {
    /// Last page before the seam (0-based); the next page follows the gap.
    pub after_page: u32,
    /// Canvas x where the gap starts and its width (lu).
    pub x: f64,
    pub gap_lu: f64,
    /// Offset (lu) from a point on `after_page` to the same canvas point
    /// expressed on the next page: `x_next = x_page - (1000 + gap_lu)`.
    pub next_page_offset: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceReport {
    pub id: ElementId,
    pub page: u32,
    pub model: String,
    /// Degrees, positive = clockwise.
    pub rotation: f64,
    /// Outer box (incl. laptop base / stand), rotated.
    pub frame: Quad,
    pub screen: ScreenReport,
    /// Elements drawn over the screen.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub covered_by: Vec<DeviceCover>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ScreenReport {
    /// Screen box before rotation (page-relative lu); with `rotation`
    /// around the device's pivot this is where the screenshot is drawn.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    /// Corner radius (lu).
    pub radius: f64,
    /// The same box rotated (page-relative corners).
    pub quad: Quad,
    /// Height of the top bar (status bar, app title) the checks keep free,
    /// lu from the screen top.
    pub header: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeviceCover {
    pub id: ElementId,
    pub kind: &'static str,
    /// Share of the screen covered (0..1).
    pub share_of_screen: f64,
    /// Share of the top bar covered (0..1).
    pub share_of_header: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverlapReport {
    /// The text of a text overlap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ElementId>,
    /// The element reaching across a seam (seam overlaps).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub element: Option<ElementId>,
    pub other: ElementId,
    pub other_kind: &'static str,
    /// Overlapping drawn pixels, lu².
    pub area_lu2: f64,
    /// Share of the text's glyph area (text overlaps).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_of_text: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub seam: bool,
}

fn kind_name(k: &ResolvedKind) -> &'static str {
    match k {
        ResolvedKind::Background { .. } => "background",
        ResolvedKind::Device(_) => "device",
        ResolvedKind::Text(_) => "text",
        ResolvedKind::Image(_) => "image",
        ResolvedKind::Shape(_) => "shape",
    }
}

fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn r2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// The geometry report of `scene`; `page` limits devices and overlaps to
/// elements with that home page (overlaps: either side).
#[allow(clippy::too_many_lines)]
pub fn scene_report(ctx: &RenderContext, scene: &ResolvedScene, page: Option<u32>) -> SceneReport {
    let probe = Probe {
        ctx,
        prepared: ctx.prepare(scene),
    };
    let th = vitrine_core::validate::Thresholds::default();
    let cx = Checks::new(&probe, &th);
    let frame = &scene.frame;
    let origin = |p: u32| {
        frame
            .pages
            .get(p as usize)
            .map_or(Vec2::ZERO, |pg| Vec2::new(pg.rect.x, pg.rect.y))
    };
    let rel = |p: u32, v: Vec2| {
        let o = origin(p);
        Vec2::new(r1(v.x - o.x), r1(v.y - o.y))
    };
    let px_per_lu = frame.images.first().map_or(1.0, |i| i.viewport.scale);
    let gallery = frame.gallery.as_ref().map(|g| GalleryReport {
        preset: g.preset.clone(),
        gap_lu: r1(g.gap),
        gap_px: r1(g.gap * px_per_lu),
        radius_lu: Vec2::new(r1(g.radius.x), r1(g.radius.y)),
        measured: g.measured,
    });
    let seams = frame
        .gaps
        .iter()
        .filter_map(|gap| {
            let before = frame
                .pages
                .iter()
                .filter(|p| p.rect.right() <= gap.x + 0.5)
                .max_by(|a, b| a.rect.x.total_cmp(&b.rect.x))?;
            Some(SeamReport {
                after_page: before.index,
                x: r1(gap.x),
                gap_lu: r1(gap.w),
                next_page_offset: r1(-(before.rect.w + gap.w)),
            })
        })
        .collect();
    let wanted = |p: u32| page.is_none_or(|f| f == p);
    let mut devices = Vec::new();
    for (j, e) in scene.elements.iter().enumerate() {
        let ResolvedKind::Device(d) = &e.props else {
            continue;
        };
        if !wanted(e.page) {
            continue;
        }
        let (screen, radius) = screen_rect(e, d);
        let band = header_band(screen);
        let outer = Rect::centered(e.center, e.size.x, e.size.y);
        let squad = rotated_quad(screen, e);
        let o = origin(e.page);
        let mut covered_by = Vec::new();
        if let Some(sbox) = intersection(&bounds_of(&squad), &frame.canvas) {
            let view = view_for(sbox, (160.0 / screen.w.max(1.0)).clamp(0.05, 1.0));
            let n = (view.width_px * view.height_px) as usize;
            let smask = quad_mask(&view, &squad);
            let bmask = quad_mask(&view, &rotated_quad(band, e));
            let (st, bt) = (
                smask.iter().filter(|x| **x).count().max(1),
                bmask.iter().filter(|x| **x).count().max(1),
            );
            for (k, other) in scene.elements.iter().enumerate().skip(j + 1) {
                if matches!(other.props, ResolvedKind::Background { .. })
                    || cx.backdrop[k]
                    || other.opacity <= 0.0
                    || !cx.bounds[k].is_some_and(|b| b.intersects(&sbox))
                {
                    continue;
                }
                let m = probe.mask(&view, k);
                let s = (0..n).filter(|&p| smask[p] && m[p]).count();
                if s == 0 {
                    continue;
                }
                let b = (0..n).filter(|&p| bmask[p] && m[p]).count();
                covered_by.push(DeviceCover {
                    id: other.id.clone(),
                    kind: kind_name(&other.props),
                    share_of_screen: r2(s as f64 / st as f64),
                    share_of_header: r2(b as f64 / bt as f64),
                });
            }
        }
        devices.push(DeviceReport {
            id: e.id.clone(),
            page: e.page,
            model: d.model.clone(),
            rotation: r1(e.rotation),
            frame: rotated_quad(outer, e).map(|v| rel(e.page, v)),
            screen: ScreenReport {
                x: r1(screen.x - o.x),
                y: r1(screen.y - o.y),
                w: r1(screen.w),
                h: r1(screen.h),
                radius: r1(radius),
                quad: squad.map(|v| rel(e.page, v)),
                header: r1(band.h),
            },
            covered_by,
        });
    }
    // Overlaps.
    let mut overlaps = Vec::new();
    let els = &scene.elements;
    let usable = |k: usize| {
        !matches!(els[k].props, ResolvedKind::Background { .. })
            && !cx.backdrop[k]
            && els[k].opacity > 0.0
            && cx.bounds[k].is_some()
    };
    for a in 0..els.len() {
        if !usable(a) {
            continue;
        }
        let a_text = matches!(els[a].props, ResolvedKind::Text(_));
        let ba = cx.bounds[a].expect("usable");
        for b in 0..els.len() {
            if a == b || !usable(b) {
                continue;
            }
            let b_text = matches!(els[b].props, ResolvedKind::Text(_));
            // Text pairs once; non-text pairs only across a seam.
            let seam = !a_text && !b_text;
            if b_text && (!a_text || b < a) {
                continue;
            }
            if seam && els[a].page == els[b].page {
                continue;
            }
            if !(wanted(els[a].page) || wanted(els[b].page)) {
                continue;
            }
            let mut region = intersection(&ba, &cx.bounds[b].expect("usable"));
            if seam {
                region = region.and_then(|r| {
                    frame
                        .pages
                        .get(els[b].page as usize)
                        .and_then(|p| intersection(&r, &p.rect))
                });
            }
            let Some(region) = region.filter(|r| r.w * r.h > 1.0) else {
                continue;
            };
            let view = view_for(region, (32.0 / region.w.min(region.h)).clamp(0.25, 1.0));
            let ma = probe.mask(&view, a);
            let mb = probe.mask(&view, b);
            let px = ma.iter().zip(&mb).filter(|(x, y)| **x && **y).count();
            let area = px as f64 / (view.scale * view.scale);
            if area < if seam { 100.0 } else { 1.0 } {
                continue;
            }
            let share_of_text = a_text.then(|| {
                let full = view_for(ba, (32.0 / ba.w.min(ba.h)).clamp(0.25, 1.0));
                let glyphs = probe.mask(&full, a).iter().filter(|x| **x).count() as f64
                    / (full.scale * full.scale);
                if glyphs > 0.0 {
                    r2((area / glyphs).min(1.0))
                } else {
                    0.0
                }
            });
            overlaps.push(OverlapReport {
                text: a_text.then(|| els[a].id.clone()),
                element: seam.then(|| els[a].id.clone()),
                other: els[b].id.clone(),
                other_kind: kind_name(&els[b].props),
                area_lu2: area.round(),
                share_of_text,
                seam,
            });
        }
    }
    SceneReport {
        gallery,
        seams,
        devices,
        overlaps,
    }
}
