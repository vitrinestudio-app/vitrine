//! Device adapter: `device_geometry` + `draw_device` for resolved devices.
//!
//! **Temporary.** This implements the interface of the catalog-driven
//! frames module (`crate::frames`, Phase 1 task 1.6) on top of the Phase 0
//! spike frames: phones, tablets, laptop lids and monitors use the spike's
//! housing/bezel/island drawing with geometry taken from the catalog;
//! laptop bases, stands, notches, punch holes and the browser window are
//! simple flat shapes. Once the real frames land, both functions forward to
//! `crate::frames`.
//!
//! Units: device-local **model units** (catalog `unit`), centred on the
//! outer box (body plus laptop base / stand) in the effective orientation.
//! The caller scales to the element width (`size.x / outer.w`).

#![allow(clippy::many_single_char_names, reason = "geometry code")]

use std::cell::RefCell;

use arrangia_core::catalog::{CutoutKind, CutoutShape, Device, Edge};
use arrangia_core::layout::{FontLibrary, ResolvedTextStyle, TextInput, TextLayouter};
use arrangia_core::model::{Align, Appearance, FrameStyle as ModelFrameStyle, Rgba};
use arrangia_core::resolve::{DeviceColor, ResolvedDevice};
use tiny_skia::{FillRule, Path, PathBuilder, Pixmap, PixmapMut, PixmapPaint, Stroke, Transform};

use crate::frames::{self as spike, Button, FrameColors, FrameStyle};
use crate::geometry::Rect;
use crate::paint::{self, circle, round_rect, solid};

/// Frame geometry in device-local units (see module docs).
#[derive(Debug, Clone)]
pub struct DeviceGeometry {
    /// Outer box, centred at (0, 0).
    pub outer: Rect,
    /// Visible display area.
    pub screen: Rect,
    pub screen_radius: f32,
    /// Outer shape (housing incl. laptop base / stand), for shadows.
    pub silhouette: Path,
}

/// Natural-orientation layout of the catalog model, centred on the outer box.
struct Natural {
    outer: Rect,
    body: Rect,
}

fn natural(d: &Device) -> Natural {
    let o = d.outer_size();
    let outer = Rect::new(-o.w / 2.0, -o.h / 2.0, o.w, o.h);
    let body = Rect::new(-d.body.w / 2.0, outer.y, d.body.w, d.body.h);
    Natural { outer, body }
}

/// Is the frame drawn rotated by 90° against its catalog geometry?
fn rotated(dev: &ResolvedDevice) -> bool {
    dev.orientation != dev.device.natural
}

/// Natural → effective orientation: 90° counter-clockwise (KONZEPT §4.1:
/// the top edge goes to the left).
fn rotation() -> Transform {
    Transform::from_row(0.0, -1.0, 1.0, 0.0, 0.0, 0.0)
}

fn rotate_rect(r: Rect) -> Rect {
    // (x, y) → (y, −x)
    Rect::new(r.y, -r.right(), r.h, r.w)
}

fn rr(r: Rect, radius: f64) -> Option<Path> {
    round_rect(
        r.x as f32,
        r.y as f32,
        r.w as f32,
        r.h as f32,
        radius as f32,
    )
}

fn display_rect(d: &Device, body: Rect) -> Rect {
    Rect::new(
        body.x + d.display.x,
        body.y + d.display.y,
        d.display.w,
        d.display.h,
    )
}

/// Housing pieces in natural orientation (body, hinge, base, neck, foot).
fn pieces(d: &Device, n: &Natural) -> Vec<(Rect, f64)> {
    let mut v = vec![(n.body, d.body.radius)];
    let mut y = n.body.bottom();
    if let Some(l) = &d.laptop {
        v.push((
            Rect::new(-l.hinge.w / 2.0, y, l.hinge.w, l.hinge.h),
            l.hinge.radius,
        ));
        y += l.hinge.h;
        v.push((
            Rect::new(-l.base.w / 2.0, y, l.base.w, l.base.h),
            l.base.radius,
        ));
    }
    if let Some(s) = &d.stand {
        v.push((
            Rect::new(-s.neck.w / 2.0, y, s.neck.w, s.neck.h),
            s.neck.radius,
        ));
        y += s.neck.h;
        v.push((
            Rect::new(-s.foot.w / 2.0, y, s.foot.w, s.foot.h),
            s.foot.radius,
        ));
    }
    v
}

