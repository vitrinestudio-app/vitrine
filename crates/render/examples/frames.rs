//! Contact sheet of all built-in parametric frames.
//!
//! ```sh
//! cargo run -p arrangia-render --release --example frames
//! ```
//!
//! Writes `docs/spike/frames_catalog_stylized.png` and
//! `docs/spike/frames_catalog_detailed.png`: every catalog model in portrait
//! and landscape (where supported), a few colour variants, browser chrome in
//! light and dark. The screenshots are generated fake UIs of the right
//! aspect (no real app content).

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "example drawing code"
)]

use std::path::Path;

use arrangia_core::geometry::Rect;
use arrangia_core::model::{Appearance, Chrome, FrameStyle, Orientation};
use arrangia_render::blur::{ShadowPx, draw_shadow};
use arrangia_render::frames::{
    catalog_device, device_geometry, device_size, draw_device, glyphs, shapes,
};
use arrangia_render::paint::{Rgba, hex, linear, shader_paint, solid, with_alpha};
use arrangia_render::raster::save_png_rgb;
use tiny_skia::{FillRule, Pixmap, Transform};

const SHEET_W: f64 = 2800.0;
const GAP: f64 = 70.0;
const LABEL_H: f64 = 70.0;

#[derive(Clone, Copy)]
enum Ui {
    /// Mobile/tablet app.
    App,
    /// Web page (marketing site).
    Web,
}

#[derive(Clone, Copy)]
struct Item {
    model: &'static str,
    orientation: Orientation,
    color: Option<&'static str>,
    width: f64,
    chrome: bool,
    dark: bool,
    ui: Ui,
    accent: &'static str,
}

const fn item(model: &'static str, orientation: Orientation, width: f64) -> Item {
    Item {
        model,
        orientation,
        color: None,
        width,
        chrome: false,
        dark: false,
        ui: Ui::App,
        accent: "#2F80ED",
    }
}

impl Item {
    const fn color(mut self, c: &'static str) -> Self {
        self.color = Some(c);
        self
    }
    const fn dark(mut self) -> Self {
        self.dark = true;
        self
    }
    const fn web(mut self) -> Self {
        self.ui = Ui::Web;
        self
    }
    const fn chrome(mut self) -> Self {
        self.chrome = true;
        self.ui = Ui::Web;
        self
    }
    const fn accent(mut self, a: &'static str) -> Self {
        self.accent = a;
        self
    }
}

fn rows() -> Vec<Vec<Item>> {
    use Orientation::{Landscape as L, Portrait as P};
    vec![
        vec![
            item("iphone-island", P, 250.0),
            item("iphone-island", P, 250.0)
                .color("natural-titanium")
                .dark()
                .accent("#F2994A"),
            item("iphone-notch", P, 250.0)
                .color("starlight")
                .accent("#27AE60"),
            item("iphone-home", P, 250.0)
                .color("silver-white")
                .accent("#EB5757"),
            item("iphone-home", P, 250.0).dark().accent("#9B51E0"),
            item("android-punchhole", P, 250.0)
                .color("sage")
                .accent("#27AE60"),
            item("android-punchhole", P, 250.0).dark(),
            item("none", P, 250.0).accent("#F2994A"),
        ],
        vec![
            item("iphone-island", L, 470.0).accent("#F2994A"),
            item("iphone-notch", L, 470.0).dark(),
            item("iphone-home", L, 470.0)
                .color("gold-white")
                .accent("#EB5757"),
            item("android-punchhole", L, 470.0)
                .color("porcelain")
                .accent("#27AE60"),
            item("none", L, 420.0).dark().accent("#9B51E0"),
        ],
        vec![
            item("ipad-modern", P, 380.0),
            item("ipad-home", P, 360.0)
                .color("silver-white")
                .accent("#27AE60"),
            item("android-tablet", P, 340.0).dark().accent("#F2994A"),
            item("ipad-modern", L, 560.0)
                .color("porcelain")
                .accent("#9B51E0"),
            item("iphone-island", P, 250.0)
                .color("#3E6FB0")
                .accent("#EB5757"),
        ],
        vec![
            item("ipad-home", L, 520.0).accent("#EB5757"),
            item("android-tablet", L, 540.0)
                .color("sage")
                .accent("#27AE60"),
            item("ipad-modern", L, 560.0).color("midnight").dark(),
        ],
        vec![
            item("macbook", L, 760.0).web(),
            item("generic-laptop", L, 760.0)
                .color("silver")
                .chrome()
                .accent("#27AE60"),
            item("macbook", L, 760.0)
                .color("starlight")
                .chrome()
                .dark()
                .accent("#F2994A"),
        ],
        vec![
            item("imac", L, 640.0).color("pastel-blue").web(),
            item("imac", L, 640.0)
                .color("silver")
                .chrome()
                .dark()
                .accent("#9B51E0"),
            item("generic-monitor", L, 640.0).web().accent("#EB5757"),
            item("imac", L, 560.0).color("pastel-pink").web(),
        ],
        vec![
            item("browser", L, 820.0).web().accent("#27AE60"),
            item("browser", L, 820.0).web().dark().accent("#F2994A"),
            item("generic-monitor", L, 640.0).color("silver").chrome(),
        ],
    ]
}

