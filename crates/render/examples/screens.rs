//! Generates the fake app screenshots of the example project
//! (`tests/fixtures/example/screenshots/`): a fictional garden planner
//! ("Fernwood") on iPhone, Android, iPad, Mac and the web, in English and
//! partly in German and French. Deterministic: same code, same pixels.
//!
//! ```sh
//! cargo run -p arrangia-render --release --example screens -- tests/fixtures/example/screenshots
//! ```
//!
//! The screens are drawn with tiny-skia and the bundled Inter fonts through
//! `arrangia_core::layout`, in points scaled to the device resolution.

#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "hand-drawn fake UI: short coordinate names, long linear drawing code"
)]

use std::path::{Path, PathBuf};

use arrangia_core::layout::{FontLibrary, ResolvedTextStyle, TextInput, TextLayouter};
use arrangia_core::model::{Align, Rgba as CoreRgba};
use arrangia_render::paint::{
    Rgba, circle, hex, linear, round_rect, shader_paint, solid, with_alpha,
};
use arrangia_render::raster::encode_png_rgb_with;
use arrangia_render::text::draw_layout;
use tiny_skia::{FillRule, LineCap, LineJoin, PathBuilder, Pixmap, Stroke, Transform};

// ------------------------------------------------------------------ palette

const BG: &str = "#F5F6F1";
const CARD: &str = "#FFFFFF";
const GREEN: &str = "#2F7A47";
const GREEN_L: &str = "#5FAE6A";
const MINT: &str = "#DDEBD8";
const INK: &str = "#18201A";
const MUTED: &str = "#6B756D";
const LINE: &str = "#E3E6E0";
const SOIL: &str = "#7A5A3C";
const SOIL_D: &str = "#5E4329";
const AMBER: &str = "#F2B33D";
const RED: &str = "#E5533D";

/// A crop: name (en, de, fr), colour.
#[derive(Clone, Copy)]
struct Crop {
    en: &'static str,
    de: &'static str,
    fr: &'static str,
    color: &'static str,
}

const fn crop(en: &'static str, de: &'static str, fr: &'static str, color: &'static str) -> Crop {
    Crop { en, de, fr, color }
}

const TOMATO: Crop = crop("Tomato", "Tomate", "Tomate", "#E5533D");
const BASIL: Crop = crop("Basil", "Basilikum", "Basilic", "#5FAE4E");
const MARIGOLD: Crop = crop("Marigold", "Ringelblume", "Tagète", "#F2A93B");
const CARROT: Crop = crop("Carrot", "Möhre", "Carotte", "#EE8A2C");
const LETTUCE: Crop = crop("Lettuce", "Salat", "Laitue", "#8DC26F");
const ONION: Crop = crop("Onion", "Zwiebel", "Oignon", "#C9A26B");
const BEAN: Crop = crop("Bean", "Bohne", "Haricot", "#3E8E5A");
const BEET: Crop = crop("Beetroot", "Rote Bete", "Betterave", "#B23A6B");
const ZUCCHINI: Crop = crop("Zucchini", "Zucchini", "Courgette", "#4E9B45");
const FENNEL: Crop = crop("Fennel", "Fenchel", "Fenouil", "#A7C957");
const POTATO: Crop = crop("Potato", "Kartoffel", "Pomme de terre", "#B8925E");
const PEPPER: Crop = crop("Pepper", "Paprika", "Poivron", "#D9452F");

#[derive(Clone, Copy, PartialEq, Eq)]
enum Lang {
    En,
    De,
    /// French: only the phone screens home, planner and companions are
    /// generated (the others fall back to English); texts come from
    /// [`FRENCH`], keyed by the English text.
    Fr,
}

impl Lang {
    fn t<'a>(self, en: &'a str, de: &'a str) -> &'a str {
        match self {
            Self::En => en,
            Self::De => de,
            Self::Fr => match FRENCH.iter().find(|(e, _)| *e == en) {
                Some((_, fr)) => fr,
                None => panic!("no French text for {en:?}"),
            },
        }
    }
    fn c(self, c: Crop) -> &'static str {
        match self {
            Self::En => c.en,
            Self::De => c.de,
            Self::Fr => c.fr,
        }
    }
}

/// French texts of the generated French screens (English → French).
/// French typography: no-break space (U+00A0) before `:`.
const FRENCH: &[(&str, &str)] = &[
    // navigation
    ("Garden", "Jardin"),
    ("Plan", "Plan"),
    ("Calendar", "Calendrier"),
    ("Harvest", "Récolte"),
    // home
    ("Spring 2026 · 4 beds", "Printemps 2026 · 4 planches"),
    ("My Garden", "Mon potager"),
    ("NEXT TASK", "PROCHAINE TÂCHE"),
    ("Water tomatoes", "Arrosage"),
    ("Today, 18:00 · Bed 1", "Tomates · aujourd’hui, 18 h"),
    ("Mark done", "C’est fait"),
    ("Beds", "Planches"),
    ("See all", "Tout voir"),
    ("Bed 1", "Planche 1"),
    ("Bed 2", "Planche 2"),
    ("Bed 3", "Planche 3"),
    ("Bed 4", "Planche 4"),
    ("Bed 5", "Planche 5"),
    ("Bed 6", "Planche 6"),
    ("Tomatoes & basil", "Tomates et basilic"),
    ("Salad mix", "Mesclun"),
    ("Carrots & onions", "Carottes et oignons"),
    ("Beans & zucchini", "Haricots et courgettes"),
    ("Peppers", "Poivrons"),
    ("Potatoes", "Pommes de terre"),
    ("6 plants · 2 tasks", "6 plants · 2 tâches"),
    ("12 plants · 1 task", "12 plants · 1 tâche"),
    ("18 plants", "18 plants"),
    ("9 plants · 3 tasks", "9 plants · 3 tâches"),
    ("8 plants", "8 plants"),
    ("14 plants · 1 task", "14 plants · 1 tâche"),
    (
        "**Tip:** Basil next to tomatoes keeps pests away.",
        "**Astuce\u{a0}:** le basilic près des tomates éloigne les ravageurs.",
    ),
    // planner
    ("Save", "OK"),
    (
        "Bed 1 · 120 × 240 cm · full sun",
        "Planche 1 · 120 × 240 cm · plein soleil",
    ),
    ("Layout", "Disposition"),
    ("List", "Liste"),
    ("Notes", "Notes"),
    ("Good neighbour", "Bon voisin"),
    ("Add plants", "Ajouter des plantes"),
    ("Search 240 plants", "Rechercher parmi 240 plantes"),
    // companions
    ("Plants", "Plantes"),
    ("VEGETABLE · NIGHTSHADE", "LÉGUME · SOLANACÉE"),
    ("Full sun", "Plein soleil"),
    ("Water often", "Arrosage régulier"),
    (
        "Repels aphids and whitefly",
        "Éloigne pucerons et aleurodes",
    ),
    ("Attracts pollinators", "Attire les pollinisateurs"),
    ("Loosens the soil", "Ameublit le sol"),
    ("Stunts growth", "Freine la croissance"),
    ("Shares blight", "Propage le mildiou"),
    ("Good neighbours", "Bons voisins"),
    ("Keep apart", "À tenir à l’écart"),
    ("Growing tips", "Conseils de culture"),
    (
        "**Plant out after the last frost.** Set tomatoes deeper than in the pot, stake them early and water at the base in the morning.",
        "**Repiquer après les dernières gelées.** Enterrer les tomates plus profond que dans le pot, les tuteurer tôt et arroser au pied le matin.",
    ),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Platform {
    Ios,
    Android,
    Tablet,
}

// ---------------------------------------------------------------- painter

/// Minimal immediate-mode UI painter in points (`s` px per pt).
struct Ui<'a> {
    pm: Pixmap,
    lay: &'a mut TextLayouter,
    s: f32,
}

