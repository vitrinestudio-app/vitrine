//! Design checks that need rendered geometry or pixels (ADR 0029):
//! `core::validate` calls them through [`vitrine_core::validate::SceneInspector`]
//! when a front end passes an [`Inspector`]; [`scene_report`] gives the same
//! geometry to `layout_report` (MCP) and the GUI.
//!
//! Everything works on small partial renders of the prepared scene
//! ([`probe`]): an element drawn alone and bare (no shadow, glow, outline)
//! is its pixel mask, the scene painted up to a text is what the text sits
//! on. Rasters are coarse (0.25–1.5 px per lu) and cover only the region a
//! check needs, so a target × language costs a few dozen small renders.
//!
//! | code | what |
//! |---|---|
//! | `text_near_device` | glyphs cross, hide behind or come closer than `device_distance` to a device |
//! | `low_contrast` | WCAG contrast of the glyphs against their surroundings, per 1 em cell |
//! | `screen_header_covered` | an element above a device covers the top bar of its screen |
//! | `screen_header_cut` | a page or image edge (or the clip) cuts a screen through its top bar |
//! | `screen_covered` | an element above a device covers ≥ `screen_cover` of the screen below the top bar |
//! | `device_cut` | a side page edge (or the outer canvas edge) cuts more than 8 lu off a device |
//! | `seam_overlap` | an element reaching into the next page overlaps one of that page |
//! | `element_in_corner` | an element reaches into a rounded corner of a store image |
//! | `page_blends_with_store` | image edges close to the store's light / dark background |

// Pixel and geometry code with short names: e(lement), d(evice), t(ext),
// w/h, x/y, k (pixel), c (cell / corner).
#![allow(clippy::many_single_char_names)]

mod probe;
mod report;

use vitrine_core::geometry::{Rect, Vec2, Viewport};
use vitrine_core::model::TargetKind;
use vitrine_core::resolve::{ResolvedDevice, ResolvedElement, ResolvedKind, ResolvedScene};
use vitrine_core::validate::{
    DEVICE_CUT_MIN, FindingCode, SceneFinding, SceneInspector, Severity, Thresholds,
};

pub use report::{
    DeviceCover, DeviceReport, OverlapReport, Quad, SceneReport, ScreenReport, scene_report,
};

use probe::{
    Draw, Probe, contrast, distance_map, intersection, luminance, outset, point, quantile,
    rotated_bounds, rotated_quad, view_for,
};

use tiny_skia::Pixmap;

use crate::frames;
use crate::scene::{Quality, RenderContext, RenderOptions, Scale};

/// Share of a page an image or shape must cover to count as a page
/// background (left out of overlap checks).
pub const BACKDROP_SHARE: f64 = 0.9;

/// Top bar of a portrait phone screen (status bar plus navigation title)
/// as a share of the screen height; tablets and landscape screens use
/// [`HEADER_SHARE_WIDE`].
pub const HEADER_SHARE: f64 = 0.14;

/// Top bar share of tablet / landscape screens.
pub const HEADER_SHARE_WIDE: f64 = 0.09;

/// Share of the top bar an element must cover for `screen_header_covered`.
const HEADER_COVER_MIN: f64 = 0.25;

/// Share of a corner square an element may fill before it counts as
/// rounded with the page (`element_in_corner`).
const CORNER_FULL: f64 = 0.9;

/// Contrast below which an image edge "blends" with the store background.
pub const BLEND_CONTRAST: f64 = 1.3;

/// Share of an image's edge pixels that must blend for
/// `page_blends_with_store`.
const BLEND_SHARE: f64 = 0.6;

/// Overlap (lu²) from which `seam_overlap` reports (≈ 24 × 24 lu).
const SEAM_MIN_AREA: f64 = 576.0;

/// Raster scale (px per lu) of all probes around texts. One fixed scale
/// lets the asset cache reuse resampled pictures between probes (a photo
/// resampled once per scene instead of once per text); 0.5 px per lu
/// keeps the glyphs of 20 lu text ≥ 10 px high, enough for the exact fill
/// colour and ±2 lu distances.
const PROBE_SCALE: f64 = 0.5;

/// Quantile of the per-pixel contrast around the glyphs of a cell that
/// counts: low enough to catch a bright spot behind a word, high enough
/// to ignore thin lines (grids, hairlines) next to the glyphs.
const RING_QUANTILE: f64 = 0.3;

/// Pixel-based design checks; see the module docs.
#[derive(Debug)]
pub struct Inspector<'a> {
    ctx: &'a RenderContext,
}

impl<'a> Inspector<'a> {
    pub fn new(ctx: &'a RenderContext) -> Self {
        Self { ctx }
    }
}

impl SceneInspector for Inspector<'_> {
    fn inspect(
        &self,
        scene: &ResolvedScene,
        thresholds: &Thresholds,
        first: bool,
    ) -> Vec<SceneFinding> {
        let probe = Probe {
            ctx: self.ctx,
            prepared: self.ctx.prepare(scene),
        };
        let cx = Checks::new(&probe, thresholds);
        let mut out = Vec::new();
        for (i, e) in scene.elements.iter().enumerate() {
            if matches!(e.props, ResolvedKind::Text(_)) && e.opacity > 0.0 {
                cx.text(i, e, &mut out);
            }
        }
        if first {
            cx.screens(&mut out);
            cx.device_cuts(&mut out);
            cx.corners(&mut out);
            cx.blends(&mut out);
        }
        cx.seams(first, &mut out);
        out
    }
}

/// Colour (0..=1) of a premultiplied pixel shown over white.
fn rgb(p: tiny_skia::PremultipliedColorU8) -> [f64; 3] {
    let a = 255 - u16::from(p.alpha());
    let c = |v: u8| f64::from((u16::from(v) + a).min(255)) / 255.0;
    [c(p.red()), c(p.green()), c(p.blue())]
}

