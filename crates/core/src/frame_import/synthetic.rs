#![allow(clippy::many_single_char_names, reason = "pixel geometry")]

//! A synthetic bezel package for tests, examples and demos: our own
//! generated PNGs (an opaque rounded frame with a transparent rounded screen
//! and an opaque island) named like Apple's Product Bezels. Never vendor
//! artwork.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::analyze::{PxRect, RgbaImage, encode_rgba_png};

/// One generated bezel.
#[derive(Debug, Clone, PartialEq)]
pub struct BezelSpec {
    /// Path inside the package (`/`-separated).
    pub file: String,
    pub width: u32,
    pub height: u32,
    /// The transparent screen rectangle (px).
    pub screen: PxRect,
    pub screen_radius: f64,
    pub outer_radius: f64,
    /// Opaque pill inside the screen near the top (portrait) / left
    /// (landscape) edge, like a Dynamic Island.
    pub island: bool,
    /// Laptop: an opaque base below the lid, outside the screen.
    pub base: bool,
    pub color: [u8; 3],
}

fn phone(file: &str, landscape: bool, color: [u8; 3]) -> BezelSpec {
    // 1320:2868 / 4 = 330 × 717, 25 px bezel.
    let (sw, sh, m) = (330, 717, 25);
    let (w, h, screen) = if landscape {
        (
            sh + 2 * m,
            sw + 2 * m,
            PxRect {
                x: m,
                y: m,
                w: sh,
                h: sw,
            },
        )
    } else {
        (
            sw + 2 * m,
            sh + 2 * m,
            PxRect {
                x: m,
                y: m,
                w: sw,
                h: sh,
            },
        )
    };
    BezelSpec {
        file: file.to_owned(),
        width: w,
        height: h,
        screen,
        screen_radius: 40.0,
        outer_radius: 62.0,
        island: true,
        base: false,
        color,
    }
}

/// The default package: phones (portrait + landscape, two colours, a
/// superseded smaller model), an iPad, a `MacBook`, an unmatched PNG.
pub fn default_specs() -> Vec<BezelSpec> {
    let black = [0x2C, 0x2C, 0x30];
    let desert = [0xBF, 0xA4, 0x8F];
    let mut out = vec![
        phone(
            "iPhone 16 Pro Max/iPhone 16 Pro Max - Black Titanium - Portrait.png",
            false,
            black,
        ),
        phone(
            "iPhone 16 Pro Max/iPhone 16 Pro Max - Black Titanium - Landscape.png",
            true,
            black,
        ),
        phone(
            "iPhone 16 Pro Max/iPhone 16 Pro Max \u{2013} Desert Titanium \u{2013} Portrait.png",
            false,
            desert,
        ),
        phone(
            "iPhone 16 Pro/iPhone 16 Pro - Black Titanium - Portrait.png",
            false,
            black,
        ),
    ];
    // iPad Pro 13: 2064:2752 / 8 = 258 × 344, 20 px bezel.
    out.push(BezelSpec {
        file: "iPad Pro 13/iPad Pro 13 - M4 - Space Black - Portrait.png".into(),
        width: 298,
        height: 384,
        screen: PxRect {
            x: 20,
            y: 20,
            w: 258,
            h: 344,
        },
        screen_radius: 14.0,
        outer_radius: 30.0,
        island: false,
        base: false,
        color: black,
    });
    // MacBook: 2880:1800 / 8 = 360 × 225 screen, lid bezel 18 px, base.
    out.push(BezelSpec {
        file: "MacBook Pro/MacBook Pro 14 - M4 - Space Black.png".into(),
        width: 440,
        height: 285,
        screen: PxRect {
            x: 40,
            y: 18,
            w: 360,
            h: 225,
        },
        screen_radius: 6.0,
        outer_radius: 14.0,
        island: false,
        base: true,
        color: black,
    });
    // A PNG no rule knows.
    let mut odd = phone(
        "Extras/Apple Vision Pro - Space Gray - Portrait.png",
        false,
        black,
    );
    odd.island = false;
    out.push(odd);
    out
}