pub fn device_geometry(dev: &ResolvedDevice) -> DeviceGeometry {
    let d = &dev.device;
    let n = natural(d);
    let mut pb = PathBuilder::new();
    for (r, radius) in pieces(d, &n) {
        if let Some(p) = rr(r, radius) {
            pb.push_path(&p);
        }
    }
    let silhouette = pb.finish().or_else(|| rr(n.outer, 0.0)).unwrap_or_else(|| {
        PathBuilder::from_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, 1.0, 1.0).expect("unit rect"))
    });
    let screen = display_rect(d, n.body);
    if rotated(dev) {
        DeviceGeometry {
            outer: rotate_rect(n.outer),
            screen: rotate_rect(screen),
            screen_radius: d.display.radius as f32,
            silhouette: silhouette
                .clone()
                .transform(rotation())
                .unwrap_or(silhouette),
        }
    } else {
        DeviceGeometry {
            outer: n.outer,
            screen,
            screen_radius: d.display.radius as f32,
            silhouette,
        }
    }
}

fn rgba(c: Rgba) -> paint::Rgba {
    [c.r, c.g, c.b, c.a]
}

fn mix(a: paint::Rgba, b: paint::Rgba, t: f32) -> paint::Rgba {
    let m = |x: u8, y: u8| (f32::from(x) * (1.0 - t) + f32::from(y) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), 255]
}

fn colors(dev: &ResolvedDevice, style: FrameStyle) -> FrameColors {
    match (&dev.palette, &dev.color) {
        (Some(p), _) => match style {
            FrameStyle::Detailed => FrameColors {
                rim_light: rgba(p.rim[0]),
                rim_mid: rgba(p.rim[1]),
                rim_dark: rgba(p.rim[2]),
                bezel: rgba(p.glass),
            },
            FrameStyle::Stylized => FrameColors {
                rim_light: rgba(p.body),
                rim_mid: rgba(p.body),
                rim_dark: rgba(p.edge),
                bezel: rgba(p.body),
            },
        },
        (None, DeviceColor::Custom(c)) => {
            let c = rgba(*c);
            FrameColors {
                rim_light: mix(c, [255, 255, 255, 255], 0.45),
                rim_mid: c,
                rim_dark: mix(c, [0, 0, 0, 255], 0.45),
                bezel: if style == FrameStyle::Detailed {
                    [6, 6, 7, 255]
                } else {
                    c
                },
            }
        }
        _ => FrameColors::graphite(),
    }
}

/// Spike geometry for the body of a catalog model (natural orientation).
fn spike_geometry(d: &Device, n: &Natural) -> spike::DeviceGeometry {
    let body = n.body;
    let rim = if d.rim > 0.0 { d.rim } else { body.w * 0.006 };
    let glass = Rect::new(
        body.x + rim,
        body.y + rim,
        body.w - 2.0 * rim,
        body.h - 2.0 * rim,
    );
    let screen = display_rect(d, body);
    let mut island = None;
    let mut camera = None;
    for c in &d.cutouts {
        let r = Rect::new(body.x + c.x, body.y + c.y, c.w, c.h);
        match c.kind {
            CutoutKind::Island => island = Some(r),
            CutoutKind::Camera => camera = Some((r.center().x, r.center().y, r.w / 2.0)),
            _ => {}
        }
    }
    let buttons = d
        .buttons
        .iter()
        .map(|b| {
            let t = 2.0 * b.depth;
            let rect = match b.edge {
                Edge::Left => Rect::new(body.x - b.depth, body.y + b.at, t, b.length),
                Edge::Right => Rect::new(body.right() - b.depth, body.y + b.at, t, b.length),
                Edge::Top => Rect::new(body.x + b.at, body.y - b.depth, b.length, t),
                Edge::Bottom => Rect::new(body.x + b.at, body.bottom() - b.depth, b.length, t),
            };
            Button { rect }
        })
        .collect();
    spike::DeviceGeometry {
        model: spike::DeviceModel::PhoneIsland,
        body,
        body_r: d.body.radius,
        rim,
        glass,
        glass_r: (d.body.radius - rim).max(0.0),
        screen,
        screen_r: d.display.radius,
        island,
        bezel_camera: camera,
        buttons,
    }
}