fn core(c: Rgba) -> CoreRgba {
    CoreRgba {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

impl<'a> Ui<'a> {
    fn new(lay: &'a mut TextLayouter, w: u32, h: u32, s: f32) -> Self {
        Self {
            pm: Pixmap::new(w, h).expect("screen size"),
            lay,
            s,
        }
    }

    /// Size in points.
    fn size(&self) -> (f32, f32) {
        (
            self.pm.width() as f32 / self.s,
            self.pm.height() as f32 / self.s,
        )
    }

    fn ts(&self) -> Transform {
        Transform::from_scale(self.s, self.s)
    }

    fn fill(&mut self, c: &str) {
        let (w, h) = self.size();
        self.rect(0.0, 0.0, w, h, 0.0, hex(c));
    }

    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgba) {
        if let Some(p) = round_rect(x, y, w, h, r) {
            let ts = self.ts();
            self.pm
                .fill_path(&p, &solid(c), FillRule::Winding, ts, None);
        }
    }

    fn outline(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, width: f32, c: Rgba) {
        if let Some(p) = round_rect(x, y, w, h, r) {
            let ts = self.ts();
            let st = Stroke {
                width,
                ..Stroke::default()
            };
            self.pm.stroke_path(&p, &solid(c), &st, ts, None);
        }
    }

    /// Soft drop shadow under a card (a few stacked translucent rects).
    fn shadow(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, a: f32) {
        for i in 1..=4 {
            let d = i as f32 * 1.5;
            self.rect(
                x - d * 0.5,
                y - d * 0.2 + d,
                w + d,
                h + d * 0.4,
                r + d,
                with_alpha([20, 40, 25, 255], a / i as f32 * 0.35),
            );
        }
    }

    fn card(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32) {
        self.shadow(x, y, w, h, r, 0.12);
        self.rect(x, y, w, h, r, hex(CARD));
    }

    /// Linear gradient from the top-left to the bottom-right corner.
    fn grad(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, a: Rgba, b: Rgba) {
        if let Some(p) = round_rect(x, y, w, h, r) {
            let ts = self.ts();
            let sh = linear((x, y), (x + w, y + h), &[(0.0, a), (1.0, b)]);
            self.pm
                .fill_path(&p, &shader_paint(sh), FillRule::Winding, ts, None);
        }
    }

    /// Vertical gradient (top → bottom).
    fn vgrad(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, a: Rgba, b: Rgba) {
        if let Some(p) = round_rect(x, y, w, h, r) {
            let ts = self.ts();
            let sh = linear((x, y), (x, y + h), &[(0.0, a), (1.0, b)]);
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

    fn polyline(&mut self, pts: &[(f32, f32)], width: f32, c: Rgba, close: bool) {
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(*x, *y);
            } else {
                pb.line_to(*x, *y);
            }
        }
        if close {
            pb.close();
        }
        if let Some(p) = pb.finish() {
            let ts = self.ts();
            let st = Stroke {
                width,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            self.pm.stroke_path(&p, &solid(c), &st, ts, None);
        }
    }

    /// Open stroke with butt caps (donut segments).
    fn arc(&mut self, pts: &[(f32, f32)], width: f32, c: Rgba) {
        let mut pb = PathBuilder::new();
        pb.move_to(pts[0].0, pts[0].1);
        for (x, y) in &pts[1..] {
            pb.line_to(*x, *y);
        }
        if let Some(p) = pb.finish() {
            let ts = self.ts();
            let st = Stroke {
                width,
                ..Stroke::default()
            };
            self.pm.stroke_path(&p, &solid(c), &st, ts, None);
        }
    }

    fn polygon(&mut self, pts: &[(f32, f32)], c: Rgba) {
        let mut pb = PathBuilder::new();
        for (i, (x, y)) in pts.iter().enumerate() {
            if i == 0 {
                pb.move_to(*x, *y);
            } else {
                pb.line_to(*x, *y);
            }
        }
        pb.close();
        if let Some(p) = pb.finish() {
            let ts = self.ts();
            self.pm
                .fill_path(&p, &solid(c), FillRule::Winding, ts, None);
        }
    }

    /// Progress ring: background circle line plus an arc from 12 o'clock.
    fn ring(&mut self, cx: f32, cy: f32, r: f32, width: f32, frac: f32, bg: Rgba, fg: Rgba) {
        let n = 96;
        let full: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let a = i as f32 / n as f32 * std::f32::consts::TAU;
                (cx + r * a.sin(), cy - r * a.cos())
            })
            .collect();
        self.polyline(&full, width, bg, true);
        let k = (n as f32 * frac) as usize;
        self.polyline(&full[..=k], width, fg, false);
    }

    /// One paragraph; `y` is the top of the box. Returns the text height.
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
    ) -> f32 {
        let style = ResolvedTextStyle {
            font: if weight >= 700 {
                "Inter Display"
            } else {
                "Inter"
            }
            .to_owned(),
            weight,
            size,
            line_height: 1.22,
            color: core(c),
            ..ResolvedTextStyle::default()
        };
        let mut input = TextInput::new(s, &style, w);
        input.align = align;
        let l = self.lay.layout(&input);
        let ts = self.ts().pre_translate(x, y);
        draw_layout(&mut self.pm, &l, self.lay.fonts(), ts, None);
        l.content.h
    }

    fn left(&mut self, x: f32, y: f32, w: f32, size: f32, weight: u16, c: &str, s: &str) -> f32 {
        self.text(x, y, w, size, weight, hex(c), s, Align::Left)
    }

    /// A small round "plant" token: coloured disc with a highlight and leaf.
    fn plant(&mut self, cx: f32, cy: f32, r: f32, color: &str) {
        let c = hex(color);
        self.dot(cx, cy + r * 0.08, r, with_alpha([0, 0, 0, 255], 0.12));
        self.dot(cx, cy, r, c);
        self.dot(
            cx - r * 0.3,
            cy - r * 0.3,
            r * 0.32,
            with_alpha([255, 255, 255, 255], 0.35),
        );
        // leaf on top
        self.leaf(cx + r * 0.35, cy - r * 0.95, r * 0.55, hex("#3E8E5A"));
    }

    fn leaf(&mut self, cx: f32, cy: f32, r: f32, c: Rgba) {
        let mut pb = PathBuilder::new();
        pb.move_to(cx - r, cy + r * 0.4);
        pb.quad_to(cx - r * 0.2, cy - r * 0.9, cx + r, cy - r * 0.5);
        pb.quad_to(cx + r * 0.2, cy + r * 0.9, cx - r, cy + r * 0.4);
        pb.close();
        if let Some(p) = pb.finish() {
            let ts = self.ts();
            self.pm
                .fill_path(&p, &solid(c), FillRule::Winding, ts, None);
        }
    }

    /// Simple line icons (`size` = box edge in pt, centred at cx, cy).
    fn icon(&mut self, kind: &str, cx: f32, cy: f32, size: f32, c: Rgba) {
        let h = size / 2.0;
        let lw = (size * 0.11).max(1.2);
        match kind {
            "home" => {
                self.polyline(
                    &[
                        (cx - h * 0.8, cy - h * 0.05),
                        (cx, cy - h * 0.8),
                        (cx + h * 0.8, cy - h * 0.05),
                    ],
                    lw,
                    c,
                    false,
                );
                self.polyline(
                    &[
                        (cx - h * 0.6, cy - h * 0.2),
                        (cx - h * 0.6, cy + h * 0.75),
                        (cx + h * 0.6, cy + h * 0.75),
                        (cx + h * 0.6, cy - h * 0.2),
                    ],
                    lw,
                    c,
                    false,
                );
            }
            "grid" => {
                let q = h * 0.75;
                for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                    self.rect(
                        cx + dx * q * 0.55 - q * 0.42,
                        cy + dy * q * 0.55 - q * 0.42,
                        q * 0.84,
                        q * 0.84,
                        q * 0.2,
                        c,
                    );
                }
            }
            "calendar" => {
                self.outline(
                    cx - h * 0.8,
                    cy - h * 0.65,
                    h * 1.6,
                    h * 1.45,
                    h * 0.25,
                    lw,
                    c,
                );
                self.rect(cx - h * 0.8, cy - h * 0.65, h * 1.6, h * 0.45, h * 0.2, c);
                self.dot(cx - h * 0.35, cy + h * 0.3, lw * 0.9, c);
                self.dot(cx + h * 0.05, cy + h * 0.3, lw * 0.9, c);
                self.dot(cx + h * 0.45, cy + h * 0.3, lw * 0.9, c);
            }
            "chart" => {
                for (i, f) in [0.5_f32, 0.9, 0.7].iter().enumerate() {
                    let bx = cx - h * 0.75 + i as f32 * h * 0.58;
                    self.rect(
                        bx,
                        cy + h * 0.75 - h * 1.5 * f,
                        h * 0.38,
                        h * 1.5 * f,
                        1.0,
                        c,
                    );
                }
            }
            "leaf" => self.leaf(cx, cy, h * 0.9, c),
            "drop" => {
                self.dot(cx, cy + h * 0.25, h * 0.55, c);
                self.polygon(
                    &[
                        (cx, cy - h * 0.85),
                        (cx - h * 0.5, cy + h * 0.1),
                        (cx + h * 0.5, cy + h * 0.1),
                    ],
                    c,
                );
            }
            "sun" => {
                self.dot(cx, cy, h * 0.42, c);
                for i in 0..8 {
                    let a = i as f32 / 8.0 * std::f32::consts::TAU;
                    let (s, co) = a.sin_cos();
                    self.polyline(
                        &[
                            (cx + s * h * 0.62, cy + co * h * 0.62),
                            (cx + s * h * 0.88, cy + co * h * 0.88),
                        ],
                        lw,
                        c,
                        false,
                    );
                }
            }
            "check" => self.polyline(
                &[
                    (cx - h * 0.55, cy),
                    (cx - h * 0.15, cy + h * 0.4),
                    (cx + h * 0.6, cy - h * 0.45),
                ],
                lw * 1.3,
                c,
                false,
            ),
            "cross" => {
                self.polyline(
                    &[
                        (cx - h * 0.45, cy - h * 0.45),
                        (cx + h * 0.45, cy + h * 0.45),
                    ],
                    lw * 1.3,
                    c,
                    false,
                );
                self.polyline(
                    &[
                        (cx - h * 0.45, cy + h * 0.45),
                        (cx + h * 0.45, cy - h * 0.45),
                    ],
                    lw * 1.3,
                    c,
                    false,
                );
            }
            "plus" => {
                self.polyline(
                    &[(cx - h * 0.6, cy), (cx + h * 0.6, cy)],
                    lw * 1.2,
                    c,
                    false,
                );
                self.polyline(
                    &[(cx, cy - h * 0.6), (cx, cy + h * 0.6)],
                    lw * 1.2,
                    c,
                    false,
                );
            }
            "back" => self.polyline(
                &[
                    (cx + h * 0.25, cy - h * 0.6),
                    (cx - h * 0.35, cy),
                    (cx + h * 0.25, cy + h * 0.6),
                ],
                lw * 1.2,
                c,
                false,
            ),
            "chevron" => self.polyline(
                &[
                    (cx - h * 0.2, cy - h * 0.45),
                    (cx + h * 0.25, cy),
                    (cx - h * 0.2, cy + h * 0.45),
                ],
                lw,
                c,
                false,
            ),
            "search" => {
                let pts: Vec<(f32, f32)> = (0..=32)
                    .map(|i| {
                        let a = i as f32 / 32.0 * std::f32::consts::TAU;
                        (
                            cx - h * 0.15 + h * 0.5 * a.sin(),
                            cy - h * 0.15 + h * 0.5 * a.cos(),
                        )
                    })
                    .collect();
                self.polyline(&pts, lw, c, true);
                self.polyline(
                    &[(cx + h * 0.22, cy + h * 0.22), (cx + h * 0.7, cy + h * 0.7)],
                    lw * 1.2,
                    c,
                    false,
                );
            }
            "gear" | "user" => {
                self.dot(cx, cy - h * 0.3, h * 0.35, c);
                self.rect(cx - h * 0.7, cy + h * 0.15, h * 1.4, h * 0.7, h * 0.35, c);
            }
            "bell" => {
                self.rect(cx - h * 0.55, cy - h * 0.6, h * 1.1, h * 1.1, h * 0.5, c);
                self.rect(cx - h * 0.75, cy + h * 0.25, h * 1.5, h * 0.25, h * 0.1, c);
                self.dot(cx, cy + h * 0.72, h * 0.18, c);
            }
            _ => self.dot(cx, cy, h * 0.6, c),
        }
    }

    fn encode(&self) -> Vec<u8> {
        encode_png_rgb_with(&self.pm, [255, 255, 255], png::Compression::High).expect("png")
    }
}

// --------------------------------------------------------- system chrome

fn status_bar(ui: &mut Ui<'_>, p: Platform, dark: bool) {
    let (w, _) = ui.size();
    let c = if dark { hex("#FFFFFF") } else { hex("#111111") };
    match p {
        Platform::Ios => {
            ui.text(28.0, 16.0, 80.0, 16.0, 600, c, "9:41", Align::Center);
            for i in 0..4 {
                let h = 4.0 + i as f32 * 2.4;
                ui.rect(w - 104.0 + i as f32 * 5.0, 30.0 - h, 3.4, h, 1.0, c);
            }
            ui.polyline(
                &[(w - 77.0, 22.5), (w - 71.0, 19.0), (w - 65.0, 22.5)],
                2.0,
                c,
                false,
            );
            ui.dot(w - 71.0, 27.0, 2.0, c);
            ui.outline(w - 58.0, 19.0, 24.0, 12.0, 3.5, 1.0, with_alpha(c, 0.45));
            ui.rect(w - 56.0, 21.0, 18.0, 8.0, 2.0, c);
            ui.rect(w - 33.0, 23.0, 1.8, 4.0, 0.9, with_alpha(c, 0.45));
        }
        Platform::Android => {
            ui.text(20.0, 12.0, 60.0, 14.0, 600, c, "9:41", Align::Left);
            ui.polygon(&[(w - 78.0, 25.0), (w - 66.0, 25.0), (w - 66.0, 13.0)], c);
            ui.polygon(&[(w - 60.0, 17.0), (w - 54.0, 25.0), (w - 48.0, 17.0)], c);
            ui.rect(w - 40.0, 13.0, 9.0, 13.0, 2.0, c);
        }
        Platform::Tablet => {
            ui.text(20.0, 10.0, 60.0, 13.0, 600, c, "9:41", Align::Left);
            ui.text(70.0, 10.0, 120.0, 13.0, 600, c, "Mon 12 May", Align::Left);
            ui.outline(w - 46.0, 11.0, 24.0, 12.0, 3.5, 1.0, with_alpha(c, 0.45));
            ui.rect(w - 44.0, 13.0, 18.0, 8.0, 2.0, c);
            ui.text(w - 92.0, 10.0, 40.0, 13.0, 600, c, "100%", Align::Right);
        }
    }
}

fn home_indicator(ui: &mut Ui<'_>, p: Platform, dark: bool) {
    let (w, h) = ui.size();
    let c = if dark { hex("#FFFFFF") } else { hex("#111111") };
    match p {
        Platform::Ios => ui.rect(w / 2.0 - 68.0, h - 12.0, 136.0, 5.0, 2.5, c),
        Platform::Android => ui.rect(w / 2.0 - 54.0, h - 10.0, 108.0, 4.0, 2.0, c),
        Platform::Tablet => ui.rect(w / 2.0 - 160.0, h - 10.0, 320.0, 5.0, 2.5, c),
    }
}

/// Bottom tab bar (iOS) / navigation bar (Android, Material style).
fn tab_bar(ui: &mut Ui<'_>, p: Platform, lang: Lang, active: usize, dark: bool) {
    let (w, h) = ui.size();
    let tabs = [
        ("home", lang.t("Garden", "Garten")),
        ("grid", lang.t("Plan", "Planen")),
        ("calendar", lang.t("Calendar", "Kalender")),
        ("chart", lang.t("Harvest", "Ernte")),
    ];
    let (bg, on, off) = if dark {
        (hex("#141B17"), hex("#6FD08C"), hex("#5E6B63"))
    } else {
        (hex("#FFFFFF"), hex(GREEN), hex("#98A29A"))
    };
    let bar = if p == Platform::Android { 78.0 } else { 84.0 };
    ui.rect(0.0, h - bar, w, bar, 0.0, bg);
    ui.rect(
        0.0,
        h - bar,
        w,
        1.0,
        0.0,
        if dark { hex("#222B25") } else { hex(LINE) },
    );
    for (i, (icon, label)) in tabs.iter().enumerate() {
        let cx = w / 8.0 * (2.0 * i as f32 + 1.0);
        let c = if i == active { on } else { off };
        if p == Platform::Android {
            if i == active {
                ui.rect(
                    cx - 30.0,
                    h - bar + 10.0,
                    60.0,
                    30.0,
                    15.0,
                    with_alpha(on, 0.18),
                );
            }
            ui.icon(icon, cx, h - bar + 25.0, 18.0, c);
            ui.text(
                cx - 45.0,
                h - bar + 45.0,
                90.0,
                11.5,
                600,
                c,
                label,
                Align::Center,
            );
        } else {
            ui.icon(icon, cx, h - bar + 20.0, 20.0, c);
            ui.text(
                cx - 45.0,
                h - bar + 36.0,
                90.0,
                10.5,
                600,
                c,
                label,
                Align::Center,
            );
        }
    }
}

/// Tablet sidebar with app name and navigation; returns its width.
fn sidebar(ui: &mut Ui<'_>, lang: Lang, active: usize) -> f32 {
    let (_, h) = ui.size();
    let sw = 250.0;
    ui.rect(0.0, 0.0, sw, h, 0.0, hex("#ECEFE7"));
    ui.rect(sw - 1.0, 0.0, 1.0, h, 0.0, hex("#DCE1D7"));
    ui.dot(36.0, 62.0, 14.0, hex(GREEN));
    ui.leaf(36.0, 62.0, 8.0, hex("#FFFFFF"));
    ui.left(58.0, 50.0, 180.0, 21.0, 700, INK, "Fernwood");
    let items = [
        ("home", lang.t("Garden", "Garten")),
        ("grid", lang.t("Bed planner", "Beetplaner")),
        ("leaf", lang.t("Plants", "Pflanzen")),
        ("calendar", lang.t("Calendar", "Kalender")),
        ("chart", lang.t("Harvest", "Ernte")),
        ("gear", lang.t("Settings", "Einstellungen")),
    ];
    for (i, (icon, label)) in items.iter().enumerate() {
        let y = 104.0 + i as f32 * 46.0;
        let on = i == active;
        if on {
            ui.rect(12.0, y, sw - 24.0, 38.0, 10.0, hex("#D6E6D0"));
        }
        let c = if on { hex(GREEN) } else { hex("#4A544C") };
        ui.icon(icon, 36.0, y + 19.0, 18.0, c);
        ui.text(58.0, y + 10.0, 170.0, 15.0, 600, c, label, Align::Left);
    }
    // Garden switcher at the bottom.
    ui.rect(12.0, h - 120.0, sw - 24.0, 64.0, 14.0, hex("#FFFFFF"));
    ui.dot(42.0, h - 88.0, 16.0, hex(MINT));
    ui.text(
        36.0,
        h - 96.5,
        12.0,
        13.0,
        600,
        hex(GREEN),
        "M",
        Align::Center,
    );
    ui.left(68.0, h - 106.0, 150.0, 14.0, 600, INK, "Maya Torres");
    ui.left(
        68.0,
        h - 86.0,
        150.0,
        12.0,
        400,
        MUTED,
        lang.t("Backyard · 4 beds", "Hof · 4 Beete"),
    );
    sw
}

