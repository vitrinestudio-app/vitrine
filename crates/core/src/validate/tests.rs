//! One small inline project per check.

use std::collections::BTreeMap;

use super::*;
use crate::model::{ImageFormat, PixelSize, StringKey};
use crate::resolve::NoAssets;

/// A two-page 1:2 strip with `elements` and one 1000x2000 target `t`
/// (extra target fields in `target`), languages en (default) + de.
fn project(elements: &str, target: &str) -> Project {
    let yaml = format!(
        "format: arrangia/1
revision: 1
name: t
languages:
  default: en
  available: [en, de]
strips:
- id: s
  page: {{aspect: 1000:2000, count: 2}}
  elements: {elements}
targets:
- id: t
  strip: s
  size: [1000, 2000]
{target}"
    );
    crate::io::project_from_yaml(&yaml).unwrap_or_else(|e| panic!("{e}\n{yaml}"))
}

fn set(p: &mut Project, lang: &str, key: &str, text: &str) {
    p.strings
        .entry(Lang::new(lang))
        .or_default()
        .insert(StringKey::new(key), text.to_owned());
}

fn run_with(p: &Project, assets: &dyn AssetLookup) -> ValidationReport {
    validate(p, &ValidateOptions::new(assets))
}

fn run(p: &Project) -> ValidationReport {
    run_with(p, &NoAssets)
}

fn codes(r: &ValidationReport) -> Vec<(Severity, FindingCode)> {
    r.findings.iter().map(|f| (f.level, f.code)).collect()
}

fn find(r: &ValidationReport, code: FindingCode) -> &Finding {
    r.with_code(code)
        .next()
        .unwrap_or_else(|| panic!("no {code} in {:#?}", r.findings))
}

fn has(r: &ValidationReport, code: FindingCode) -> bool {
    r.with_code(code).next().is_some()
}

fn shots(files: &[(&str, (u32, u32))]) -> BTreeMap<String, (u32, u32)> {
    files.iter().map(|(p, s)| ((*p).to_owned(), *s)).collect()
}

const HEADLINE: &str = "
  - id: title
    kind: text
    page: 0
    text: title
    box: {top: 100, left: 80, right: 80}";

#[test]
fn clean_project_has_no_findings() {
    let mut p = project(HEADLINE, "");
    set(&mut p, "en", "title", "Hello");
    set(&mut p, "de", "title", "Hallo");
    let r = run(&p);
    assert!(r.is_empty(), "{:#?}", r.findings);
    assert!(!r.has_errors());
    assert_eq!(r.max_level(), None);
}

#[test]
fn codes_are_stable_snake_case() {
    for c in FindingCode::ALL {
        let json = serde_json::to_string(c).unwrap();
        assert_eq!(json, format!("\"{}\"", c.as_str()));
    }
    assert_eq!(FindingCode::TextInGap.to_string(), "text_in_gap");
}

