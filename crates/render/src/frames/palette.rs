//! Frame colours: catalog palette entries, free `#hex` colours, helpers.

use vitrine_core::catalog::ColorVariant;
use vitrine_core::resolve::{DeviceColor, ResolvedDevice};

use crate::paint::{Rgba, hex};

/// Colours used to draw one frame.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Flat housing colour (`stylized`).
    pub body: Rgba,
    /// Flat edge colour (`stylized`).
    pub edge: Rgba,
    /// Material gradient light / mid / dark (`detailed`).
    pub rim: [Rgba; 3],
    /// Front glass around the display.
    pub glass: Rgba,
}

fn rgba(c: vitrine_core::model::Rgba) -> Rgba {
    [c.r, c.g, c.b, 255]
}

impl Palette {
    pub fn from_variant(v: &ColorVariant) -> Self {
        Self {
            body: rgba(v.body),
            edge: rgba(v.edge),
            rim: v.rim.map(rgba),
            glass: rgba(v.glass),
        }
    }

    /// Derives a full palette from one housing colour (`color: "#hex"`).
    pub fn from_color(c: Rgba) -> Self {
        Self {
            body: c,
            edge: shade(c, -0.14),
            rim: [shade(c, 0.45), shade(c, -0.05), shade(c, -0.45)],
            glass: hex("#060607"),
        }
    }

    /// Neutral dark fallback (unknown colour names, colourless models).
    pub fn neutral() -> Self {
        Self {
            body: hex("#2C2C30"),
            edge: hex("#1C1C1F"),
            rim: [hex("#8A8A90"), hex("#3C3C41"), hex("#1B1B1E")],
            glass: hex("#060607"),
        }
    }

    pub fn for_device(dev: &ResolvedDevice) -> Self {
        match (&dev.color, &dev.palette) {
            (_, Some(v)) => Self::from_variant(v),
            (DeviceColor::Custom(c), None) => Self::from_color(rgba(*c)),
            (DeviceColor::Named(name), None) => vitrine_core::catalog::catalog()
                .devices
                .colors
                .get(name)
                .map_or_else(Self::neutral, Self::from_variant),
            (DeviceColor::None, None) => Self::neutral(),
        }
    }
}

/// Relative luminance 0..1 (sRGB weights, no linearisation; good enough
/// to decide between light and dark accents).
pub fn luma(c: Rgba) -> f32 {
    (0.2126 * f32::from(c[0]) + 0.7152 * f32::from(c[1]) + 0.0722 * f32::from(c[2])) / 255.0
}

pub fn is_light(c: Rgba) -> bool {
    luma(c) > 0.5
}

/// Lightens (`amount > 0`, towards white) or darkens (towards black).
pub fn shade(c: Rgba, amount: f32) -> Rgba {
    let f = |v: u8| {
        let v = f32::from(v);
        let out = if amount >= 0.0 {
            v + (255.0 - v) * amount
        } else {
            v * (1.0 + amount)
        };
        out.round().clamp(0.0, 255.0) as u8
    };
    [f(c[0]), f(c[1]), f(c[2]), c[3]]
}

/// Linear mix `a → b` by `t`.
pub fn mix(a: Rgba, b: Rgba, t: f32) -> Rgba {
    let f = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    [f(a[0], b[0]), f(a[1], b[1]), f(a[2], b[2]), f(a[3], b[3])]
}

/// Black or white at `alpha` — whichever contrasts with `on`.
pub fn contrast(on: Rgba, alpha: f32) -> Rgba {
    let a = (alpha * 255.0).round() as u8;
    if is_light(on) {
        [0, 0, 0, a]
    } else {
        [255, 255, 255, a]
    }
}
