//! Browser window chrome (`browser` model, `chrome: browser` on laptops and
//! desktops): tab bar with window controls and one tab, toolbar with
//! navigation icons and the URL field showing `url`. Generic artwork; the
//! sizes come from `browser_chrome` in `devices.yaml` (px at the reference
//! width, scaled with the window width).

use tiny_skia::{FillRule, LineCap, LineJoin, Path, PathBuilder, Pixmap, Stroke, Transform};
use vitrine_core::catalog::BrowserTheme;
use vitrine_core::model::Appearance;

use super::Ctx;
use super::glyphs;
use super::layout::{ChromeArea, browser_chrome};
use super::shapes;
use crate::paint::{Rgba, hex, solid, with_alpha};
use vitrine_core::geometry::Rect;

fn rgba(c: vitrine_core::model::Rgba) -> Rgba {
    [c.r, c.g, c.b, c.a]
}

fn theme(cx: &Ctx<'_, '_>) -> Option<&'static BrowserTheme> {
    let name = match cx.dev.appearance {
        Some(Appearance::Dark) => "dark",
        _ => "light",
    };
    browser_chrome().themes.get(name)
}

/// Stand-alone browser window.
pub fn window(cx: &mut Ctx<'_, '_>, img: Option<&Pixmap>) {
    let Some(area) = cx.l.chrome else {
        cx.screen(img);
        return;
    };
    chrome(cx, area);
    cx.screen(img);
    // Window outline: a hairline keeps the edge crisp on any background.
    let Some(t) = theme(cx) else { return };
    let dark = cx.dev.appearance == Some(Appearance::Dark);
    let line = if dark {
        [255, 255, 255, 40]
    } else {
        with_alpha(rgba(t.muted), 0.35)
    };
    let p = shapes::round(cx.l.body, cx.l.body_r);
    cx.stroke(p.as_ref(), &solid(line), area.k * 1.0);
}

