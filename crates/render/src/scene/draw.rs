//! The painter: backgrounds, then elements in z order. Each element is
//! drawn with its local transform (centre at the origin, rotation around
//! the pivot); opacity and clipping go through an offscreen layer covering
//! only the element's pixel bounds.

use tiny_skia::{
    FillRule, FilterQuality, Mask, Path, PathBuilder, Pattern, Pixmap, PixmapPaint, Shader,
    SpreadMode, Stroke, Transform,
};
use vitrine_core::geometry::{Rect, Vec2, Viewport};
use vitrine_core::model::{FitMode, ImageFillMode, Rgba, ShapeKind, StrokeJoin};
use vitrine_core::resolve::{
    ImportedFrame, ResolvedDevice, ResolvedElement, ResolvedFill, ResolvedKind,
    ResolvedShadowLayer, ResolvedShape, ResolvedStop, TextEffects,
};

use super::assets::Resample;
use super::{IssueKind, PreparedScene, Quality, RenderContext, RenderIssue, Rendered};
use crate::blur::{ShadowPx, draw_blurred, draw_shadow};
use crate::frames;
use crate::geometry::view_transform;
use crate::paint::{css_gradient_line, linear, radial, round_rect, shader_paint, solid};
use crate::raster::Filter;
use crate::text::{TextPaths, draw_text_with_effects, to_paths};

/// Shown where an image asset is missing or broken.
const MISSING_ASSET: [u8; 4] = [0xFF, 0x00, 0xFF, 0x60];

/// Screen of an imported frame without screenshot.
const SCREEN_PLACEHOLDER: [u8; 4] = [0xE6, 0xE9, 0xEE, 0xFF];

pub(super) fn paint(
    ctx: &RenderContext,
    prepared: &PreparedScene<'_>,
    view: &Viewport,
    quality: Quality,
) -> Rendered {
    let mut pm = Pixmap::new(view.width_px.max(1), view.height_px.max(1)).expect("viewport size");
    let mut p = Painter {
        ctx,
        prepared,
        view: *view,
        vt: view_transform(view),
        filter: quality.filter(),
        issues: Vec::new(),
    };
    p.backgrounds(&mut pm);
    for (i, e) in prepared.scene.elements.iter().enumerate() {
        p.element(&mut pm, i, e);
    }
    Rendered {
        pixmap: pm,
        viewport: *view,
        issues: p.issues,
    }
}

struct Painter<'a> {
    ctx: &'a RenderContext,
    prepared: &'a PreparedScene<'a>,
    view: Viewport,
    vt: Transform,
    filter: Filter,
    issues: Vec<RenderIssue>,
}

/// What an element draws, prepared in local coordinates.
enum Item<'a> {
    Fill {
        fill: &'a ResolvedFill,
        rect: Rect,
    },
    Shape {
        shape: &'a ResolvedShape,
        path: Path,
        rect: Rect,
    },
    Image {
        asset: &'a str,
        fit: FitMode,
        rect: Rect,
        natural: (f64, f64),
    },
    /// Image that could not be loaded.
    Missing {
        rect: Rect,
    },
    Device {
        dev: &'a ResolvedDevice,
        geo: frames::DeviceGeometry,
        /// Element size (lu); the frame's outer box.
        size: Vec2,
    },
    /// A device drawn with an installed vendor frame (ADR 0012).
    Imported {
        dev: &'a ResolvedDevice,
        frame: &'a ImportedFrame,
        /// Element size (lu) = the bezel's outer box.
        rect: Rect,
    },
    Text {
        paths: TextPaths,
        origin: (f32, f32),
    },
}

fn tsr(r: Rect) -> Option<tiny_skia::Rect> {
    tiny_skia::Rect::from_xywh(r.x as f32, r.y as f32, r.w as f32, r.h as f32)
}

fn rect_path(r: Rect) -> Option<Path> {
    tsr(r).map(PathBuilder::from_rect)
}

fn rgba(c: Rgba) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

/// Uniform scale of a transform (px per local unit).
fn scale_of(ts: Transform) -> f64 {
    f64::from(ts.sx).hypot(f64::from(ts.ky))
}