#[test]
fn unknown_references() {
    let elements = "
  - id: txt
    kind: text
    page: 0
    text: title
    style: nope
    color: \"@missing\"
    box: {top: 100, left: 80, right: 80}
  - id: phone
    kind: device
    page: 0
    at: {x: 500, y: 1000}
    width: 600
    model: iphone-island
    color: plaid
  - id: alien
    kind: device
    page: 0
    at: {x: 500, y: 1000}
    width: 600
    model: nokia-3310
  - id: lost
    kind: shape
    page: 7
    at: {x: 500, y: 500}
    width: 100
    height: 100
    shape: rect
    fill: \"#FF0000\"
  variants:
    v:
      overrides:
        ghost: {visible: false}";
    let target = "  variant: nope
  gallery: no-such-gallery
  screenshots: no-such-variant
  device_map: {iphone-notch: nope}
";
    let mut p = project(elements, target);
    set(&mut p, "en", "title", "Hi");
    set(&mut p, "de", "title", "Hi");
    let r = run(&p);
    let c = codes(&r);
    for want in [
        (Severity::Warn, FindingCode::UnknownTextStyle),
        (Severity::Error, FindingCode::UnknownColor),
        (Severity::Warn, FindingCode::UnknownDeviceColor),
        (Severity::Error, FindingCode::UnknownDevice),
        (Severity::Error, FindingCode::PageOutOfRange),
        (Severity::Warn, FindingCode::UnknownVariant),
        (Severity::Warn, FindingCode::UnknownGallery),
        (Severity::Info, FindingCode::UnknownScreenshotVariant),
    ] {
        assert!(c.contains(&want), "{want:?} missing in {c:?}");
    }
    // Language independent: reported once, without language, with page.
    let color = r.with_code(FindingCode::UnknownColor).collect::<Vec<_>>();
    assert_eq!(color.len(), 1);
    assert_eq!(color[0].lang, None);
    assert_eq!(color[0].element.as_ref().unwrap().as_str(), "txt");
    assert_eq!(find(&r, FindingCode::PageOutOfRange).page, Some(7));
    assert!(r.has_errors());
}

#[test]
fn override_for_unknown_element() {
    let elements = "[]
  variants:
    v:
      overrides:
        ghost: {visible: false}";
    let p = project(elements, "  variant: v\n");
    let r = run(&p);
    assert_eq!(
        find(&r, FindingCode::UnknownOverrideElement).level,
        Severity::Warn
    );
}

#[test]
fn unknown_strip_and_target_filter() {
    let mut p = project("[]", "");
    p.targets[0].strip = crate::model::StripId::new("gone");
    let r = run(&p);
    assert_eq!(codes(&r), [(Severity::Error, FindingCode::UnknownStrip)]);
    let mut o = ValidateOptions::new(&NoAssets);
    o.targets = Some(vec!["nope".into()]);
    let r = validate(&p, &o);
    assert_eq!(codes(&r), [(Severity::Error, FindingCode::UnknownTarget)]);
}

#[test]
fn language_filter() {
    let mut p = project(HEADLINE, "");
    set(&mut p, "en", "title", "Hello");
    let mut o = ValidateOptions::new(&NoAssets);
    o.langs = Some(vec!["en".into()]);
    assert!(validate(&p, &o).is_empty());
    o.langs = Some(vec!["de".into(), "fi".into()]);
    let r = validate(&p, &o);
    assert!(has(&r, FindingCode::TextFallback));
    assert_eq!(
        find(&r, FindingCode::LangNotAvailable).message,
        "language `fi` is not listed in `languages.available`"
    );
}

#[test]
fn text_overflow_per_language() {
    let elements = "
  - id: title
    kind: text
    page: 0
    text: title
    box: {top: 100, left: 80, right: 80}
    fit: {shrink_to: 40, max_lines: 1}";
    let mut p = project(elements, "");
    set(&mut p, "en", "title", "Short");
    set(
        &mut p,
        "de",
        "title",
        "Ein sehr viel längerer deutscher Titel, der niemals in eine einzige Zeile passt",
    );
    let r = run(&p);
    let f = find(&r, FindingCode::TextOverflow);
    assert_eq!(f.level, Severity::Error);
    assert_eq!(f.lang.as_ref().unwrap().as_str(), "de");
    assert_eq!(f.page, Some(0));
    assert_eq!(r.with_code(FindingCode::TextOverflow).count(), 1);
}

#[test]
fn emergency_break_without_hyphen() {
    let elements = "
  - id: narrow
    kind: text
    page: 0
    at: {x: 500, y: 500}
    width: 300
    text: word";
    let mut p = project(elements, "");
    for l in ["en", "de"] {
        set(&mut p, l, "word", "Donaudampfschifffahrtsgesellschaft");
    }
    let r = run(&p);
    let f = find(&r, FindingCode::EmergencyBreak);
    assert_eq!(f.level, Severity::Warn);
    assert!(f.message.contains("soft hyphen"), "{}", f.message);
    // With soft hyphens there is no emergency break.
    for l in ["en", "de"] {
        set(
            &mut p,
            l,
            "word",
            r"Donau\-dampf\-schiff\-fahrts\-gesell\-schaft",
        );
    }
    assert!(!has(&run(&p), FindingCode::EmergencyBreak));
}

