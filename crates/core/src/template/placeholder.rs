//! Generated placeholder screenshots for templates: a neutral, abstract app
//! screen (header, cards, tab bar) in the device's screen aspect. Templates
//! never contain real app screenshots or vendor graphics; the gallery server
//! regenerates these on upload.

use std::collections::BTreeMap;

use crate::model::{Aspect, Orientation, Project};

use super::walk;

/// Width of a portrait placeholder in px (landscape: height).
const SHORT_SIDE: u32 = 480;
/// Aspect used when the device has a free screen (browser) or is unknown.
const DEFAULT_ASPECT: (u32, u32) = (9, 19);

/// Muted accent colours, picked per key so screens differ.
const ACCENTS: [[u8; 3]; 5] = [
    [0x5B, 0x7D, 0xB1],
    [0x6A, 0x9C, 0x89],
    [0xB0, 0x7D, 0x62],
    [0x8A, 0x74, 0xB0],
    [0x5E, 0x93, 0xA8],
];

struct Canvas {
    w: u32,
    h: u32,
    px: Vec<u8>,
}

impl Canvas {
    fn new(w: u32, h: u32) -> Self {
        Self {
            w,
            h,
            px: vec![0; (w * h * 3) as usize],
        }
    }

    fn set(&mut self, x: u32, y: u32, c: [u8; 3]) {
        let i = ((y * self.w + x) * 3) as usize;
        self.px[i..i + 3].copy_from_slice(&c);
    }

    /// Fills a rounded rectangle (fractions of the canvas width/height are
    /// converted by the caller).
    #[allow(clippy::many_single_char_names)]
    fn round_rect(&mut self, x: f64, y: f64, w: f64, h: f64, r: f64, c: [u8; 3]) {
        let (x0, y0) = (x.max(0.0) as u32, y.max(0.0) as u32);
        let x1 = ((x + w).min(f64::from(self.w))) as u32;
        let y1 = ((y + h).min(f64::from(self.h))) as u32;
        for py in y0..y1 {
            for px in x0..x1 {
                let (fx, fy) = (f64::from(px) + 0.5, f64::from(py) + 0.5);
                let dx = (x + r - fx).max(fx - (x + w - r)).max(0.0);
                let dy = (y + r - fy).max(fy - (y + h - r)).max(0.0);
                if dx * dx + dy * dy <= r * r {
                    self.set(px, py, c);
                }
            }
        }
    }

    fn png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut enc = png::Encoder::new(&mut out, self.w, self.h);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.set_compression(png::Compression::High);
        // Writing into a Vec cannot fail for a correctly sized buffer.
        if let Ok(mut w) = enc.write_header() {
            let _ = w.write_image_data(&self.px);
            let _ = w.finish();
        }
        out
    }
}