/// Screen frame for the platform: background, status bar, navigation.
/// Returns the content area (x, y, w) and the bottom of the usable area.
fn chrome(
    ui: &mut Ui<'_>,
    p: Platform,
    lang: Lang,
    tab: usize,
    side: usize,
    dark: bool,
) -> (f32, f32, f32, f32) {
    let (w, h) = ui.size();
    ui.fill(if dark { "#0F1512" } else { BG });
    if p == Platform::Tablet {
        let sw = sidebar(ui, lang, side);
        status_bar(ui, p, dark);
        return (sw + 32.0, 40.0, w - sw - 64.0, h - 24.0);
    }
    status_bar(ui, p, dark);
    tab_bar(ui, p, lang, tab, dark);
    let top = if p == Platform::Ios { 58.0 } else { 44.0 };
    (18.0, top, w - 36.0, h - 90.0)
}

fn header(ui: &mut Ui<'_>, x: f32, y: f32, w: f32, kicker: &str, title: &str, dark: bool) -> f32 {
    let (k, t) = if dark {
        ("#8FA396", "#FFFFFF")
    } else {
        (MUTED, INK)
    };
    ui.left(x, y, w, 14.0, 600, k, kicker);
    ui.left(x, y + 20.0, w, 32.0, 700, t, title);
    y + 66.0
}

fn avatar(ui: &mut Ui<'_>, cx: f32, cy: f32, r: f32, initials: &str, bg: &str, fg: &str) {
    ui.dot(cx, cy, r, hex(bg));
    ui.text(
        cx - r,
        cy - r * 0.62,
        r * 2.0,
        r * 0.9,
        600,
        hex(fg),
        initials,
        Align::Center,
    );
}

// ------------------------------------------------------- handheld screens

/// Beds overview with the next task.
fn screen_home(ui: &mut Ui<'_>, p: Platform, lang: Lang) {
    let (x, y0, w, bottom) = chrome(ui, p, lang, 0, 0, false);
    let y = header(
        ui,
        x,
        y0 + 8.0,
        w - 60.0,
        lang.t("Spring 2026 · 4 beds", "Frühling 2026 · 4 Beete"),
        lang.t("My Garden", "Mein Garten"),
        false,
    );
    avatar(ui, x + w - 22.0, y0 + 40.0, 21.0, "MT", MINT, GREEN);

    // Next task card
    let ch = 160.0;
    ui.shadow(x, y + 6.0, w, ch, 24.0, 0.25);
    ui.grad(x, y + 6.0, w, ch, 24.0, hex("#2A7243"), hex("#62B06C"));
    let cy = y + 6.0;
    ui.text(
        x + 20.0,
        cy + 20.0,
        w,
        12.5,
        600,
        with_alpha(hex("#FFFFFF"), 0.8),
        lang.t("NEXT TASK", "NÄCHSTE AUFGABE"),
        Align::Left,
    );
    ui.text(
        x + 20.0,
        cy + 40.0,
        w - 140.0,
        25.0,
        700,
        hex("#FFFFFF"),
        lang.t("Water tomatoes", "Tomaten gießen"),
        Align::Left,
    );
    ui.text(
        x + 20.0,
        cy + 76.0,
        w - 140.0,
        14.0,
        400,
        with_alpha(hex("#FFFFFF"), 0.85),
        lang.t("Today, 18:00 · Bed 1", "Heute, 18:00 · Beet 1"),
        Align::Left,
    );
    let bw = if lang == Lang::En { 118.0 } else { 150.0 };
    ui.rect(
        x + 20.0,
        cy + 108.0,
        bw,
        34.0,
        17.0,
        with_alpha(hex("#FFFFFF"), 0.24),
    );
    ui.text(
        x + 20.0,
        cy + 116.5,
        bw,
        14.0,
        600,
        hex("#FFFFFF"),
        lang.t("Mark done", "Erledigt"),
        Align::Center,
    );
    ui.ring(
        x + w - 62.0,
        cy + 80.0,
        38.0,
        8.0,
        0.72,
        with_alpha(hex("#FFFFFF"), 0.25),
        hex("#FFFFFF"),
    );
    ui.text(
        x + w - 112.0,
        cy + 68.0,
        100.0,
        20.0,
        700,
        hex("#FFFFFF"),
        "72%",
        Align::Center,
    );

    // Beds
    let y = y + ch + 30.0;
    ui.left(x, y, 200.0, 21.0, 700, INK, lang.t("Beds", "Beete"));
    ui.text(
        x + w - 120.0,
        y + 4.0,
        120.0,
        14.0,
        600,
        hex(GREEN),
        lang.t("See all", "Alle"),
        Align::Right,
    );
    let beds: [(&str, &str, &str, &str, &str, &str, [Crop; 3]); 6] = [
        (
            "Bed 1",
            "Beet 1",
            "Tomatoes & basil",
            "Tomaten & Basilikum",
            "6 plants · 2 tasks",
            "6 Pflanzen · 2 Aufgaben",
            [TOMATO, BASIL, MARIGOLD],
        ),
        (
            "Bed 2",
            "Beet 2",
            "Salad mix",
            "Salatmischung",
            "12 plants · 1 task",
            "12 Pflanzen · 1 Aufgabe",
            [LETTUCE, BEET, ONION],
        ),
        (
            "Bed 3",
            "Beet 3",
            "Carrots & onions",
            "Möhren & Zwiebeln",
            "18 plants",
            "18 Pflanzen",
            [CARROT, ONION, LETTUCE],
        ),
        (
            "Bed 4",
            "Beet 4",
            "Beans & zucchini",
            "Bohnen & Zucchini",
            "9 plants · 3 tasks",
            "9 Pflanzen · 3 Aufgaben",
            [BEAN, ZUCCHINI, MARIGOLD],
        ),
        (
            "Bed 5",
            "Beet 5",
            "Peppers",
            "Paprika",
            "8 plants",
            "8 Pflanzen",
            [PEPPER, BASIL, TOMATO],
        ),
        (
            "Bed 6",
            "Beet 6",
            "Potatoes",
            "Kartoffeln",
            "14 plants · 1 task",
            "14 Pflanzen · 1 Aufgabe",
            [POTATO, BEAN, MARIGOLD],
        ),
    ];
    let cols = if w > 600.0 { 3 } else { 2 };
    let gap = 12.0;
    let cw = (w - gap * (cols as f32 - 1.0)) / cols as f32;
    let rh = 164.0;
    let mut by = y + 36.0;
    let rows_fit = ((bottom - 72.0 - by + gap) / (rh + gap)).floor().max(1.0) as usize;
    for (i, (n_en, n_de, s_en, s_de, m_en, m_de, crops)) in
        beds.iter().enumerate().take(rows_fit * cols)
    {
        let col = (i % cols) as f32;
        let row = (i / cols) as f32;
        let bx = x + col * (cw + gap);
        by = y + 36.0 + row * (rh + gap);
        ui.card(bx, by, cw, rh, 20.0);
        ui.rect(bx + 10.0, by + 10.0, cw - 20.0, 80.0, 14.0, hex("#E9EFE3"));
        let n = 5;
        for r in 0..2 {
            for c in 0..n {
                let k = (i * 3 + r * 2 + c) % 3;
                let px = bx + 10.0 + (cw - 20.0) / n as f32 * (c as f32 + 0.5);
                let py = by + 10.0 + 80.0 / 2.0 * (r as f32 + 0.5) + 4.0;
                ui.plant(
                    px,
                    py,
                    10.0 + ((c + r + i) % 2) as f32 * 2.0,
                    crops[k].color,
                );
            }
        }
        ui.left(
            bx + 14.0,
            by + 98.0,
            cw - 28.0,
            16.5,
            600,
            INK,
            lang.t(n_en, n_de),
        );
        ui.left(
            bx + 14.0,
            by + 119.0,
            cw - 28.0,
            13.5,
            400,
            INK,
            lang.t(s_en, s_de),
        );
        ui.left(
            bx + 14.0,
            by + 139.0,
            cw - 28.0,
            12.5,
            400,
            MUTED,
            lang.t(m_en, m_de),
        );
    }
    // Tip
    let ty = by + rh + 16.0;
    if ty + 56.0 < bottom {
        ui.rect(x, ty, w, 56.0, 18.0, hex("#FFF4DC"));
        ui.dot(x + 28.0, ty + 28.0, 13.0, hex(AMBER));
        ui.icon("sun", x + 28.0, ty + 28.0, 18.0, hex("#FFFFFF"));
        ui.text(
            x + 52.0,
            ty + 12.0,
            w - 66.0,
            13.5,
            400,
            hex("#5A4210"),
            lang.t(
                "**Tip:** Basil next to tomatoes keeps pests away.",
                "**Tipp:** Basilikum neben Tomaten hält Schädlinge fern.",
            ),
            Align::Left,
        );
    }
    home_indicator(ui, p, false);
}

/// Drag & drop bed planner.
fn screen_planner(ui: &mut Ui<'_>, p: Platform, lang: Lang) {
    let (x, y0, w, bottom) = chrome(ui, p, lang, 1, 1, false);
    ui.icon("back", x + 6.0, y0 + 20.0, 20.0, hex(GREEN));
    ui.text(
        x + 18.0,
        y0 + 11.0,
        150.0,
        15.0,
        600,
        hex(GREEN),
        lang.t("Garden", "Garten"),
        Align::Left,
    );
    ui.rect(x + w - 74.0, y0 + 4.0, 74.0, 32.0, 16.0, hex(GREEN));
    ui.text(
        x + w - 74.0,
        y0 + 11.0,
        74.0,
        14.0,
        600,
        hex("#FFFFFF"),
        lang.t("Save", "Fertig"),
        Align::Center,
    );
    let y = header(
        ui,
        x,
        y0 + 48.0,
        w,
        lang.t(
            "Bed 1 · 120 × 240 cm · full sun",
            "Beet 1 · 120 × 240 cm · sonnig",
        ),
        lang.t("Tomatoes & basil", "Tomaten & Basilikum"),
        false,
    );
    // segmented control
    let seg = [
        lang.t("Layout", "Anordnung"),
        lang.t("List", "Liste"),
        lang.t("Notes", "Notizen"),
    ];
    let sw = w.min(360.0);
    ui.rect(x, y + 4.0, sw, 34.0, 10.0, hex("#E6E9E2"));
    let s3 = sw / 3.0;
    ui.card(x + 3.0, y + 7.0, s3 - 6.0, 28.0, 8.0);
    for (i, s) in seg.iter().enumerate() {
        ui.text(
            x + s3 * i as f32,
            y + 13.0,
            s3,
            13.5,
            600,
            hex(if i == 0 { INK } else { MUTED }),
            s,
            Align::Center,
        );
    }
    // the bed
    let by = y + 56.0;
    let sheet_h = 168.0;
    let bh = bottom - sheet_h - by - 18.0;
    let bw = if w > 600.0 { w * 0.62 } else { w };
    ui.shadow(x, by, bw, bh, 22.0, 0.3);
    ui.rect(x, by, bw, bh, 22.0, hex("#9C7650"));
    ui.vgrad(
        x + 10.0,
        by + 10.0,
        bw - 20.0,
        bh - 20.0,
        15.0,
        hex(SOIL),
        hex(SOIL_D),
    );
    let cols = 3;
    let rows = ((bh - 20.0) / 120.0).floor().max(3.0) as usize;
    let cw = (bw - 20.0) / cols as f32;
    let rh = (bh - 20.0) / rows as f32;
    let plan = [
        TOMATO, BASIL, TOMATO, MARIGOLD, TOMATO, BASIL, BASIL, TOMATO, MARIGOLD, TOMATO, BASIL,
        TOMATO, MARIGOLD, BASIL, TOMATO,
    ];
    // grid lines
    for c in 1..cols {
        let lx = x + 10.0 + cw * c as f32;
        ui.rect(
            lx - 0.5,
            by + 18.0,
            1.0,
            bh - 36.0,
            0.0,
            with_alpha(hex("#FFFFFF"), 0.12),
        );
    }
    for r in 1..rows {
        let ly = by + 10.0 + rh * r as f32;
        ui.rect(
            x + 18.0,
            ly - 0.5,
            bw - 36.0,
            1.0,
            0.0,
            with_alpha(hex("#FFFFFF"), 0.12),
        );
    }
    let empty = (rows - 2) * cols + 1; // drop target
    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            let cx = x + 10.0 + cw * (c as f32 + 0.5);
            let cy = by + 10.0 + rh * (r as f32 + 0.5) - 8.0;
            if i == empty {
                ui.outline(
                    cx - 30.0,
                    cy - 30.0,
                    60.0,
                    60.0,
                    30.0,
                    2.0,
                    with_alpha(hex("#FFFFFF"), 0.7),
                );
                ui.icon("plus", cx, cy, 18.0, with_alpha(hex("#FFFFFF"), 0.8));
                continue;
            }
            let crop = plan[i % plan.len()];
            ui.plant(cx, cy, 24.0, crop.color);
            ui.text(
                cx - cw / 2.0,
                cy + 32.0,
                cw,
                11.5,
                600,
                with_alpha(hex("#FFFFFF"), 0.92),
                lang.c(crop),
                Align::Center,
            );
        }
    }
    // a plant being dragged over the empty cell
    let (dx, dy) = (
        x + 10.0 + cw * 1.5 + 16.0,
        by + 10.0 + rh * (rows as f32 - 1.5) - 22.0,
    );
    ui.dot(dx + 6.0, dy + 12.0, 34.0, with_alpha([0, 0, 0, 255], 0.25));
    ui.dot(dx, dy, 34.0, hex("#FFFFFF"));
    ui.plant(dx, dy + 2.0, 24.0, MARIGOLD.color);
    // compatibility badge
    let label = lang.t("Good neighbour", "Guter Nachbar");
    let lw = match lang {
        Lang::En => 128.0,
        Lang::De => 132.0,
        Lang::Fr => 112.0,
    };
    ui.shadow(dx - lw / 2.0, dy + 42.0, lw, 28.0, 14.0, 0.2);
    ui.rect(dx - lw / 2.0, dy + 42.0, lw, 28.0, 14.0, hex("#E7F5E4"));
    ui.icon("check", dx - lw / 2.0 + 16.0, dy + 56.0, 12.0, hex(GREEN));
    ui.text(
        dx - lw / 2.0 + 26.0,
        dy + 48.5,
        lw - 30.0,
        12.0,
        600,
        hex(GREEN),
        label,
        Align::Left,
    );

    // tablet: side panel with plant info
    if w > 600.0 {
        let px = x + bw + 20.0;
        let pw = w - bw - 20.0;
        ui.card(px, by, pw, bh, 20.0);
        ui.left(
            px + 18.0,
            by + 18.0,
            pw - 36.0,
            18.0,
            700,
            INK,
            lang.t("In this bed", "In diesem Beet"),
        );
        let list = [(TOMATO, "6"), (BASIL, "5"), (MARIGOLD, "3")];
        for (i, (c, n)) in list.iter().enumerate() {
            let ry = by + 60.0 + i as f32 * 54.0;
            ui.plant(px + 34.0, ry + 18.0, 15.0, c.color);
            ui.left(px + 60.0, ry + 9.0, pw - 110.0, 15.0, 600, INK, lang.c(*c));
            ui.text(
                px + pw - 60.0,
                ry + 9.0,
                40.0,
                15.0,
                600,
                hex(MUTED),
                n,
                Align::Right,
            );
        }
        let ty = by + 240.0;
        ui.rect(px + 14.0, ty, pw - 28.0, 1.0, 0.0, hex(LINE));
        ui.left(
            px + 18.0,
            ty + 18.0,
            pw - 36.0,
            15.0,
            700,
            INK,
            lang.t("Companion check", "Nachbarschafts-Check"),
        );
        ui.ring(
            px + pw / 2.0,
            ty + 110.0,
            52.0,
            10.0,
            0.92,
            hex("#E6EFE2"),
            hex(GREEN_L),
        );
        ui.text(px, ty + 94.0, pw, 26.0, 700, hex(INK), "92", Align::Center);
        ui.text(
            px + 18.0,
            ty + 180.0,
            pw - 36.0,
            13.0,
            400,
            hex(MUTED),
            lang.t(
                "All plants in this bed get along well.",
                "Alle Pflanzen in diesem Beet vertragen sich.",
            ),
            Align::Center,
        );
    }

    // plant palette sheet
    let sy = bottom - sheet_h;
    let (sx, sw) = if p == Platform::Tablet {
        (x, w)
    } else {
        (0.0, w + 36.0)
    };
    ui.shadow(sx, sy, sw, sheet_h, 26.0, 0.18);
    ui.rect(sx, sy, sw, sheet_h + 6.0, 26.0, hex(CARD));
    if p != Platform::Tablet {
        ui.rect(sx, sy + 40.0, sw, sheet_h - 34.0, 0.0, hex(CARD));
    }
    ui.rect(x + w / 2.0 - 20.0, sy + 8.0, 40.0, 4.0, 2.0, hex("#D5D9D2"));
    ui.left(
        x,
        sy + 22.0,
        w,
        17.0,
        700,
        INK,
        lang.t("Add plants", "Pflanzen hinzufügen"),
    );
    ui.rect(x, sy + 50.0, w, 36.0, 12.0, hex("#F0F2EC"));
    ui.icon("search", x + 20.0, sy + 68.0, 16.0, hex(MUTED));
    ui.left(
        x + 38.0,
        sy + 59.0,
        w - 60.0,
        14.0,
        400,
        MUTED,
        lang.t("Search 240 plants", "240 Pflanzen durchsuchen"),
    );
    let chips = [
        MARIGOLD, CARROT, LETTUCE, ONION, BEAN, BEET, PEPPER, ZUCCHINI,
    ];
    let mut cx = x;
    for c in chips {
        let name = lang.c(c);
        let cwid = 46.0 + name.chars().count() as f32 * 7.4;
        if cx + cwid > x + w + 40.0 {
            break;
        }
        let selected = c.en == "Marigold";
        ui.rect(
            cx,
            sy + 100.0,
            cwid,
            38.0,
            19.0,
            if selected {
                hex("#E7F5E4")
            } else {
                hex("#F0F2EC")
            },
        );
        if selected {
            ui.outline(cx, sy + 100.0, cwid, 38.0, 19.0, 1.5, hex(GREEN_L));
        }
        ui.plant(cx + 20.0, sy + 120.0, 9.0, c.color);
        ui.left(cx + 36.0, sy + 110.5, cwid, 13.5, 600, INK, name);
        cx += cwid + 8.0;
    }
    home_indicator(ui, p, false);
}

