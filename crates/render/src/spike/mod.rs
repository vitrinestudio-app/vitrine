//! Phase 0 rendering spike: a hard-coded scene rendered through the
//! page → canvas mapping (no YAML, no model yet).
//!
//! The scene follows `docs/KONZEPT.md` §2/§3: four pages (1320:2868), a
//! canvas-wide gradient background, a diagonal band anchored to page 1, a
//! phone on the right edge of page 0 (reaching into page 1), a second phone on
//! page 2, a landscape tablet on page 3 and a two-line headline per page.

#![allow(
    clippy::too_many_lines,
    reason = "spike scene and renderer are long linear code"
)]

pub mod compose;
pub mod screens;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tiny_skia::{FillRule, Mask, Pixmap, Transform};

use crate::blur::{ShadowPx, draw_shadow};
use crate::frames::{DeviceGeometry, DeviceModel, FrameColors, FrameStyle, draw_device};
use crate::geometry::{AnchorY, PAGE_W, Rect, StripLayout, Viewport};
use crate::paint::{Rgba, css_gradient_line, hex, linear, round_rect, shader_paint, solid};
use crate::raster::SourceImage;
use crate::text::{
    Align, Fit, FontLibrary, TextEngine, TextPaths, TextStyle, layout_fit, parse_markup, to_paths,
};

/// Canvas design aspect (iPhone 6.9″ portrait).
pub const DESIGN_ASPECT: (f64, f64) = (1320.0, 2868.0);

/// Design page height in lu.
pub fn design_page_height() -> f64 {
    PAGE_W * DESIGN_ASPECT.1 / DESIGN_ASPECT.0
}

/// Placeholder App Store iPhone gallery gap in lu — **not yet measured**.
pub const PLACEHOLDER_GAP_LU: f64 = 40.0;

/// Position of an element relative to its home page.
#[derive(Debug, Clone, Copy)]
pub struct Placement {
    pub page: i64,
    pub x: f64,
    pub y: f64,
    pub anchor: AnchorY,
    /// Degrees, positive = clockwise.
    pub rotation: f64,
}

#[derive(Debug, Clone)]
pub enum ElementKind {
    /// Long rounded band (a "shape"), filled with a gradient along its length.
    Band {
        length: f64,
        thickness: f64,
        stops: Vec<(f32, Rgba)>,
    },
    Device {
        model: DeviceModel,
        width: f64,
        screenshot: &'static str,
        colors: FrameColors,
    },
    /// Text bound to its home page (`clip: page`), box with margins.
    Text {
        key: &'static str,
        top: f64,
        left: f64,
        right: f64,
        size: f32,
        min_size: f32,
        max_lines: usize,
    },
}

#[derive(Debug, Clone)]
pub struct Element {
    pub id: &'static str,
    pub at: Placement,
    pub kind: ElementKind,
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub page_count: usize,
    pub design_page_height: f64,
    /// CSS angle (degrees) and stops of the canvas-wide background gradient.
    pub bg_angle: f32,
    pub bg_stops: Vec<(f32, Rgba)>,
    pub elements: Vec<Element>,
    /// Translations: language → key → markup.
    pub texts: HashMap<&'static str, HashMap<&'static str, &'static str>>,
}

/// Format variant (`docs/KONZEPT.md` §3.4): content scale + per-element overrides.
#[derive(Debug, Clone, Default)]
pub struct Variant {
    pub content_scale: f64,
    pub overrides: HashMap<&'static str, Placement>,
}

impl Variant {
    pub fn none() -> Self {
        Self {
            content_scale: 1.0,
            overrides: HashMap::new(),
        }
    }
}

const H0: f64 = 1000.0 * 2868.0 / 1320.0;
const BAND_Y: f64 = 1400.0;
const BAND_ANGLE: f64 = -18.0;