/// Inside a rounded rect (radius `r`) given by its edges.
fn in_rrect(x: f64, y: f64, l: f64, t: f64, rgt: f64, b: f64, r: f64) -> bool {
    if x < l || x > rgt || y < t || y > b {
        return false;
    }
    let cx = x.clamp(l + r, rgt - r);
    let cy = y.clamp(t + r, b - r);
    (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

/// Renders a spec (2 × 2 supersampled edges).
pub fn render(spec: &BezelSpec) -> RgbaImage {
    let (w, h) = (spec.width, spec.height);
    let s = spec.screen;
    let (sl, st) = (f64::from(s.x), f64::from(s.y));
    let (sr, sb) = (sl + f64::from(s.w), st + f64::from(s.h));
    let lid_bottom = if spec.base { sb + 18.0 } else { f64::from(h) };
    let portrait = s.h >= s.w;
    // Island pill: 30 % of the short side long, 10 % thick, 12 px from the edge.
    let short = f64::from(s.w.min(s.h));
    let (il, it, ir, ib) = if portrait {
        let iw = short * 0.3;
        let cx = f64::midpoint(sl, sr);
        (
            cx - iw / 2.0,
            st + 12.0,
            cx + iw / 2.0,
            st + 12.0 + short * 0.1,
        )
    } else {
        let ih = short * 0.3;
        let cy = f64::midpoint(st, sb);
        (
            sl + 12.0,
            cy - ih / 2.0,
            sl + 12.0 + short * 0.1,
            cy + ih / 2.0,
        )
    };
    let island_r = short * 0.05;
    let mut data = vec![0u8; w as usize * h as usize * 4];
    for y in 0..h {
        for x in 0..w {
            let mut cover = 0u32;
            for (ox, oy) in [(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)] {
                let (px, py) = (f64::from(x) + ox, f64::from(y) + oy);
                let lid = in_rrect(
                    px,
                    py,
                    0.0,
                    0.0,
                    f64::from(w),
                    lid_bottom,
                    spec.outer_radius,
                );
                let base = spec.base
                    && in_rrect(
                        px,
                        py,
                        0.0,
                        lid_bottom + 2.0,
                        f64::from(w),
                        f64::from(h),
                        6.0,
                    );
                let screen = in_rrect(px, py, sl, st, sr, sb, spec.screen_radius);
                let island = spec.island && in_rrect(px, py, il, it, ir, ib, island_r);
                if ((lid || base) && !screen) || island {
                    cover += 1;
                }
            }
            let i = (y as usize * w as usize + x as usize) * 4;
            let [r, g, b] = spec.color;
            // A lighter rim line 3 px inside the outer edge, to see scaling.
            let rim = x < 3 || y < 3 || x >= w - 3;
            let shade = |c: u8| if rim { c.saturating_add(60) } else { c };
            data[i..i + 4].copy_from_slice(&[
                shade(r),
                shade(g),
                shade(b),
                (cover * 255 / 4) as u8,
            ]);
        }
    }
    RgbaImage {
        width: w,
        height: h,
        data,
    }
}

/// Writes the specs as PNGs (plus a non-PNG licence file) below `dir`.
pub fn write_folder(dir: &Path, specs: &[BezelSpec]) -> Result<(), String> {
    for spec in specs {
        let path = spec.file.split('/').fold(dir.to_owned(), |p, c| p.join(c));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let png = encode_rgba_png(&render(spec))?;
        fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    fs::write(
        dir.join("License.rtf"),
        "synthetic test package, not Apple artwork\n",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Writes the specs into a zip archive at `path` (inside a top folder, plus
/// macOS resource-fork junk that must be ignored).
pub fn write_zip(path: &Path, specs: &[BezelSpec]) -> Result<PathBuf, String> {
    let file = fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let e = |e: zip::result::ZipError| e.to_string();
    for spec in specs {
        let png = encode_rgba_png(&render(spec))?;
        zip.start_file(format!("Bezels/{}", spec.file), opts)
            .map_err(e)?;
        zip.write_all(&png).map_err(|e| e.to_string())?;
        zip.start_file(
            format!("__MACOSX/Bezels/._{}", spec.file.replace('/', "_")),
            opts,
        )
        .map_err(e)?;
        zip.write_all(b"junk").map_err(|e| e.to_string())?;
    }
    zip.start_file("Bezels/License.rtf", opts).map_err(e)?;
    zip.write_all(b"synthetic").map_err(|e| e.to_string())?;
    zip.finish().map_err(e)?;
    Ok(path.to_owned())
}