/// Draws the chrome into `area`; the screenshot goes below it.
pub fn chrome(cx: &mut Ctx<'_, '_>, area: ChromeArea) {
    let Some(t) = theme(cx) else { return };
    let bc = browser_chrome();
    let k = area.k;
    let w = area.rect.w / k; // chrome px (= reference width)
    let (tab_bar, toolbar) = (bc.tab_bar, bc.toolbar);
    let r = area.radius / k;
    // All drawing below is in chrome px.
    let ts = cx
        .ts
        .pre_translate(area.rect.x as f32, area.rect.y as f32)
        .pre_scale(k as f32, k as f32);
    let dev = cx.dev;
    let url = dev.url.as_deref();
    let dark = dev.appearance == Some(Appearance::Dark);
    let mut d = Draw { cx, ts };

    // Frame (tab strip background); reaches a bit below the chrome so the
    // screenshot edge never shows the background through anti-aliasing.
    let frame = Rect::new(0.0, 0.0, w, tab_bar + toolbar + 4.0);
    d.fill(shapes::rect(frame, [r, r, 0.0, 0.0], false), rgba(t.frame));

    // Window controls.
    let tl = &bc.traffic_lights;
    let cy = tab_bar / 2.0 + 1.0;
    for (i, c) in ["#FF5F57", "#FEBC2E", "#28C840"].iter().enumerate() {
        let x = tl.left + tl.diameter / 2.0 + i as f64 * (tl.diameter + tl.spacing);
        d.fill(shapes::circle(x, cy, tl.diameter / 2.0), hex(c));
        d.stroke(
            shapes::circle(x, cy, tl.diameter / 2.0 - 0.25),
            [0, 0, 0, 30],
            0.5,
        );
    }

    // Active tab with flared bottom corners, merging into the toolbar.
    let tab_x = tl.left + 3.0 * tl.diameter + 2.0 * tl.spacing + 22.0;
    let tab_top = 7.0;
    let tw = bc.tab.width.min(w * 0.4);
    let tr = bc.tab.radius;
    d.fill(
        tab_shape(tab_x, tab_top, tw, tab_bar - tab_top + 0.5, tr),
        rgba(t.tab),
    );
    let tab_mid = f64::midpoint(tab_top, tab_bar);
    d.fill(shapes::circle(tab_x + 18.0, tab_mid, 7.0), hex("#4C8BF5"));
    d.fill(
        shapes::circle(tab_x + 18.0, tab_mid, 3.0),
        [255, 255, 255, 220],
    );
    let title = host(url).unwrap_or("New Tab").to_owned();
    d.text(
        &title,
        tab_x + 32.0,
        tab_mid,
        12.0,
        tw - 32.0 - 30.0,
        rgba(t.text),
    );
    let close_x = tab_x + tw - 18.0;
    d.cross(close_x, tab_mid, 4.0, rgba(t.muted), 1.3);
    // New tab button.
    let plus_x = tab_x + tw + 22.0;
    d.line(
        &[(plus_x - 6.0, tab_mid), (plus_x + 6.0, tab_mid)],
        rgba(t.muted),
        1.4,
    );
    d.line(
        &[(plus_x, tab_mid - 6.0), (plus_x, tab_mid + 6.0)],
        rgba(t.muted),
        1.4,
    );

    // Toolbar.
    d.fill(
        shapes::plain(Rect::new(0.0, tab_bar, w, toolbar + 4.0)),
        rgba(t.toolbar),
    );
    let ty = tab_bar + toolbar / 2.0;
    let muted = rgba(t.muted);
    // back / forward arrows, reload
    d.line(
        &[(30.0, ty - 6.0), (24.0, ty), (30.0, ty + 6.0)],
        muted,
        1.6,
    );
    d.line(&[(24.0, ty), (36.0, ty)], muted, 1.6);
    let faint = with_alpha(muted, 0.45);
    d.line(
        &[(62.0, ty - 6.0), (68.0, ty), (62.0, ty + 6.0)],
        faint,
        1.6,
    );
    d.line(&[(56.0, ty), (68.0, ty)], faint, 1.6);
    d.reload(96.0, ty, 6.5, muted);
    // URL field.
    let uh = bc.url_field.height;
    let ux = 120.0;
    let uw = (w - ux - 76.0).max(40.0);
    let field = Rect::new(ux, ty - uh / 2.0, uw, uh);
    d.fill(
        shapes::round(field, bc.url_field.radius.min(uh / 2.0)),
        rgba(t.url_field),
    );
    d.lock(ux + 18.0, ty, muted);
    let url = url.unwrap_or("");
    let shown = url
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');
    let (h, rest) = shown.find('/').map_or((shown, ""), |i| shown.split_at(i));
    let tx = ux + 34.0;
    let max = uw - 34.0 - 16.0;
    let used = d.text(h, tx, ty, 14.0, max, rgba(t.text));
    if !rest.is_empty() && used < max {
        d.text(rest, tx + used, ty, 14.0, max - used, muted);
    }
    // Profile and menu.
    let px = w - 52.0;
    d.fill(shapes::circle(px, ty, 11.0), with_alpha(muted, 0.35));
    d.fill(shapes::circle(px, ty - 3.0, 4.0), rgba(t.toolbar));
    for i in -1..=1 {
        d.fill(
            shapes::circle(w - 22.0, ty + f64::from(i) * 5.0, 1.7),
            muted,
        );
    }
    // Separator between chrome and page.
    let sep = if dark {
        [0, 0, 0, 120]
    } else {
        with_alpha(muted, 0.3)
    };
    d.fill(
        shapes::plain(Rect::new(0.0, tab_bar + toolbar - 1.0, w, 1.0)),
        sep,
    );
}

/// Host part of a URL (`https://example.com/x` → `example.com`).
fn host(url: Option<&str>) -> Option<&str> {
    let u = url?
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let h = u.split(['/', '?', '#']).next()?;
    (!h.is_empty()).then_some(h)
}

/// Tab outline: rounded top corners, outward flares at the bottom.
fn tab_shape(x: f64, y: f64, w: f64, h: f64, r: f64) -> Option<Path> {
    let k = 0.552_284_8;
    let b = y + h;
    let p = |x: f64, y: f64| (x as f32, y as f32);
    let mut pb = PathBuilder::new();
    let s = p(x - r, b);
    pb.move_to(s.0, s.1);
    let c = [p(x - r + k * r, b), p(x, b - r + k * r), p(x, b - r)];
    pb.cubic_to(c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1);
    let l = p(x, y + r);
    pb.line_to(l.0, l.1);
    let c = [p(x, y + r - k * r), p(x + r - k * r, y), p(x + r, y)];
    pb.cubic_to(c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1);
    let l = p(x + w - r, y);
    pb.line_to(l.0, l.1);
    let c = [
        p(x + w - r + k * r, y),
        p(x + w, y + r - k * r),
        p(x + w, y + r),
    ];
    pb.cubic_to(c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1);
    let l = p(x + w, b - r);
    pb.line_to(l.0, l.1);
    let c = [
        p(x + w, b - r + k * r),
        p(x + w + r - k * r, b),
        p(x + w + r, b),
    ];
    pb.cubic_to(c[0].0, c[0].1, c[1].0, c[1].1, c[2].0, c[2].1);
    pb.close();
    pb.finish()
}