fn centered(w: f64, h: f64) -> Rect {
    Rect::new(-w / 2.0, -h / 2.0, w, h)
}

/// `(sw, sh)` fitted into `area` (contain or cover), positioned by `focus`.
fn fitted(sw: f64, sh: f64, area: Rect, cover: bool, focus: (f64, f64)) -> Rect {
    if sw <= 0.0 || sh <= 0.0 {
        return area;
    }
    let k = if cover {
        (area.w / sw).max(area.h / sh)
    } else {
        (area.w / sw).min(area.h / sh)
    };
    let (w, h) = (sw * k, sh * k);
    Rect::new(
        area.x + (area.w - w) * focus.0,
        area.y + (area.h - h) * focus.1,
        w,
        h,
    )
}

fn px(v: f64) -> u32 {
    v.round()
        .clamp(1.0, f64::from(super::assets::MAX_PIXMAP_SIDE)) as u32
}

/// Pattern shader mapping `img` onto `r` (local coordinates).
fn image_shader(img: &Pixmap, r: Rect, spread: SpreadMode) -> Shader<'_> {
    Pattern::new(
        img.as_ref(),
        spread,
        FilterQuality::Bilinear,
        1.0,
        Transform::from_row(
            (r.w / f64::from(img.width())) as f32,
            0.0,
            0.0,
            (r.h / f64::from(img.height())) as f32,
            r.x as f32,
            r.y as f32,
        ),
    )
}

fn stops(s: &[ResolvedStop]) -> Vec<(f32, [u8; 4])> {
    s.iter().map(|s| (s.pos as f32, rgba(s.color))).collect()
}

