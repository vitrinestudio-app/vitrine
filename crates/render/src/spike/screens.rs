//! Generated test screenshots (a fake garden-planner app UI) at real device
//! resolutions. They are spike assets only; real projects import screenshots.

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    reason = "hand-drawn fake UI: short coordinate names, long linear drawing code"
)]

use tiny_skia::{FillRule, Pixmap, Transform};

use crate::paint::{Rgba, circle, hex, linear, round_rect, shader_paint, solid, with_alpha};
use crate::text::draw_layout;
use arrangia_core::layout::{ResolvedSpanStyle, ResolvedTextStyle, TextInput, TextLayouter};
use arrangia_core::model::Align;

/// Minimal immediate-mode UI painter in points (`s` px per pt).
struct Ui<'a> {
    pm: Pixmap,
    layouter: &'a mut TextLayouter,
    s: f32,
}

impl Ui<'_> {
    fn ts(&self) -> Transform {
        Transform::from_scale(self.s, self.s)
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
        if let Some(p) = round_rect(x, y, w, h, r) {
            let ts = self.ts();
            self.pm
                .fill_path(&p, &solid(c), FillRule::Winding, ts, None);
        }
    }

    fn grad_rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, a: Rgba, b: Rgba) {
        if let Some(p) = round_rect(x, y, w, h, r) {
            let ts = self.ts();
            let sh = linear((x, y), (x + w, y + h), &[(0.0, a), (1.0, b)]);
            self.pm
                .fill_path(&p, &shader_paint(sh), FillRule::Winding, ts, None);
        }
    }

    fn dot(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        if let Some(p) = circle(cx, cy, r) {
            let ts = self.ts();
            self.pm
                .fill_path(&p, &solid(c), FillRule::Winding, ts, None);
        }
    }

    /// Single paragraph; `y` is the top of the line box.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        size: f32,
        weight: u16,
        c: Rgba,
        s: &str,
        align: Align,
    ) {
        let style = ResolvedTextStyle {
            font: if size >= 26.0 {
                "Inter Display"
            } else {
                "Inter"
            }
            .to_owned(),
            weight,
            size,
            line_height: 1.2,
            color: super::core_color(c),
            ..ResolvedTextStyle::default()
        };
        let spans = [(
            "accent".to_owned(),
            ResolvedSpanStyle {
                color: Some(super::core_color(hex("#2F8A4C"))),
                ..ResolvedSpanStyle::default()
            },
        )]
        .into_iter()
        .collect();
        let l = self.layouter.layout(&TextInput {
            spans: Some(&spans),
            align,
            ..TextInput::new(s, &style, w)
        });
        let ts = self.ts().pre_translate(x, y);
        draw_layout(&mut self.pm, &l, self.layouter.fonts(), ts, None);
    }

    fn status_bar(&mut self, w: f32, dark: bool) {
        let c = if dark { hex("#FFFFFF") } else { hex("#111111") };
        self.text(34.0, 18.0, 80.0, 17.0, 600, c, "9:41", Align::Center);
        // signal bars
        for i in 0..4 {
            let h = 4.0 + i as f32 * 2.6;
            self.rect(w - 112.0 + i as f32 * 5.5, 33.0 - h, 3.6, h, 1.0, c);
        }
        // wifi as three arcs approximated by dots
        self.dot(w - 80.0, 30.0, 2.2, c);
        self.rect(w - 86.0, 22.0, 12.0, 2.4, 1.2, c);
        // battery
        self.rect(w - 64.0, 20.0, 26.0, 13.0, 4.0, with_alpha(c, 0.4));
        self.rect(w - 62.0, 22.0, 19.0, 9.0, 2.5, c);
        self.rect(w - 37.0, 24.5, 2.0, 4.0, 1.0, with_alpha(c, 0.4));
    }

    fn home_indicator(&mut self, w: f32, h: f32, c: Rgba) {
        self.rect(w / 2.0 - 70.0, h - 13.0, 140.0, 5.0, 2.5, c);
    }
}

const GREEN: &str = "#2F7A47";
const INK: &str = "#18201A";
const MUTED: &str = "#6B756D";