/// Painter in chrome px.
struct Draw<'c, 'p, 'q> {
    cx: &'c mut Ctx<'p, 'q>,
    ts: Transform,
}

impl Draw<'_, '_, '_> {
    fn fill(&mut self, p: Option<Path>, c: Rgba) {
        if let Some(p) = p {
            self.cx
                .pm
                .fill_path(&p, &solid(c), FillRule::Winding, self.ts, None);
        }
    }

    fn stroke(&mut self, p: Option<Path>, c: Rgba, width: f64) {
        if let Some(p) = p {
            let s = Stroke {
                width: width as f32,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            self.cx.pm.stroke_path(&p, &solid(c), &s, self.ts, None);
        }
    }

    fn line(&mut self, pts: &[(f64, f64)], c: Rgba, width: f64) {
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(*x as f32, *y as f32);
            } else {
                pb.line_to(*x as f32, *y as f32);
            }
        }
        self.stroke(pb.finish(), c, width);
    }

    fn cross(&mut self, x: f64, y: f64, r: f64, c: Rgba, width: f64) {
        self.line(&[(x - r, y - r), (x + r, y + r)], c, width);
        self.line(&[(x - r, y + r), (x + r, y - r)], c, width);
    }

    /// Circular arrow (reload).
    fn reload(&mut self, x: f64, y: f64, r: f64, c: Rgba) {
        let mut pb = PathBuilder::new();
        let n = 24;
        for i in 0..=n {
            let a = (40.0 + 290.0 * f64::from(i) / f64::from(n)).to_radians();
            let (px, py) = (x + r * a.cos(), y - r * a.sin());
            if i == 0 {
                pb.move_to(px as f32, py as f32);
            } else {
                pb.line_to(px as f32, py as f32);
            }
        }
        self.stroke(pb.finish(), c, 1.6);
        // Arrow head at the start of the arc (40°).
        let a = 40f64.to_radians();
        let (hx, hy) = (x + r * a.cos(), y - r * a.sin());
        let mut pb = PathBuilder::new();
        pb.move_to((hx + 0.5) as f32, (hy - 4.5) as f32);
        pb.line_to((hx + 0.8) as f32, (hy + 0.6) as f32);
        pb.line_to((hx - 4.4) as f32, (hy + 0.4) as f32);
        pb.close();
        self.fill(pb.finish(), c);
    }

    fn lock(&mut self, x: f64, y: f64, c: Rgba) {
        let body = Rect::new(x - 5.0, y - 1.5, 10.0, 8.0);
        self.fill(shapes::round(body, 1.6), c);
        let mut pb = PathBuilder::new();
        pb.move_to((x - 3.2) as f32, (y - 1.5) as f32);
        pb.line_to((x - 3.2) as f32, (y - 4.0) as f32);
        pb.cubic_to(
            (x - 3.2) as f32,
            (y - 8.3) as f32,
            (x + 3.2) as f32,
            (y - 8.3) as f32,
            (x + 3.2) as f32,
            (y - 4.0) as f32,
        );
        pb.line_to((x + 3.2) as f32, (y - 1.5) as f32);
        self.stroke(pb.finish(), c, 1.5);
    }

    /// Single-line text, vertically centred on `cy`; returns its width.
    fn text(&mut self, s: &str, x: f64, cy: f64, size: f64, max: f64, c: Rgba) -> f64 {
        let Some(l) = glyphs::label(s, size as f32, max as f32) else {
            return 0.0;
        };
        let ts = self.ts.pre_translate(x as f32, cy as f32 + l.cap / 2.0);
        self.cx
            .pm
            .fill_path(&l.path, &solid(c), FillRule::Winding, ts, None);
        f64::from(l.width)
    }
}