impl Painter<'_> {
    fn issue(&mut self, e: Option<&ResolvedElement>, message: String) {
        let issue = RenderIssue {
            element: e.map(|e| e.id.clone()),
            page: e.map(|e| e.page),
            kind: IssueKind::Asset { message },
        };
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }

    fn backgrounds(&mut self, pm: &mut Pixmap) {
        let canvas = self.prepared.scene.frame.canvas;
        // `contain` banners show more than the canvas: the background
        // extends into the margins.
        let vis = self.view.visible();
        let area = canvas.union(&Rect::new(
            vis.x - 1.0,
            vis.y - 1.0,
            vis.w + 2.0,
            vis.h + 2.0,
        ));
        let vt = self.vt;
        let Some(path) = rect_path(area) else { return };
        for fill in &self.prepared.scene.backgrounds {
            self.fill(pm, fill, &path, canvas, vt, None);
        }
    }

    /// Fills `area_path` with `fill` whose geometry (gradient line, image
    /// placement) is defined by `geom`; both in the local space of `ts`.
    /// Image fills: `cover`/`stretch`/`contain` map onto `geom` (`contain`
    /// centred, transparent around), `tile` repeats the image at its natural
    /// size (1 px or SVG unit = 1 lu) from the top-left of `geom`.
    fn fill(
        &mut self,
        pm: &mut Pixmap,
        fill: &ResolvedFill,
        area_path: &Path,
        geom: Rect,
        ts: Transform,
        e: Option<&ResolvedElement>,
    ) {
        let mask: Option<&Mask> = None;
        match fill {
            ResolvedFill::Color { color } => {
                pm.fill_path(area_path, &solid(rgba(*color)), FillRule::Winding, ts, mask);
            }
            ResolvedFill::LinearGradient { angle, stops: s } if !s.is_empty() => {
                let (p0, p1) = css_gradient_line(
                    geom.x as f32,
                    geom.y as f32,
                    geom.w as f32,
                    geom.h as f32,
                    *angle as f32,
                );
                let paint = shader_paint(linear(p0, p1, &stops(s)));
                pm.fill_path(area_path, &paint, FillRule::Winding, ts, mask);
            }
            ResolvedFill::RadialGradient {
                center,
                radius,
                stops: s,
            } if !s.is_empty() => {
                let c = (
                    (geom.x + center.x * geom.w) as f32,
                    (geom.y + center.y * geom.h) as f32,
                );
                let r = (radius * geom.w.max(geom.h)).max(1e-3) as f32;
                let paint = shader_paint(radial(c, r, &stops(s)));
                pm.fill_path(area_path, &paint, FillRule::Winding, ts, mask);
            }
            ResolvedFill::LinearGradient { .. } | ResolvedFill::RadialGradient { .. } => {}
            ResolvedFill::Image { asset, fit, focus } => {
                let natural = match self.ctx.assets.natural_size(asset) {
                    Ok(n) => n,
                    Err(err) => {
                        self.issue(e, err.to_string());
                        pm.fill_path(
                            area_path,
                            &solid(MISSING_ASSET),
                            FillRule::Winding,
                            ts,
                            mask,
                        );
                        return;
                    }
                };
                let s = scale_of(ts);
                let focus = (focus.x, focus.y);
                let (target, spread, mode) = match fit {
                    ImageFillMode::Cover => (geom, SpreadMode::Pad, Resample::Cover { focus }),
                    ImageFillMode::Stretch => (geom, SpreadMode::Pad, Resample::Stretch),
                    ImageFillMode::Contain => {
                        (geom, SpreadMode::Pad, Resample::ContainOn([0, 0, 0, 0]))
                    }
                    ImageFillMode::Tile => (
                        Rect::new(geom.x, geom.y, natural.0, natural.1),
                        SpreadMode::Repeat,
                        Resample::Stretch,
                    ),
                };
                match self.ctx.assets.pixmap(
                    asset,
                    px(target.w * s),
                    px(target.h * s),
                    mode,
                    self.filter,
                ) {
                    Ok(img) => {
                        let paint = shader_paint(image_shader(&img, target, spread));
                        pm.fill_path(area_path, &paint, FillRule::Winding, ts, mask);
                    }
                    Err(err) => self.issue(e, err.to_string()),
                }
            }
        }
    }

    fn item<'e>(&mut self, index: usize, e: &'e ResolvedElement) -> Option<Item<'e>> {
        let (w, h) = (e.size.x, e.size.y);
        Some(match &e.props {
            ResolvedKind::Background { fill } => Item::Fill {
                fill,
                rect: centered(w, h),
            },
            ResolvedKind::Shape(shape) => {
                let rect = centered(w, h);
                let path = match shape.shape {
                    ShapeKind::Rect if shape.radius > 0.0 => round_rect(
                        rect.x as f32,
                        rect.y as f32,
                        rect.w as f32,
                        rect.h as f32,
                        shape.radius as f32,
                    ),
                    ShapeKind::Rect => rect_path(rect),
                    ShapeKind::Ellipse => tsr(rect).and_then(PathBuilder::from_oval),
                    ShapeKind::Line => {
                        let mut pb = PathBuilder::new();
                        pb.move_to(rect.x as f32, 0.0);
                        pb.line_to(rect.right() as f32, 0.0);
                        pb.finish()
                    }
                }?;
                Item::Shape { shape, path, rect }
            }
            ResolvedKind::Image(img) => match self.ctx.assets.natural_size(&img.asset) {
                Ok(natural) => {
                    let h = if img.auto_height && natural.0 > 0.0 {
                        w * natural.1 / natural.0
                    } else {
                        h
                    };
                    Item::Image {
                        asset: &img.asset,
                        fit: img.fit,
                        rect: centered(w, h),
                        natural,
                    }
                }
                Err(err) => {
                    self.issue(Some(e), err.to_string());
                    Item::Missing {
                        rect: centered(w, h),
                    }
                }
            },
            ResolvedKind::Device(dev) if dev.imported.is_some() => Item::Imported {
                dev,
                frame: dev.imported.as_ref()?,
                rect: centered(w, h),
            },
            ResolvedKind::Device(dev) => {
                let size = Vec2::new(w, h);
                Item::Device {
                    dev,
                    geo: frames::device_geometry(dev, size),
                    size,
                }
            }
            ResolvedKind::Text(_) => {
                // Box origin relative to the element centre (the rotation
                // pivot frame); group offsets moved both alike.
                let placed = self.prepared.text(index)?;
                Item::Text {
                    paths: to_paths(&placed.layout, &self.ctx.fonts),
                    origin: (
                        (placed.origin.x - e.center.x) as f32,
                        (placed.origin.y - e.center.y) as f32,
                    ),
                }
            }
        })
    }

    /// Local bounds of what an item draws (before shadows).
    fn bounds(item: &Item<'_>) -> Option<tiny_skia::Rect> {
        match item {
            Item::Fill { rect, .. }
            | Item::Image { rect, .. }
            | Item::Missing { rect }
            | Item::Imported { rect, .. } => tsr(*rect),
            Item::Shape { shape, path, .. } => {
                let b = path.bounds();
                let m = shape.stroke.as_ref().map_or(0.0, |s| s.width as f32) / 2.0 + 1.0;
                tiny_skia::Rect::from_ltrb(b.left() - m, b.top() - m, b.right() + m, b.bottom() + m)
            }
            Item::Device { geo, .. } => {
                // Margin for buttons sticking out of the housing.
                let o = geo.outer;
                let m = o.w.max(o.h) * 0.03;
                tsr(Rect::new(o.x - m, o.y - m, o.w + 2.0 * m, o.h + 2.0 * m))
            }
            Item::Text { paths, origin } => {
                let b = paths.visual_bounds()?;
                tiny_skia::Rect::from_ltrb(
                    b.left() + origin.0 - 1.0,
                    b.top() + origin.1 - 1.0,
                    b.right() + origin.0 + 1.0,
                    b.bottom() + origin.1 + 1.0,
                )
            }
        }
    }

    fn shadows(e: &ResolvedElement) -> &[ResolvedShadowLayer] {
        match &e.props {
            ResolvedKind::Device(d) => &d.shadow,
            ResolvedKind::Shape(s) => &s.shadow,
            ResolvedKind::Image(i) => &i.shadow,
            ResolvedKind::Text(t) => &t.style.shadow,
            ResolvedKind::Background { .. } => &[],
        }
    }

    fn element(&mut self, pm: &mut Pixmap, index: usize, e: &ResolvedElement) {
        if e.opacity <= 0.0 {
            return;
        }
        let pivot = e.center + e.pivot;
        let lt = self
            .vt
            .pre_translate(pivot.x as f32, pivot.y as f32)
            .pre_rotate(e.rotation as f32)
            .pre_translate(-e.pivot.x as f32, -e.pivot.y as f32);
        // Cheap cull before preparing (text paths, geometry): a circle around
        // the element.
        let vis = self.view.visible();
        let mut shadow_lu = Self::shadows(e)
            .iter()
            .map(ResolvedShadowLayer::outset)
            .fold(0.0, f64::max);
        if let ResolvedKind::Text(t) = &e.props {
            // Outline/glow of the style or of any span style it may use.
            let spans = self.prepared.scene.spans.values().map(|s| {
                let fx = TextEffects {
                    stroke: s.stroke,
                    glow: s.glow,
                    shadow: s.shadow.clone().unwrap_or_default(),
                };
                fx.visual_outset()
            });
            shadow_lu = spans.fold(t.style.effects().visual_outset(), f64::max);
        }
        let r =
            (e.size.x.hypot(e.size.y) / 2.0 + e.pivot.x.hypot(e.pivot.y)) * 1.1 + shadow_lu + 2.0;
        let auto_h = matches!(&e.props, ResolvedKind::Text(t) if t.frame.auto_height)
            || matches!(&e.props, ResolvedKind::Image(i) if i.auto_height);
        if !auto_h && !Rect::new(e.center.x - r, e.center.y - r, 2.0 * r, 2.0 * r).intersects(&vis)
        {
            return;
        }
        let Some(item) = self.item(index, e) else {
            return;
        };
        let Some(local) = Self::bounds(&item) else {
            return;
        };
        let margin = (shadow_lu * self.view.scale) as f32 + 2.0;
        let Some(b) = local.transform(lt).and_then(|b| b.outset(margin, margin)) else {
            return;
        };
        let (vw, vh) = (pm.width() as f32, pm.height() as f32);
        let x0 = b.left().floor().max(0.0);
        let y0 = b.top().floor().max(0.0);
        let x1 = b.right().ceil().min(vw);
        let y1 = b.bottom().ceil().min(vh);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        // Clip rect in pixels (the view transform is axis-aligned).
        let mut mask = None;
        if let Some(c) = e.clip {
            let Some(cp) = tsr(c).and_then(|r| r.transform(self.vt)) else {
                return;
            };
            if cp.right() <= x0 || cp.left() >= x1 || cp.bottom() <= y0 || cp.top() >= y1 {
                return;
            }
            let inside = cp.left() <= x0 && cp.top() <= y0 && cp.right() >= x1 && cp.bottom() >= y1;
            if !inside {
                let mut m = Mask::new(pm.width(), pm.height()).expect("mask size");
                m.fill_path(
                    &PathBuilder::from_rect(cp),
                    FillRule::Winding,
                    true,
                    Transform::identity(),
                );
                mask = Some(m);
            }
        }
        if e.opacity >= 1.0 && mask.is_none() {
            self.draw_item(pm, &item, e, lt);
            return;
        }
        let (lw, lh) = ((x1 - x0) as u32, (y1 - y0) as u32);
        let Some(mut layer) = Pixmap::new(lw, lh) else {
            return;
        };
        self.draw_item(&mut layer, &item, e, lt.post_translate(-x0, -y0));
        pm.draw_pixmap(
            x0 as i32,
            y0 as i32,
            layer.as_ref(),
            &PixmapPaint {
                opacity: e.opacity.clamp(0.0, 1.0) as f32,
                ..PixmapPaint::default()
            },
            Transform::identity(),
            mask.as_ref(),
        );
    }

    fn shadow(&self, pm: &mut Pixmap, path: &Path, ts: Transform, layers: &[ResolvedShadowLayer]) {
        let s = self.view.scale;
        for l in layers {
            draw_shadow(
                pm,
                path,
                ts,
                ShadowPx {
                    sigma: l.blur * s,
                    dx: l.x * s,
                    dy: l.y * s,
                    color: rgba(l.color),
                },
            );
        }
    }

    /// Drop shadow from the alpha channel of what `paint` draws into `path`
    /// (images with transparency, like CSS `filter: drop-shadow`).
    fn alpha_shadow(
        &self,
        pm: &mut Pixmap,
        path: &Path,
        paint: &tiny_skia::Paint<'_>,
        ts: Transform,
        layers: &[ResolvedShadowLayer],
    ) {
        let Some(bounds) = path.clone().transform(ts).map(|p| p.bounds()) else {
            return;
        };
        let s = self.view.scale;
        for l in layers {
            draw_blurred(
                pm,
                bounds,
                ShadowPx {
                    sigma: l.blur * s,
                    dx: l.x * s,
                    dy: l.y * s,
                    color: rgba(l.color),
                },
                |mask, to_mask| {
                    mask.fill_path(
                        path,
                        paint,
                        FillRule::Winding,
                        ts.post_concat(to_mask),
                        None,
                    );
                },
            );
        }
    }

    /// The device's screenshot resampled (and fitted) to `w × h` px.
    fn screenshot(
        &mut self,
        dev: &ResolvedDevice,
        e: &ResolvedElement,
        w: f64,
        h: f64,
    ) -> Option<std::sync::Arc<Pixmap>> {
        let path = dev.screenshot.as_ref()?.path.as_deref()?;
        let mode = match dev.fit {
            FitMode::Cover => Resample::Cover { focus: (0.5, 0.5) },
            FitMode::Contain => Resample::ContainOn([0, 0, 0, 255]),
            FitMode::Stretch => Resample::Stretch,
        };
        match self
            .ctx
            .assets
            .pixmap(path, px(w), px(h), mode, self.filter)
        {
            Ok(p) => Some(p),
            Err(err) => {
                self.issue(Some(e), err.to_string());
                None
            }
        }
    }

    /// An imported device at `scale` px/lu as one pixmap covering `rect`:
    /// the screenshot in the detected screen rect, clipped by the screen
    /// mask, the bezel PNG on top.
    fn imported_device(
        &mut self,
        dev: &ResolvedDevice,
        frame: &ImportedFrame,
        rect: Rect,
        e: &ResolvedElement,
        scale: f64,
    ) -> Result<Pixmap, String> {
        let cache = self
            .ctx
            .frame_assets()
            .ok_or("imported frames are not available")?;
        let (cw, ch) = (px(rect.w * scale), px(rect.h * scale));
        let bezel = cache
            .pixmap(&frame.file, cw, ch, Resample::Stretch, self.filter)
            .map_err(|e| e.to_string())?;
        let mask = cache
            .pixmap(&frame.mask, cw, ch, Resample::Stretch, Filter::Bilinear)
            .map_err(|e| e.to_string())?;
        let mut comp = Pixmap::new(cw, ch).ok_or("empty frame")?;
        let (kx, ky) = (
            f64::from(cw) / f64::from(frame.size.w.max(1)),
            f64::from(ch) / f64::from(frame.size.h.max(1)),
        );
        let sr = Rect::new(
            f64::from(frame.screen.x) * kx,
            f64::from(frame.screen.y) * ky,
            f64::from(frame.screen.w) * kx,
            f64::from(frame.screen.h) * ky,
        );
        if let Some(p) = rect_path(sr) {
            match self.screenshot(dev, e, sr.w, sr.h) {
                Some(img) => {
                    let paint = shader_paint(image_shader(&img, sr, SpreadMode::Pad));
                    comp.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
                }
                None => {
                    comp.fill_path(
                        &p,
                        &solid(SCREEN_PLACEHOLDER),
                        FillRule::Winding,
                        Transform::identity(),
                        None,
                    );
                }
            }
        }
        // Clip by the mask (grey value; the mask PNG is opaque).
        for (px, m) in comp
            .data_mut()
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(mask.data().as_chunks::<4>().0)
        {
            let k = u16::from(m[0]);
            if k < 255 {
                for c in px.iter_mut() {
                    *c = ((u16::from(*c) * k + 127) / 255) as u8;
                }
            }
        }
        comp.draw_pixmap(
            0,
            0,
            bezel.as_ref().as_ref(),
            &PixmapPaint::default(),
            Transform::identity(),
            None,
        );
        Ok(comp)
    }

    #[allow(clippy::too_many_lines, reason = "one arm per element kind")]
    fn draw_item(&mut self, pm: &mut Pixmap, item: &Item<'_>, e: &ResolvedElement, lt: Transform) {
        let shadows = Self::shadows(e);
        match item {
            Item::Fill { fill, rect } => {
                if let Some(p) = rect_path(*rect) {
                    self.fill(pm, fill, &p, *rect, lt, Some(e));
                }
            }
            Item::Shape { shape, path, rect } => {
                let stroke = shape.stroke.as_ref().map(|s| {
                    (
                        solid(rgba(s.color)),
                        Stroke {
                            width: s.width as f32,
                            line_join: match s.join {
                                StrokeJoin::Round => tiny_skia::LineJoin::Round,
                                StrokeJoin::Miter => tiny_skia::LineJoin::Miter,
                                StrokeJoin::Bevel => tiny_skia::LineJoin::Bevel,
                            },
                            ..Stroke::default()
                        },
                    )
                });
                if !shadows.is_empty() {
                    let silhouette = if shape.fill.is_some() || shape.shape == ShapeKind::Line {
                        stroke
                            .as_ref()
                            .filter(|_| shape.shape == ShapeKind::Line)
                            .and_then(|(_, st)| path.stroke(st, 1.0))
                            .or_else(|| Some(path.clone()))
                    } else {
                        stroke.as_ref().and_then(|(_, st)| path.stroke(st, 1.0))
                    };
                    if let Some(sil) = silhouette {
                        self.shadow(pm, &sil, lt, shadows);
                    }
                }
                if let Some(fill) = &shape.fill {
                    if shape.shape == ShapeKind::Line {
                        // A line without stroke: its fill colour as stroke.
                        if stroke.is_none()
                            && let ResolvedFill::Color { color } = fill
                        {
                            let st = Stroke {
                                width: rect.h.max(1.0) as f32,
                                ..Stroke::default()
                            };
                            pm.stroke_path(path, &solid(rgba(*color)), &st, lt, None);
                        }
                    } else {
                        self.fill(pm, fill, path, *rect, lt, Some(e));
                    }
                }
                if let Some((paint, st)) = &stroke {
                    pm.stroke_path(path, paint, st, lt, None);
                }
            }
            Item::Image {
                asset,
                fit,
                rect,
                natural,
            } => {
                let s = scale_of(lt);
                let (target, mode) = match fit {
                    FitMode::Cover => (*rect, Resample::Cover { focus: (0.5, 0.5) }),
                    FitMode::Stretch => (*rect, Resample::Stretch),
                    FitMode::Contain => (
                        fitted(natural.0, natural.1, *rect, false, (0.5, 0.5)),
                        Resample::Stretch,
                    ),
                };
                if let Some(p) = rect_path(target) {
                    let img = self.ctx.assets.pixmap(
                        asset,
                        px(target.w * s),
                        px(target.h * s),
                        mode,
                        self.filter,
                    );
                    match img {
                        Ok(img) => {
                            let paint = shader_paint(image_shader(&img, target, SpreadMode::Pad));
                            if !shadows.is_empty() {
                                if self.ctx.assets.is_opaque(asset) {
                                    // Same result as the alpha mask, cheaper.
                                    self.shadow(pm, &p, lt, shadows);
                                } else {
                                    self.alpha_shadow(pm, &p, &paint, lt, shadows);
                                }
                            }
                            pm.fill_path(&p, &paint, FillRule::Winding, lt, None);
                        }
                        Err(err) => {
                            if !shadows.is_empty() {
                                self.shadow(pm, &p, lt, shadows);
                            }
                            self.issue(Some(e), err.to_string());
                        }
                    }
                }
            }
            Item::Missing { rect } => {
                if let Some(p) = rect_path(*rect) {
                    pm.fill_path(&p, &solid(MISSING_ASSET), FillRule::Winding, lt, None);
                }
            }
            Item::Device { dev, geo, size } => {
                if !shadows.is_empty() {
                    self.shadow(pm, &geo.silhouette, lt, shadows);
                }
                let s = scale_of(lt);
                let shot = self.screenshot(dev, e, geo.screen.w * s, geo.screen.h * s);
                frames::draw_device(&mut pm.as_mut(), dev, *size, shot.as_deref(), lt);
            }
            Item::Imported { dev, frame, rect } => {
                match self.imported_device(dev, frame, *rect, e, scale_of(lt)) {
                    Ok(comp) => {
                        if let Some(p) = rect_path(*rect) {
                            let paint = shader_paint(image_shader(&comp, *rect, SpreadMode::Pad));
                            if !shadows.is_empty() {
                                self.alpha_shadow(pm, &p, &paint, lt, shadows);
                            }
                            pm.fill_path(&p, &paint, FillRule::Winding, lt, None);
                        }
                    }
                    Err(err) => {
                        // Unreadable frame files: the parametric frame,
                        // squeezed into the bezel's box.
                        self.issue(Some(e), err);
                        let size = Vec2::new(rect.w, rect.h);
                        let geo = frames::device_geometry(dev, size);
                        let s = scale_of(lt);
                        let shot = self.screenshot(dev, e, geo.screen.w * s, geo.screen.h * s);
                        frames::draw_device(&mut pm.as_mut(), dev, size, shot.as_deref(), lt);
                    }
                }
            }
            Item::Text { paths, origin } => {
                let ts = lt.pre_translate(origin.0, origin.1);
                draw_text_with_effects(pm, paths, ts, self.view.scale);
            }
        }
    }
}