fn main() -> anyhow::Result<()> {
    // `-- --closeup <dir>`: large single renders for visual review instead.
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--closeup") {
        let dir = args.get(i + 1).map_or(".", String::as_str);
        return closeups(Path::new(dir));
    }
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/spike");
    std::fs::create_dir_all(&out)?;
    for (style, name) in [
        (FrameStyle::Stylized, "stylized"),
        (FrameStyle::Detailed, "detailed"),
    ] {
        let pm = sheet(&style, name);
        let path = out.join(format!("frames_catalog_{name}.png"));
        let bytes = save_png_rgb(&pm, &path, [255, 255, 255])?;
        println!(
            "{}  {}x{}  {:.1} MB",
            path.display(),
            pm.width(),
            pm.height(),
            bytes as f64 / 1e6
        );
    }
    Ok(())
}

fn closeups(dir: &Path) -> anyhow::Result<()> {
    use Orientation::{Landscape as L, Portrait as P};
    std::fs::create_dir_all(dir)?;
    let items = [
        item("iphone-island", P, 900.0).color("natural-titanium"),
        item("iphone-notch", L, 1400.0).color("silver").dark(),
        item("iphone-home", P, 800.0).color("silver-white"),
        item("android-punchhole", P, 800.0).color("sage"),
        item("macbook", L, 1600.0).color("silver").chrome(),
        item("imac", L, 1400.0).color("pastel-green").web(),
        item("ipad-modern", L, 1400.0).color("space-gray"),
        item("browser", L, 1200.0).web().dark(),
    ];
    for (style, name) in [
        (FrameStyle::Stylized, "stylized"),
        (FrameStyle::Detailed, "detailed"),
    ] {
        for (n, it) in items.iter().enumerate() {
            let p = place(*it, &style);
            let (w, h) = (p.size.x + 160.0, p.size.y + 160.0);
            let mut pm = Pixmap::new(w as u32, h as u32).expect("size");
            pm.fill(tiny_skia::Color::from_rgba8(226, 230, 236, 255));
            draw_item(&mut pm, &p, w / 2.0, h / 2.0, 4.0);
            save_png_rgb(
                &pm,
                &dir.join(format!("{name}_{n}_{}.png", it.model)),
                [255; 3],
            )?;
        }
    }
    Ok(())
}

struct Placed {
    item: Item,
    dev: arrangia_core::resolve::ResolvedDevice,
    size: arrangia_core::geometry::Vec2,
}

fn place(item: Item, style: &FrameStyle) -> Placed {
    let mut dev = catalog_device(item.model, item.orientation, style.clone(), item.color)
        .expect("catalog model");
    if item.chrome {
        dev.chrome = Some(Chrome::Browser);
    }
    if item.model == "browser" || item.chrome {
        dev.url = Some(if item.dark {
            "https://app.example.org/dashboard/overview".into()
        } else {
            "https://www.example.com/features".into()
        });
        dev.appearance = Some(if item.dark {
            Appearance::Dark
        } else {
            Appearance::Light
        });
    }
    let aspect = match (item.model, item.orientation) {
        ("browser", _) => Some(1440.0 / 900.0),
        ("none", Orientation::Portrait) => Some(1179.0 / 2556.0),
        ("none", Orientation::Landscape) => Some(2752.0 / 2064.0),
        _ => None,
    };
    let size = device_size(&dev, item.width, aspect);
    Placed { item, dev, size }
}