/// Plant detail with companions.
fn screen_companions(ui: &mut Ui<'_>, p: Platform, lang: Lang) {
    let (x, y0, w, bottom) = chrome(ui, p, lang, 1, 2, false);
    ui.icon("back", x + 6.0, y0 + 20.0, 20.0, hex(GREEN));
    ui.text(
        x + 18.0,
        y0 + 11.0,
        150.0,
        15.0,
        600,
        hex(GREEN),
        lang.t("Plants", "Pflanzen"),
        Align::Left,
    );
    // hero
    let hy = y0 + 46.0;
    ui.grad(x, hy, w, 150.0, 24.0, hex("#FCE3DA"), hex("#F8C8B6"));
    ui.dot(
        x + w - 70.0,
        hy + 75.0,
        74.0,
        with_alpha(hex("#FFFFFF"), 0.35),
    );
    ui.plant(x + w - 70.0, hy + 80.0, 44.0, TOMATO.color);
    ui.left(
        x + 20.0,
        hy + 26.0,
        w - 160.0,
        13.0,
        600,
        "#9A4B36",
        lang.t("VEGETABLE · NIGHTSHADE", "GEMÜSE · NACHTSCHATTEN"),
    );
    ui.left(
        x + 20.0,
        hy + 46.0,
        w - 160.0,
        32.0,
        700,
        "#3B1C14",
        lang.c(TOMATO),
    );
    ui.text(
        x + 20.0,
        hy + 88.0,
        w - 160.0,
        14.0,
        400,
        hex("#7A4B3E"),
        "_Solanum lycopersicum_",
        Align::Left,
    );
    // facts
    let fy = hy + 166.0;
    let facts = [
        ("sun", lang.t("Full sun", "Volle Sonne"), "#F2B33D"),
        ("drop", lang.t("Water often", "Viel Wasser"), "#4C9BD6"),
        ("grid", "60 cm", "#5FAE4E"),
    ];
    let fw = (w - 20.0) / 3.0;
    for (i, (icon, label, c)) in facts.iter().enumerate() {
        let fx = x + i as f32 * (fw + 10.0);
        ui.card(fx, fy, fw, 64.0, 16.0);
        ui.icon(icon, fx + fw / 2.0, fy + 22.0, 18.0, hex(c));
        ui.text(fx, fy + 38.0, fw, 12.5, 600, hex(INK), label, Align::Center);
    }
    // neighbours
    let mut y = fy + 92.0;
    let good = [
        (
            BASIL,
            lang.t(
                "Repels aphids and whitefly",
                "Hält Blattläuse und Weiße Fliege fern",
            ),
        ),
        (
            MARIGOLD,
            lang.t("Attracts pollinators", "Lockt Bestäuber an"),
        ),
        (CARROT, lang.t("Loosens the soil", "Lockert den Boden")),
    ];
    let bad = [
        (FENNEL, lang.t("Stunts growth", "Hemmt das Wachstum")),
        (POTATO, lang.t("Shares blight", "Überträgt Krautfäule")),
    ];
    for (title, list, ok) in [
        (lang.t("Good neighbours", "Gute Nachbarn"), &good[..], true),
        (lang.t("Keep apart", "Lieber getrennt"), &bad[..], false),
    ] {
        ui.left(x, y, w, 19.0, 700, INK, title);
        y += 34.0;
        let rh = 62.0;
        let card_h = rh * list.len() as f32;
        if y + card_h > bottom {
            break;
        }
        ui.card(x, y, w, card_h, 18.0);
        for (i, (c, why)) in list.iter().enumerate() {
            let ry = y + i as f32 * rh;
            if i > 0 {
                ui.rect(x + 64.0, ry, w - 64.0, 1.0, 0.0, hex(LINE));
            }
            ui.plant(x + 32.0, ry + rh / 2.0 + 2.0, 15.0, c.color);
            ui.left(x + 62.0, ry + 12.0, w - 120.0, 15.5, 600, INK, lang.c(*c));
            ui.left(x + 62.0, ry + 34.0, w - 110.0, 12.5, 400, MUTED, why);
            let (bg, fg, icon) = if ok {
                ("#E7F5E4", GREEN, "check")
            } else {
                ("#FDE7E3", RED, "cross")
            };
            ui.dot(x + w - 28.0, ry + rh / 2.0, 14.0, hex(bg));
            ui.icon(icon, x + w - 28.0, ry + rh / 2.0, 13.0, hex(fg));
        }
        y += card_h + 26.0;
    }
    if y + 170.0 < bottom {
        ui.left(
            x,
            y,
            w,
            19.0,
            700,
            INK,
            lang.t("Growing tips", "Anbautipps"),
        );
        y += 34.0;
        ui.rect(x, y, w, 120.0, 18.0, hex("#FFF4DC"));
        ui.dot(x + 32.0, y + 32.0, 14.0, hex(AMBER));
        ui.icon("sun", x + 32.0, y + 32.0, 18.0, hex("#FFFFFF"));
        ui.text(
            x + 60.0,
            y + 18.0,
            w - 84.0,
            14.0,
            400,
            hex("#5A4210"),
            lang.t(
                "**Plant out after the last frost.** Set tomatoes deeper than in the pot, stake them early and water at the base in the morning.",
                "**Erst nach dem letzten Frost auspflanzen.** Tomaten tiefer setzen als im Topf, früh anbinden und morgens von unten gießen.",
            ),
            Align::Left,
        );
    }
    home_indicator(ui, p, false);
}

/// Harvest statistics (dark).
fn screen_harvest(ui: &mut Ui<'_>, p: Platform, lang: Lang) {
    let dark = p != Platform::Tablet;
    let (x, y0, w, bottom) = chrome(ui, p, lang, 3, 4, dark);
    let y = header(
        ui,
        x,
        y0 + 8.0,
        w,
        lang.t("Season 2026", "Saison 2026"),
        lang.t("Harvest", "Ernte"),
        dark,
    );
    let (card, ink, muted, track) = if dark {
        (
            hex("#18211C"),
            hex("#FFFFFF"),
            hex("#8FA396"),
            hex("#26302A"),
        )
    } else {
        (hex("#FFFFFF"), hex(INK), hex(MUTED), hex("#EEF1EC"))
    };
    let ch = 320.0;
    if !dark {
        ui.shadow(x, y + 4.0, w, ch, 24.0, 0.12);
    }
    ui.rect(x, y + 4.0, w, ch, 24.0, card);
    let cy = y + 4.0;
    ui.text(
        x + 20.0,
        cy + 20.0,
        w,
        12.5,
        600,
        muted,
        lang.t("TOTAL THIS SEASON", "GESAMT IN DIESER SAISON"),
        Align::Left,
    );
    ui.text(
        x + 20.0,
        cy + 40.0,
        w,
        42.0,
        700,
        ink,
        lang.t("38.4 kg", "38,4 kg"),
        Align::Left,
    );
    ui.text(
        x + 20.0,
        cy + 94.0,
        w,
        14.0,
        600,
        hex("#6FD08C"),
        lang.t("+12 % vs. 2025", "+12 % ggü. 2025"),
        Align::Left,
    );
    let vals = [0.22, 0.4, 0.36, 0.58, 0.8, 0.95, 0.72, 0.48];
    let months = if lang == Lang::De {
        ["Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt"]
    } else {
        ["Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct"]
    };
    let bw = (w - 40.0) / vals.len() as f32;
    let base = cy + ch - 34.0;
    for (i, v) in vals.iter().enumerate() {
        let bh = 150.0 * v;
        let bx = x + 20.0 + i as f32 * bw;
        let (c0, c1) = if i == 5 {
            ("#FFC65C", "#F29A2E")
        } else {
            ("#5BCB7E", "#2F7A47")
        };
        ui.vgrad(bx + 5.0, base - bh, bw - 10.0, bh, 7.0, hex(c0), hex(c1));
        ui.text(
            bx,
            base + 8.0,
            bw,
            11.0,
            400,
            muted,
            months[i],
            Align::Center,
        );
    }
    let rows = [
        (TOMATO, "12.1 kg", "12,1 kg", 0.9),
        (ZUCCHINI, "9.8 kg", "9,8 kg", 0.72),
        (POTATO, "7.5 kg", "7,5 kg", 0.56),
        (CARROT, "4.2 kg", "4,2 kg", 0.32),
        (BEAN, "2.9 kg", "2,9 kg", 0.22),
        (BASIL, "1.9 kg", "1,9 kg", 0.14),
        (LETTUCE, "1.2 kg", "1,2 kg", 0.1),
    ];
    let mut ry = cy + ch + 28.0;
    ui.text(
        x,
        ry,
        300.0,
        20.0,
        700,
        ink,
        lang.t("By crop", "Nach Kultur"),
        Align::Left,
    );
    ry += 38.0;
    for (c, en, de, frac) in rows {
        if ry + 52.0 > bottom {
            break;
        }
        if !dark {
            ui.shadow(x, ry, w, 52.0, 14.0, 0.08);
        }
        ui.rect(x, ry, w, 52.0, 14.0, card);
        ui.plant(x + 26.0, ry + 27.0, 10.0, c.color);
        ui.text(
            x + 48.0,
            ry + 9.0,
            200.0,
            15.0,
            600,
            ink,
            lang.c(c),
            Align::Left,
        );
        let tw = w - 160.0;
        ui.rect(x + 48.0, ry + 34.0, tw, 5.0, 2.5, track);
        ui.rect(x + 48.0, ry + 34.0, tw * frac, 5.0, 2.5, hex(c.color));
        ui.text(
            x + w - 110.0,
            ry + 17.0,
            94.0,
            15.0,
            600,
            if dark { hex("#DDE6DF") } else { ink },
            lang.t(en, de),
            Align::Right,
        );
        ry += 60.0;
    }
    home_indicator(ui, p, dark);
}