fn allows(e: &ResolvedElement, code: FindingCode) -> bool {
    e.allow.iter().any(|a| a == code.as_str())
}

/// Shared state of one inspection.
struct Checks<'p, 'c, 's> {
    probe: &'p Probe<'c, 's>,
    th: &'p Thresholds,
    /// Canvas rects that end up in an export (images).
    shown: Vec<Rect>,
    /// Per element: page background (left out of overlap checks).
    backdrop: Vec<bool>,
    bounds: Vec<Option<Rect>>,
}

impl<'p, 'c, 's> Checks<'p, 'c, 's> {
    fn new(probe: &'p Probe<'c, 's>, th: &'p Thresholds) -> Self {
        let scene = probe.scene();
        let bounds: Vec<Option<Rect>> =
            (0..scene.elements.len()).map(|i| probe.bounds(i)).collect();
        let backdrop = scene
            .elements
            .iter()
            .zip(&bounds)
            .map(|(e, b)| is_backdrop(scene, e, b.as_ref()))
            .collect();
        Self {
            probe,
            th,
            shown: scene.frame.images.iter().map(|i| i.source).collect(),
            backdrop,
            bounds,
        }
    }

    fn scene(&self) -> &ResolvedScene {
        self.probe.scene()
    }

    fn shown(&self, p: Vec2) -> bool {
        self.shown.iter().any(|r| r.contains(p))
    }

    /// Page-relative position for messages.
    fn on_page(&self, page: u32, p: Vec2) -> (f64, f64) {
        self.scene()
            .frame
            .pages
            .get(page as usize)
            .map_or((p.x, p.y), |pg| (p.x - pg.rect.x, p.y - pg.rect.y))
    }

    // ------------------------------------------------------------ texts

