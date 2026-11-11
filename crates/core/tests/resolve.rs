//! `core::resolve` on the sample project in `tests/data/sample`.

#![allow(clippy::many_single_char_names, clippy::float_cmp)]

use std::collections::BTreeSet;
use std::path::Path;

use vitrine_core::Project;
use vitrine_core::resolve::{
    DiagnosticCode, Level, ResolveError, ResolveOptions, ResolvedKind, ResolvedScene,
    ScreenshotSource, resolve,
};

fn sample() -> Project {
    Project::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sample")).unwrap()
}

fn assets() -> BTreeSet<String> {
    [
        "screenshots/en/ios-phone/home.png",
        "screenshots/de/ios-phone/home.png",
        "screenshots/en/ios-tablet/home.jpg",
        "screenshots/en/android-phone/home.png",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

fn run(project: &Project, target: &str, lang: &str) -> ResolvedScene {
    let files = assets();
    resolve(project, target, lang, &ResolveOptions::new(&files)).unwrap()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

/// Snapshot form: numbers rounded to 4 decimals so the snapshots do not
/// depend on the last bits of floating point maths.
fn snapshot_value(scene: &ResolvedScene) -> serde_json::Value {
    fn round(v: &mut serde_json::Value) {
        match v {
            serde_json::Value::Number(n) => {
                if let Some(f) = n.as_f64()
                    && !n.is_i64()
                    && !n.is_u64()
                {
                    let r = (f * 10_000.0).round() / 10_000.0;
                    *v = if r == r.trunc() && r.abs() < 1e15 {
                        serde_json::json!(r as i64)
                    } else {
                        serde_json::json!(r)
                    };
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(round),
            serde_json::Value::Object(o) => o.values_mut().for_each(round),
            _ => {}
        }
    }
    let mut v = serde_json::to_value(scene).unwrap();
    round(&mut v);
    v
}

#[test]
fn pages_target() {
    let p = sample();
    let s = run(&p, "ios-phone-69", "de");
    let f = &s.frame;
    assert!(close(f.layout.page_height, 1000.0 * 2868.0 / 1320.0));
    assert_eq!(f.images.len(), 5);
    assert_eq!(f.pages[1].id, "planner");
    assert!(close(f.layout.gap, 45.0), "appstore-iphone gap");
    // Element order follows z: the band (z -1) comes first.
    assert_eq!(s.elements[0].id.as_str(), "band");
    // The page background restricted to this target is there.
    assert!(s.element("page4-bg").is_some());
    let hero = s.element("hero-phone").unwrap();
    let ResolvedKind::Device(dev) = &hero.props else {
        panic!()
    };
    assert_eq!(dev.model, "iphone-island");
    let shot = dev.screenshot.as_ref().unwrap();
    assert_eq!(shot.source, ScreenshotSource::LangVariant);
    assert_eq!(
        shot.path.as_deref(),
        Some("screenshots/de/ios-phone/home.png")
    );
    // Centre on the right edge of page 0, not moved by the gap.
    assert!(close(hero.center.x, 1000.0));
    let ResolvedKind::Text(t) = &s.element("title-1").unwrap().props else {
        panic!()
    };
    assert_eq!(t.text.as_deref(), Some("Plane deine Beete **in Minuten**"));
    assert_eq!(t.text_lang.as_ref().unwrap().as_str(), "de");
    // The `de` language override: box, style and size.
    assert!(close(t.frame.rect.x, 60.0) && close(t.frame.rect.w, 880.0));
    assert_eq!(t.style.weight, 500, "style `sub`");
    assert!(close(t.style.size, 64.0));
    assert!(t.hyphenate, "element value");
    assert_eq!(t.font_fallbacks["Inter"], ["Noto Sans JP", "Noto Sans SC"]);
    // English: no override.
    let en = run(&p, "ios-phone-69", "en");
    let ResolvedKind::Text(t) = &en.element("title-1").unwrap().props else {
        panic!()
    };
    assert!(close(t.frame.rect.x, 80.0) && close(t.frame.rect.w, 840.0));
    assert_eq!(t.style.weight, 800);
    assert!(close(t.style.size, 72.0));
    // French: `fit` and `hyphenate` replaced.
    let fr = run(&p, "ios-phone-69", "fr");
    let ResolvedKind::Text(t) = &fr.element("title-1").unwrap().props else {
        panic!()
    };
    assert_eq!((t.shrink_to, t.max_lines), (Some(40.0), None));
    assert!(!t.hyphenate);
    // Groups: the offset applies on top of the children's home page.
    let badge = s.element("badge").unwrap();
    assert!(close(badge.center.x, f.layout.page_x(2) + 820.0));
    assert!(close(badge.center.y, 340.0));
    assert_eq!(badge.groups[0].as_str(), "badge-group");
    insta::assert_yaml_snapshot!("pages_ios_phone_69_de", snapshot_value(&s));
}

#[test]
fn grouped_target_with_variant_and_device_map() {
    let p = sample();
    let before = p.clone();
    let s = run(&p, "ios-ipad-13-landscape", "fr");
    assert_eq!(p, before, "resolve must not mutate the project");
    let f = &s.frame;
    assert_eq!(s.variant.as_ref().unwrap().as_str(), "tablet-landscape");
    assert!(close(f.layout.page_height, 1500.0));
    assert_eq!(f.images.len(), 3, "5 pages in groups of 2");
    assert_eq!(f.images[2].pages, [4]);
    assert!(close(f.layout.gap, 80.0), "40 lu per image = 80 page lu");
    assert!(close(f.images[1].source.x, 2080.0));
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::PartialImage)
    );
    // Variant: sub-1 hidden, title box from the override, page4-bg not in
    // this target.
    assert!(s.element("sub-1").is_none());
    assert!(s.element("page4-bg").is_none());
    let ResolvedKind::Text(t) = &s.element("title-1").unwrap().props else {
        panic!()
    };
    assert!(close(t.frame.rect.y, 90.0) && close(t.frame.rect.x, 70.0));
    assert_eq!(t.text_lang.as_ref().unwrap().as_str(), "en");
    // device_map + content_scale + bottom anchor on the 1500 lu column.
    let hero = s.element("hero-phone").unwrap();
    let ResolvedKind::Device(dev) = &hero.props else {
        panic!()
    };
    assert_eq!(dev.model, "ipad-modern");
    assert_eq!(dev.mapped_from.as_deref(), Some("iphone-island"));
    assert!(close(hero.size.x, 820.0 * 0.75));
    let outer = dev.device.outer_size();
    assert!(close(hero.size.y, 820.0 * 0.75 * outer.h / outer.w));
    assert!(close(hero.rotation, -4.0));
    assert!(close(
        hero.center.y,
        1500.0 + 1500.0 - 1000.0 * 2868.0 / 1320.0
    ));
    assert!(close(dev.shadow[0].blur, 48.0 * 0.75));
    let shot = dev.screenshot.as_ref().unwrap();
    assert_eq!(shot.source, ScreenshotSource::FallbackVariant);
    assert_eq!(
        shot.path.as_deref(),
        Some("screenshots/en/ios-tablet/home.jpg")
    );
    for code in [
        DiagnosticCode::TextFallback,
        DiagnosticCode::ScreenshotFallback,
    ] {
        let d = s.diagnostics.iter().find(|d| d.code == code).unwrap();
        assert_eq!(d.level, Level::Info);
    }
    insta::assert_yaml_snapshot!("grouped_ipad_landscape_fr", snapshot_value(&s));
}

#[test]
fn banner_target() {
    let p = sample();
    let s = run(&p, "blog-header", "en");
    let f = &s.frame;
    assert_eq!(f.images.len(), 1);
    assert!(close(f.layout.gap, 0.0), "gaps: none");
    let img = &f.images[0];
    // Region pages 0–2 = 3000 lu wide, cover into 2400×1000 → 0.8 px/lu,
    // 1250 lu visible from the top.
    assert!(close(img.viewport.scale, 0.8));
    assert!(close(img.source.x, 0.0) && close(img.source.w, 3000.0));
    assert!(close(img.source.h, 1250.0));
    assert_eq!(img.pages, [0, 1, 2]);
    // No variant: the banner uses the strip's own page height.
    assert!(close(f.layout.page_height, f.layout.reference_page_height));
    insta::assert_yaml_snapshot!("banner_blog_header_en", snapshot_value(&s));
}

#[test]
fn android_target_maps_devices_and_adapts_anchors() {
    let p = sample();
    let s = run(&p, "android-phone", "de-AT");
    assert!(close(s.frame.layout.page_height, 2000.0));
    let hero = s.element("hero-phone").unwrap();
    let ResolvedKind::Device(dev) = &hero.props else {
        panic!()
    };
    assert_eq!(dev.model, "android-punchhole");
    // Bottom anchor: y keeps its distance to the page bottom.
    assert!(close(
        hero.center.y,
        2172.0 + 2000.0 - 1000.0 * 2868.0 / 1320.0
    ));
    // Title stays at the top; `de-AT` uses the `de` language override.
    let title = s.element("title-1").unwrap();
    let ResolvedKind::Text(t) = &title.props else {
        panic!()
    };
    assert!(close(t.frame.rect.y, 120.0));
    assert_eq!(t.text_lang.as_ref().unwrap().as_str(), "de");
    // de-AT is not an available language; the chain still finds `de`.
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::LangNotAvailable)
    );
    // No screenshot for android-phone in `de`: fallback to `en`.
    let shot = dev.screenshot.as_ref().unwrap();
    assert_eq!(shot.source, ScreenshotSource::FallbackVariant);
    // play-phone: gap override 30 in profile lu.
    let g = s.frame.gallery.as_ref().unwrap();
    let profile = vitrine_core::catalog::gallery("play-phone").unwrap();
    let expected = profile.gap_on_canvas(30.0, p.target("android-phone").unwrap().size, 1);
    assert!(close(g.gap, expected));
}

#[test]
fn missing_screenshot_is_a_placeholder() {
    let p = sample();
    let s = run(&p, "ios-phone-69", "en");
    let ResolvedKind::Device(_) = &s.element("hero-phone").unwrap().props else {
        panic!()
    };
    // The web strip is not used by this target; check with a web target.
    let mut p = p;
    let mut t = p.targets[0].clone();
    t.id = vitrine_core::model::TargetId::new("web");
    t.strip = vitrine_core::model::StripId::new("web");
    t.size = vitrine_core::model::PixelSize::new(2880, 1800);
    t.screenshots = None;
    t.gallery = None;
    t.store = None;
    p.targets.push(t);
    let s = run(&p, "web", "en");
    let ResolvedKind::Device(dev) = &s.element("web-browser").unwrap().props else {
        panic!()
    };
    let shot = dev.screenshot.as_ref().unwrap();
    assert_eq!(shot.source, ScreenshotSource::Missing);
    assert_eq!(shot.key, "dashboard");
    assert_eq!(
        shot.variant.as_deref(),
        Some("web"),
        "catalog default variant"
    );
    assert!(dev.free_aspect);
    let d = s
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::ScreenshotMissing)
        .unwrap();
    assert_eq!(d.level, Level::Warn);
    assert_eq!(d.element.as_ref().unwrap().as_str(), "web-browser");
}