fn sheet(style: &FrameStyle, name: &str) -> Pixmap {
    let rows: Vec<Vec<Placed>> = rows()
        .into_iter()
        .map(|r| r.into_iter().map(|i| place(i, style)).collect())
        .collect();
    let title_h = 140.0;
    let height: f64 = title_h
        + rows
            .iter()
            .map(|r| r.iter().map(|p| p.size.y).fold(0.0, f64::max) + LABEL_H + GAP)
            .sum::<f64>()
        + GAP;
    let mut pm = Pixmap::new(SHEET_W as u32, height as u32).expect("sheet size");
    let bg = linear(
        (0.0, 0.0),
        (0.0, height as f32),
        &[(0.0, hex("#EEF0F4")), (1.0, hex("#D9DEE6"))],
    );
    if let Some(r) = tiny_skia::Rect::from_xywh(0.0, 0.0, SHEET_W as f32, height as f32) {
        pm.fill_rect(r, &shader_paint(bg), Transform::identity(), None);
    }
    text(
        &mut pm,
        &format!("Arrangia parametric frames · {name}"),
        60.0,
        86.0,
        40.0,
        hex("#1F2430"),
    );
    text(
        &mut pm,
        "generated from crates/core/data/devices.yaml · fake screenshots",
        60.0,
        122.0,
        20.0,
        hex("#6B7380"),
    );

    let mut y = title_h;
    for row in &rows {
        let row_h = row.iter().map(|p| p.size.y).fold(0.0, f64::max);
        let total: f64 = row.iter().map(|p| p.size.x).sum::<f64>() + GAP * (row.len() as f64 - 1.0);
        let mut x = (SHEET_W - total) / 2.0;
        for p in row {
            let cx = x + p.size.x / 2.0;
            // Devices sit on a common baseline per row.
            let cy = y + row_h - p.size.y / 2.0;
            draw_item(&mut pm, p, cx, cy, 0.0);
            let o = match p.item.orientation {
                Orientation::Portrait => "portrait",
                Orientation::Landscape => "landscape",
            };
            let mut detail = o.to_owned();
            if let Some(c) = p.item.color.or(p.dev.device.default_color.as_deref()) {
                detail.push_str(" · ");
                detail.push_str(c);
            }
            if p.item.chrome {
                detail.push_str(" · chrome");
            }
            if p.dev.appearance == Some(Appearance::Dark) {
                detail.push_str(" · dark");
            }
            centered_text(
                &mut pm,
                p.item.model,
                cx,
                y + row_h + 32.0,
                18.0,
                hex("#2C323D"),
            );
            centered_text(&mut pm, &detail, cx, y + row_h + 54.0, 15.0, hex("#6B7380"));
            x += p.size.x + GAP;
        }
        y += row_h + LABEL_H + GAP;
    }
    pm
}

fn draw_item(pm: &mut Pixmap, p: &Placed, cx: f64, cy: f64, degrees: f32) {
    let g = device_geometry(&p.dev, p.size);
    // Snap the screen to whole pixels so the generated UI is not resampled
    // at a half-pixel offset (the scene renderer gets the same effect from
    // its own resampling; here the shot is drawn 1:1).
    let cx = (cx + g.screen.x).round() - g.screen.x;
    let cy = (cy + g.screen.y).round() - g.screen.y;
    let ts = Transform::from_translate(cx as f32, cy as f32).pre_rotate(degrees);
    let k = p.size.x / 600.0;
    for (sigma, dy, a) in [(22.0, 18.0, 0.28), (5.0, 4.0, 0.22)] {
        draw_shadow(
            pm,
            &g.silhouette,
            ts,
            ShadowPx {
                sigma: sigma * k.max(0.5),
                dx: 0.0,
                dy: dy * k.max(0.5),
                color: [10, 20, 40, (a * 255.0) as u8],
            },
        );
    }
    let (w, h) = (
        g.screen.w.round().max(1.0) as u32,
        g.screen.h.round().max(1.0) as u32,
    );
    let shot = match p.item.ui {
        Ui::App => fake_app(w, h, p.item.dark, hex(p.item.accent)),
        Ui::Web => fake_web(w, h, p.item.dark, hex(p.item.accent)),
    };
    draw_device(&mut pm.as_mut(), &p.dev, p.size, shot.as_ref(), ts);
}

// ---------------------------------------------------------------- text