/// Week calendar with tasks and weather.
fn screen_calendar(ui: &mut Ui<'_>, p: Platform, lang: Lang) {
    let (x, y0, w, bottom) = chrome(ui, p, lang, 2, 3, false);
    let y = header(
        ui,
        x,
        y0 + 8.0,
        w - 60.0,
        lang.t("Week 20", "Woche 20"),
        lang.t("May 2026", "Mai 2026"),
        false,
    );
    ui.dot(x + w - 22.0, y0 + 40.0, 21.0, hex("#E7F5E4"));
    ui.icon("bell", x + w - 22.0, y0 + 40.0, 18.0, hex(GREEN));
    // week strip
    let days = if lang == Lang::De {
        ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"]
    } else {
        ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"]
    };
    let dw = w / 7.0;
    for (i, d) in days.iter().enumerate() {
        let dx = x + i as f32 * dw;
        let sel = i == 1;
        if sel {
            ui.vgrad(
                dx + 4.0,
                y + 4.0,
                dw - 8.0,
                70.0,
                18.0,
                hex("#3A8A52"),
                hex(GREEN),
            );
        }
        let (c1, c2) = if sel {
            ("#FFFFFF", "#FFFFFF")
        } else {
            (MUTED, INK)
        };
        ui.text(dx, y + 14.0, dw, 12.0, 600, hex(c1), d, Align::Center);
        ui.text(
            dx,
            y + 34.0,
            dw,
            19.0,
            700,
            hex(c2),
            &format!("{}", 11 + i),
            Align::Center,
        );
        if [0, 1, 3, 5].contains(&i) {
            ui.dot(
                dx + dw / 2.0,
                y + 64.0,
                2.5,
                if sel { hex("#FFFFFF") } else { hex(GREEN_L) },
            );
        }
    }
    // weather
    let wy = y + 92.0;
    ui.grad(x, wy, w, 92.0, 20.0, hex("#5AA9E6"), hex("#8FD0F5"));
    ui.icon("sun", x + 44.0, wy + 46.0, 40.0, hex("#FFE27A"));
    ui.text(
        x + 80.0,
        wy + 18.0,
        100.0,
        30.0,
        700,
        hex("#FFFFFF"),
        "22°",
        Align::Left,
    );
    ui.text(
        x + 80.0,
        wy + 56.0,
        w - 100.0,
        13.0,
        600,
        hex("#FFFFFF"),
        lang.t(
            "Sunny · no frost until Sunday",
            "Sonnig · kein Frost bis Sonntag",
        ),
        Align::Left,
    );
    ui.text(
        x + w - 160.0,
        wy + 24.0,
        140.0,
        12.5,
        600,
        with_alpha(hex("#FFFFFF"), 0.9),
        lang.t("Good day\nto plant out", "Guter Tag\nzum Auspflanzen"),
        Align::Right,
    );
    // tasks
    let mut ty = wy + 118.0;
    ui.left(x, ty, w, 19.0, 700, INK, lang.t("Today", "Heute"));
    ui.text(
        x + w - 120.0,
        ty + 3.0,
        120.0,
        14.0,
        600,
        hex(GREEN),
        lang.t("3 of 5 done", "3 von 5 erledigt"),
        Align::Right,
    );
    ty += 34.0;
    let tasks = [
        (
            true,
            "drop",
            lang.t("Water tomatoes", "Tomaten gießen"),
            lang.t("Bed 1 · 07:30", "Beet 1 · 07:30"),
            "#4C9BD6",
        ),
        (
            true,
            "leaf",
            lang.t("Pinch out side shoots", "Tomaten ausgeizen"),
            lang.t("Bed 1 · 6 plants", "Beet 1 · 6 Pflanzen"),
            GREEN_L,
        ),
        (
            true,
            "grid",
            lang.t("Sow lettuce", "Salat säen"),
            lang.t("Bed 2 · row 3", "Beet 2 · Reihe 3"),
            "#8DC26F",
        ),
        (
            false,
            "sun",
            lang.t("Harden off peppers", "Paprika abhärten"),
            lang.t("Greenhouse · 2 h", "Gewächshaus · 2 Std."),
            AMBER,
        ),
        (
            false,
            "chart",
            lang.t("Log first radishes", "Erste Radieschen erfassen"),
            lang.t("Bed 3", "Beet 3"),
            "#B23A6B",
        ),
        (
            false,
            "drop",
            lang.t("Feed zucchini", "Zucchini düngen"),
            lang.t("Bed 4 · compost tea", "Beet 4 · Komposttee"),
            "#4C9BD6",
        ),
        (
            false,
            "leaf",
            lang.t("Thin out carrots", "Möhren vereinzeln"),
            lang.t("Bed 3 · row 1–2", "Beet 3 · Reihe 1–2"),
            "#EE8A2C",
        ),
        (
            false,
            "grid",
            lang.t("Plant out basil", "Basilikum auspflanzen"),
            lang.t("Bed 1 · 5 plants", "Beet 1 · 5 Pflanzen"),
            GREEN_L,
        ),
    ];
    for (done, icon, title, sub, c) in tasks {
        if ty + 64.0 > bottom {
            break;
        }
        ui.card(x, ty, w, 64.0, 16.0);
        ui.dot(x + 30.0, ty + 32.0, 17.0, with_alpha(hex(c), 0.16));
        ui.icon(icon, x + 30.0, ty + 32.0, 17.0, hex(c));
        ui.left(
            x + 58.0,
            ty + 13.0,
            w - 120.0,
            15.0,
            600,
            if done { MUTED } else { INK },
            title,
        );
        ui.left(x + 58.0, ty + 35.0, w - 120.0, 12.5, 400, MUTED, sub);
        if done {
            ui.dot(x + w - 28.0, ty + 32.0, 12.0, hex(GREEN));
            ui.icon("check", x + w - 28.0, ty + 32.0, 12.0, hex("#FFFFFF"));
        } else {
            ui.outline(
                x + w - 40.0,
                ty + 20.0,
                24.0,
                24.0,
                12.0,
                1.8,
                hex("#C5CCC3"),
            );
        }
        ty += 72.0;
    }
    home_indicator(ui, p, false);
}

// ----------------------------------------------------------- mac screens

/// macOS window frame (title bar with traffic lights, sidebar). Returns the
/// content x.
fn mac_window(ui: &mut Ui<'_>, lang: Lang, active: usize, title: &str) -> f32 {
    let (w, h) = ui.size();
    ui.fill("#FAFBF8");
    let sw = 230.0;
    ui.rect(0.0, 0.0, sw, h, 0.0, hex("#E9ECE5"));
    ui.rect(sw, 0.0, 1.0, h, 0.0, hex("#D6DAD2"));
    for (i, c) in ["#FF5F57", "#FEBC2E", "#28C840"].iter().enumerate() {
        ui.dot(20.0 + i as f32 * 20.0, 22.0, 6.0, hex(c));
    }
    // toolbar
    ui.rect(sw + 1.0, 0.0, w - sw, 52.0, 0.0, hex("#FAFBF8"));
    ui.rect(sw + 1.0, 52.0, w - sw, 1.0, 0.0, hex(LINE));
    ui.left(sw + 24.0, 15.0, 500.0, 17.0, 700, INK, title);
    ui.rect(w - 260.0, 12.0, 180.0, 28.0, 8.0, hex("#EEF0EB"));
    ui.icon("search", w - 244.0, 26.0, 13.0, hex(MUTED));
    ui.left(
        w - 228.0,
        18.0,
        150.0,
        12.5,
        400,
        MUTED,
        lang.t("Search", "Suchen"),
    );
    ui.rect(w - 68.0, 12.0, 48.0, 28.0, 8.0, hex(GREEN));
    ui.icon("plus", w - 44.0, 26.0, 14.0, hex("#FFFFFF"));
    // sidebar
    ui.text(
        16.0,
        54.0,
        180.0,
        11.0,
        600,
        hex("#8A938B"),
        lang.t("GARDEN", "GARTEN"),
        Align::Left,
    );
    let items = [
        ("home", lang.t("Overview", "Übersicht")),
        ("grid", lang.t("Garden plan", "Gartenplan")),
        ("calendar", lang.t("Season timeline", "Saisonplan")),
        ("chart", lang.t("Harvest", "Ernte")),
        ("leaf", lang.t("Plant library", "Pflanzenlexikon")),
    ];
    for (i, (icon, label)) in items.iter().enumerate() {
        let y = 74.0 + i as f32 * 32.0;
        let on = i == active;
        if on {
            ui.rect(8.0, y, sw - 16.0, 28.0, 7.0, hex("#D6E3D0"));
        }
        let c = if on { hex(GREEN) } else { hex("#3F4841") };
        ui.icon(icon, 26.0, y + 14.0, 14.0, c);
        ui.text(42.0, y + 6.5, 170.0, 13.0, 600, c, label, Align::Left);
    }
    ui.text(
        16.0,
        250.0,
        180.0,
        11.0,
        600,
        hex("#8A938B"),
        lang.t("BEDS", "BEETE"),
        Align::Left,
    );
    let beds = [
        ("Bed 1", TOMATO),
        ("Bed 2", LETTUCE),
        ("Bed 3", CARROT),
        ("Bed 4", BEAN),
        ("Greenhouse", PEPPER),
    ];
    for (i, (n, c)) in beds.iter().enumerate() {
        let y = 270.0 + i as f32 * 30.0;
        ui.dot(26.0, y + 13.0, 5.0, hex(c.color));
        let n = if lang == Lang::De {
            n.replace("Bed", "Beet")
                .replace("Greenhouse", "Gewächshaus")
        } else {
            (*n).to_owned()
        };
        ui.text(
            42.0,
            y + 5.5,
            170.0,
            13.0,
            400,
            hex("#3F4841"),
            &n,
            Align::Left,
        );
    }
    avatar(ui, 30.0, h - 34.0, 14.0, "MT", MINT, GREEN);
    ui.left(52.0, h - 42.0, 170.0, 13.0, 600, INK, "Maya Torres");
    sw + 1.0
}