#[test]
fn errors() {
    let p = sample();
    let files = assets();
    let o = ResolveOptions::new(&files);
    assert_eq!(
        resolve(&p, "nope", "en", &o).unwrap_err(),
        ResolveError::UnknownTarget("nope".into())
    );
    assert!(matches!(
        resolve(&p, "ios-phone-69", "a b", &o).unwrap_err(),
        ResolveError::InvalidLang(_)
    ));
}

#[test]
fn free_aspect_device_follows_the_screenshot() {
    let mut p = sample();
    let mut t = p.targets[0].clone();
    t.id = vitrine_core::model::TargetId::new("web");
    t.strip = vitrine_core::model::StripId::new("web");
    t.size = vitrine_core::model::PixelSize::new(2880, 1800);
    t.screenshots = None;
    t.gallery = None;
    t.store = None;
    p.targets.push(t);
    // 1440x1080 (4:3) instead of the catalog's 16:10 browser content.
    let files: std::collections::BTreeMap<String, (u32, u32)> =
        [("screenshots/en/web/dashboard.png".to_owned(), (1440, 1080))].into();
    let s = resolve(&p, "web", "en", &ResolveOptions::new(&files)).unwrap();
    let browser = s.element("web-browser").unwrap();
    let ResolvedKind::Device(dev) = &browser.props else {
        panic!()
    };
    assert!(dev.free_aspect);
    let shot = dev.screenshot.as_ref().unwrap();
    assert_eq!(
        shot.size.unwrap(),
        vitrine_core::model::PixelSize::new(1440, 1080)
    );
    // Display 1440 x 1080, chrome 88 above it: body 1440 x 1168.
    assert!(close(dev.device.display.h, 1080.0));
    assert!(close(dev.device.body.h, 1168.0));
    assert!(close(browser.size.x, 860.0));
    assert!(close(browser.size.y, 860.0 * 1168.0 / 1440.0));
    // Without dimensions the catalog aspect stays.
    let names: BTreeSet<String> = files.keys().cloned().collect();
    let s = resolve(&p, "web", "en", &ResolveOptions::new(&names)).unwrap();
    let browser = s.element("web-browser").unwrap();
    assert!(close(browser.size.y, 860.0 * 988.0 / 1440.0));
    // A device with a fixed aspect ignores the screenshot size.
    let ResolvedKind::Device(dev) = &s.element("web-laptop").unwrap().props else {
        panic!()
    };
    assert!(!dev.free_aspect);
}