/// The spike scene.
pub fn demo_scene() -> Scene {
    let white = hex("#FFFFFF");
    let headline = |id, page, key| Element {
        id,
        at: Placement {
            page,
            x: 0.0,
            y: 0.0,
            anchor: AnchorY::Top,
            rotation: 0.0,
        },
        kind: ElementKind::Text {
            key,
            top: 170.0,
            left: 80.0,
            right: 80.0,
            size: 104.0,
            min_size: 56.0,
            max_lines: 2,
        },
    };
    let _ = white;
    let elements = vec![
        Element {
            id: "band",
            at: Placement {
                page: 1,
                x: 560.0,
                y: BAND_Y,
                anchor: AnchorY::Center,
                rotation: BAND_ANGLE,
            },
            kind: ElementKind::Band {
                length: 9000.0,
                thickness: 330.0,
                stops: vec![
                    (0.0, hex("#FFD27A26")),
                    (0.5, hex("#FFFFFF1F")),
                    (1.0, hex("#9FD4FF29")),
                ],
            },
        },
        Element {
            id: "accent-line",
            at: Placement {
                page: 1,
                // Parallel to the band, just below its lower edge.
                x: 560.0 + 200.0 * BAND_ANGLE.to_radians().sin().abs(),
                y: BAND_Y + 200.0 * BAND_ANGLE.to_radians().cos(),
                anchor: AnchorY::Center,
                rotation: BAND_ANGLE,
            },
            kind: ElementKind::Band {
                length: 9000.0,
                thickness: 14.0,
                stops: vec![(0.0, hex("#FFC65C")), (1.0, hex("#FFC65C"))],
            },
        },
        Element {
            id: "hero-phone",
            at: Placement {
                page: 0,
                x: 1000.0,
                y: H0 - 680.0,
                anchor: AnchorY::Bottom,
                rotation: -8.0,
            },
            kind: ElementKind::Device {
                model: DeviceModel::PhoneIsland,
                width: 800.0,
                screenshot: "phone-home",
                colors: FrameColors::silver(),
            },
        },
        Element {
            id: "harvest-phone",
            at: Placement {
                page: 2,
                x: 500.0,
                y: H0 - 600.0,
                anchor: AnchorY::Bottom,
                rotation: 6.0,
            },
            kind: ElementKind::Device {
                model: DeviceModel::PhoneIsland,
                width: 700.0,
                screenshot: "phone-harvest",
                colors: FrameColors::graphite(),
            },
        },
        Element {
            id: "tablet",
            at: Placement {
                page: 3,
                x: 600.0,
                y: H0 - 720.0,
                anchor: AnchorY::Bottom,
                rotation: -5.0,
            },
            kind: ElementKind::Device {
                model: DeviceModel::TabletLandscape,
                width: 1150.0,
                screenshot: "tablet-planner",
                colors: FrameColors::graphite(),
            },
        },
        headline("title-0", 0, "title.0"),
        headline("title-1", 1, "title.1"),
        headline("title-2", 2, "title.2"),
        headline("title-3", 3, "title.3"),
    ];
    let en: HashMap<_, _> = [
        ("title.0", "Plan your beds [accent]in minutes[/]"),
        ("title.1", "Smart **companion planting** tips"),
        ("title.2", "Track every [accent]harvest[/] at a glance"),
        ("title.3", "Your garden, now [accent]on iPad[/]"),
    ]
    .into_iter()
    .collect();
    let de: HashMap<_, _> = [
        ("title.0", "Plane deine Beete [accent]in Minuten[/]"),
        ("title.1", "**Mischkulturempfehlungen** für jedes Beet"),
        (
            "title.2",
            "Behalte jede [accent]Ernte[/] auf einen Blick im Griff",
        ),
        ("title.3", "Dein Garten, jetzt auch [accent]auf dem iPad[/]"),
    ]
    .into_iter()
    .collect();
    Scene {
        page_count: 4,
        design_page_height: H0,
        bg_angle: 100.0,
        bg_stops: vec![
            (0.0, hex("#0B3B2C")),
            (0.38, hex("#14614A")),
            (0.7, hex("#1A5C66")),
            (1.0, hex("#23365F")),
        ],
        elements,
        texts: [("en", en), ("de", de)].into_iter().collect(),
    }
}