#[test]
fn automatic_hyphenation_is_reported_as_info() {
    let elements = "
  - id: narrow
    kind: text
    page: 0
    at: {x: 500, y: 500}
    width: 300
    text: word
    hyphenate: true
    langs:
      ja: {font_size: 40}";
    let mut p = project(elements, "");
    for l in ["en", "de"] {
        set(&mut p, l, "word", "Donaudampfschifffahrtsgesellschaft");
    }
    let r = run(&p);
    let f = find(&r, FindingCode::Hyphenated);
    assert_eq!(f.level, Severity::Info);
    assert!(
        f.message.contains("Donaudampfschifffahrtsgesellschaft"),
        "{}",
        f.message
    );
    assert!(!has(&r, FindingCode::EmergencyBreak));
    // The override for a language outside `available` is reported.
    let f = find(&r, FindingCode::LangNotAvailable);
    assert!(f.message.contains("`ja`"), "{}", f.message);
}

#[test]
fn text_fallback_levels() {
    let mut p = project(HEADLINE, "");
    p.languages.available.push(Lang::new("de-AT"));
    set(&mut p, "en", "title", "Hello");
    let r = run(&p);
    // de falls back to the default language: not translated.
    let de: Vec<_> = r
        .with_code(FindingCode::TextFallback)
        .filter(|f| f.lang.as_ref().unwrap().as_str() == "de")
        .collect();
    assert_eq!(de[0].level, Severity::Warn);
    // de-AT → de is a deliberate prefix fallback.
    set(&mut p, "de", "title", "Hallo");
    let r = run(&p);
    let f = find(&r, FindingCode::TextFallback);
    assert_eq!(f.level, Severity::Info);
    assert_eq!(f.lang.as_ref().unwrap().as_str(), "de-AT");
}

#[test]
fn missing_text() {
    let mut p = project(HEADLINE, "");
    let r = run(&p);
    let f = find(&r, FindingCode::TextMissing);
    assert_eq!(f.level, Severity::Error);
    assert_eq!(f.message, "text `title` exists in no language");
    // Only in `de`: missing for `en` (whose chain is just `en`).
    set(&mut p, "de", "title", "Hallo");
    let r = run(&p);
    let f = find(&r, FindingCode::TextMissing);
    assert_eq!(f.lang.as_ref().unwrap().as_str(), "en");
    assert!(f.message.contains("missing in `en`"));
}

#[test]
fn markup_problems() {
    let mut p = project(HEADLINE, "");
    set(&mut p, "en", "title", "**bold");
    set(&mut p, "de", "title", "[nope]x[/]");
    let r = run(&p);
    let m = find(&r, FindingCode::InvalidMarkup);
    assert_eq!(
        (m.level, m.lang.as_ref().unwrap().as_str()),
        (Severity::Warn, "en")
    );
    let s = find(&r, FindingCode::UnknownSpanStyle);
    assert_eq!(s.lang.as_ref().unwrap().as_str(), "de");
}