fn plants_dots(ui: &mut Ui<'_>, x: f32, y: f32, w: f32, h: f32, seed: u32) {
    let colors = [
        hex("#E5533D"),
        hex("#5FAE4E"),
        hex("#F29A2E"),
        hex("#8DC26F"),
        hex("#C24A8A"),
        hex("#3E8E5A"),
    ];
    let cols = 5;
    let rows = 3;
    let cw = w / cols as f32;
    let rh = h / rows as f32;
    for r in 0..rows {
        for c in 0..cols {
            let k = (seed.wrapping_mul(31) + r * 7 + c * 3) as usize % colors.len();
            let rad = cw.min(rh)
                * if (r + c + seed).is_multiple_of(3) {
                    0.36
                } else {
                    0.28
                };
            ui.dot(
                x + cw * (c as f32 + 0.5),
                y + rh * (r as f32 + 0.5),
                rad,
                colors[k],
            );
        }
    }
}

/// Phone "My Garden" home screen, 1320 × 2868 px.
pub fn phone_home(layouter: &mut TextLayouter) -> Pixmap {
    let (w, h, s) = (440.0, 956.0, 3.0);
    let mut ui = Ui {
        pm: Pixmap::new(1320, 2868).expect("size"),
        layouter,
        s,
    };
    ui.rect(0.0, 0.0, w, h, 0.0, hex("#F5F6F1"));
    ui.status_bar(w, false);
    ui.text(
        20.0,
        64.0,
        300.0,
        15.0,
        600,
        hex(MUTED),
        "Spring 2026 · 4 beds",
        Align::Left,
    );
    ui.text(
        20.0,
        84.0,
        360.0,
        34.0,
        700,
        hex(INK),
        "My Garden",
        Align::Left,
    );
    ui.dot(w - 42.0, 104.0, 20.0, hex("#DDE8D8"));
    ui.text(
        w - 62.0,
        93.0,
        40.0,
        17.0,
        600,
        hex(GREEN),
        "TM",
        Align::Center,
    );

    // hero card
    ui.grad_rect(
        16.0,
        146.0,
        w - 32.0,
        168.0,
        24.0,
        hex("#2F7A47"),
        hex("#5FAE6A"),
    );
    ui.text(
        36.0,
        166.0,
        300.0,
        14.0,
        600,
        with_alpha(hex("#FFFFFF"), 0.8),
        "NEXT TASK",
        Align::Left,
    );
    ui.text(
        36.0,
        188.0,
        260.0,
        26.0,
        700,
        hex("#FFFFFF"),
        "Water tomatoes",
        Align::Left,
    );
    ui.text(
        36.0,
        226.0,
        260.0,
        15.0,
        400,
        with_alpha(hex("#FFFFFF"), 0.85),
        "Today, 18:00 · Bed 1",
        Align::Left,
    );
    ui.rect(
        36.0,
        264.0,
        120.0,
        32.0,
        16.0,
        with_alpha(hex("#FFFFFF"), 0.22),
    );
    ui.text(
        36.0,
        270.5,
        120.0,
        15.0,
        600,
        hex("#FFFFFF"),
        "Mark done",
        Align::Center,
    );
    // progress ring (approximated by two concentric dots)
    ui.dot(w - 86.0, 230.0, 46.0, with_alpha(hex("#FFFFFF"), 0.25));
    ui.dot(w - 86.0, 230.0, 36.0, hex("#3F8E55"));
    ui.text(
        w - 126.0,
        214.0,
        80.0,
        24.0,
        700,
        hex("#FFFFFF"),
        "72%",
        Align::Center,
    );

    // beds grid
    ui.text(20.0, 338.0, 200.0, 22.0, 700, hex(INK), "Beds", Align::Left);
    ui.text(
        w - 120.0,
        342.0,
        100.0,
        15.0,
        600,
        hex(GREEN),
        "See all",
        Align::Right,
    );
    let names = [
        ("Bed 1", "Tomatoes & basil", "6 plants · 2 tasks"),
        ("Bed 2", "Salad mix", "12 plants · 1 task"),
        ("Bed 3", "Carrots & onions", "18 plants"),
        ("Bed 4", "Herbs", "9 plants · 3 tasks"),
    ];
    let cw = (w - 16.0 * 2.0 - 12.0) / 2.0;
    for (i, (n, sub, meta)) in names.iter().enumerate() {
        let col = (i % 2) as f32;
        let row = (i / 2) as f32;
        let x = 16.0 + col * (cw + 12.0);
        let y = 378.0 + row * 212.0;
        ui.rect(x, y, cw, 200.0, 20.0, hex("#FFFFFF"));
        ui.rect(x + 10.0, y + 10.0, cw - 20.0, 96.0, 13.0, hex("#E9EFE3"));
        plants_dots(&mut ui, x + 16.0, y + 16.0, cw - 32.0, 84.0, i as u32);
        ui.text(
            x + 14.0,
            y + 116.0,
            cw - 28.0,
            17.0,
            600,
            hex(INK),
            n,
            Align::Left,
        );
        ui.text(
            x + 14.0,
            y + 139.0,
            cw - 28.0,
            14.0,
            400,
            hex(INK),
            sub,
            Align::Left,
        );
        ui.text(
            x + 14.0,
            y + 162.0,
            cw - 28.0,
            13.0,
            400,
            hex(MUTED),
            meta,
            Align::Left,
        );
    }

    // tip card
    ui.rect(16.0, 812.0, w - 32.0, 62.0, 18.0, hex("#FFF4DC"));
    ui.dot(44.0, 843.0, 14.0, hex("#F2B33D"));
    ui.text(
        68.0,
        822.0,
        w - 100.0,
        14.0,
        400,
        hex("#5A4210"),
        "**Tip:** Basil next to tomatoes keeps pests away.",
        Align::Left,
    );

    // tab bar
    ui.rect(0.0, h - 84.0, w, 84.0, 0.0, hex("#FFFFFF"));
    ui.rect(0.0, h - 84.0, w, 1.0, 0.0, hex("#E3E6E0"));
    let tabs = ["Garden", "Plan", "Harvest", "More"];
    for (i, t) in tabs.iter().enumerate() {
        let cx = w / 8.0 * (2.0 * i as f32 + 1.0);
        let c = if i == 0 { hex(GREEN) } else { hex("#9AA39C") };
        ui.rect(cx - 12.0, h - 74.0, 24.0, 24.0, 7.0, c);
        ui.text(cx - 40.0, h - 46.0, 80.0, 11.0, 600, c, t, Align::Center);
    }
    ui.home_indicator(w, h, hex("#111111"));
    ui.pm
}