/// Pixel bounds of `path` under `ts`, grown by `margin` px and clipped to
/// `w × h`.
fn px_bounds(
    path: &Path,
    ts: Transform,
    margin: f32,
    w: u32,
    h: u32,
) -> Option<(i32, i32, u32, u32)> {
    let b = path.clone().transform(ts)?.bounds();
    let x0 = (b.left() - margin).floor().max(0.0);
    let y0 = (b.top() - margin).floor().max(0.0);
    let x1 = (b.right() + margin).ceil().min(w as f32);
    let y1 = (b.bottom() + margin).ceil().min(h as f32);
    (x1 > x0 && y1 > y0).then_some((x0 as i32, y0 as i32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// Rotates a pixmap 90° clockwise.
fn rotate_cw(src: &Pixmap) -> Option<Pixmap> {
    let (w, h) = (src.width() as usize, src.height() as usize);
    let mut dst = Pixmap::new(src.height(), src.width())?;
    let s = src.data();
    let d = dst.data_mut();
    for y in 0..w {
        for x in 0..h {
            // dst(x, y) = src(y, h − 1 − x)
            let si = ((h - 1 - x) * w + y) * 4;
            let di = (y * h + x) * 4;
            d[di..di + 4].copy_from_slice(&s[si..si + 4]);
        }
    }
    Some(dst)
}

pub fn draw_device(
    pixmap: &mut PixmapMut<'_>,
    dev: &ResolvedDevice,
    screen: Option<&Pixmap>,
    transform: Transform,
) {
    let geo = device_geometry(dev);
    // The spike frames draw into a `Pixmap`: render into a temporary one
    // covering the device and composite.
    let Some((x, y, w, h)) = px_bounds(
        &geo.silhouette,
        transform,
        4.0,
        pixmap.width(),
        pixmap.height(),
    ) else {
        return;
    };
    let Some(mut tmp) = Pixmap::new(w, h) else {
        return;
    };
    let local = transform.post_translate(-x as f32, -y as f32);
    draw_into(&mut tmp, dev, &geo, screen, local);
    pixmap.draw_pixmap(
        x,
        y,
        tmp.as_ref(),
        &PixmapPaint::default(),
        Transform::identity(),
        None,
    );
}

fn draw_into(
    pm: &mut Pixmap,
    dev: &ResolvedDevice,
    geo: &DeviceGeometry,
    screen: Option<&Pixmap>,
    transform: Transform,
) {
    let d = &dev.device;
    let n = natural(d);
    let rot = rotated(dev);
    let nts = if rot {
        transform.pre_concat(rotation())
    } else {
        transform
    };
    let rotated_shot = match (rot, screen) {
        (true, Some(s)) => rotate_cw(s),
        _ => None,
    };
    let shot = rotated_shot.as_ref().or(screen);
    let style = match dev.frame {
        ModelFrameStyle::Stylized => FrameStyle::Stylized,
        ModelFrameStyle::Detailed | ModelFrameStyle::Imported(_) => FrameStyle::Detailed,
    };
    let colors = colors(dev, style);

    // Laptop base / stand (behind the body).
    for (r, radius) in pieces(d, &n).into_iter().skip(1) {
        if let Some(p) = rr(r, radius) {
            let c = if style == FrameStyle::Detailed {
                colors.rim_mid
            } else {
                colors.rim_dark
            };
            pm.fill_path(&p, &solid(c), FillRule::Winding, nts, None);
        }
    }

    if d.browser {
        draw_browser(pm, dev, &n, shot, nts);
    } else if d.body.w <= d.display.w && d.body.h <= d.display.h {
        // `none`: just the rounded screenshot.
        let s = display_rect(d, n.body);
        fill_screen(pm, s, d.display.radius, shot, nts);
    } else {
        let g = spike_geometry(d, &n);
        spike::draw_device(pm, &g, style, &colors, shot, nts);
        // Cutouts the spike does not know.
        for c in &d.cutouts {
            let r = Rect::new(n.body.x + c.x, n.body.y + c.y, c.w, c.h);
            let black = solid([0, 0, 0, 255]);
            match (c.kind, c.shape) {
                (CutoutKind::Island | CutoutKind::Camera, _) => {}
                (CutoutKind::HomeButton, _) => {
                    if let Some(p) =
                        circle(r.center().x as f32, r.center().y as f32, (r.w / 2.0) as f32)
                    {
                        let stroke = Stroke {
                            width: (r.w * 0.06) as f32,
                            ..Stroke::default()
                        };
                        pm.stroke_path(&p, &solid(colors.rim_dark), &stroke, nts, None);
                    }
                }
                (_, CutoutShape::Circle) => {
                    if let Some(p) =
                        circle(r.center().x as f32, r.center().y as f32, (r.w / 2.0) as f32)
                    {
                        pm.fill_path(&p, &black, FillRule::Winding, nts, None);
                    }
                }
                (CutoutKind::Speaker, _) => {
                    if let Some(p) = rr(r, r.h / 2.0) {
                        pm.fill_path(&p, &solid([40, 40, 44, 255]), FillRule::Winding, nts, None);
                    }
                }
                _ => {
                    if let Some(p) = rr(r, c.radius) {
                        pm.fill_path(&p, &black, FillRule::Winding, nts, None);
                    }
                }
            }
        }
    }
    if screen.is_none() {
        placeholder(pm, dev, geo, transform);
    }
}

fn fill_screen(pm: &mut Pixmap, s: Rect, radius: f64, shot: Option<&Pixmap>, ts: Transform) {
    let Some(path) = rr(s, radius) else { return };
    if let Some(img) = shot {
        let pat_ts = Transform::from_row(
            s.w as f32 / img.width() as f32,
            0.0,
            0.0,
            s.h as f32 / img.height() as f32,
            s.x as f32,
            s.y as f32,
        );
        let pat = tiny_skia::Pattern::new(
            img.as_ref(),
            tiny_skia::SpreadMode::Pad,
            tiny_skia::FilterQuality::Bilinear,
            1.0,
            pat_ts,
        );
        pm.fill_path(
            &path,
            &paint::shader_paint(pat),
            FillRule::Winding,
            ts,
            None,
        );
    } else {
        pm.fill_path(
            &path,
            &solid(paint::hex("#2A2A2E")),
            FillRule::Winding,
            ts,
            None,
        );
    }
}

/// Minimal browser window: chrome bar, traffic lights, URL field, content.
fn draw_browser(
    pm: &mut Pixmap,
    dev: &ResolvedDevice,
    n: &Natural,
    shot: Option<&Pixmap>,
    ts: Transform,
) {
    let d = &dev.device;
    let dark = dev.appearance == Some(Appearance::Dark);
    let (frame, field) = if dark {
        ([0x35, 0x36, 0x3A, 255], [0x20, 0x21, 0x24, 255])
    } else {
        ([0xDE, 0xE1, 0xE6, 255], [0xF1, 0xF3, 0xF4, 255])
    };
    let b = n.body;
    if let Some(p) = rr(b, d.body.radius) {
        pm.fill_path(&p, &solid(frame), FillRule::Winding, ts, None);
    }
    let bar = d.display.y;
    for (i, c) in [
        [0xFF, 0x5F, 0x57, 255],
        [0xFE, 0xBC, 0x2E, 255],
        [0x28, 0xC8, 0x40, 255],
    ]
    .into_iter()
    .enumerate()
    {
        let cx = b.x + bar * 0.25 + i as f64 * bar * 0.22;
        if let Some(p) = circle(cx as f32, (b.y + bar / 2.0) as f32, (bar * 0.07) as f32) {
            pm.fill_path(&p, &solid(c), FillRule::Winding, ts, None);
        }
    }
    let url = Rect::new(b.x + b.w * 0.22, b.y + bar * 0.25, b.w * 0.56, bar * 0.5);
    if let Some(p) = rr(url, url.h / 2.0) {
        pm.fill_path(&p, &solid(field), FillRule::Winding, ts, None);
    }
    let s = display_rect(d, b);
    fill_screen(pm, s, d.display.radius, shot, ts);
}

thread_local! {
    static LAYOUTER: RefCell<Option<TextLayouter>> = const { RefCell::new(None) };
}

/// The screenshot key on the empty screen (KONZEPT §4.3).
fn placeholder(pm: &mut Pixmap, dev: &ResolvedDevice, geo: &DeviceGeometry, ts: Transform) {
    let Some(key) = dev.screenshot.as_ref().map(|s| s.key.clone()) else {
        return;
    };
    let s = geo.screen;
    let style = ResolvedTextStyle {
        size: (s.w.min(s.h) * 0.07) as f32,
        weight: 600,
        color: Rgba::rgb(0xA0, 0xA0, 0xA8),
        ..ResolvedTextStyle::default()
    };
    LAYOUTER.with(|l| {
        let mut l = l.borrow_mut();
        let l = l.get_or_insert_with(|| TextLayouter::new(FontLibrary::bundled()));
        let layout = l.layout(&TextInput {
            align: Align::Center,
            ..TextInput::new(&key, &style, (s.w * 0.9) as f32)
        });
        let x = s.x + s.w * 0.05;
        let y = s.center().y - f64::from(layout.r#box.h) / 2.0;
        crate::text::draw_layout(
            pm,
            &layout,
            l.fonts(),
            ts.pre_translate(x as f32, y as f32),
            None,
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotate_cw_moves_top_left_to_top_right() {
        let mut pm = Pixmap::new(3, 2).unwrap();
        pm.data_mut()[0..4].copy_from_slice(&[255, 0, 0, 255]);
        let r = rotate_cw(&pm).unwrap();
        assert_eq!((r.width(), r.height()), (2, 3));
        assert_eq!(r.pixel(1, 0).unwrap().red(), 255);
    }

    #[test]
    fn rotate_rect_swaps_size() {
        let r = rotate_rect(Rect::new(-10.0, -20.0, 20.0, 40.0));
        assert_eq!(r, Rect::new(-20.0, -10.0, 40.0, 20.0));
    }
}