/// Top view of a garden with beds, paths and an inspector.
fn mac_plan(ui: &mut Ui<'_>, lang: Lang) {
    let x0 = mac_window(ui, lang, 1, lang.t("Garden plan", "Gartenplan"));
    let (w, h) = ui.size();
    let iw = 300.0;
    let (gx, gy, gw, gh) = (x0 + 24.0, 76.0, w - x0 - iw - 48.0, h - 100.0);
    // lawn
    ui.vgrad(gx, gy, gw, gh, 18.0, hex("#CFE3BF"), hex("#BCD8A8"));
    // texture dots
    let mut seed = 7_u32;
    for _ in 0..260 {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let px = gx + 10.0 + (seed >> 8) as f32 % (gw - 20.0);
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        let py = gy + 10.0 + (seed >> 8) as f32 % (gh - 20.0);
        ui.dot(px, py, 1.6, with_alpha(hex("#8DBA74"), 0.5));
    }
    // paths
    ui.rect(gx + gw * 0.47, gy, 34.0, gh, 0.0, hex("#E6D8BE"));
    ui.rect(gx, gy + gh * 0.52, gw, 30.0, 0.0, hex("#E6D8BE"));
    let beds: [(f32, f32, f32, f32, &str, &str, [Crop; 3], bool); 5] = [
        (
            0.04,
            0.06,
            0.39,
            0.4,
            "Bed 1",
            "Beet 1",
            [TOMATO, BASIL, MARIGOLD],
            true,
        ),
        (
            0.56,
            0.06,
            0.4,
            0.18,
            "Bed 2",
            "Beet 2",
            [LETTUCE, BEET, ONION],
            false,
        ),
        (
            0.56,
            0.28,
            0.4,
            0.18,
            "Bed 3",
            "Beet 3",
            [CARROT, ONION, LETTUCE],
            false,
        ),
        (
            0.04,
            0.64,
            0.39,
            0.3,
            "Bed 4",
            "Beet 4",
            [BEAN, ZUCCHINI, MARIGOLD],
            false,
        ),
        (
            0.56,
            0.64,
            0.4,
            0.3,
            "Greenhouse",
            "Gewächshaus",
            [PEPPER, TOMATO, BASIL],
            false,
        ),
    ];
    for (bx, by, bw, bh, en, de, crops, sel) in beds {
        let (bx, by, bw, bh) = (gx + gw * bx, gy + gh * by, gw * bw, gh * bh);
        let glass = en == "Greenhouse";
        ui.shadow(bx, by, bw, bh, 10.0, 0.25);
        ui.rect(
            bx,
            by,
            bw,
            bh,
            10.0,
            hex(if glass { "#DDEEF0" } else { "#9C7650" }),
        );
        if glass {
            ui.rect(
                bx + 6.0,
                by + 6.0,
                bw - 12.0,
                bh - 12.0,
                6.0,
                hex("#EEF7F7"),
            );
        } else {
            ui.vgrad(
                bx + 6.0,
                by + 6.0,
                bw - 12.0,
                bh - 12.0,
                6.0,
                hex(SOIL),
                hex(SOIL_D),
            );
        }
        let cols = (bw / 46.0).floor().max(1.0) as usize;
        let rows = ((bh - 30.0) / 46.0).floor().max(1.0) as usize;
        let cw = (bw - 12.0) / cols as f32;
        let rh = (bh - 34.0) / rows as f32;
        for r in 0..rows {
            for c in 0..cols {
                let k = (r * 2 + c) % 3;
                ui.plant(
                    bx + 6.0 + cw * (c as f32 + 0.5),
                    by + 30.0 + rh * (r as f32 + 0.5),
                    12.0,
                    crops[k].color,
                );
            }
        }
        ui.rect(
            bx + 10.0,
            by + 10.0,
            14.0 + lang.t(en, de).chars().count() as f32 * 7.0,
            20.0,
            6.0,
            with_alpha(hex("#FFFFFF"), 0.9),
        );
        ui.left(bx + 17.0, by + 13.0, 160.0, 11.5, 600, INK, lang.t(en, de));
        if sel {
            ui.outline(
                bx - 4.0,
                by - 4.0,
                bw + 8.0,
                bh + 8.0,
                13.0,
                3.0,
                hex("#3D8BFD"),
            );
            for (hx, hy) in [
                (bx - 4.0, by - 4.0),
                (bx + bw + 4.0, by - 4.0),
                (bx - 4.0, by + bh + 4.0),
                (bx + bw + 4.0, by + bh + 4.0),
            ] {
                ui.dot(hx, hy, 5.0, hex("#FFFFFF"));
                ui.outline(hx - 5.0, hy - 5.0, 10.0, 10.0, 5.0, 2.0, hex("#3D8BFD"));
            }
        }
    }
    // compass + scale
    ui.rect(
        gx + 16.0,
        gy + gh - 36.0,
        120.0,
        22.0,
        6.0,
        with_alpha(hex("#FFFFFF"), 0.85),
    );
    ui.rect(gx + 26.0, gy + gh - 27.0, 50.0, 4.0, 1.0, hex(INK));
    ui.left(gx + 84.0, gy + gh - 32.0, 60.0, 11.0, 600, INK, "1 m");
    // inspector
    let ix = w - iw - 12.0;
    ui.card(ix, 76.0, iw - 12.0, h - 100.0, 14.0);
    let iw = iw - 12.0;
    ui.left(
        ix + 20.0,
        96.0,
        iw - 40.0,
        11.0,
        600,
        "#8A938B",
        lang.t("SELECTED BED", "AUSGEWÄHLTES BEET"),
    );
    ui.left(
        ix + 20.0,
        114.0,
        iw - 40.0,
        22.0,
        700,
        INK,
        lang.t("Bed 1", "Beet 1"),
    );
    ui.left(
        ix + 20.0,
        146.0,
        iw - 40.0,
        13.0,
        400,
        MUTED,
        lang.t(
            "Raised bed · 120 × 240 cm · full sun",
            "Hochbeet · 120 × 240 cm · sonnig",
        ),
    );
    let stats = [
        ("6", lang.t("Tomatoes", "Tomaten")),
        ("5", lang.t("Basil", "Basilikum")),
        ("3", lang.t("Marigolds", "Ringelbl.")),
    ];
    let sw = (iw - 40.0 - 16.0) / 3.0;
    for (i, (n, l)) in stats.iter().enumerate() {
        let sx = ix + 20.0 + i as f32 * (sw + 8.0);
        ui.rect(sx, 178.0, sw, 64.0, 10.0, hex("#F2F5EF"));
        ui.text(sx, 188.0, sw, 22.0, 700, hex(INK), n, Align::Center);
        ui.text(sx, 218.0, sw, 11.0, 600, hex(MUTED), l, Align::Center);
    }
    ui.left(
        ix + 20.0,
        266.0,
        iw - 40.0,
        15.0,
        700,
        INK,
        lang.t("Companion check", "Nachbarschafts-Check"),
    );
    let checks = [
        (
            true,
            lang.t("Basil protects tomatoes", "Basilikum schützt Tomaten"),
        ),
        (
            true,
            lang.t("Marigolds attract bees", "Ringelblumen locken Bienen"),
        ),
        (
            false,
            lang.t(
                "Keep fennel out of this bed",
                "Fenchel nicht in dieses Beet",
            ),
        ),
    ];
    for (i, (ok, t)) in checks.iter().enumerate() {
        let y = 296.0 + i as f32 * 40.0;
        let (bg, fg, ic) = if *ok {
            ("#E7F5E4", GREEN, "check")
        } else {
            ("#FDE7E3", RED, "cross")
        };
        ui.dot(ix + 32.0, y + 14.0, 11.0, hex(bg));
        ui.icon(ic, ix + 32.0, y + 14.0, 11.0, hex(fg));
        ui.left(ix + 52.0, y + 6.0, iw - 70.0, 13.0, 400, INK, t);
    }
    ui.left(
        ix + 20.0,
        430.0,
        iw - 40.0,
        15.0,
        700,
        INK,
        lang.t("Next tasks", "Nächste Aufgaben"),
    );
    let tasks = [
        (
            lang.t("Water", "Gießen"),
            lang.t("Today", "Heute"),
            "#4C9BD6",
        ),
        (
            lang.t("Pinch out", "Ausgeizen"),
            lang.t("Tomorrow", "Morgen"),
            GREEN_L,
        ),
        (lang.t("Feed", "Düngen"), lang.t("Friday", "Freitag"), AMBER),
    ];
    for (i, (t, d, c)) in tasks.iter().enumerate() {
        let y = 460.0 + i as f32 * 44.0;
        ui.rect(ix + 20.0, y, iw - 40.0, 36.0, 9.0, hex("#F6F8F4"));
        ui.rect(ix + 20.0, y, 4.0, 36.0, 2.0, hex(c));
        ui.left(ix + 34.0, y + 10.0, 150.0, 13.0, 600, INK, t);
        ui.text(
            ix + iw - 140.0,
            y + 10.5,
            110.0,
            12.0,
            400,
            hex(MUTED),
            d,
            Align::Right,
        );
    }
    ui.rect(ix + 20.0, h - 86.0, iw - 40.0, 40.0, 10.0, hex(GREEN));
    ui.text(
        ix + 20.0,
        h - 74.0,
        iw - 40.0,
        14.0,
        600,
        hex("#FFFFFF"),
        lang.t("Open bed planner", "Beetplaner öffnen"),
        Align::Center,
    );
}

/// Season timeline (Gantt chart of crops).
fn mac_timeline(ui: &mut Ui<'_>, lang: Lang) {
    let x0 = mac_window(
        ui,
        lang,
        2,
        lang.t("Season timeline 2026", "Saisonplan 2026"),
    );
    let (w, h) = ui.size();
    let (x, y) = (x0 + 28.0, 76.0);
    let cw = w - x - 28.0;
    // stat cards
    let stats = [
        (
            "24",
            lang.t("crops planned", "Kulturen geplant"),
            "#E7F5E4",
            GREEN,
        ),
        (
            "8",
            lang.t("sowings this month", "Aussaaten im Monat"),
            "#FFF4DC",
            "#B5810F",
        ),
        (
            "3",
            lang.t("frost warnings", "Frostwarnungen"),
            "#E3F0FB",
            "#2F78B5",
        ),
        (
            "38 kg",
            lang.t("harvest so far", "bisherige Ernte"),
            "#FCE3DA",
            "#B5432C",
        ),
    ];
    let sw = (cw - 3.0 * 14.0) / 4.0;
    for (i, (n, l, bg, fg)) in stats.iter().enumerate() {
        let sx = x + i as f32 * (sw + 14.0);
        ui.rect(sx, y, sw, 78.0, 14.0, hex(bg));
        ui.left(sx + 18.0, y + 14.0, sw, 26.0, 700, fg, n);
        ui.left(sx + 18.0, y + 50.0, sw, 12.5, 600, fg, l);
    }
    // chart
    let ty = y + 100.0;
    let th = h - ty - 20.0;
    ui.card(x, ty, cw, th, 14.0);
    let label_w = 150.0;
    let months = if lang == Lang::De {
        [
            "Feb", "Mär", "Apr", "Mai", "Jun", "Jul", "Aug", "Sep", "Okt",
        ]
    } else {
        [
            "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct",
        ]
    };
    let gx = x + label_w;
    let gw = cw - label_w - 20.0;
    let mw = gw / months.len() as f32;
    for (i, m) in months.iter().enumerate() {
        let mx = gx + i as f32 * mw;
        if i % 2 == 0 {
            ui.rect(mx, ty + 44.0, mw, th - 56.0, 0.0, hex("#F7F9F5"));
        }
        ui.text(mx, ty + 18.0, mw, 12.0, 600, hex(MUTED), m, Align::Center);
    }
    let crops: [(Crop, [(f32, f32, &str); 3]); 9] = [
        (
            TOMATO,
            [
                (0.2, 1.4, "#F6C8BE"),
                (2.6, 2.0, "#7BC48A"),
                (4.6, 3.0, "#E5533D"),
            ],
        ),
        (
            PEPPER,
            [
                (0.0, 1.8, "#F6C8BE"),
                (2.8, 2.0, "#7BC48A"),
                (5.0, 2.6, "#D9452F"),
            ],
        ),
        (
            BASIL,
            [
                (1.0, 1.2, "#F6C8BE"),
                (2.8, 1.5, "#7BC48A"),
                (4.0, 3.5, "#5FAE4E"),
            ],
        ),
        (
            LETTUCE,
            [
                (1.2, 0.8, "#F6C8BE"),
                (2.0, 1.2, "#7BC48A"),
                (3.2, 2.5, "#8DC26F"),
            ],
        ),
        (
            CARROT,
            [
                (1.6, 0.0, "#F6C8BE"),
                (1.6, 3.4, "#7BC48A"),
                (5.0, 3.0, "#EE8A2C"),
            ],
        ),
        (
            BEAN,
            [
                (2.8, 0.0, "#F6C8BE"),
                (2.8, 2.0, "#7BC48A"),
                (4.8, 2.6, "#3E8E5A"),
            ],
        ),
        (
            ZUCCHINI,
            [
                (1.6, 1.0, "#F6C8BE"),
                (3.0, 1.6, "#7BC48A"),
                (4.6, 2.8, "#4E9B45"),
            ],
        ),
        (
            POTATO,
            [
                (1.8, 0.0, "#F6C8BE"),
                (1.8, 3.4, "#7BC48A"),
                (5.2, 2.2, "#B8925E"),
            ],
        ),
        (
            ONION,
            [
                (1.4, 0.0, "#F6C8BE"),
                (1.4, 3.6, "#7BC48A"),
                (5.0, 1.6, "#C9A26B"),
            ],
        ),
    ];
    let rh = ((th - 120.0) / crops.len() as f32).min(48.0);
    for (i, (c, bars)) in crops.iter().enumerate() {
        let ry = ty + 52.0 + i as f32 * rh;
        ui.plant(x + 30.0, ry + rh / 2.0, 10.0, c.color);
        ui.left(
            x + 50.0,
            ry + rh / 2.0 - 8.0,
            label_w - 50.0,
            13.5,
            600,
            INK,
            lang.c(*c),
        );
        for (s, len, col) in bars {
            if *len <= 0.0 {
                continue;
            }
            ui.rect(
                gx + s * mw + 2.0,
                ry + rh * 0.22,
                len * mw - 4.0,
                rh * 0.56,
                rh * 0.28,
                hex(col),
            );
        }
    }
    // today line
    let today = gx + 3.4 * mw;
    ui.rect(today - 1.0, ty + 40.0, 2.0, th - 92.0, 1.0, hex("#3D8BFD"));
    ui.rect(today - 28.0, ty + 36.0, 56.0, 18.0, 9.0, hex("#3D8BFD"));
    ui.text(
        today - 28.0,
        ty + 39.0,
        56.0,
        10.5,
        600,
        hex("#FFFFFF"),
        lang.t("Today", "Heute"),
        Align::Center,
    );
    // legend
    let ly = ty + th - 34.0;
    let legend = [
        ("#F6C8BE", lang.t("Sow indoors", "Vorziehen")),
        ("#7BC48A", lang.t("Grow outside", "Im Beet")),
        ("#E5533D", lang.t("Harvest", "Ernte")),
    ];
    let mut lx = gx;
    for (c, l) in legend {
        ui.rect(lx, ly + 2.0, 22.0, 12.0, 6.0, hex(c));
        ui.left(lx + 30.0, ly, 160.0, 12.5, 400, MUTED, l);
        lx += 170.0;
    }
}