/// Phone "Harvest" statistics screen, 1320 × 2868 px (dark).
pub fn phone_harvest(layouter: &mut TextLayouter) -> Pixmap {
    let (w, h, s) = (440.0, 956.0, 3.0);
    let mut ui = Ui {
        pm: Pixmap::new(1320, 2868).expect("size"),
        layouter,
        s,
    };
    ui.rect(0.0, 0.0, w, h, 0.0, hex("#0F1512"));
    ui.status_bar(w, true);
    ui.text(
        20.0,
        64.0,
        300.0,
        15.0,
        600,
        hex("#8FA396"),
        "Season 2026",
        Align::Left,
    );
    ui.text(
        20.0,
        84.0,
        360.0,
        34.0,
        700,
        hex("#FFFFFF"),
        "Harvest",
        Align::Left,
    );

    ui.rect(16.0, 146.0, w - 32.0, 330.0, 24.0, hex("#18211C"));
    ui.text(
        36.0,
        166.0,
        300.0,
        14.0,
        600,
        hex("#8FA396"),
        "TOTAL THIS SEASON",
        Align::Left,
    );
    ui.text(
        36.0,
        188.0,
        300.0,
        44.0,
        700,
        hex("#FFFFFF"),
        "38.4 kg",
        Align::Left,
    );
    ui.text(
        36.0,
        244.0,
        300.0,
        15.0,
        600,
        hex("#6FD08C"),
        "+12 % vs. 2025",
        Align::Left,
    );
    let vals = [0.25, 0.42, 0.38, 0.6, 0.82, 0.95, 0.7, 0.5];
    let months = ["Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct"];
    let bw = (w - 72.0) / vals.len() as f32;
    for (i, v) in vals.iter().enumerate() {
        let bh = 150.0 * v;
        let x = 36.0 + i as f32 * bw;
        let c0 = if i == 5 {
            hex("#FFC65C")
        } else {
            hex("#4FBF73")
        };
        let c1 = if i == 5 {
            hex("#F29A2E")
        } else {
            hex("#2F7A47")
        };
        ui.grad_rect(x + 5.0, 440.0 - bh, bw - 10.0, bh, 7.0, c0, c1);
        ui.text(
            x,
            446.0,
            bw,
            11.0,
            400,
            hex("#8FA396"),
            months[i],
            Align::Center,
        );
    }

    let rows = [
        ("Tomatoes", "12.1 kg", "#E5533D", 0.9),
        ("Zucchini", "9.8 kg", "#5FAE4E", 0.72),
        ("Potatoes", "7.5 kg", "#C9A26B", 0.56),
        ("Carrots", "4.2 kg", "#F29A2E", 0.32),
        ("Beans", "2.9 kg", "#8DC26F", 0.22),
        ("Herbs", "1.9 kg", "#3E8E5A", 0.14),
    ];
    ui.text(
        20.0,
        500.0,
        200.0,
        22.0,
        700,
        hex("#FFFFFF"),
        "By crop",
        Align::Left,
    );
    for (i, (n, v, c, frac)) in rows.iter().enumerate() {
        let y = 540.0 + i as f32 * 58.0;
        ui.rect(16.0, y, w - 32.0, 50.0, 14.0, hex("#18211C"));
        ui.dot(40.0, y + 25.0, 9.0, hex(c));
        ui.text(
            60.0,
            y + 8.0,
            200.0,
            16.0,
            600,
            hex("#FFFFFF"),
            n,
            Align::Left,
        );
        ui.rect(60.0, y + 33.0, 220.0, 5.0, 2.5, hex("#26302A"));
        ui.rect(60.0, y + 33.0, 220.0 * frac, 5.0, 2.5, hex(c));
        ui.text(
            w - 140.0,
            y + 14.0,
            110.0,
            16.0,
            600,
            hex("#DDE6DF"),
            v,
            Align::Right,
        );
    }
    ui.rect(0.0, h - 84.0, w, 84.0, 0.0, hex("#151C18"));
    let tabs = ["Garden", "Plan", "Harvest", "More"];
    for (i, t) in tabs.iter().enumerate() {
        let cx = w / 8.0 * (2.0 * i as f32 + 1.0);
        let c = if i == 2 {
            hex("#6FD08C")
        } else {
            hex("#5E6B63")
        };
        ui.rect(cx - 12.0, h - 74.0, 24.0, 24.0, 7.0, c);
        ui.text(cx - 40.0, h - 46.0, 80.0, 11.0, 600, c, t, Align::Center);
    }
    ui.home_indicator(w, h, hex("#FFFFFF"));
    ui.pm
}