/// Headline style of the spike.
pub fn headline_style(size: f32) -> TextStyle {
    TextStyle {
        family: "Inter Display",
        weight: 700,
        bold_weight: 900,
        size,
        line_height: 1.08,
        color: hex("#FFFFFF"),
        align: Align::Center,
        span_colors: [("accent".to_owned(), hex("#FFC65C"))]
            .into_iter()
            .collect(),
    }
}

/// A laid-out text element (independent of target resolution).
#[derive(Debug, Clone)]
pub struct PreparedText {
    pub paths: TextPaths,
    pub size: f32,
    pub lines: usize,
    pub fits: bool,
    pub height: f32,
    pub width: f32,
}

/// Lays out all text elements of `scene` for `lang` with `engine`.
pub fn prepare_texts(
    scene: &Scene,
    lang: &str,
    engine: &mut dyn TextEngine,
    fonts: &FontLibrary,
) -> Texts {
    let mut out = HashMap::new();
    for el in &scene.elements {
        if let ElementKind::Text {
            key,
            left,
            right,
            size,
            min_size,
            max_lines,
            ..
        } = &el.kind
        {
            let markup = scene
                .texts
                .get(lang)
                .and_then(|t| t.get(key))
                .or_else(|| scene.texts.get("en").and_then(|t| t.get(key)))
                .copied()
                .unwrap_or(key);
            let spans = parse_markup(markup);
            let width = (PAGE_W - left - right) as f32;
            let fitted = layout_fit(
                engine,
                &spans,
                &headline_style(*size),
                width,
                &Fit {
                    min_size: *min_size,
                    max_lines: *max_lines,
                    max_height: None,
                    balance: true,
                },
            );
            out.insert(
                el.id,
                PreparedText {
                    paths: to_paths(&fitted.layout, fonts),
                    size: fitted.size,
                    lines: fitted.layout.line_count,
                    fits: fitted.fits,
                    height: fitted.layout.height,
                    width: fitted.layout.max_line_width,
                },
            );
        }
    }
    out
}

type ResizeCache = HashMap<(&'static str, u32, u32), Arc<Pixmap>>;

/// Laid-out texts of one language, by element id.
pub type Texts = HashMap<&'static str, PreparedText>;

/// Screenshot sources plus a cache of Lanczos3-resampled copies per pixel size.
#[derive(Debug, Default)]
pub struct Screenshots {
    sources: HashMap<&'static str, SourceImage>,
    cache: Mutex<ResizeCache>,
}

impl Screenshots {
    pub fn insert(&mut self, key: &'static str, img: SourceImage) {
        self.sources.insert(key, img);
    }

    /// Resampled screenshot for a screen of `w × h` px (cached).
    pub fn get(&self, key: &'static str, w: u32, h: u32) -> Option<Arc<Pixmap>> {
        let mut cache = self.cache.lock().ok()?;
        if let Some(p) = cache.get(&(key, w, h)) {
            return Some(Arc::clone(p));
        }
        let src = self.sources.get(key)?;
        let pm = Arc::new(src.resized_cover(w, h).ok()?);
        cache.insert((key, w, h), Arc::clone(&pm));
        Some(pm)
    }

    pub fn clear_cache(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
    }
}

/// Everything that varies per output target.
#[derive(Debug, Clone)]
pub struct RenderParams<'a> {
    pub layout: StripLayout,
    pub variant: &'a Variant,
    pub style: FrameStyle,
}

/// Shadow of devices in lu: (sigma, dy, opacity).
const SHADOWS: [(f64, f64, f32); 2] = [(48.0, 46.0, 0.42), (10.0, 10.0, 0.30)];