/// Harvest dashboard.
fn mac_harvest(ui: &mut Ui<'_>, lang: Lang) {
    let x0 = mac_window(ui, lang, 3, lang.t("Harvest 2026", "Ernte 2026"));
    let (w, h) = ui.size();
    let (x, y) = (x0 + 28.0, 76.0);
    let cw = w - x - 28.0;
    let lw = cw * 0.62;
    // line chart card
    ui.card(x, y, lw, 380.0, 14.0);
    ui.left(
        x + 22.0,
        y + 20.0,
        300.0,
        12.0,
        600,
        "#8A938B",
        lang.t("TOTAL HARVEST", "GESAMTERNTE"),
    );
    ui.left(
        x + 22.0,
        y + 40.0,
        300.0,
        34.0,
        700,
        INK,
        lang.t("38.4 kg", "38,4 kg"),
    );
    ui.left(
        x + 22.0,
        y + 84.0,
        300.0,
        13.0,
        600,
        "#2E9E55",
        lang.t("+12 % vs. 2025", "+12 % ggü. 2025"),
    );
    let (cx, cy, cwid, chh) = (x + 22.0, y + 130.0, lw - 44.0, 210.0);
    for i in 0..5 {
        ui.rect(
            cx,
            cy + chh * i as f32 / 4.0,
            cwid,
            1.0,
            0.0,
            hex("#EEF1EC"),
        );
    }
    let a = [0.02, 0.05, 0.1, 0.18, 0.34, 0.55, 0.74, 0.88, 0.97, 1.0];
    let b = [0.01, 0.04, 0.08, 0.15, 0.27, 0.44, 0.6, 0.72, 0.8, 0.84];
    let pts = |v: &[f32]| -> Vec<(f32, f32)> {
        v.iter()
            .enumerate()
            .map(|(i, f)| {
                (
                    cx + cwid * i as f32 / (v.len() - 1) as f32,
                    cy + chh * (1.0 - f),
                )
            })
            .collect()
    };
    let pa = pts(&a);
    let mut area = pa.clone();
    area.push((cx + cwid, cy + chh));
    area.push((cx, cy + chh));
    ui.polygon(&area, with_alpha(hex(GREEN_L), 0.18));
    ui.polyline(&pts(&b), 2.5, hex("#C5CCC3"), false);
    ui.polyline(&pa, 3.5, hex(GREEN), false);
    let (lx, ly) = pa[6];
    ui.dot(lx, ly, 7.0, hex("#FFFFFF"));
    ui.dot(lx, ly, 5.0, hex(GREEN));
    ui.rect(lx - 50.0, ly - 44.0, 100.0, 30.0, 8.0, hex(INK));
    ui.text(
        lx - 50.0,
        ly - 37.0,
        100.0,
        12.5,
        600,
        hex("#FFFFFF"),
        lang.t("Aug · 28.4 kg", "Aug · 28,4 kg"),
        Align::Center,
    );
    // donut card
    let dx = x + lw + 18.0;
    let dw = cw - lw - 18.0;
    ui.card(dx, y, dw, 380.0, 14.0);
    ui.left(
        dx + 22.0,
        y + 20.0,
        dw,
        12.0,
        600,
        "#8A938B",
        lang.t("BY CROP", "NACH KULTUR"),
    );
    let shares = [
        (TOMATO, 0.32),
        (ZUCCHINI, 0.26),
        (POTATO, 0.2),
        (CARROT, 0.11),
        (BEAN, 0.07),
        (BASIL, 0.04),
    ];
    let (ox, oy, r) = (dx + dw / 2.0, y + 160.0, 80.0);
    let mut start = 0.0_f32;
    for (c, f) in shares {
        let n = 40;
        let pts: Vec<(f32, f32)> = (0..=n)
            .map(|i| {
                let a = (start + f * i as f32 / n as f32) * std::f32::consts::TAU;
                (ox + r * a.sin(), oy - r * a.cos())
            })
            .collect();
        ui.arc(&pts, 30.0, hex(c.color));
        start += f;
    }
    ui.text(
        ox - 60.0,
        oy - 14.0,
        120.0,
        22.0,
        700,
        hex(INK),
        "6",
        Align::Center,
    );
    ui.text(
        ox - 60.0,
        oy + 12.0,
        120.0,
        11.0,
        600,
        hex(MUTED),
        lang.t("crops", "Kulturen"),
        Align::Center,
    );
    for (i, (c, f)) in shares.iter().take(4).enumerate() {
        let ry = y + 272.0 + i as f32 * 24.0;
        ui.dot(dx + 30.0, ry + 8.0, 5.0, hex(c.color));
        ui.left(dx + 44.0, ry, dw - 100.0, 13.0, 400, INK, lang.c(*c));
        ui.text(
            dx + dw - 80.0,
            ry,
            58.0,
            13.0,
            600,
            hex(MUTED),
            &format!("{} %", (f * 100.0).round()),
            Align::Right,
        );
    }
    // table
    let ty = y + 400.0;
    ui.card(x, ty, cw, h - ty - 20.0, 14.0);
    let cols = [
        lang.t("Date", "Datum"),
        lang.t("Crop", "Kultur"),
        lang.t("Bed", "Beet"),
        lang.t("Amount", "Menge"),
        lang.t("Quality", "Qualität"),
    ];
    let colx = [0.0, 0.2, 0.45, 0.65, 0.82];
    for (i, c) in cols.iter().enumerate() {
        ui.left(
            x + 24.0 + (cw - 48.0) * colx[i],
            ty + 18.0,
            200.0,
            12.0,
            600,
            "#8A938B",
            c,
        );
    }
    let rows = [
        ("12 Aug", "12. Aug", TOMATO, "1", "2.4 kg", "2,4 kg", 5),
        ("11 Aug", "11. Aug", ZUCCHINI, "4", "3.1 kg", "3,1 kg", 4),
        ("10 Aug", "10. Aug", BEAN, "4", "0.8 kg", "0,8 kg", 5),
        ("9 Aug", "9. Aug", CARROT, "3", "1.2 kg", "1,2 kg", 4),
        ("8 Aug", "8. Aug", BASIL, "1", "0.2 kg", "0,2 kg", 5),
        ("7 Aug", "7. Aug", POTATO, "6", "4.6 kg", "4,6 kg", 4),
        ("6 Aug", "6. Aug", TOMATO, "1", "1.9 kg", "1,9 kg", 5),
        ("5 Aug", "5. Aug", PEPPER, "5", "0.6 kg", "0,6 kg", 3),
        ("4 Aug", "4. Aug", LETTUCE, "2", "0.9 kg", "0,9 kg", 4),
    ];
    for (i, (d_en, d_de, c, bed, a_en, a_de, q)) in rows.iter().enumerate() {
        let ry = ty + 46.0 + i as f32 * 40.0;
        if ry + 30.0 > h - 24.0 {
            break;
        }
        ui.rect(x + 16.0, ry, cw - 32.0, 1.0, 0.0, hex("#EEF1EC"));
        let cx = |k: usize| x + 24.0 + (cw - 48.0) * colx[k];
        ui.left(cx(0), ry + 12.0, 150.0, 13.0, 400, INK, lang.t(d_en, d_de));
        ui.plant(cx(1) + 8.0, ry + 20.0, 7.0, c.color);
        ui.left(cx(1) + 24.0, ry + 12.0, 150.0, 13.0, 600, INK, lang.c(*c));
        ui.left(
            cx(2),
            ry + 12.0,
            150.0,
            13.0,
            400,
            INK,
            &format!("{} {bed}", lang.t("Bed", "Beet")),
        );
        ui.left(cx(3), ry + 12.0, 150.0, 13.0, 600, INK, lang.t(a_en, a_de));
        for k in 0..5 {
            ui.dot(
                cx(4) + 6.0 + k as f32 * 16.0,
                ry + 20.0,
                5.0,
                if k < *q { hex(AMBER) } else { hex("#E3E6E0") },
            );
        }
    }
}

// ----------------------------------------------------------- web screens

fn web_nav(ui: &mut Ui<'_>, lang: Lang, active: usize) {
    let (w, _) = ui.size();
    ui.rect(0.0, 0.0, w, 64.0, 0.0, hex("#FFFFFF"));
    ui.rect(0.0, 64.0, w, 1.0, 0.0, hex(LINE));
    ui.dot(44.0, 32.0, 15.0, hex(GREEN));
    ui.leaf(44.0, 32.0, 8.5, hex("#FFFFFF"));
    ui.left(68.0, 20.0, 200.0, 20.0, 700, INK, "Fernwood");
    let items = [
        lang.t("Community garden", "Gemeinschaftsgarten"),
        lang.t("Plots", "Parzellen"),
        lang.t("Seed swap", "Saatgut-Tausch"),
        lang.t("Events", "Termine"),
    ];
    let mut nx = 250.0;
    for (i, it) in items.iter().enumerate() {
        let tw = it.chars().count() as f32 * 8.2 + 28.0;
        if i == active {
            ui.rect(nx, 16.0, tw, 32.0, 16.0, hex("#E7F5E4"));
        }
        ui.text(
            nx,
            23.5,
            tw,
            14.5,
            600,
            hex(if i == active { GREEN } else { "#4A544C" }),
            it,
            Align::Center,
        );
        nx += tw + 8.0;
    }
    ui.icon("bell", w - 100.0, 32.0, 18.0, hex("#4A544C"));
    avatar(ui, w - 50.0, 32.0, 17.0, "MT", MINT, GREEN);
}

/// Community garden plot map.
fn web_plots(ui: &mut Ui<'_>, lang: Lang) {
    let (w, h) = ui.size();
    ui.fill("#F7F8F4");
    web_nav(ui, lang, 1);
    let (x, y) = (40.0, 92.0);
    ui.left(
        x,
        y,
        700.0,
        30.0,
        700,
        INK,
        lang.t("Riverside Community Garden", "Gemeinschaftsgarten am Fluss"),
    );
    ui.left(
        x,
        y + 42.0,
        700.0,
        15.0,
        400,
        MUTED,
        lang.t(
            "48 plots · 61 gardeners · next work day Saturday, 10:00",
            "48 Parzellen · 61 Gärtner · nächster Gartentag Samstag, 10:00",
        ),
    );
    // filter chips
    let chips = [
        lang.t("All plots", "Alle"),
        lang.t("Free", "Frei"),
        lang.t("Needs water", "Braucht Wasser"),
        lang.t("My plots", "Meine"),
    ];
    let mut cx = w - 40.0;
    for (i, c) in chips.iter().enumerate().rev() {
        let cw = c.chars().count() as f32 * 7.6 + 30.0;
        cx -= cw;
        ui.rect(
            cx,
            y + 6.0,
            cw,
            32.0,
            16.0,
            if i == 0 { hex(INK) } else { hex("#FFFFFF") },
        );
        if i != 0 {
            ui.outline(cx, y + 6.0, cw, 32.0, 16.0, 1.0, hex("#DADFD6"));
        }
        ui.text(
            cx,
            y + 14.0,
            cw,
            13.0,
            600,
            hex(if i == 0 { "#FFFFFF" } else { "#4A544C" }),
            c,
            Align::Center,
        );
        cx -= 8.0;
    }
    // map
    let (mx, my) = (x, y + 84.0);
    let pw = 380.0;
    let (mw, mh) = (w - 2.0 * x - pw - 24.0, h - my - 32.0);
    ui.card(mx, my, mw, mh, 18.0);
    ui.vgrad(
        mx + 10.0,
        my + 10.0,
        mw - 20.0,
        mh - 20.0,
        12.0,
        hex("#D7E8C8"),
        hex("#C6DFB3"),
    );
    let cols = 8;
    let rows = 5;
    let gw = (mw - 60.0) / cols as f32;
    let gh = (mh - 96.0) / rows as f32;
    let names = [
        "AK", "JB", "LM", "RS", "TO", "EW", "PN", "CH", "DM", "SV", "IK", "FR",
    ];
    for r in 0..rows {
        for c in 0..cols {
            let i = r * cols + c;
            let px = mx + 30.0 + c as f32 * gw;
            let py = my + 30.0 + r as f32 * gh;
            let state = (i * 7 + 3) % 10;
            let (fill, label_c) = match state {
                0 | 5 => ("#FFFFFF", MUTED),
                1 | 7 => ("#F6D7A7", "#7A5410"),
                _ => ("#8CBF7A", "#FFFFFF"),
            };
            let sel = i == 13;
            ui.rect(px + 3.0, py + 3.0, gw - 6.0, gh - 6.0, 8.0, hex(fill));
            if state == 0 || state == 5 {
                ui.outline(
                    px + 3.0,
                    py + 3.0,
                    gw - 6.0,
                    gh - 6.0,
                    8.0,
                    1.5,
                    hex("#B9CBAE"),
                );
            }
            if sel {
                ui.outline(px, py, gw, gh, 10.0, 3.0, hex("#3D8BFD"));
            }
            ui.text(
                px + 8.0,
                py + 9.0,
                40.0,
                11.0,
                600,
                hex(label_c),
                &format!("{}", i + 1),
                Align::Left,
            );
            if !(state == 0 || state == 5) {
                ui.dot(px + gw - 18.0, py + gh - 18.0, 9.0, hex("#FFFFFF"));
                ui.text(
                    px + gw - 27.0,
                    py + gh - 23.5,
                    18.0,
                    8.0,
                    600,
                    hex(GREEN),
                    names[i % names.len()],
                    Align::Center,
                );
            }
        }
    }
    // legend
    let legend = [
        ("#8CBF7A", lang.t("Planted", "Bepflanzt")),
        ("#F6D7A7", lang.t("Needs water", "Braucht Wasser")),
        ("#FFFFFF", lang.t("Free", "Frei")),
    ];
    let mut lx = mx + 24.0;
    let ly = my + mh - 46.0;
    for (c, l) in legend {
        ui.rect(lx, ly, 14.0, 14.0, 4.0, hex(c));
        ui.outline(lx, ly, 14.0, 14.0, 4.0, 1.0, hex("#B9CBAE"));
        ui.left(lx + 20.0, ly - 1.0, 140.0, 12.5, 400, INK, l);
        lx += 130.0;
    }
    // detail panel
    let px = w - x - pw;
    ui.card(px, my, pw, mh, 18.0);
    ui.left(
        px + 24.0,
        my + 22.0,
        pw,
        12.0,
        600,
        "#8A938B",
        lang.t("PLOT 14", "PARZELLE 14"),
    );
    ui.left(
        px + 24.0,
        my + 42.0,
        pw - 48.0,
        24.0,
        700,
        INK,
        lang.t("The Sunflower Corner", "Die Sonnenblumen-Ecke"),
    );
    avatar(ui, px + 40.0, my + 102.0, 16.0, "JB", "#FCE3DA", "#B5432C");
    avatar(ui, px + 64.0, my + 102.0, 16.0, "LM", "#E3F0FB", "#2F78B5");
    ui.left(
        px + 92.0,
        my + 94.0,
        pw - 110.0,
        13.5,
        400,
        MUTED,
        lang.t("Jonas & Lea · since 2023", "Jonas & Lea · seit 2023"),
    );
    // photo-like banner
    ui.grad(
        px + 24.0,
        my + 134.0,
        pw - 48.0,
        120.0,
        12.0,
        hex("#F8D66D"),
        hex("#8CBF7A"),
    );
    for i in 0..6 {
        let fx = px + 60.0 + i as f32 * 52.0;
        ui.rect(fx - 2.0, my + 190.0, 4.0, 64.0, 2.0, hex("#3E8E5A"));
        ui.dot(fx, my + 182.0 - (i % 2) as f32 * 14.0, 16.0, hex("#F2B33D"));
        ui.dot(fx, my + 182.0 - (i % 2) as f32 * 14.0, 6.0, hex("#7A5410"));
    }
    let facts = [
        (lang.t("Size", "Größe"), "3 × 6 m"),
        (lang.t("Sun", "Sonne"), lang.t("Full sun", "Volle Sonne")),
        (
            lang.t("Last watered", "Zuletzt gegossen"),
            lang.t("Yesterday", "Gestern"),
        ),
        (
            lang.t("Growing", "Wächst"),
            lang.t("Sunflowers, beans", "Sonnenblumen, Bohnen"),
        ),
    ];
    for (i, (k, v)) in facts.iter().enumerate() {
        let fy = my + 276.0 + i as f32 * 40.0;
        ui.rect(px + 24.0, fy, pw - 48.0, 1.0, 0.0, hex("#EEF1EC"));
        ui.left(px + 24.0, fy + 12.0, 160.0, 13.5, 400, MUTED, k);
        ui.text(
            px + 150.0,
            fy + 12.0,
            pw - 174.0,
            13.5,
            600,
            hex(INK),
            v,
            Align::Right,
        );
    }
    let ay = my + 448.0;
    ui.left(
        px + 24.0,
        ay,
        pw,
        12.0,
        600,
        "#8A938B",
        lang.t("RECENT ACTIVITY", "LETZTE AKTIVITÄT"),
    );
    let acts = [
        (
            "drop",
            "#4C9BD6",
            lang.t("Lea watered the plot", "Lea hat gegossen"),
            lang.t("Yesterday, 19:20", "Gestern, 19:20"),
        ),
        (
            "leaf",
            GREEN_L,
            lang.t(
                "Jonas planted runner beans",
                "Jonas hat Feuerbohnen gepflanzt",
            ),
            lang.t("Sunday", "Sonntag"),
        ),
        (
            "sun",
            AMBER,
            lang.t("First sunflower opened", "Erste Sonnenblume blüht"),
            lang.t("Last week", "Letzte Woche"),
        ),
    ];
    for (i, (ic, c, t, d)) in acts.iter().enumerate() {
        let ay = ay + 26.0 + i as f32 * 50.0;
        if ay + 44.0 > my + mh - 84.0 {
            break;
        }
        ui.dot(px + 40.0, ay + 18.0, 15.0, with_alpha(hex(c), 0.16));
        ui.icon(ic, px + 40.0, ay + 18.0, 15.0, hex(c));
        ui.left(px + 66.0, ay + 3.0, pw - 90.0, 13.5, 600, INK, t);
        ui.left(px + 66.0, ay + 22.0, pw - 90.0, 12.0, 400, MUTED, d);
    }
    let by = my + mh - 70.0;
    ui.rect(px + 24.0, by, (pw - 56.0) / 2.0, 44.0, 12.0, hex(GREEN));
    ui.text(
        px + 24.0,
        by + 13.0,
        (pw - 56.0) / 2.0,
        14.0,
        600,
        hex("#FFFFFF"),
        lang.t("Message", "Nachricht"),
        Align::Center,
    );
    ui.outline(
        px + 32.0 + (pw - 56.0) / 2.0,
        by,
        (pw - 56.0) / 2.0,
        44.0,
        12.0,
        1.5,
        hex("#C5CCC3"),
    );
    ui.text(
        px + 32.0 + (pw - 56.0) / 2.0,
        by + 13.0,
        (pw - 56.0) / 2.0,
        14.0,
        600,
        hex(INK),
        lang.t("Water for them", "Gießen helfen"),
        Align::Center,
    );
}