/// Tablet "Bed planner" screen, landscape 2752 × 2064 px.
pub fn tablet_planner(layouter: &mut TextLayouter) -> Pixmap {
    let (w, h, s) = (1376.0, 1032.0, 2.0);
    let mut ui = Ui {
        pm: Pixmap::new(2752, 2064).expect("size"),
        layouter,
        s,
    };
    ui.rect(0.0, 0.0, w, h, 0.0, hex("#F5F6F1"));
    // sidebar
    ui.rect(0.0, 0.0, 300.0, h, 0.0, hex("#ECEFE7"));
    ui.text(24.0, 18.0, 80.0, 15.0, 600, hex(INK), "9:41", Align::Left);
    ui.text(
        24.0,
        64.0,
        260.0,
        30.0,
        700,
        hex(INK),
        "Garden",
        Align::Left,
    );
    let items = [
        "Overview",
        "Bed planner",
        "Calendar",
        "Harvest",
        "Seeds",
        "Settings",
    ];
    for (i, it) in items.iter().enumerate() {
        let y = 120.0 + i as f32 * 48.0;
        if i == 1 {
            ui.rect(12.0, y, 276.0, 40.0, 10.0, hex("#D8E6D3"));
        }
        let c = if i == 1 { hex(GREEN) } else { hex("#4A544C") };
        ui.rect(28.0, y + 10.0, 20.0, 20.0, 6.0, c);
        ui.text(62.0, y + 9.0, 200.0, 17.0, 600, c, it, Align::Left);
    }
    // header
    ui.text(
        332.0,
        56.0,
        600.0,
        30.0,
        700,
        hex(INK),
        "Bed 1 · Tomatoes & basil",
        Align::Left,
    );
    ui.text(
        332.0,
        96.0,
        600.0,
        16.0,
        400,
        hex(MUTED),
        "120 × 240 cm · sunny · planted 14 May",
        Align::Left,
    );
    ui.rect(w - 230.0, 60.0, 200.0, 40.0, 20.0, hex(GREEN));
    ui.text(
        w - 230.0,
        70.0,
        200.0,
        16.0,
        600,
        hex("#FFFFFF"),
        "Add plant",
        Align::Center,
    );

    // bed grid
    let (gx, gy, gw, gh) = (332.0, 140.0, 680.0, 860.0);
    ui.rect(gx, gy, gw, gh, 24.0, hex("#8B6B4A"));
    ui.rect(
        gx + 14.0,
        gy + 14.0,
        gw - 28.0,
        gh - 28.0,
        16.0,
        hex("#6E5136"),
    );
    let cols = 4;
    let rows = 6;
    let cw = (gw - 28.0) / cols as f32;
    let rh = (gh - 28.0) / rows as f32;
    let crops = [
        ("Tomato", "#E5533D"),
        ("Basil", "#5FAE4E"),
        ("Tomato", "#E5533D"),
        ("Marigold", "#F2B33D"),
        ("Basil", "#5FAE4E"),
        ("Pepper", "#C24A3A"),
    ];
    for r in 0..rows {
        for c in 0..cols {
            let (name, col) = crops[(r * 2 + c) % crops.len()];
            let cx = gx + 14.0 + cw * (c as f32 + 0.5);
            let cy = gy + 14.0 + rh * (r as f32 + 0.5);
            ui.dot(cx, cy - 10.0, 34.0, with_alpha(hex(col), 0.25));
            ui.dot(cx, cy - 10.0, 24.0, hex(col));
            ui.text(
                cx - 70.0,
                cy + 30.0,
                140.0,
                13.0,
                600,
                hex("#F3EDE4"),
                name,
                Align::Center,
            );
        }
    }
    // inspector
    let ix = gx + gw + 24.0;
    let iw = w - ix - 24.0;
    ui.rect(ix, gy, iw, 420.0, 22.0, hex("#FFFFFF"));
    ui.text(
        ix + 20.0,
        gy + 20.0,
        iw - 40.0,
        22.0,
        700,
        hex(INK),
        "Tomato",
        Align::Left,
    );
    ui.text(
        ix + 20.0,
        gy + 52.0,
        iw - 40.0,
        15.0,
        400,
        hex(MUTED),
        "Solanum lycopersicum",
        Align::Left,
    );
    ui.text(
        ix + 20.0,
        gy + 92.0,
        iw - 40.0,
        13.0,
        600,
        hex(MUTED),
        "GOOD NEIGHBOURS",
        Align::Left,
    );
    for (i, n) in ["Basil", "Marigold", "Carrot", "Parsley"]
        .iter()
        .enumerate()
    {
        let x = ix + 20.0 + (i % 2) as f32 * 140.0;
        let y = gy + 116.0 + (i / 2) as f32 * 44.0;
        ui.rect(x, y, 128.0, 34.0, 17.0, hex("#E3F1DE"));
        ui.text(
            x,
            y + 8.0,
            128.0,
            14.0,
            600,
            hex("#2F6B3A"),
            n,
            Align::Center,
        );
    }
    ui.text(
        ix + 20.0,
        gy + 222.0,
        iw - 40.0,
        13.0,
        600,
        hex(MUTED),
        "AVOID",
        Align::Left,
    );
    for (i, n) in ["Fennel", "Potato"].iter().enumerate() {
        let x = ix + 20.0 + i as f32 * 140.0;
        ui.rect(x, gy + 246.0, 128.0, 34.0, 17.0, hex("#FBE3DF"));
        ui.text(
            x,
            gy + 254.0,
            128.0,
            14.0,
            600,
            hex("#A33A2A"),
            n,
            Align::Center,
        );
    }
    ui.text(
        ix + 20.0,
        gy + 306.0,
        iw - 40.0,
        15.0,
        400,
        hex(INK),
        "Water every 2 days. Harvest from **late July**.",
        Align::Left,
    );

    ui.rect(ix, gy + 444.0, iw, 416.0, 22.0, hex("#FFFFFF"));
    ui.text(
        ix + 20.0,
        gy + 464.0,
        iw - 40.0,
        22.0,
        700,
        hex(INK),
        "This week",
        Align::Left,
    );
    let tasks = [
        ("Water bed 1", "Today"),
        ("Pinch out side shoots", "Wed"),
        ("Sow lettuce", "Fri"),
        ("Fertilise", "Sun"),
    ];
    for (i, (t, d)) in tasks.iter().enumerate() {
        let y = gy + 510.0 + i as f32 * 80.0;
        ui.dot(
            ix + 34.0,
            y + 22.0,
            11.0,
            if i == 0 { hex(GREEN) } else { hex("#D5DBD2") },
        );
        ui.text(
            ix + 56.0,
            y + 8.0,
            iw - 140.0,
            17.0,
            600,
            hex(INK),
            t,
            Align::Left,
        );
        ui.text(
            ix + 56.0,
            y + 32.0,
            iw - 140.0,
            14.0,
            400,
            hex(MUTED),
            d,
            Align::Left,
        );
    }
    ui.pm
}