#[test]
fn text_in_gap_and_clipped() {
    // A free text box centred on the right page edge: with `clip: none` it
    // reaches into the gallery gap, with the default `clip: page` it is cut.
    let el = |clip: &str| {
        format!(
            "
  - id: edge
    kind: text
    page: 0
    at: {{x: 1000, y: 500}}
    width: 400
    text: t{clip}"
        )
    };
    let gallery = "  gallery: appstore-iphone\n";
    let mut p = project(&el("\n    clip: none"), gallery);
    for l in ["en", "de"] {
        set(&mut p, l, "t", "Across the gap");
    }
    let r = run(&p);
    let f = find(&r, FindingCode::TextInGap);
    assert_eq!(f.level, Severity::Warn);
    assert!(!has(&r, FindingCode::TextClipped));
    let mut p2 = project(&el(""), gallery);
    p2.strings.clone_from(&p.strings);
    let r = run(&p2);
    assert!(has(&r, FindingCode::TextClipped));
    assert!(!has(&r, FindingCode::TextInGap));
    // Without a gallery there is no gap.
    let mut p3 = project(&el("\n    clip: none"), "");
    p3.strings.clone_from(&p.strings);
    assert!(!has(&run(&p3), FindingCode::TextInGap));
}

const PHONE: &str = "
  - id: phone
    kind: device
    page: 0
    at: {x: 500, y: 1000}
    width: 600
    model: iphone-island
    screenshot: home";

#[test]
fn missing_screenshot_depends_on_mode() {
    let p = project(PHONE, "  screenshots: ios-phone\n");
    let r = run(&p);
    let f = find(&r, FindingCode::ScreenshotMissing);
    assert_eq!(f.level, Severity::Error);
    assert_eq!(f.page, Some(0));
    let mut o = ValidateOptions::new(&NoAssets);
    o.mode = ValidateMode::Preview;
    let r = validate(&p, &o);
    assert_eq!(
        find(&r, FindingCode::ScreenshotMissing).level,
        Severity::Warn
    );
}

#[test]
fn screenshot_fallback_is_info() {
    let p = project(PHONE, "  screenshots: ios-phone\n");
    let files = shots(&[("screenshots/en/ios-phone/home.png", (1320, 2868))]);
    let r = run_with(&p, &files);
    let f = find(&r, FindingCode::ScreenshotFallback);
    assert_eq!(f.level, Severity::Info);
    assert_eq!(f.lang.as_ref().unwrap().as_str(), "de");
    assert!(f.message.contains("default `en` screenshot for `home`"));
    assert_eq!(
        codes(&r),
        [(Severity::Info, FindingCode::ScreenshotFallback)]
    );
}

#[test]
fn screenshot_aspect_mismatch() {
    let p = project(PHONE, "  screenshots: ios-phone\n");
    let files = shots(&[
        ("screenshots/en/ios-phone/home.png", (1080, 1920)),
        ("screenshots/de/ios-phone/home.png", (1080, 1920)),
    ]);
    let r = run_with(&p, &files);
    let f: Vec<_> = r.with_code(FindingCode::ScreenshotAspect).collect();
    assert_eq!(f.len(), 2, "one per file");
    assert!(f[0].message.contains("wrong device?"));
    // Within 2 %: fine (1206x2622 on the 1320:2868 display).
    let files = shots(&[
        ("screenshots/en/ios-phone/home.png", (1206, 2622)),
        ("screenshots/de/ios-phone/home.png", (1206, 2622)),
    ]);
    assert!(run_with(&p, &files).is_empty());
}

#[test]
fn screenshot_upscale() {
    let p = project(PHONE, "  screenshots: ios-phone\n");
    // The display is ~560 px wide at 1 px/lu.
    let small = shots(&[
        ("screenshots/en/ios-phone/home.png", (330, 717)),
        ("screenshots/de/ios-phone/home.png", (330, 717)),
    ]);
    let r = run_with(&p, &small);
    assert_eq!(
        find(&r, FindingCode::ScreenshotUpscale).level,
        Severity::Warn
    );
    assert!(!has(&r, FindingCode::ScreenshotAspect));
    let big = shots(&[
        ("screenshots/en/ios-phone/home.png", (660, 1434)),
        ("screenshots/de/ios-phone/home.png", (660, 1434)),
    ]);
    assert!(!has(&run_with(&p, &big), FindingCode::ScreenshotUpscale));
    // Threshold is configurable.
    let mut o = ValidateOptions::new(&small);
    o.thresholds.max_upscale = 3.0;
    assert!(!has(&validate(&p, &o), FindingCode::ScreenshotUpscale));
}