/// Seed swap listing.
fn web_seeds(ui: &mut Ui<'_>, lang: Lang) {
    let (w, h) = ui.size();
    ui.fill("#F7F8F4");
    web_nav(ui, lang, 2);
    let (x, y) = (40.0, 92.0);
    // hero
    let hw = w - 2.0 * x;
    ui.grad(x, y, hw, 184.0, 20.0, hex("#2A7243"), hex("#7CC287"));
    ui.left(
        x + 36.0,
        y + 34.0,
        700.0,
        30.0,
        700,
        "#FFFFFF",
        lang.t("Seed swap", "Saatgut-Tausch"),
    );
    ui.text(x + 36.0, y + 80.0, 560.0, 15.0, 400, with_alpha(hex("#FFFFFF"), 0.9), lang.t("Share what you saved, find what you need. 126 varieties from 34 gardeners near you.", "Teile, was du gesammelt hast, und finde, was dir fehlt. 126 Sorten von 34 Gärtnern in deiner Nähe."), Align::Left);
    ui.rect(x + 36.0, y + 128.0, 170.0, 36.0, 18.0, hex("#FFFFFF"));
    ui.text(
        x + 36.0,
        y + 138.0,
        170.0,
        13.5,
        600,
        hex(GREEN),
        lang.t("Offer seeds", "Saatgut anbieten"),
        Align::Center,
    );
    for (i, c) in [TOMATO, CARROT, BEET, BEAN, MARIGOLD].iter().enumerate() {
        let cx = x + hw - 330.0 + i as f32 * 64.0;
        ui.dot(
            cx,
            y + 86.0 + (i % 2) as f32 * 22.0,
            34.0,
            with_alpha(hex("#FFFFFF"), 0.18),
        );
        ui.plant(cx, y + 90.0 + (i % 2) as f32 * 22.0, 22.0, c.color);
    }
    // cards
    let cards = [
        (
            TOMATO,
            lang.t("'Black Krim' tomato", "Tomate 'Schwarze Krim'"),
            lang.t("Heirloom · 20 seeds", "Alte Sorte · 20 Samen"),
            "Jonas B.",
            "#FCE3DA",
        ),
        (
            BEAN,
            lang.t("Runner bean 'Lady Di'", "Feuerbohne 'Lady Di'"),
            lang.t("Climbing · 15 seeds", "Kletternd · 15 Samen"),
            "Lea M.",
            "#E7F5E4",
        ),
        (
            CARROT,
            lang.t("Carrot 'Purple Haze'", "Möhre 'Purple Haze'"),
            lang.t("Early · 200 seeds", "Früh · 200 Samen"),
            "Ravi S.",
            "#FFF1DE",
        ),
        (
            BEET,
            lang.t("Beetroot 'Chioggia'", "Rote Bete 'Chioggia'"),
            lang.t("Striped · 50 seeds", "Geringelt · 50 Samen"),
            "Tom O.",
            "#F8E1EC",
        ),
        (
            MARIGOLD,
            lang.t("French marigold", "Studentenblume"),
            lang.t("Companion · 40 seeds", "Begleitpflanze · 40 Samen"),
            "Emma W.",
            "#FFF4DC",
        ),
        (
            BASIL,
            lang.t("Genovese basil", "Basilikum 'Genovese'"),
            lang.t("Herb · 100 seeds", "Kraut · 100 Samen"),
            "Paul N.",
            "#E9F3E2",
        ),
        (
            PEPPER,
            lang.t("Sweet pepper 'Corno'", "Paprika 'Corno'"),
            lang.t("Long · 25 seeds", "Lang · 25 Samen"),
            "Clara H.",
            "#FBE0DC",
        ),
        (
            LETTUCE,
            lang.t("Lettuce 'Merveille'", "Salat 'Merveille'"),
            lang.t("Butterhead · 80 seeds", "Kopfsalat · 80 Samen"),
            "Dana M.",
            "#EEF6E6",
        ),
    ];
    let cols = 4;
    let gap = 20.0;
    let cw = (hw - gap * (cols as f32 - 1.0)) / cols as f32;
    let cy0 = y + 208.0;
    let ch = ((h - cy0 - 24.0 - gap) / 2.0).min(320.0);
    for (i, (c, name, meta, who, tint)) in cards.iter().enumerate() {
        let cx = x + (i % cols) as f32 * (cw + gap);
        let cy = cy0 + (i / cols) as f32 * (ch + gap);
        if cy + ch > h - 10.0 {
            break;
        }
        ui.card(cx, cy, cw, ch, 16.0);
        let ih = ch * 0.48;
        ui.rect(cx + 8.0, cy + 8.0, cw - 16.0, ih, 11.0, hex(tint));
        ui.plant(cx + cw / 2.0, cy + 8.0 + ih / 2.0 + 6.0, ih * 0.27, c.color);
        ui.left(cx + 18.0, cy + ih + 24.0, cw - 36.0, 15.5, 600, INK, name);
        ui.left(cx + 18.0, cy + ih + 48.0, cw - 36.0, 13.0, 400, MUTED, meta);
        ui.left(cx + 18.0, cy + ch - 34.0, cw - 120.0, 12.5, 600, MUTED, who);
        ui.rect(
            cx + cw - 104.0,
            cy + ch - 42.0,
            88.0,
            30.0,
            15.0,
            hex("#E7F5E4"),
        );
        ui.text(
            cx + cw - 104.0,
            cy + ch - 34.5,
            88.0,
            12.5,
            600,
            hex(GREEN),
            lang.t("Request", "Anfragen"),
            Align::Center,
        );
    }
}

// ------------------------------------------------------------------ main

type ScreenFn = fn(&mut Ui<'_>, Platform, Lang);
type DeskFn = fn(&mut Ui<'_>, Lang);

fn main() -> anyhow::Result<()> {
    let out = PathBuf::from(
        std::env::args()
            .nth(1)
            .unwrap_or_else(|| "tests/fixtures/example/screenshots".into()),
    );
    let mut lay = TextLayouter::new(FontLibrary::bundled());
    let mut total = 0usize;
    let mut write = |rel: &str, bytes: Vec<u8>| -> anyhow::Result<()> {
        let path = rel
            .split('/')
            .fold(out.clone(), |p: PathBuf, part| p.join(part));
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        total += bytes.len();
        println!("{:>8} KB  {}", bytes.len() / 1024, display(&path));
        std::fs::write(&path, bytes)?;
        Ok(())
    };

    let screens: [(&str, ScreenFn); 5] = [
        ("home", screen_home),
        ("planner", screen_planner),
        ("companions", screen_companions),
        ("harvest", screen_harvest),
        ("calendar", screen_calendar),
    ];
    // (folder, platform, px size, px per pt, languages per screen)
    for (name, f) in screens {
        let mut ui = Ui::new(&mut lay, 1206, 2622, 3.0);
        f(&mut ui, Platform::Ios, Lang::En);
        write(&format!("en/ios-phone/{name}.png"), ui.encode())?;
        // German phone screens, except the calendar (falls back to en).
        if name != "calendar" {
            let mut ui = Ui::new(&mut lay, 1206, 2622, 3.0);
            f(&mut ui, Platform::Ios, Lang::De);
            write(&format!("de/ios-phone/{name}.png"), ui.encode())?;
        }
        // French phone screens only for the first three pages (harvest
        // and calendar fall back to en).
        if matches!(name, "home" | "planner" | "companions") {
            let mut ui = Ui::new(&mut lay, 1206, 2622, 3.0);
            f(&mut ui, Platform::Ios, Lang::Fr);
            write(&format!("fr/ios-phone/{name}.png"), ui.encode())?;
        }
        let mut ui = Ui::new(&mut lay, 1080, 2400, 2.7);
        f(&mut ui, Platform::Android, Lang::En);
        write(&format!("en/android-phone/{name}.png"), ui.encode())?;
        let mut ui = Ui::new(&mut lay, 1548, 2064, 2.0);
        f(&mut ui, Platform::Tablet, Lang::En);
        write(&format!("en/ios-tablet/{name}.png"), ui.encode())?;
    }

    let macs: [(&str, DeskFn, u32, u32, f32); 3] = [
        ("garden-plan", mac_plan, 2880, 1800, 2.0),
        ("timeline", mac_timeline, 2560, 1440, 2.0),
        ("harvest-stats", mac_harvest, 2880, 1800, 2.0),
    ];
    for (name, f, w, h, s) in macs {
        for (lang, code) in [(Lang::En, "en"), (Lang::De, "de")] {
            let mut ui = Ui::new(&mut lay, w, h, s);
            f(&mut ui, lang);
            write(&format!("{code}/mac/{name}.png"), ui.encode())?;
        }
    }
    // The phone next to the Mac (no variant folder: used by every target).
    let mut ui = Ui::new(&mut lay, 1206, 2622, 3.0);
    screen_harvest(&mut ui, Platform::Ios, Lang::En);
    write("en/phone-sync.png", ui.encode())?;

    let webs: [(&str, DeskFn, u32, u32, f32); 2] = [
        ("plots", web_plots, 1600, 1000, 1600.0 / 1440.0),
        ("seed-swap", web_seeds, 1600, 900, 1600.0 / 1440.0),
    ];
    for (name, f, w, h, s) in webs {
        for (lang, code) in [(Lang::En, "en"), (Lang::De, "de")] {
            let mut ui = Ui::new(&mut lay, w, h, s);
            f(&mut ui, lang);
            write(&format!("{code}/web/{name}.png"), ui.encode())?;
        }
    }
    println!("total {:.2} MB", total as f64 / 1_048_576.0);
    Ok(())
}

fn display(p: &Path) -> String {
    p.display().to_string()
}