    /// `low_contrast` and `text_near_device` of text element `i`.
    #[allow(clippy::too_many_lines)]
    fn text(&self, i: usize, e: &ResolvedElement, out: &mut Vec<SceneFinding>) {
        let ResolvedKind::Text(t) = &e.props else {
            return;
        };
        let Some(placed) = self.probe.prepared.text(i) else {
            return;
        };
        let want_contrast = !allows(e, FindingCode::LowContrast) && e.opacity >= 0.5;
        // Giant display type (≥ 3 × the large-text size) is decoration that
        // designs layer with devices on purpose.
        let want_near = !allows(e, FindingCode::TextNearDevice)
            && f64::from(placed.layout.font_size) < 3.0 * self.th.large_text;
        let fs = f64::from(placed.layout.font_size).max(1.0);
        let ring_lu = (0.15 * fs).max(3.0);
        let reach = self.th.device_distance + 4.0;
        let Some(ink) = self.bounds[i] else { return };
        let devices: Vec<usize> = if want_near {
            self.scene()
                .elements
                .iter()
                .enumerate()
                .filter(|(j, d)| {
                    matches!(d.props, ResolvedKind::Device(_))
                        && d.opacity > 0.0
                        && self.bounds[*j].is_some_and(|b| b.intersects(&outset(ink, reach)))
                })
                .map(|(j, _)| j)
                .collect()
        } else {
            Vec::new()
        };
        if !want_contrast && devices.is_empty() {
            return;
        }
        let margin = if devices.is_empty() {
            ring_lu
        } else {
            ring_lu.max(reach)
        };
        let Some(region) = intersection(&outset(ink, margin + 2.0), &self.scene().frame.canvas)
        else {
            return;
        };
        let view = view_for(region, PROBE_SCALE);
        let (w, h) = (view.width_px as usize, view.height_px as usize);
        let fill = self
            .probe
            .paint(&view, false, |j, _| (j == i).then_some(Draw::Bare));
        let alpha: Vec<u8> = fill.pixels().iter().map(|p| p.alpha()).collect();
        let shown: Vec<bool> = (0..w * h).map(|k| self.shown(point(&view, k))).collect();
        let glyph: Vec<bool> = alpha
            .iter()
            .zip(&shown)
            .map(|(&a, &s)| a >= 128 && s)
            .collect();
        let glyphs = glyph.iter().filter(|g| **g).count();
        if glyphs == 0 {
            return;
        }
        let key = t.key.as_str();
        // ---- text near / on a device
        for &j in &devices {
            let d = &self.scene().elements[j];
            let mut dmask = self.probe.mask(&view, j);
            // A text on a card (or any element) lying over the device sits
            // on that card: glyphs and device parts under such carriers
            // do not count.
            let mut glyph = glyph.clone();
            if j < i {
                for k in j + 1..i {
                    let o = &self.scene().elements[k];
                    if matches!(
                        o.props,
                        ResolvedKind::Background { .. } | ResolvedKind::Text(_)
                    ) || self.backdrop[k]
                        || o.opacity < 0.5
                        || !self.bounds[k].is_some_and(|b| b.intersects(&region))
                    {
                        continue;
                    }
                    let carrier = self.probe.mask(&view, k);
                    for (p, c) in carrier.iter().enumerate() {
                        if *c {
                            glyph[p] = false;
                            dmask[p] = false;
                        }
                    }
                }
            }
            let glyphs = glyph.iter().filter(|g| **g).count();
            if glyphs == 0 {
                continue;
            }
            let over = glyph.iter().zip(&dmask).filter(|(g, m)| **g && **m).count();
            let message = if over > 0 {
                let share = over as f64 / glyphs as f64;
                if j > i {
                    format!(
                        "text `{key}` is partly hidden behind device `{}` ({:.0} % of its glyphs); \
                         move the text or the device, or raise the text's z",
                        d.id,
                        share * 100.0
                    )
                } else if share < 0.01 {
                    format!(
                        "text `{key}` touches the frame of device `{}`; keep at least {:.0} lu \
                         between text and frame",
                        d.id, self.th.device_distance
                    )
                } else if share < 0.97 {
                    format!(
                        "text `{key}` runs over the edge of device `{}` ({:.0} % of its glyphs on \
                         the device); keep at least {:.0} lu between text and frame",
                        d.id,
                        share * 100.0,
                        self.th.device_distance
                    )
                } else {
                    continue; // placed on the device on purpose
                }
            } else {
                let dist = distance_map(&dmask, w, h);
                let min = glyph
                    .iter()
                    .zip(&dist)
                    .filter(|(g, _)| **g)
                    .map(|(_, d)| f64::from(*d))
                    .fold(f64::INFINITY, f64::min)
                    / view.scale;
                if min >= self.th.device_distance {
                    continue;
                }
                out.push(SceneFinding {
                    level: Severity::Info,
                    code: FindingCode::TextNearDevice,
                    element: Some(e.id.clone()),
                    lang_dependent: true,
                    message: format!(
                        "text `{key}` comes within {min:.0} lu of device `{}` (keep at least \
                         {:.0} lu; `checks.device_distance`)",
                        d.id, self.th.device_distance
                    ),
                });
                continue;
            };
            out.push(SceneFinding {
                level: Severity::Warn,
                code: FindingCode::TextNearDevice,
                element: Some(e.id.clone()),
                lang_dependent: true,
                message,
            });
        }
        // ---- contrast against what is behind the glyphs
        if !want_contrast {
            return;
        }
        let comp = self
            .probe
            .paint(&view, true, |j, _| (j <= i).then_some(Draw::Full));
        let px = comp.pixels();
        let fpx = fill.pixels();
        let solid: Vec<bool> = alpha.iter().map(|&a| a >= 32).collect();
        let dist = distance_map(&solid, w, h);
        let ring_px = (ring_lu * view.scale).max(1.5) as f32;
        let clip = e.clip;
        let op = e.opacity.clamp(0.0, 1.0);
        let cell = (fs * view.scale).max(4.0) as usize;
        let (cw, ch) = (w.div_ceil(cell), h.div_ceil(cell));
        // Per cell: the glyph colour (un-premultiplied fill, exact) and
        // what shows right around the glyphs (everything painted up to the
        // text, including its own outline / glow / shadow and highlights).
        let mut inner: Vec<Vec<[f64; 3]>> = vec![Vec::new(); cw * ch];
        // Ring pixel indices per cell.
        let mut ring: Vec<Vec<usize>> = vec![Vec::new(); cw * ch];
        for k in 0..w * h {
            let c = (k / w / cell) * cw + (k % w) / cell;
            if alpha[k] >= 128 && shown[k] {
                let a = f64::from(fpx[k].alpha());
                inner[c].push([
                    f64::from(fpx[k].red()) / a,
                    f64::from(fpx[k].green()) / a,
                    f64::from(fpx[k].blue()) / a,
                ]);
            } else if alpha[k] == 0
                && dist[k] > 0.5
                && dist[k] <= ring_px
                && shown[k]
                && clip.is_none_or(|r| r.contains(point(&view, k)))
            {
                ring[c].push(k);
            }
        }
        // Contrast of the text (colour `fill_c`, shown with opacity `op`)
        // against backdrop colour `b`; also returns the text luminance.
        let ratio_of = |fill_c: [f64; 3], b: [f64; 3]| {
            let shown_text = [0, 1, 2].map(|i| op * fill_c[i] + (1.0 - op) * b[i]);
            let t_l = probe::rel_luminance(shown_text[0], shown_text[1], shown_text[2]);
            let b_l = probe::rel_luminance(b[0], b[1], b[2]);
            (contrast(t_l, b_l), t_l, b_l)
        };
        // (ratio, cell, text luminance, median backdrop luminance, fill)
        let mut worst: Option<(f64, usize, f64, f64, [f64; 3])> = None;
        for c in 0..cw * ch {
            if inner[c].len() < 3 || ring[c].len() < 6 {
                continue;
            }
            let n = inner[c].len() as f64;
            let fill_c = inner[c].iter().fold([0.0; 3], |s, p| {
                [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n]
            });
            let mut ratios: Vec<f64> = Vec::with_capacity(ring[c].len());
            let mut bgs: Vec<f64> = Vec::with_capacity(ring[c].len());
            let mut tl = 0.0;
            for &k in &ring[c] {
                let (r, t_l, b_l) = ratio_of(fill_c, rgb(px[k]));
                tl = t_l;
                ratios.push(r);
                bgs.push(b_l);
            }
            let r = quantile(&mut ratios, RING_QUANTILE);
            if worst.is_none_or(|w| r < w.0) {
                worst = Some((r, c, tl, quantile(&mut bgs, 0.5), fill_c));
            }
        }
        let Some((ratio, c, text_l, bg_l, fill_c)) = worst else {
            return;
        };
        let need = self.th.required_contrast(fs, t.style.weight);
        if ratio >= need {
            return;
        }
        let cx = view.origin_x + ((c % cw) as f64 + 0.5) * cell as f64 / view.scale;
        let cy = view.origin_y + ((c / cw) as f64 + 0.5) * cell as f64 / view.scale;
        let (x, y) = self.on_page(e.page, Vec2::new(cx, cy));
        let fix = if text_l > bg_l {
            "lighten the text or darken what is behind it"
        } else {
            "darken the text or lighten what is behind it"
        };
        // The ring pixels of the worst cell below the needed ratio: where
        // do they come from?
        let bad: Vec<usize> = ring[c]
            .iter()
            .copied()
            .filter(|&k| ratio_of(fill_c, rgb(px[k])).0 < need)
            .collect();
        let source = self.contrast_source(i, &view, &comp, &bad);
        out.push(SceneFinding {
            level: Severity::Warn,
            code: FindingCode::LowContrast,
            element: Some(e.id.clone()),
            lang_dependent: true,
            message: format!(
                "text `{key}` has a contrast of {ratio:.1}:1 against what is behind it near \
                 x {x:.0}, y {y:.0} lu{source} (WCAG AA needs {need}:1 for {fs:.0} lu text); \
                 {fix}, or add a scrim, shadow or outline"
            ),
        });
    }