fn text(pm: &mut Pixmap, s: &str, x: f64, baseline: f64, size: f32, c: Rgba) -> f64 {
    let Some(l) = glyphs::label(s, size, 10_000.0) else {
        return 0.0;
    };
    pm.fill_path(
        &l.path,
        &solid(c),
        FillRule::Winding,
        Transform::from_translate(x as f32, baseline as f32),
        None,
    );
    f64::from(l.width)
}

fn centered_text(pm: &mut Pixmap, s: &str, cx: f64, baseline: f64, size: f32, c: Rgba) {
    if let Some(l) = glyphs::label(s, size, 10_000.0) {
        pm.fill_path(
            &l.path,
            &solid(c),
            FillRule::Winding,
            Transform::from_translate(cx as f32 - l.width / 2.0, baseline as f32),
            None,
        );
    }
}

// ------------------------------------------------------------ fake UIs

struct Canvas {
    pm: Pixmap,
}

impl Canvas {
    fn new(w: u32, h: u32, bg: Rgba) -> Option<Self> {
        let mut pm = Pixmap::new(w, h)?;
        pm.fill(tiny_skia::Color::from_rgba8(bg[0], bg[1], bg[2], 255));
        Some(Self { pm })
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64, c: Rgba) {
        if let Some(p) = shapes::round(Rect::new(x, y, w, h), r) {
            self.pm.fill_path(
                &p,
                &solid(c),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    fn grad(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64, a: Rgba, b: Rgba) {
        if let Some(p) = shapes::round(Rect::new(x, y, w, h), r) {
            let sh = linear(
                (x as f32, y as f32),
                ((x + w) as f32, (y + h) as f32),
                &[(0.0, a), (1.0, b)],
            );
            self.pm.fill_path(
                &p,
                &shader_paint(sh),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    fn circle(&mut self, x: f64, y: f64, r: f64, c: Rgba) {
        if let Some(p) = shapes::circle(x, y, r) {
            self.pm.fill_path(
                &p,
                &solid(c),
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    fn text(&mut self, s: &str, x: f64, baseline: f64, size: f64, c: Rgba) -> f64 {
        text(&mut self.pm, s, x, baseline, size as f32, c)
    }
}

struct Theme {
    bg: Rgba,
    card: Rgba,
    text: Rgba,
    muted: Rgba,
    line: Rgba,
}

fn theme(dark: bool) -> Theme {
    if dark {
        Theme {
            bg: hex("#0F1115"),
            card: hex("#1B1E25"),
            text: hex("#F2F3F5"),
            muted: hex("#3A3F4B"),
            line: hex("#262A33"),
        }
    } else {
        Theme {
            bg: hex("#F4F5F7"),
            card: hex("#FFFFFF"),
            text: hex("#14161B"),
            muted: hex("#D9DCE2"),
            line: hex("#E6E8EC"),
        }
    }
}

fn lighter(c: Rgba) -> Rgba {
    arrangia_render::frames::palette::shade(c, 0.35)
}

/// Mobile / tablet app: status bar, title, hero card, list or grid, tab bar.
fn fake_app(w: u32, h: u32, dark: bool, accent: Rgba) -> Option<Pixmap> {
    let t = theme(dark);
    let mut c = Canvas::new(w, h, t.bg)?;
    let (wf, hf) = (f64::from(w), f64::from(h));
    let wide = wf > hf * 1.15;
    let u = wf.min(hf) / 100.0; // 1 % of the short side
    // Status bar.
    let sb = u * 5.5;
    c.text("9:41", u * 7.0, sb, u * 3.6, t.text);
    c.rect(
        wf - u * 12.0,
        sb - u * 2.6,
        u * 6.0,
        u * 2.8,
        u * 0.8,
        t.text,
    );
    c.rect(
        wf - u * 20.0,
        sb - u * 2.6,
        u * 5.0,
        u * 2.8,
        u * 0.6,
        t.muted,
    );
    let pad = u * 6.0;
    let mut y = u * 14.0;
    if wide {
        // Sidebar + content grid.
        let side = wf * 0.24;
        c.rect(0.0, 0.0, side, hf, 0.0, t.card);
        c.text("Overview", pad, y + u * 4.0, u * 5.0, t.text);
        for i in 0..6 {
            let yy = y + u * 12.0 + f64::from(i) * u * 9.0;
            let col = if i == 1 { accent } else { t.muted };
            c.circle(pad + u * 2.0, yy, u * 2.0, col);
            c.rect(
                pad + u * 6.0,
                yy - u * 1.2,
                side * 0.45,
                u * 2.4,
                u * 1.2,
                t.muted,
            );
        }
        let x0 = side + pad;
        let cw = wf - x0 - pad;
        c.text("Dashboard", x0, y + u * 5.0, u * 6.5, t.text);
        y += u * 11.0;
        c.grad(x0, y, cw, hf * 0.3, u * 3.0, accent, lighter(accent));
        c.text(
            "Weekly progress",
            x0 + u * 4.0,
            y + u * 7.0,
            u * 3.6,
            [255; 4],
        );
        // Bar chart inside the hero.
        for i in 0..9 {
            let bh = (hf * 0.3 - u * 14.0) * (0.3 + 0.7 * ((f64::from(i) * 1.7).sin().abs()));
            let bx = x0 + u * 4.0 + f64::from(i) * cw / 10.0;
            c.rect(
                bx,
                y + hf * 0.3 - bh - u * 3.0,
                cw / 16.0,
                bh,
                u,
                [255, 255, 255, 170],
            );
        }
        y += hf * 0.3 + u * 4.0;
        let cols = 3.0;
        let gw = (cw - u * 4.0 * (cols - 1.0)) / cols;
        let gh = (hf - y - pad).max(u * 10.0);
        for i in 0..3 {
            let gx = x0 + f64::from(i) * (gw + u * 4.0);
            c.rect(gx, y, gw, gh, u * 3.0, t.card);
            c.circle(gx + u * 6.0, y + u * 6.0, u * 3.0, with_alpha(accent, 0.25));
            c.rect(
                gx + u * 4.0,
                y + u * 13.0,
                gw * 0.6,
                u * 2.6,
                u * 1.3,
                t.text,
            );
            c.rect(
                gx + u * 4.0,
                y + u * 18.0,
                gw * 0.4,
                u * 2.2,
                u * 1.1,
                t.muted,
            );
        }
        return Some(c.pm);
    }
    c.text("Today", pad, y + u * 8.0, u * 9.0, t.text);
    c.circle(
        wf - pad - u * 4.5,
        y + u * 5.0,
        u * 4.5,
        with_alpha(accent, 0.25),
    );
    y += u * 15.0;
    let hero_h = hf * 0.2;
    c.grad(
        pad,
        y,
        wf - 2.0 * pad,
        hero_h,
        u * 5.0,
        accent,
        lighter(accent),
    );
    c.text(
        "Next up",
        pad + u * 5.0,
        y + u * 9.0,
        u * 4.2,
        [255, 255, 255, 220],
    );
    c.text(
        "Focus session",
        pad + u * 5.0,
        y + u * 17.0,
        u * 6.5,
        [255; 4],
    );
    c.rect(
        pad + u * 5.0,
        y + hero_h - u * 12.0,
        u * 26.0,
        u * 7.0,
        u * 3.5,
        [255, 255, 255, 70],
    );
    c.circle(
        wf - pad - hero_h * 0.32,
        y + hero_h / 2.0,
        hero_h * 0.22,
        [255, 255, 255, 60],
    );
    y += hero_h + u * 6.0;
    let tab_h = u * 16.0;
    let row_h = u * 15.0;
    let mut i = 0;
    while y + row_h < hf - tab_h - u * 2.0 {
        c.rect(pad, y, wf - 2.0 * pad, row_h - u * 3.0, u * 3.5, t.card);
        let col = [accent, hex("#27AE60"), hex("#F2994A"), hex("#9B51E0")][i % 4];
        c.circle(
            pad + u * 7.5,
            y + (row_h - u * 3.0) / 2.0,
            u * 4.0,
            with_alpha(col, 0.85),
        );
        c.rect(
            pad + u * 15.0,
            y + u * 3.2,
            wf * 0.38,
            u * 2.6,
            u * 1.3,
            t.text,
        );
        c.rect(
            pad + u * 15.0,
            y + u * 7.4,
            wf * 0.24,
            u * 2.2,
            u * 1.1,
            t.muted,
        );
        y += row_h;
        i += 1;
    }
    // Tab bar.
    c.rect(0.0, hf - tab_h, wf, tab_h, 0.0, t.card);
    c.rect(0.0, hf - tab_h, wf, u * 0.3, 0.0, t.line);
    for j in 0..4 {
        let x = wf * (0.125 + 0.25 * f64::from(j));
        let col = if j == 0 { accent } else { t.muted };
        c.rect(
            x - u * 3.0,
            hf - tab_h + u * 3.5,
            u * 6.0,
            u * 6.0,
            u * 1.8,
            col,
        );
    }
    c.rect(wf * 0.35, hf - u * 3.0, wf * 0.3, u * 1.2, u * 0.6, t.text);
    Some(c.pm)
}

/// Marketing web page: nav bar, hero with headline and illustration,
/// feature cards.
fn fake_web(w: u32, h: u32, dark: bool, accent: Rgba) -> Option<Pixmap> {
    let t = theme(dark);
    let mut c = Canvas::new(w, h, t.bg)?;
    let (wf, hf) = (f64::from(w), f64::from(h));
    let u = wf / 100.0;
    let pad = u * 6.0;
    // Nav.
    let nav = (hf * 0.09).max(u * 4.0);
    c.rect(0.0, 0.0, wf, nav, 0.0, t.card);
    c.rect(0.0, nav, wf, u * 0.08, 0.0, t.line);
    c.circle(pad + u * 1.2, nav / 2.0, u * 1.2, accent);
    c.text("Acme", pad + u * 3.2, nav / 2.0 + u * 0.6, u * 1.7, t.text);
    for i in 0..4 {
        c.rect(
            wf * 0.42 + f64::from(i) * u * 8.0,
            nav / 2.0 - u * 0.4,
            u * 5.5,
            u * 0.8,
            u * 0.4,
            t.muted,
        );
    }
    c.rect(
        wf - pad - u * 10.0,
        nav / 2.0 - u * 1.6,
        u * 10.0,
        u * 3.2,
        u * 1.6,
        accent,
    );
    // Hero.
    let hy = nav + (hf - nav) * 0.12;
    c.text("Ship your app", pad, hy + u * 4.5, u * 5.2, t.text);
    c.text("in every language.", pad, hy + u * 10.5, u * 5.2, accent);
    for i in 0..3 {
        c.rect(
            pad,
            hy + u * 15.0 + f64::from(i) * u * 2.2,
            wf * (0.36 - 0.06 * f64::from(i)),
            u * 1.0,
            u * 0.5,
            t.muted,
        );
    }
    c.rect(pad, hy + u * 23.0, u * 14.0, u * 4.0, u * 2.0, accent);
    c.rect(
        pad + u * 16.0,
        hy + u * 23.0,
        u * 14.0,
        u * 4.0,
        u * 2.0,
        t.muted,
    );
    // Illustration.
    let ix = wf * 0.54;
    let iw = wf - ix - pad;
    let ih = (hf - nav) * 0.5;
    c.grad(ix, hy - u * 1.0, iw, ih, u * 2.0, accent, lighter(accent));
    c.rect(
        ix + iw * 0.08,
        hy + ih * 0.12,
        iw * 0.5,
        ih * 0.66,
        u * 1.2,
        t.card,
    );
    c.rect(
        ix + iw * 0.62,
        hy + ih * 0.3,
        iw * 0.3,
        ih * 0.5,
        u * 1.2,
        [255, 255, 255, 120],
    );
    for i in 0..4 {
        c.rect(
            ix + iw * 0.12,
            hy + ih * (0.2 + 0.12 * f64::from(i)),
            iw * 0.36,
            u * 0.9,
            u * 0.45,
            t.muted,
        );
    }
    // Feature cards.
    let fy = hy + ih + u * 4.0;
    let fh = hf - fy - u * 3.0;
    if fh > u * 4.0 {
        let fw = (wf - 2.0 * pad - u * 4.0) / 3.0;
        for i in 0..3 {
            let fx = pad + f64::from(i) * (fw + u * 2.0);
            c.rect(fx, fy, fw, fh + u * 6.0, u * 1.5, t.card);
            c.circle(fx + u * 3.5, fy + u * 3.5, u * 1.6, with_alpha(accent, 0.3));
            c.rect(
                fx + u * 2.0,
                fy + u * 7.0,
                fw * 0.5,
                u * 1.1,
                u * 0.55,
                t.text,
            );
            c.rect(
                fx + u * 2.0,
                fy + u * 9.5,
                fw * 0.75,
                u * 0.8,
                u * 0.4,
                t.muted,
            );
        }
    }
    Some(c.pm)
}