#[test]
fn free_aspect_device_has_no_aspect_warning() {
    let elements = "
  - id: web
    kind: device
    page: 0
    at: {x: 500, y: 500}
    width: 900
    model: browser
    screenshot: dash";
    let p = project(elements, "");
    let files = shots(&[
        ("screenshots/en/web/dash.png", (1600, 1600)),
        ("screenshots/de/web/dash.png", (1600, 1600)),
    ]);
    let r = run_with(&p, &files);
    assert!(r.is_empty(), "{:#?}", r.findings);
}

#[test]
fn browser_chrome_on_laptop_shrinks_the_expected_aspect() {
    let elements = "
  - id: laptop
    kind: device
    page: 0
    at: {x: 500, y: 1000}
    width: 900
    model: macbook
    chrome: browser
    screenshot: site";
    let p = project(elements, "  screenshots: web\n");
    // The 16:10 display minus the browser bars is about 16:9.
    let wide = shots(&[
        ("screenshots/en/web/site.png", (1600, 900)),
        ("screenshots/de/web/site.png", (1600, 900)),
    ]);
    let r = run_with(&p, &wide);
    assert!(!has(&r, FindingCode::ScreenshotAspect), "{:#?}", r.findings);
    let full = shots(&[
        ("screenshots/en/web/site.png", (1600, 1000)),
        ("screenshots/de/web/site.png", (1600, 1000)),
    ]);
    assert!(has(&run_with(&p, &full), FindingCode::ScreenshotAspect));
    // Without chrome the full 16:10 display is expected.
    let p = project(
        &elements.replace("    chrome: browser\n", ""),
        "  screenshots: web\n",
    );
    assert!(has(&run_with(&p, &wide), FindingCode::ScreenshotAspect));
    assert!(!has(&run_with(&p, &full), FindingCode::ScreenshotAspect));
}

#[test]
fn store_size_and_count() {
    // 1000x2000 is not an App Store size.
    let p = project("[]", "  store: {apple: {display_type: APP_IPHONE_61}}\n");
    let r = run(&p);
    let f = find(&r, FindingCode::StoreSize);
    assert_eq!(f.level, Severity::Error);
    assert!(f.message.contains("1206x2622"), "{}", f.message);
    // Rejected by the ASC uploader although listed on Apple's help page.
    let mut p = project("[]", "  store: {apple: {display_type: APP_IPHONE_67}}\n");
    p.targets[0].size = PixelSize::new(1260, 2736);
    assert!(has(&run(&p), FindingCode::StoreSize));
    p.targets[0].size = PixelSize::new(1320, 2868);
    assert!(!has(&run(&p), FindingCode::StoreSize));
    // More than 10 images.
    p.strips[0].page.count = 11;
    let r = run(&p);
    let f = find(&r, FindingCode::StoreCount);
    assert_eq!(f.level, Severity::Error);
    // Google: long side at most 2x the short side; fewer than 2 images.
    let mut p = project("[]", "  store: {google: {image_type: phoneScreenshots}}\n");
    p.targets[0].size = PixelSize::new(1080, 2400);
    p.strips[0].page.count = 1;
    let r = run(&p);
    assert!(find(&r, FindingCode::StoreSize).message.contains("2x"));
    assert_eq!(find(&r, FindingCode::StoreCount).level, Severity::Warn);
}

#[test]
fn feature_graphic_size() {
    let mut p = project(
        "[]",
        "  kind: banner\n  store: {google: {image_type: featureGraphic}}\n",
    );
    p.targets[0].size = PixelSize::new(1000, 500);
    assert!(has(&run(&p), FindingCode::StoreSize));
    p.targets[0].size = PixelSize::new(1024, 500);
    let r = run(&p);
    assert!(!has(&r, FindingCode::StoreSize));
    assert!(!has(&r, FindingCode::StoreCount), "a banner is one image");
}