    /// Where the low-contrast ring pixels `bad` (indices into `view`) of
    /// text `i` come from, as a message fragment: the text's own shadow,
    /// glow or outline (they count as backdrop on purpose), or the element
    /// drawn there (topmost below the text), or nothing if unclear.
    fn contrast_source(&self, i: usize, view: &Viewport, comp: &Pixmap, bad: &[usize]) -> String {
        if bad.is_empty() {
            return String::new();
        }
        let scene = self.scene();
        let e = &scene.elements[i];
        let own_effects = match &e.props {
            ResolvedKind::Text(t) => {
                t.style.stroke.is_some()
                    || t.style.glow.is_some()
                    || !t.style.shadow.is_empty()
                    || self.probe.prepared.text(i).is_some_and(|p| {
                        !p.layout.highlights.is_empty()
                            || p.layout
                                .effects
                                .iter()
                                .any(|f| *f != vitrine_core::resolve::TextEffects::default())
                    })
            }
            _ => false,
        };
        if own_effects {
            // The same scene with the text drawn bare: pixels that change
            // are the text's own effects.
            let plain = self.probe.paint(view, true, |j, _| match j.cmp(&i) {
                std::cmp::Ordering::Less => Some(Draw::Full),
                std::cmp::Ordering::Equal => Some(Draw::Bare),
                std::cmp::Ordering::Greater => None,
            });
            let (a, b) = (comp.pixels(), plain.pixels());
            let changed = bad
                .iter()
                .filter(|&&k| {
                    let (p, q) = (rgb(a[k]), rgb(b[k]));
                    (0..3).map(|c| (p[c] - q[c]).abs()).sum::<f64>() > 0.1
                })
                .count();
            if 2 * changed >= bad.len() {
                return " – mostly the text's own shadow, glow, outline or highlight (they count \
                        as backdrop: give them a colour that contrasts with the text)"
                    .to_owned();
            }
        }
        // Topmost element below the text at each bad pixel.
        let region = Rect::new(
            view.origin_x,
            view.origin_y,
            f64::from(view.width_px) / view.scale,
            f64::from(view.height_px) / view.scale,
        );
        let mut owner: Vec<Option<usize>> = vec![None; bad.len()];
        let mut left = bad.len();
        for j in (0..i).rev() {
            if left == 0 {
                break;
            }
            let o = &scene.elements[j];
            if o.opacity < 0.3 || !self.bounds[j].is_some_and(|b| b.intersects(&region)) {
                continue;
            }
            let m = self.probe.mask(view, j);
            for (n, &k) in bad.iter().enumerate() {
                if owner[n].is_none() && m[k] {
                    owner[n] = Some(j);
                    left -= 1;
                }
            }
        }
        let mut counts: Vec<(Option<usize>, usize)> = Vec::new();
        for o in owner {
            match counts.iter_mut().find(|(x, _)| *x == o) {
                Some(c) => c.1 += 1,
                None => counts.push((o, 1)),
            }
        }
        let Some((top, n)) = counts.into_iter().max_by_key(|c| c.1) else {
            return String::new();
        };
        if 2 * n < bad.len() {
            return String::new();
        }
        match top {
            Some(j) => format!(" (mostly `{}` there)", scene.elements[j].id),
            None => " (mostly the canvas background there)".to_owned(),
        }
    }

    // ---------------------------------------------------------- screens

    /// `screen_header_cut`, `screen_header_covered` and `screen_covered` of
    /// every device.
    fn screens(&self, out: &mut Vec<SceneFinding>) {
        let scene = self.scene();
        for (j, e) in scene.elements.iter().enumerate() {
            let ResolvedKind::Device(d) = &e.props else {
                continue;
            };
            if e.opacity <= 0.0 {
                continue;
            }
            let (screen, _) = screen_rect(e, d);
            let band = header_band(screen);
            if !allows(e, FindingCode::ScreenHeaderCut) {
                self.header_cut(e, screen, band, out);
            }
            self.header_covered(j, e, band, out);
            self.screen_covered(j, e, screen, band, out);
        }
    }

    /// Elements other than devices and page backgrounds drawn above device
    /// `j` whose bounds meet `region`.
    fn above(&self, j: usize, region: &Rect, code: FindingCode) -> Vec<usize> {
        let scene = self.scene();
        scene
            .elements
            .iter()
            .enumerate()
            .skip(j + 1)
            .filter(|(k, o)| {
                // Another device over the screen is a deliberate stack.
                !matches!(
                    o.props,
                    ResolvedKind::Background { .. } | ResolvedKind::Device(_)
                ) && !self.backdrop[*k]
                    && o.opacity >= 0.3
                    && !allows(o, code)
                    && self.bounds[*k].is_some_and(|b| b.intersects(region))
            })
            .map(|(k, _)| k)
            .collect()
    }

