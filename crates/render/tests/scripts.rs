//! Non-Latin scripts on the script fixture (`tests/fixtures/scripts`,
//! ADR 0028): bundled fonts only, no golden images – the tests check glyph
//! coverage, line breaking, RTL alignment and that every text draws.
//!
//! Set `VITRINE_RENDER_DEBUG_DIR` to also write the rendered pages there.

use std::path::{Path, PathBuf};

use tiny_skia::Pixmap;
use vitrine_core::Project;
use vitrine_core::layout::FontLibrary;
use vitrine_core::resolve::{FsAssets, ResolveOptions, resolve};
use vitrine_core::validate::{FindingCode, ValidateOptions, validate};
use vitrine_render::raster::save_png_rgb;
use vitrine_render::scene::{Quality, RenderContext, RenderOptions, Scale};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/scripts")
}

fn project() -> Project {
    Project::load(fixture()).expect("script fixture loads")
}

/// Findings of `code` for `lang` (bundled fonts only).
fn findings(p: &Project, lang: &str, code: FindingCode) -> Vec<String> {
    let root = fixture();
    let fs = FsAssets::new(&root);
    let mut opts = ValidateOptions::new(&fs);
    opts.fonts = FontLibrary::bundled();
    opts.langs = Some(vec![lang.to_owned()]);
    validate(p, &opts)
        .findings
        .into_iter()
        .filter(|f| f.code == code)
        .map(|f| f.message)
        .collect()
}

/// Page `image` of `lang` at 0.5×.
fn render(p: &Project, lang: &str, image: usize) -> Pixmap {
    let root = fixture();
    let ctx = RenderContext::with_fonts(&root, FontLibrary::bundled());
    let scene = resolve(
        p,
        "pages",
        lang,
        &ResolveOptions::new(&FsAssets::new(&root)),
    )
    .unwrap();
    let r = ctx.rasterize_prepared(
        &ctx.prepare(&scene),
        image,
        &RenderOptions {
            scale: Scale::Factor(0.5),
            quality: Quality::Export,
        },
    );
    if let Ok(dir) = std::env::var("VITRINE_RENDER_DEBUG_DIR") {
        save_png_rgb(
            &r.pixmap,
            &Path::new(&dir).join(format!("scripts-{lang}-{image}.png")),
            [255, 255, 255],
        )
        .unwrap();
    }
    r.pixmap
}

/// Dark (text) pixels in columns `x0..x1`.
fn ink(pm: &Pixmap, x0: u32, x1: u32) -> usize {
    let mut n = 0;
    for y in 0..pm.height() {
        for x in x0..x1.min(pm.width()) {
            let c = pm.pixel(x, y).unwrap().demultiply();
            if u32::from(c.red()) + u32::from(c.green()) + u32::from(c.blue()) < 300 {
                n += 1;
            }
        }
    }
    n
}

/// Arabic, Hebrew, Devanagari, Bengali, Tamil and Thai are covered by the
/// bundled Noto fonts: no missing glyphs, no emergency breaks (Thai has no
/// spaces: dictionary line breaking), everything fits.
#[test]
fn bundled_script_fonts_cover_rtl_indic_and_thai() {
    let p = project();
    for lang in ["ar", "he", "hi", "bn", "ta", "th"] {
        for code in [
            FindingCode::MissingGlyphs,
            FindingCode::EmergencyBreak,
            FindingCode::TextOverflow,
            FindingCode::UnknownFont,
        ] {
            let f = findings(&p, lang, code);
            assert!(f.is_empty(), "{lang}: {code:?}: {f:?}");
        }
        let pm = render(&p, lang, 0);
        assert!(ink(&pm, 0, pm.width()) > 1000, "{lang}: page 0 draws text");
    }
}

/// CJK is not bundled: without installed fonts the characters are
/// reported (with "Use installed fonts" the language's system font fills
/// them – not tested, machine dependent).
#[test]
fn cjk_needs_installed_or_project_fonts() {
    let p = project();
    for lang in ["ja", "zh-Hans", "ko"] {
        let f = findings(&p, lang, FindingCode::MissingGlyphs);
        assert!(!f.is_empty(), "{lang}: missing glyphs reported");
    }
}

/// `align: right` (language override) puts Arabic and Hebrew on the right;
/// English stays left. Alignment is physical in every language: every
/// line of a right-aligned text ends at the right margin, so the strip
/// just inside the right margin has more ink than the one inside the left.
#[test]
fn rtl_text_follows_the_physical_alignment() {
    let p = project();
    for (lang, right) in [("ar", true), ("he", true), ("en", false)] {
        let pm = render(&p, lang, 0);
        let w = pm.width();
        // Page 1000 lu = 300 px; margins 80 lu = 24 px.
        let (l, r) = (ink(&pm, 24, 34), ink(&pm, w - 34, w - 24));
        if right {
            assert!(
                r > l,
                "{lang}: right-aligned (left edge {l}, right edge {r})"
            );
        } else {
            assert!(
                l > r,
                "{lang}: left-aligned (left edge {l}, right edge {r})"
            );
        }
        // Nothing outside the box.
        assert_eq!(
            ink(&pm, 0, 20) + ink(&pm, w - 20, w),
            0,
            "{lang}: inside the margins"
        );
    }
    // Centred RTL text stays inside the page and centred.
    let pm = render(&p, "ar", 1);
    let w = pm.width();
    let (l, r) = (ink(&pm, 0, w / 2), ink(&pm, w / 2, w));
    let ratio = l as f64 / r.max(1) as f64;
    assert!(
        (0.6..1.6).contains(&ratio),
        "ar centred: left {l}, right {r}"
    );
    assert_eq!(
        ink(&pm, 0, 20) + ink(&pm, w - 20, w),
        0,
        "ar centred: inside"
    );
}
