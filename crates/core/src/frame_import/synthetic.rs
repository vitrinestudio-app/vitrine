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

/// Screen sizes (portrait, px) of the two synthetic iPhone models, like
/// the 6.9" Pro Max and the 6.3" Pro (the App Store Connect sizes).
pub const PRO_MAX_SCREEN: (u32, u32) = (1320, 2868);
pub const PRO_SCREEN: (u32, u32) = (1206, 2622);

/// A phone bezel with the portrait screen `screen` divided by `div`.
fn phone(file: &str, landscape: bool, color: [u8; 3], screen: (u32, u32), div: u32) -> BezelSpec {
    let div = div.max(1);
    let (sw, sh, m) = (screen.0 / div, screen.1 / div, 100 / div);
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
        screen_radius: f64::from(160 / div),
        outer_radius: f64::from(250 / div),
        island: true,
        base: false,
        color,
    }
}

/// The default package at 1/4 of the real sizes (fast tests), see
/// [`specs`].
pub fn default_specs() -> Vec<BezelSpec> {
    specs(4)
}

/// The package with phone screens divided by `div` (`1` = real sizes,
/// 1320×2868 and 1206×2622): two iPhone models ("16 Pro Max" and
/// "16 Pro", portrait + landscape, plus a second Pro Max colour), a
/// smaller duplicate of one file (superseded), an iPad, a `MacBook` and an
/// unmatched PNG.
pub fn specs(div: u32) -> Vec<BezelSpec> {
    let black = [0x2C, 0x2C, 0x30];
    let desert = [0xBF, 0xA4, 0x8F];
    // The "Pro" a little warmer, to tell the two models apart in renders.
    let pro = [0x44, 0x30, 0x34];
    let mut out = vec![
        phone(
            "iPhone 16 Pro Max/iPhone 16 Pro Max - Black Titanium - Portrait.png",
            false,
            black,
            PRO_MAX_SCREEN,
            div,
        ),
        phone(
            "iPhone 16 Pro Max/iPhone 16 Pro Max - Black Titanium - Landscape.png",
            true,
            black,
            PRO_MAX_SCREEN,
            div,
        ),
        phone(
            "iPhone 16 Pro Max/iPhone 16 Pro Max \u{2013} Desert Titanium \u{2013} Portrait.png",
            false,
            desert,
            PRO_MAX_SCREEN,
            div,
        ),
        phone(
            "iPhone 16 Pro/iPhone 16 Pro - Black Titanium - Portrait.png",
            false,
            pro,
            PRO_SCREEN,
            div,
        ),
        phone(
            "iPhone 16 Pro/iPhone 16 Pro - Black Titanium - Landscape.png",
            true,
            pro,
            PRO_SCREEN,
            div,
        ),
        // The same file again at half size: superseded by the larger one.
        phone(
            "Small/iPhone 16 Pro - Black Titanium - Portrait.png",
            false,
            pro,
            PRO_SCREEN,
            div * 2,
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
        PRO_MAX_SCREEN,
        4,
    );
    odd.island = false;
    out.push(odd);
    out
}

/// Non-PNG files of the synthetic package: Photoshop files (ignored), the
/// licence (shown to the user) and one other file (skipped).
pub const EXTRA_FILES: [(&str, &str); 4] = [
    (
        "iPhone 16 Pro Max/iPhone 16 Pro Max - Black Titanium.psd",
        "8BPS synthetic",
    ),
    ("iPhone 16 Pro/iPhone 16 Pro.psd", "8BPS synthetic"),
    ("License.rtf", "synthetic test package, not Apple artwork\n"),
    ("Read Me.txt", "synthetic\n"),
];

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

/// Writes the specs as PNGs (plus [`EXTRA_FILES`]) below `dir`.
pub fn write_folder(dir: &Path, specs: &[BezelSpec]) -> Result<(), String> {
    for spec in specs {
        let path = spec.file.split('/').fold(dir.to_owned(), |p, c| p.join(c));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let png = encode_rgba_png(&render(spec))?;
        fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    for (file, text) in EXTRA_FILES {
        let path = file.split('/').fold(dir.to_owned(), |p, c| p.join(c));
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
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
    for (file, text) in EXTRA_FILES {
        zip.start_file(format!("Bezels/{file}"), opts).map_err(e)?;
        zip.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(e)?;
    Ok(path.to_owned())
}

// ----- own frame pack (ADR 0024) ---------------------------------------------------------

/// `frame.yaml` of the synthetic frame pack ([`write_pack`]); `SX` … `SR`
/// are replaced with the painted frame's screen.
pub const PACK_MANIFEST: &str = r"# Synthetic frame pack (generated, not vendor artwork).
# Format reference: docs/guide/frame-packs.md
format: vitrine-frame/1
name: Clay phone
author: Vitrine Studio tests
license: CC0-1.0
models: [iphone-island, android-punchhole]
frames:
  # Transparent screen: detected automatically.
  - file: clay-sand-portrait.png
    color: Sand
  - file: clay-sand-landscape.png
    color: Sand
  - file: clay-ink-portrait.png
    color: Ink
  # Painted (opaque) screen: given explicitly and cut out.
  - file: painted.png
    color: Painted
    screen: {x: SX, y: SY, w: SW, h: SH}
    radius: SR
    cut_screen: true
  # SVG, rasterized at 1200 px on the long side.
  - file: outline.svg
    color: Outline
    raster: 1200
";

/// Clay colours of the pack.
const SAND: [u8; 3] = [0xD9, 0xC8, 0xAE];
const INK: [u8; 3] = [0x26, 0x2B, 0x33];

/// Writes the synthetic frame pack "Clay phone" into the folder `dir`
/// (phone screens divided by `div` like [`specs`]): PNGs with a
/// transparent screen (two colours, portrait + landscape), one with a
/// painted screen (`screen` + `cut_screen`), one SVG, `frame.yaml` and a
/// licence file. Our own drawings, never vendor artwork.
pub fn write_pack(dir: &Path, div: u32) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let write = |name: &str, bytes: &[u8]| {
        let p = dir.join(name);
        fs::write(&p, bytes).map_err(|e| format!("{}: {e}", p.display()))
    };
    let sand_p = phone("clay-sand-portrait.png", false, SAND, PRO_SCREEN, div);
    let sand_l = phone("clay-sand-landscape.png", true, SAND, PRO_SCREEN, div);
    let ink_p = phone("clay-ink-portrait.png", false, INK, PRO_SCREEN, div);
    for s in [&sand_p, &sand_l, &ink_p] {
        write(&s.file, &encode_rgba_png(&render(s))?)?;
    }
    // Painted screen: the bezel with an opaque dark screen.
    let mut painted = phone("painted.png", false, SAND, PRO_SCREEN, div);
    painted.island = false;
    let mut img = render(&painted);
    let s = painted.screen;
    for y in s.y..s.y + s.h {
        for x in s.x..s.x + s.w {
            let i = (y as usize * img.width as usize + x as usize) * 4;
            if img.data[i + 3] < 255 {
                img.data[i..i + 4].copy_from_slice(&[0x10, 0x10, 0x14, 255]);
            }
        }
    }
    write("painted.png", &encode_rgba_png(&img)?)?;
    write("outline.svg", OUTLINE_SVG.as_bytes())?;
    let manifest = PACK_MANIFEST
        .replace("SX", &s.x.to_string())
        .replace("SY", &s.y.to_string())
        .replace("SW", &s.w.to_string())
        .replace("SH", &s.h.to_string())
        .replace("SR", &format!("{}", painted.screen_radius));
    write(super::pack::PACK_FILE, manifest.as_bytes())?;
    write(
        "LICENSE.txt",
        b"Synthetic test frames generated by Vitrine Studio. CC0-1.0.\n",
    )?;
    Ok(())
}

/// A simple outlined phone as SVG (transparent screen 412×892 at 24,24 in
/// SVG units, corner radius 50).
pub const OUTLINE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="460" height="940" viewBox="0 0 460 940">
  <path fill-rule="evenodd" fill="#3A6EA5"
        d="M70 0 H390 A70 70 0 0 1 460 70 V870 A70 70 0 0 1 390 940 H70 A70 70 0 0 1 0 870 V70 A70 70 0 0 1 70 0 Z
           M74 24 H386 A50 50 0 0 1 436 74 V866 A50 50 0 0 1 386 916 H74 A50 50 0 0 1 24 866 V74 A50 50 0 0 1 74 24 Z"/>
  <rect x="190" y="40" width="80" height="20" rx="10" fill="#3A6EA5"/>
</svg>
"##;

/// Writes the synthetic frame pack as a zip at `path` (inside a top
/// folder `Clay phone/`, plus macOS junk).
pub fn write_pack_zip(path: &Path, div: u32) -> Result<PathBuf, String> {
    let tmp = std::env::temp_dir().join(format!(
        "vitrine-synthetic-pack-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    write_pack(&tmp, div)?;
    let result = (|| {
        let file = fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut zip = zip::ZipWriter::new(file);
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let e = |e: zip::result::ZipError| e.to_string();
        let mut names: Vec<_> = fs::read_dir(&tmp)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        for name in names {
            let bytes = fs::read(tmp.join(&name)).map_err(|e| e.to_string())?;
            zip.start_file(format!("Clay phone/{name}"), opts)
                .map_err(e)?;
            zip.write_all(&bytes).map_err(|e| e.to_string())?;
        }
        zip.start_file("__MACOSX/Clay phone/._frame.yaml", opts)
            .map_err(e)?;
        zip.write_all(b"junk").map_err(|e| e.to_string())?;
        zip.finish().map_err(e)?;
        Ok(path.to_owned())
    })();
    let _ = fs::remove_dir_all(&tmp);
    result
}