#[test]
fn store_format_and_slots() {
    let mut catalog = crate::catalog::catalog().clone();
    catalog
        .targets
        .rules
        .get_mut("apple-screenshots")
        .unwrap()
        .formats = vec![ImageFormat::Png];
    let mut p = project("[]", "  store: {apple: {display_type: APP_IPHONE_67}}\n");
    p.targets[0].size = PixelSize::new(1320, 2868);
    p.targets[0].format = Some(ImageFormat::Jpeg);
    let mut o = ValidateOptions::new(&NoAssets);
    o.catalog = &catalog;
    assert_eq!(
        codes(&validate(&p, &o)),
        [(Severity::Error, FindingCode::StoreFormat)]
    );
    // Unknown and unverified slots.
    let p = project("[]", "  store: {apple: {display_type: APP_IPHONE_99}}\n");
    assert_eq!(
        codes(&run(&p)),
        [(Severity::Warn, FindingCode::UnknownStoreSlot)]
    );
    let mut p = project("[]", "  store: {apple: {display_type: APP_IPHONE_DUO}}\n");
    p.targets[0].size = PixelSize::new(1398, 2034);
    let r = run(&p);
    assert_eq!(
        find(&r, FindingCode::UnverifiedStoreSlot).level,
        Severity::Info
    );
    assert!(!has(&r, FindingCode::UnknownStoreSlot));
    assert!(!has(&r, FindingCode::StoreSize));
}

#[test]
fn aspect_deviation_suggests_a_variant() {
    let mut p = project("[]", "");
    p.targets[0].size = PixelSize::new(1000, 1500);
    let r = run(&p);
    let f = find(&r, FindingCode::VariantSuggested);
    assert_eq!(f.level, Severity::Info);
    assert!(f.message.contains("-25 %"), "{}", f.message);
    // Within the threshold (1:2 → 1080:2160 is equal; 9:16 is −11 %).
    p.targets[0].size = PixelSize::new(1080, 1920);
    assert!(run(&p).is_empty());
    // A variant silences it.
    let mut p = project("[]\n  variants:\n    tablet: {}", "  variant: tablet\n");
    p.targets[0].size = PixelSize::new(1000, 1500);
    assert!(run(&p).is_empty());
}

#[test]
fn elements_outside_every_page() {
    let elements = "
  - id: lost
    kind: shape
    page: 0
    at: {x: -900, y: 500}
    width: 200
    height: 200
    shape: rect
    fill: \"#FF0000\"
  - id: across
    kind: shape
    page: 0
    at: {x: 1000, y: 500}
    width: 200
    height: 200
    shape: rect
    fill: \"#FF0000\"";
    let p = project(elements, "  gallery: appstore-iphone\n");
    let r = run(&p);
    let f: Vec<_> = r.with_code(FindingCode::ElementOutside).collect();
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].element.as_ref().unwrap().as_str(), "lost");
    assert_eq!(f[0].lang, None);
    // A device overlapping a gap is the point of gaps: no finding.
    let p = project(
        "
  - id: phone
    kind: device
    page: 0
    at: {x: 1000, y: 1000}
    width: 600
    model: iphone-island",
        "  gallery: appstore-iphone\n",
    );
    assert!(run(&p).is_empty());
}

#[test]
fn report_serializes() {
    let p = project(HEADLINE, "");
    let r = run(&p);
    assert_eq!(r.count(Severity::Error), 2, "missing in en and de");
    let json = serde_json::to_value(&r).unwrap();
    assert_eq!(json["findings"][0]["code"], "text_missing");
    assert_eq!(json["findings"][0]["level"], "error");
    let back: ValidationReport = serde_json::from_value(json).unwrap();
    assert_eq!(back, r);
}