/// Renders the visible part of the canvas into a new pixmap.
pub fn render(
    scene: &Scene,
    texts: &Texts,
    shots: &Screenshots,
    params: &RenderParams<'_>,
    view: &Viewport,
) -> Pixmap {
    let mut pm = Pixmap::new(view.width_px, view.height_px).expect("non-zero viewport");
    let vt = view.transform();
    let layout = &params.layout;
    let visible = view.visible();

    // Background: one gradient over the whole canvas, pages *and* gaps
    // (stretch: it follows the target page height).
    let c = layout.canvas_rect();
    let (p0, p1) = css_gradient_line(
        c.x as f32,
        c.y as f32,
        c.w as f32,
        c.h as f32,
        scene.bg_angle,
    );
    if let Some(r) = tiny_skia::Rect::from_xywh(
        visible.x as f32 - 1.0,
        visible.y as f32 - 1.0,
        visible.w as f32 + 2.0,
        visible.h as f32 + 2.0,
    ) {
        pm.fill_rect(r, &shader_paint(linear(p0, p1, &scene.bg_stops)), vt, None);
    }

    for el in &scene.elements {
        let at = params
            .variant
            .overrides
            .get(el.id)
            .copied()
            .unwrap_or(el.at);
        let (cx, cy) = layout.to_canvas(at.page, at.x, at.y, at.anchor);
        let place = vt
            .pre_translate(cx as f32, cy as f32)
            .pre_rotate(at.rotation as f32);
        match &el.kind {
            ElementKind::Band {
                length,
                thickness,
                stops,
            } => {
                let (l, t) = (*length as f32, *thickness as f32);
                if let Some(p) = round_rect(-l / 2.0, -t / 2.0, l, t, t / 2.0) {
                    let sh = linear((-l / 2.0, 0.0), (l / 2.0, 0.0), stops);
                    pm.fill_path(&p, &shader_paint(sh), FillRule::Winding, place, None);
                }
            }
            ElementKind::Device {
                model,
                width,
                screenshot,
                colors,
            } => {
                let k = params.variant.content_scale;
                let aspect = match model {
                    DeviceModel::PhoneIsland => DESIGN_ASPECT.1 / DESIGN_ASPECT.0,
                    DeviceModel::TabletLandscape => 2064.0 / 2752.0,
                };
                let g = DeviceGeometry::new(*model, width * k, aspect);
                // Cull devices that are entirely outside the viewport.
                let radius = (g.body.w.hypot(g.body.h)) / 2.0 + 200.0;
                if !Rect::new(cx - radius, cy - radius, 2.0 * radius, 2.0 * radius)
                    .intersects(&visible)
                {
                    continue;
                }
                if let Some(body) = g.body_path() {
                    for (sigma, dy, alpha) in SHADOWS {
                        draw_shadow(
                            &mut pm,
                            &body,
                            place,
                            ShadowPx {
                                sigma: sigma * k * view.scale,
                                dx: 0.0,
                                dy: dy * k * view.scale,
                                color: [0, 0, 0, (alpha * 255.0) as u8],
                            },
                        );
                    }
                }
                let sw = (g.screen.w * view.scale).round().max(1.0) as u32;
                let sh = (g.screen.h * view.scale).round().max(1.0) as u32;
                let shot = shots.get(screenshot, sw, sh);
                let colors = match params.style {
                    FrameStyle::Detailed => *colors,
                    FrameStyle::Stylized => colors.flat(),
                };
                draw_device(&mut pm, &g, params.style, &colors, shot.as_deref(), place);
            }
            ElementKind::Text {
                top, left, right, ..
            } => {
                let Some(t) = texts.get(el.id) else { continue };
                let page = layout.page_rect(at.page.max(0) as usize);
                if !page.intersects(&visible) {
                    continue;
                }
                let bx = page.x + left;
                let by = layout.anchor_y(*top, at.anchor);
                let _ = right;
                let ts = vt.pre_translate(bx as f32, by as f32);
                // clip: page — only build a mask if the text actually
                // leaves its page (e.g. emergency overflow).
                let needs_clip = t.paths.paths.iter().any(|(p, _)| {
                    let b = p.bounds();
                    f64::from(b.left()) + bx < page.x
                        || f64::from(b.right()) + bx > page.right()
                        || f64::from(b.bottom()) + by > page.bottom()
                });
                let mask = if needs_clip {
                    page_mask(&pm, page, vt)
                } else {
                    None
                };
                for (path, color) in &t.paths.paths {
                    pm.fill_path(path, &solid(*color), FillRule::Winding, ts, mask.as_ref());
                }
            }
        }
    }
    pm
}