    /// `screen_header_covered`: an element above the device covers at
    /// least [`HEADER_COVER_MIN`] of the top bar.
    fn header_covered(
        &self,
        j: usize,
        e: &ResolvedElement,
        band: Rect,
        out: &mut Vec<SceneFinding>,
    ) {
        let scene = self.scene();
        let quad = rotated_quad(band, e);
        let Some(band_box) = intersection(&probe::bounds_of(&quad), &scene.frame.canvas) else {
            return;
        };
        let view = view_for(band_box, (96.0 / band.w).clamp(0.1, 1.0));
        let band_mask = quad_mask(&view, &quad);
        let total = band_mask.iter().filter(|b| **b).count();
        if total == 0 {
            return;
        }
        for k in self.above(j, &band_box, FindingCode::ScreenHeaderCovered) {
            let o = &scene.elements[k];
            let m = self.probe.mask(&view, k);
            let covered = band_mask.iter().zip(&m).filter(|(b, m)| **b && **m).count();
            let share = covered as f64 / total as f64;
            if share >= HEADER_COVER_MIN {
                out.push(SceneFinding {
                    level: Severity::Info,
                    code: FindingCode::ScreenHeaderCovered,
                    element: Some(o.id.clone()),
                    lang_dependent: matches!(o.props, ResolvedKind::Text(_)),
                    message: format!(
                        "`{}` covers {:.0} % of the top bar (status bar, app title) of the \
                         screen of `{}`; move it below the bar or onto the frame",
                        o.id,
                        share * 100.0,
                        e.id
                    ),
                });
            }
        }
    }

    /// `screen_covered`: an element above the device covers at least
    /// [`Thresholds::screen_cover`] of the screen below its top bar (the
    /// app content the screenshot is meant to show).
    fn screen_covered(
        &self,
        j: usize,
        e: &ResolvedElement,
        screen: Rect,
        band: Rect,
        out: &mut Vec<SceneFinding>,
    ) {
        let scene = self.scene();
        let body = Rect::new(
            screen.x,
            band.bottom(),
            screen.w,
            screen.bottom() - band.bottom(),
        );
        if body.h <= 0.0 || self.th.screen_cover >= 1.0 {
            return;
        }
        let quad = rotated_quad(body, e);
        let Some(body_box) = intersection(&probe::bounds_of(&quad), &scene.frame.canvas) else {
            return;
        };
        let view = view_for(body_box, (160.0 / screen.w.max(1.0)).clamp(0.05, 1.0));
        // Only the visible part of the screen counts (a screen cut by the
        // page edge is measured on what the export shows).
        let body_mask: Vec<bool> = quad_mask(&view, &quad)
            .into_iter()
            .enumerate()
            .map(|(p, m)| {
                m && {
                    let q = point(&view, p);
                    self.shown(q) && e.clip.is_none_or(|c| c.contains(q))
                }
            })
            .collect();
        let total = body_mask.iter().filter(|b| **b).count();
        if total == 0 {
            return;
        }
        for k in self.above(j, &body_box, FindingCode::ScreenCovered) {
            let o = &scene.elements[k];
            let m = self.probe.mask(&view, k);
            let covered = body_mask.iter().zip(&m).filter(|(b, m)| **b && **m).count();
            let share = covered as f64 / total as f64;
            if share >= self.th.screen_cover {
                out.push(SceneFinding {
                    level: Severity::Info,
                    code: FindingCode::ScreenCovered,
                    element: Some(o.id.clone()),
                    lang_dependent: matches!(o.props, ResolvedKind::Text(_)),
                    message: format!(
                        "`{}` covers {:.0} % of the screen of `{}` below its top bar (the app \
                         content; reported from {:.0} %, `checks.screen_cover`); move it onto the \
                         frame or beside the device, or make it smaller",
                        o.id,
                        share * 100.0,
                        e.id,
                        self.th.screen_cover * 100.0
                    ),
                });
            }
        }
    }

    /// A horizontal page or image edge, or the device's clip, cutting the
    /// (unrotated) screen through its top bar.
    fn header_cut(
        &self,
        e: &ResolvedElement,
        screen: Rect,
        band: Rect,
        out: &mut Vec<SceneFinding>,
    ) {
        let rot = e.rotation.rem_euclid(360.0);
        if rot > 1.0 && rot < 359.0 {
            return; // tilted: no straight cut line
        }
        let scene = self.scene();
        let page_at = |y: f64, x0: f64, x1: f64| {
            scene
                .frame
                .pages
                .iter()
                .find(|p| (p.rect.y - y).abs() < 0.5 && p.rect.x < x1 && p.rect.right() > x0)
        };
        // (cut y, cause)
        let mut cuts: Vec<(f64, String)> = scene
            .frame
            .images
            .iter()
            .filter(|img| img.source.x < screen.right() && img.source.right() > screen.x)
            .map(|img| {
                let y = img.source.y;
                let cause = match page_at(y, screen.x, screen.right()) {
                    Some(p) => format!("the top edge of page {}", p.index + 1),
                    None => format!("the top edge of export image {}", img.index + 1),
                };
                (y, cause)
            })
            .collect();
        if let Some(c) = e.clip {
            let cause = match page_at(c.y, c.x, c.right()) {
                Some(p) => format!(
                    "its clip (`clip: page`, the top edge of page {})",
                    p.index + 1
                ),
                None => "its clip".to_owned(),
            };
            cuts.push((c.y, cause));
        }
        let lo = screen.y + 0.005 * screen.h;
        if let Some((cut, cause)) = cuts.into_iter().find(|&(c, _)| c > lo && c < band.bottom()) {
            out.push(SceneFinding {
                level: Severity::Info,
                code: FindingCode::ScreenHeaderCut,
                element: Some(e.id.clone()),
                lang_dependent: false,
                message: format!(
                    "{cause} cuts the screen of `{}` {:.0} lu below its top, through the \
                     status bar / app title ({:.0} lu): part of the bar shows; move the device \
                     so the cut lies below the bar or the whole bar shows",
                    e.id,
                    cut - screen.y,
                    band.h
                ),
            });
        }
    }