fn mix(a: [u8; 3], b: [u8; 3], t: f64) -> [u8; 3] {
    let m = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// A placeholder screen of `w`×`h` px; `seed` picks the accent colour.
#[allow(clippy::many_single_char_names)]
pub fn render(w: u32, h: u32, seed: &str) -> Vec<u8> {
    let (w, h) = (w.clamp(16, 4096), h.clamp(16, 4096));
    let accent = ACCENTS[seed.bytes().map(usize::from).sum::<usize>() % ACCENTS.len()];
    let mut c = Canvas::new(w, h);
    let (top, bottom) = ([0xF4, 0xF6, 0xFA], [0xE4, 0xE9, 0xF1]);
    for y in 0..h {
        let row = mix(top, bottom, f64::from(y) / f64::from(h));
        for x in 0..w {
            c.set(x, y, row);
        }
    }
    let (fw, fh) = (f64::from(w), f64::from(h));
    let unit = fw.min(fh);
    let pad = unit * 0.06;
    let radius = unit * 0.035;
    // Header band and title bar.
    let header = fh * 0.16;
    c.round_rect(0.0, 0.0, fw, header, 0.0, mix(accent, [255; 3], 0.55));
    c.round_rect(
        pad,
        header * 0.55,
        fw * 0.45,
        unit * 0.05,
        unit * 0.025,
        mix(accent, [0; 3], 0.15),
    );
    // Hero card.
    let mut y = header + pad;
    let hero = fh * 0.2;
    c.round_rect(pad, y, fw - 2.0 * pad, hero, radius, accent);
    c.round_rect(
        pad * 2.0,
        y + hero - pad * 1.6,
        fw * 0.4,
        unit * 0.04,
        unit * 0.02,
        mix(accent, [255; 3], 0.6),
    );
    y += hero + pad;
    // List cards until the tab bar.
    let tab = fh * 0.09;
    let card = (unit * 0.2).max(8.0);
    let line = [0xD3, 0xD9, 0xE3];
    while y + card < fh - tab - pad * 0.5 {
        c.round_rect(pad, y, fw - 2.0 * pad, card, radius, [0xFF; 3]);
        let dot = card * 0.5;
        c.round_rect(
            pad * 1.6,
            y + (card - dot) / 2.0,
            dot,
            dot,
            dot / 2.0,
            mix(accent, [255; 3], 0.35),
        );
        let tx = pad * 1.6 + dot + pad * 0.6;
        let lw = (fw - pad - tx - pad * 0.6).max(1.0);
        c.round_rect(
            tx,
            y + card * 0.3,
            lw * 0.75,
            card * 0.14,
            card * 0.07,
            line,
        );
        c.round_rect(
            tx,
            y + card * 0.56,
            lw * 0.5,
            card * 0.12,
            card * 0.06,
            [0xE3, 0xE7, 0xEE],
        );
        y += card + pad * 0.6;
    }
    // Tab bar with four items.
    c.round_rect(0.0, fh - tab, fw, tab, 0.0, [0xFF; 3]);
    for i in 0..4 {
        let cx = fw * (0.125 + 0.25 * f64::from(i));
        let d = tab * 0.32;
        let color = if i == 0 { accent } else { line };
        c.round_rect(cx - d / 2.0, fh - tab * 0.68, d, d, d * 0.3, color);
    }
    c.png()
}

/// Pixel size of a placeholder for a device model (screen aspect from the
/// catalog, turned for landscape).
pub fn size_for(model: Option<&str>, orientation: Option<Orientation>) -> (u32, u32) {
    let device = model.and_then(crate::catalog::device);
    let (aw, ah) = device
        .and_then(|d| d.screenshot.aspect)
        .map_or(DEFAULT_ASPECT, |a: Aspect| (a.w, a.h));
    let natural_landscape = aw > ah;
    let want_landscape = match orientation {
        Some(Orientation::Landscape) => true,
        Some(Orientation::Portrait) => false,
        None => device.is_some_and(|d| d.natural == Orientation::Landscape),
    };
    let (aw, ah) = if natural_landscape == want_landscape {
        (aw, ah)
    } else {
        (ah, aw)
    };
    let (aw, ah) = (f64::from(aw.max(1)), f64::from(ah.max(1)));
    if aw <= ah {
        (SHORT_SIDE, (f64::from(SHORT_SIDE) * ah / aw).round() as u32)
    } else {
        ((f64::from(SHORT_SIDE) * aw / ah).round() as u32, SHORT_SIDE)
    }
}

/// Placeholder files for every screenshot key of the project's canvases:
/// `screenshots/<default lang>/<key>.png` (one file serves all variants).
pub fn for_project(project: &Project) -> BTreeMap<String, Vec<u8>> {
    let lang = project.languages.default.as_str();
    let mut out = BTreeMap::new();
    for canvas in &project.canvases {
        for u in walk::screenshots(canvas) {
            let path = format!("screenshots/{lang}/{}.png", u.key);
            if out.contains_key(&path) || super::archive::check_path(&path).is_err() {
                continue;
            }
            let (w, h) = size_for(u.model.as_deref(), u.orientation);
            out.insert(path, render(w, h, &u.key));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_png_of_the_requested_size() {
        let png = render(120, 260, "home");
        let size = imagesize::blob_size(&png).unwrap();
        assert_eq!((size.width, size.height), (120, 260));
        assert_ne!(render(120, 260, "home"), render(120, 260, "homf"));
    }

    #[test]
    fn sizes_follow_the_device() {
        let (w, h) = size_for(Some("iphone-island"), None);
        assert_eq!(w, SHORT_SIDE);
        assert!(h > 2 * w, "{w}x{h}");
        let (w, h) = size_for(Some("iphone-island"), Some(Orientation::Landscape));
        assert!(w > 2 * h, "{w}x{h}");
        assert_eq!(size_for(None, None).0, SHORT_SIDE);
    }
}