fn page_mask(pm: &Pixmap, page: Rect, vt: Transform) -> Option<Mask> {
    let mut mask = Mask::new(pm.width(), pm.height())?;
    let r = tiny_skia::Rect::from_xywh(page.x as f32, page.y as f32, page.w as f32, page.h as f32)?;
    mask.fill_path(
        &tiny_skia::PathBuilder::from_rect(r),
        FillRule::Winding,
        true,
        vt,
    );
    Some(mask)
}

/// Viewport of export image `image` at `width_px × height_px`.
pub fn image_viewport(
    layout: &StripLayout,
    image: usize,
    width_px: u32,
    height_px: u32,
) -> Viewport {
    Viewport::exact(layout.image_rect(image), width_px, height_px)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::cosmic::CosmicEngine;
    use crate::text::parley_engine::ParleyEngine;

    /// A vertical white bar anchored to page 0, centred 20 lu right of its
    /// right edge — i.e. inside a 40 lu gap.
    fn bar_scene() -> Scene {
        Scene {
            page_count: 2,
            design_page_height: H0,
            bg_angle: 90.0,
            bg_stops: vec![(0.0, hex("#000000")), (1.0, hex("#000000"))],
            elements: vec![Element {
                id: "bar",
                at: Placement {
                    page: 0,
                    x: 1020.0,
                    y: H0 / 2.0,
                    anchor: AnchorY::Center,
                    rotation: 90.0,
                },
                kind: ElementKind::Band {
                    length: 4000.0,
                    thickness: 30.0,
                    stops: vec![(0.0, hex("#FFFFFF")), (1.0, hex("#FFFFFF"))],
                },
            }],
            texts: HashMap::new(),
        }
    }

    fn max_red(pm: &Pixmap) -> u8 {
        pm.data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| p[0])
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn content_in_gap_is_omitted_from_page_exports() {
        let scene = bar_scene();
        let shots = Screenshots::default();
        let texts = Texts::new();
        let variant = Variant::none();
        let render_pages = |gap: f64| -> Vec<Pixmap> {
            let layout = StripLayout::new(2, H0, gap).for_target(132, 287, 1);
            let params = RenderParams {
                layout,
                variant: &variant,
                style: FrameStyle::Detailed,
            };
            (0..2)
                .map(|i| {
                    render(
                        &scene,
                        &texts,
                        &shots,
                        &params,
                        &image_viewport(&layout, i, 132, 287),
                    )
                })
                .collect()
        };
        let gapped = render_pages(40.0);
        assert!(max_red(&gapped[0]) < 10, "bar must not leak into page 0");
        assert!(max_red(&gapped[1]) < 10, "bar must not leak into page 1");
        let gapless = render_pages(0.0);
        assert!(
            max_red(&gapless[1]) > 200,
            "without gap the bar is on page 1"
        );
    }

    #[test]
    fn german_headlines_shrink_more_and_engines_agree() {
        let fonts = FontLibrary::bundled();
        let scene = demo_scene();
        let mut cosmic = CosmicEngine::new(&fonts);
        let mut parley = ParleyEngine::new(&fonts);
        for engine in [&mut cosmic as &mut dyn TextEngine, &mut parley] {
            let en = prepare_texts(&scene, "en", engine, &fonts);
            let de = prepare_texts(&scene, "de", engine, &fonts);
            let en1 = &en["title-1"];
            let de1 = &de["title-1"];
            assert!(de1.fits && en1.fits);
            assert!(de1.size < en1.size - 20.0, "{} vs {}", de1.size, en1.size);
            for t in en.values().chain(de.values()) {
                assert!(t.lines <= 2);
                assert!(t.width <= 840.5, "no overflow: {}", t.width);
            }
        }
        let c = prepare_texts(&scene, "de", &mut cosmic, &fonts);
        let p = prepare_texts(&scene, "de", &mut parley, &fonts);
        for id in c.keys() {
            assert!((c[id].size - p[id].size).abs() < 0.5, "{id}");
            assert_eq!(c[id].lines, p[id].lines, "{id}");
        }
    }
}