    // ------------------------------------------------------ device cuts

    /// `device_cut`: a device cut off by more than [`DEVICE_CUT_MIN`] lu at
    /// the left or right edge of its page (`clip: page`) or of the canvas
    /// (`clip: none`). Cuts at the top and bottom are a common deliberate
    /// crop and not reported; a device running on into the next page
    /// without clip is not cut.
    fn device_cuts(&self, out: &mut Vec<SceneFinding>) {
        let scene = self.scene();
        for (j, e) in scene.elements.iter().enumerate() {
            if !matches!(e.props, ResolvedKind::Device(_))
                || e.opacity <= 0.0
                || allows(e, FindingCode::DeviceCut)
            {
                continue;
            }
            let edge = e.clip.unwrap_or(scene.frame.canvas);
            // Unclipped bounds incl. buttons (3 % margin, as `Probe::bounds`).
            let m = e.size.x.max(e.size.y) * 0.03;
            let b = rotated_bounds(
                Rect::centered(e.center, e.size.x + 2.0 * m, e.size.y + 2.0 * m),
                e,
            );
            let (y0, y1) = (b.y.max(edge.y), b.bottom().min(edge.bottom()));
            if y1 <= y0 || b.right() <= edge.x || b.x >= edge.right() {
                continue; // nothing visible (`element_outside`)
            }
            let mut sides = Vec::new();
            for left in [true, false] {
                let (lo, hi) = if left {
                    (b.x, edge.x)
                } else {
                    (edge.right(), b.right())
                };
                if hi - lo <= DEVICE_CUT_MIN {
                    continue;
                }
                // The part beyond the edge, within the visible height.
                let view = view_for(Rect::new(lo, y0, hi - lo, y1 - y0), 0.5);
                let alpha = self.probe.alpha_unclipped(&view, j);
                let half = 0.5 / view.scale;
                let lost = alpha
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| **a >= 128)
                    .map(|(k, _)| {
                        let p = point(&view, k);
                        if left {
                            edge.x - p.x + half
                        } else {
                            p.x + half - edge.right()
                        }
                    })
                    .fold(0.0, f64::max);
                if lost > DEVICE_CUT_MIN {
                    sides.push((left, lost));
                }
            }
            if sides.is_empty() {
                continue;
            }
            let what = if e.clip.is_some() {
                format!("page {}", e.page + 1)
            } else {
                "the canvas".to_owned()
            };
            let parts: Vec<String> = sides
                .iter()
                .map(|(left, lost)| {
                    format!(
                        "{lost:.0} lu at the {} edge of {what}",
                        if *left { "left" } else { "right" }
                    )
                })
                .collect();
            let tilt = if e.rotation.rem_euclid(360.0) == 0.0 {
                String::new()
            } else {
                format!(
                    " (tilted {:.0}°: half its rotated width is w/2·cos + h/2·sin)",
                    e.rotation
                )
            };
            let fix = match sides.as_slice() {
                [(true, lost)] => format!("move it {lost:.0} lu right"),
                [(false, lost)] => format!("move it {lost:.0} lu left"),
                _ => "make it smaller".to_owned(),
            };
            out.push(SceneFinding {
                level: Severity::Info,
                code: FindingCode::DeviceCut,
                element: Some(e.id.clone()),
                lang_dependent: false,
                message: format!(
                    "device `{}` is cut off: {}{tilt}; {fix}, make it smaller or tilt it \
                     less, or `allow: [device_cut]` if the crop is intended",
                    e.id,
                    parts.join(" and ")
                ),
            });
        }
    }
    // ---------------------------------------------------------- corners

    /// `element_in_corner`: elements (not devices) reaching into the part
    /// of an image corner the store rounds off.
    fn corners(&self, out: &mut Vec<SceneFinding>) {
        let scene = self.scene();
        if scene.frame.kind != TargetKind::Pages {
            return;
        }
        let Some(g) = &scene.frame.gallery else {
            return;
        };
        let (rx, ry) = (g.radius.x, g.radius.y);
        if rx < 4.0 || ry < 4.0 {
            return;
        }
        let zone_area = rx * ry * (1.0 - std::f64::consts::FRAC_PI_4);
        let names = ["top-left", "top-right", "bottom-right", "bottom-left"];
        for (k, e) in scene.elements.iter().enumerate() {
            if matches!(
                e.props,
                ResolvedKind::Background { .. } | ResolvedKind::Device(_)
            ) || self.backdrop[k]
                || e.opacity < 0.3
                || allows(e, FindingCode::ElementInCorner)
            {
                continue;
            }
            let Some(b) = self.bounds[k] else { continue };
            let mut hits = Vec::new();
            for img in &scene.frame.images {
                let s = img.source;
                let corners = [
                    (Rect::new(s.x, s.y, rx, ry), Vec2::new(s.x + rx, s.y + ry)),
                    (
                        Rect::new(s.right() - rx, s.y, rx, ry),
                        Vec2::new(s.right() - rx, s.y + ry),
                    ),
                    (
                        Rect::new(s.right() - rx, s.bottom() - ry, rx, ry),
                        Vec2::new(s.right() - rx, s.bottom() - ry),
                    ),
                    (
                        Rect::new(s.x, s.bottom() - ry, rx, ry),
                        Vec2::new(s.x + rx, s.bottom() - ry),
                    ),
                ];
                for (ci, (sq, c)) in corners.iter().enumerate() {
                    if !b.intersects(sq) {
                        continue;
                    }
                    let view = view_for(*sq, (48.0 / rx).clamp(0.1, 2.0));
                    let alpha = self.probe.alpha(&view, k);
                    let mut m: Vec<bool> = alpha.iter().map(|a| *a >= 128).collect();
                    self.hide_covered(k, &view, sq, &mut m);
                    let n = m
                        .iter()
                        .enumerate()
                        .filter(|(p, on)| {
                            if !**on {
                                return false;
                            }
                            let q = point(&view, *p);
                            let (dx, dy) = ((q.x - c.x) / rx, (q.y - c.y) / ry);
                            dx * dx + dy * dy > 1.0
                        })
                        .count();
                    let area = n as f64 / (view.scale * view.scale);
                    // An element filling the whole corner square is rounded
                    // like the page itself (scrims, full-bleed pictures):
                    // only an edge or corner of its own inside the square
                    // looks cut off.
                    let fill = alpha.iter().filter(|a| **a >= 16).count() as f64
                        / alpha.len().max(1) as f64;
                    if fill < CORNER_FULL && area >= (0.03 * zone_area).max(20.0) {
                        hits.push(format!("image {} {}", img.index + 1, names[ci]));
                    }
                }
            }
            if hits.is_empty() {
                continue;
            }
            let measured = if g.measured == Some("no") {
                ", radius not yet measured"
            } else {
                ""
            };
            out.push(SceneFinding {
                level: Severity::Info,
                code: FindingCode::ElementInCorner,
                element: Some(e.id.clone()),
                lang_dependent: matches!(e.props, ResolvedKind::Text(_)),
                message: format!(
                    "`{}` reaches into the rounded corner the store cuts off ({}; radius {:.0} lu{measured}); \
                     keep square shapes clear of the corners or round them",
                    e.id,
                    hits.join(", "),
                    rx
                ),
            });
        }
    }

    /// Clears the pixels of `mask` (element `k` in `view` over `region`)
    /// that opaque elements drawn above `k` hide: only what shows can look
    /// cut off.
    fn hide_covered(&self, k: usize, view: &Viewport, region: &Rect, mask: &mut [bool]) {
        for (o, oe) in self.scene().elements.iter().enumerate().skip(k + 1) {
            if !mask.iter().any(|x| *x) {
                return;
            }
            if matches!(oe.props, ResolvedKind::Background { .. })
                || oe.opacity < 0.9
                || !self.bounds[o].is_some_and(|b| b.intersects(region))
            {
                continue;
            }
            for (x, h) in mask.iter_mut().zip(self.probe.mask(view, o)) {
                *x &= !h;
            }
        }
    }

    // ----------------------------------------------------------- blends

    /// `page_blends_with_store`: edge pixels of the images against the
    /// gallery's light and dark background.
    fn blends(&self, out: &mut Vec<SceneFinding>) {
        let scene = self.scene();
        let Some(g) = &scene.frame.gallery else {
            return;
        };
        if scene.frame.kind != TargetKind::Pages || g.gap <= 0.0 {
            return;
        }
        let backs: Vec<(&str, vitrine_core::model::Rgba)> = [
            g.background.map(|c| ("light", c)),
            g.background_dark.map(|c| ("dark", c)),
        ]
        .into_iter()
        .flatten()
        .collect();
        if backs.is_empty() {
            return;
        }
        let opts = RenderOptions {
            scale: Scale::Width(64),
            quality: Quality::Preview,
        };
        let edges: Vec<Vec<f64>> = (0..scene.frame.images.len())
            .map(|i| {
                let pm = self
                    .probe
                    .ctx
                    .rasterize_prepared(&self.probe.prepared, i, &opts)
                    .pixmap;
                let (w, h) = (pm.width() as usize, pm.height() as usize);
                pm.pixels()
                    .iter()
                    .enumerate()
                    .filter(|(k, _)| {
                        let (x, y) = (k % w, k / w);
                        x < 2 || y < 2 || x + 2 >= w || y + 2 >= h
                    })
                    .map(|(_, p)| luminance(*p))
                    .collect()
            })
            .collect();
        for (name, c) in backs {
            let bl = probe::rel_luminance(
                f64::from(c.r) / 255.0,
                f64::from(c.g) / 255.0,
                f64::from(c.b) / 255.0,
            );
            let images: Vec<String> = edges
                .iter()
                .enumerate()
                .filter(|(_, e)| {
                    !e.is_empty()
                        && e.iter()
                            .filter(|&&l| contrast(l, bl) < BLEND_CONTRAST)
                            .count() as f64
                            >= BLEND_SHARE * e.len() as f64
                })
                .map(|(i, _)| (i + 1).to_string())
                .collect();
            if images.is_empty() {
                continue;
            }
            out.push(SceneFinding {
                level: Severity::Info,
                code: FindingCode::PageBlendsWithStore,
                element: None,
                lang_dependent: false,
                message: format!(
                    "the edges of image{} {} are close to the store's {name} background ({c}): \
                     in the gallery the screenshots run into it (fine for a deliberate look; a \
                     tinted background or a thin frame line separates them)",
                    if images.len() == 1 { "" } else { "s" },
                    images.join(", ")
                ),
            });
        }
    }

    // ------------------------------------------------------------ seams

    /// `seam_overlap`: an element reaching from its home page into another
    /// page overlaps (in pixels) an element of that page. Pairs with a
    /// text are language-dependent and checked every time, others only on
    /// the first language.
    fn seams(&self, first: bool, out: &mut Vec<SceneFinding>) {
        let scene = self.scene();
        let pages = &scene.frame.pages;
        let n = scene.elements.len();
        let candidate = |k: usize| {
            let e = &scene.elements[k];
            !matches!(e.props, ResolvedKind::Background { .. })
                && !self.backdrop[k]
                && e.opacity > 0.0
                && self.bounds[k].is_some()
        };
        for a in 0..n {
            if !candidate(a) || allows(&scene.elements[a], FindingCode::SeamOverlap) {
                continue;
            }
            let ea = &scene.elements[a];
            let ba = self.bounds[a].expect("candidate");
            for b in 0..n {
                if a == b || !candidate(b) {
                    continue;
                }
                let eb = &scene.elements[b];
                let text = matches!(ea.props, ResolvedKind::Text(_))
                    || matches!(eb.props, ResolvedKind::Text(_));
                if eb.page == ea.page
                    || (!first && !text)
                    || ea.groups.iter().any(|g| eb.groups.contains(g))
                    || allows(eb, FindingCode::SeamOverlap)
                    // Drawn below the other element: the other one lies on
                    // it (a text or card on a bridging tile) – intended.
                    || a < b
                {
                    continue;
                }
                let Some(page) = pages.get(eb.page as usize) else {
                    continue;
                };
                let Some(region) = intersection(&ba, &page.rect)
                    .and_then(|r| intersection(&r, &self.bounds[b].expect("candidate")))
                else {
                    continue;
                };
                if region.w * region.h < SEAM_MIN_AREA {
                    continue;
                }
                let view = view_for(region, 0.25_f64.max(16.0 / region.w.min(region.h)).min(1.0));
                let ma = self.probe.mask(&view, a);
                let mb = self.probe.mask(&view, b);
                let px = ma.iter().zip(&mb).filter(|(x, y)| **x && **y).count();
                let area = px as f64 / (view.scale * view.scale);
                if area < SEAM_MIN_AREA {
                    continue;
                }
                out.push(SceneFinding {
                    level: Severity::Info,
                    code: FindingCode::SeamOverlap,
                    element: Some(ea.id.clone()),
                    lang_dependent: text,
                    message: format!(
                        "`{}` reaches from page {} into page {} and overlaps `{}` there \
                         ({area:.0} lu²); check the gallery – intended?",
                        ea.id,
                        ea.page + 1,
                        eb.page + 1,
                        eb.id
                    ),
                });
            }
        }
    }
}

