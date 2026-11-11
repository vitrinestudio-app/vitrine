//! `core::validate` on the sample project in `tests/data/sample`.

use std::collections::BTreeMap;
use std::path::Path;

use vitrine_core::Project;
use vitrine_core::validate::{
    FindingCode, Severity, ValidateMode, ValidateOptions, ValidationReport, validate,
};

fn sample() -> Project {
    Project::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sample")).unwrap()
}

/// Screenshots with pixel sizes: a matching iPhone screenshot, an Android
/// one in the wrong aspect, a small iPad one (upscaled) and a browser
/// screenshot (free aspect).
fn assets() -> BTreeMap<String, (u32, u32)> {
    [
        ("screenshots/en/ios-phone/home.png", (1320, 2868)),
        ("screenshots/de/ios-phone/home.png", (1320, 2868)),
        ("screenshots/en/ios-tablet/home.jpg", (600, 800)),
        ("screenshots/en/android-phone/home.png", (1080, 1920)),
        ("screenshots/en/web/dashboard.png", (1440, 1000)),
    ]
    .into_iter()
    .map(|(p, s)| (p.to_owned(), s))
    .collect()
}

fn run(project: &Project) -> ValidationReport {
    let files = assets();
    validate(project, &ValidateOptions::new(&files))
}

#[test]
fn sample_report() {
    let report = run(&sample());
    insta::assert_yaml_snapshot!("sample_report", report);
}

#[test]
fn sample_findings() {
    let p = sample();
    let report = run(&p);
    // No errors with all screenshots present (fallbacks are fine).
    assert!(!report.has_errors(), "{:#?}", report.findings);
    assert_eq!(report.max_level(), Some(Severity::Warn));
    // `fr` and `es` have no translations: warnings per language.
    let fallback: Vec<_> = report.with_code(FindingCode::TextFallback).collect();
    assert!(fallback.iter().all(|f| f.level == Severity::Warn));
    assert!(
        fallback
            .iter()
            .any(|f| f.lang.as_ref().unwrap().as_str() == "fr")
    );
    // The Android screenshot has the wrong aspect, the iPad one is too small.
    assert_eq!(report.with_code(FindingCode::ScreenshotAspect).count(), 1);
    assert_eq!(report.with_code(FindingCode::ScreenshotUpscale).count(), 1);
    // Without screenshot files every device is an error for export …
    let none = std::collections::BTreeSet::<String>::new();
    let report = validate(&p, &ValidateOptions::new(&none));
    assert!(report.has_errors());
    // … but only a warning while editing.
    let mut o = ValidateOptions::new(&none);
    o.mode = ValidateMode::Preview;
    o.targets = Some(vec!["ios-phone-69".into()]);
    o.langs = Some(vec!["en".into()]);
    let report = validate(&p, &o);
    assert!(!report.has_errors());
    let missing = report
        .with_code(FindingCode::ScreenshotMissing)
        .next()
        .unwrap();
    assert_eq!(missing.target.as_ref().unwrap().as_str(), "ios-phone-69");
    assert_eq!(missing.element.as_ref().unwrap().as_str(), "hero-phone");
}