/// An image or shape covering at least [`BACKDROP_SHARE`] of its home page,
/// or a page background element.
fn is_backdrop(scene: &ResolvedScene, e: &ResolvedElement, b: Option<&Rect>) -> bool {
    match e.props {
        ResolvedKind::Background { .. } => true,
        ResolvedKind::Image(_) | ResolvedKind::Shape(_) => {
            let (Some(b), Some(p)) = (b, scene.frame.pages.get(e.page as usize)) else {
                return false;
            };
            intersection(b, &p.rect)
                .is_some_and(|i| i.w * i.h >= BACKDROP_SHARE * p.rect.w * p.rect.h)
        }
        _ => false,
    }
}

/// Screen rect of a device on the canvas before rotation, and its corner
/// radius (lu).
pub fn screen_rect(e: &ResolvedElement, d: &ResolvedDevice) -> (Rect, f64) {
    let local = if let Some(f) = &d.imported {
        let (bw, bh) = (f64::from(f.size.w.max(1)), f64::from(f.size.h.max(1)));
        let (w, h) = (e.size.x, e.size.y);
        (
            Rect::new(
                -w / 2.0 + w * f64::from(f.screen.x) / bw,
                -h / 2.0 + h * f64::from(f.screen.y) / bh,
                w * f64::from(f.screen.w) / bw,
                h * f64::from(f.screen.h) / bh,
            ),
            0.0,
        )
    } else if let Some(m) = d
        .device
        .frame_metrics(d.orientation, e.size.x)
        .filter(|_| d.chrome != Some(vitrine_core::model::Chrome::Browser))
    {
        // The catalog's frame metrics (also `catalog_devices.frame`; kept
        // equal to the drawn frame by `frames::tests`), relative to the
        // element box's top-left corner.
        (
            Rect::new(
                m.screen.x - e.size.x / 2.0,
                m.screen.y - m.height / 2.0,
                m.screen.w,
                m.screen.h,
            ),
            m.screen.radius,
        )
    } else {
        // Free-aspect models (browser, `none`) and browser chrome on a
        // notebook: the drawn geometry.
        let g = frames::device_geometry(d, e.size);
        (g.screen, f64::from(g.screen_radius))
    };
    let (r, radius) = local;
    (
        Rect::new(r.x + e.center.x, r.y + e.center.y, r.w, r.h),
        radius,
    )
}

/// Top bar of a screen (status bar plus title), unrotated.
pub fn header_band(screen: Rect) -> Rect {
    let share = if screen.w > 0.8 * screen.h {
        HEADER_SHARE_WIDE
    } else {
        HEADER_SHARE
    };
    Rect::new(screen.x, screen.y, screen.w, screen.h * share)
}

/// Pixels of `view` inside the convex quad `q`.
fn quad_mask(view: &Viewport, q: &[Vec2; 4]) -> Vec<bool> {
    let n = (view.width_px * view.height_px) as usize;
    (0..n)
        .map(|k| {
            let p = point(view, k);
            let mut sign = 0_i8;
            for i in 0..4 {
                let (a, b) = (q[i], q[(i + 1) % 4]);
                let c = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
                if c.abs() < 1e-12 {
                    continue;
                }
                let side = if c > 0.0 { 1 } else { -1 };
                if sign == 0 {
                    sign = side;
                } else if side != sign {
                    return false;
                }
            }
            true
        })
        .collect()
}

#[cfg(test)]
mod tests;
