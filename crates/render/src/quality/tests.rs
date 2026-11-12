//! Tests of the pixel design checks on small inline projects (bundled
//! fonts, parametric frames, no assets).

use vitrine_core::Project;
use vitrine_core::layout::FontLibrary;
use vitrine_core::model::{Lang, StringKey};
use vitrine_core::resolve::{NoAssets, resolve};
use vitrine_core::validate::{
    FindingCode, SceneFinding, SceneInspector, Severity, Thresholds, ValidateOptions, validate,
};

use super::{Inspector, scene_report};
use crate::scene::RenderContext;

/// Two 1000x2000 pages with a canvas background, `elements` and one
/// target `t` with the App Store iPhone gallery.
fn project(background: &str, elements: &str) -> Project {
    let yaml = format!(
        "format: vitrine/1
revision: 1
name: t
languages:
  default: en
  available: [en]
canvases:
- id: s
  page: {{aspect: 1000:2000, count: 2}}
  backgrounds: [\"{background}\"]
  elements: {elements}
targets:
- id: t
  canvas: s
  size: [1000, 2000]
  gallery: appstore-iphone
"
    );
    let mut p = vitrine_core::io::project_from_yaml(&yaml).unwrap_or_else(|e| panic!("{e}"));
    p.strings
        .entry(Lang::new("en"))
        .or_default()
        .insert(StringKey::new("title"), "Plan your week".into());
    p
}

fn ctx() -> RenderContext {
    RenderContext::with_fonts(std::env::temp_dir(), FontLibrary::bundled())
}

fn inspect(p: &Project) -> Vec<SceneFinding> {
    let ctx = ctx();
    let scene = resolve(p, "t", "en", &ctx.resolve_options(&NoAssets)).expect("resolve");
    Inspector::new(&ctx).inspect(&scene, &Thresholds::default(), true)
}

fn with(found: &[SceneFinding], code: FindingCode) -> Vec<&SceneFinding> {
    found.iter().filter(|f| f.code == code).collect()
}

fn text(color: &str, at: (f64, f64)) -> String {
    format!(
        "
  - id: title
    kind: text
    page: 0
    at: {{x: {}, y: {}}}
    width: 600
    text: title
    color: \"{color}\"
    font_size: 60",
        at.0, at.1
    )
}

const PHONE: &str = "
  - id: phone
    kind: device
    page: 0
    at: {x: 500, y: 1200}
    width: 560
    model: iphone-island";

#[test]
fn low_contrast_is_measured_against_the_rendered_background() {
    // #777777 on white: 4.48:1, below 4.5 for 60 lu (normal) text.
    let found = inspect(&project("#FFFFFF", &text("#777777", (500.0, 300.0))));
    let low = with(&found, FindingCode::LowContrast);
    assert_eq!(low.len(), 1, "{found:#?}");
    assert_eq!(low[0].level, Severity::Warn);
    assert!(low[0].message.contains("4.5:1"), "{}", low[0].message);
    // Black on white is fine.
    let found = inspect(&project("#FFFFFF", &text("#111111", (500.0, 300.0))));
    assert!(
        with(&found, FindingCode::LowContrast).is_empty(),
        "{found:#?}"
    );
}

#[test]
fn low_contrast_sees_a_light_spot_behind_white_text() {
    // White text on a dark page, but over a light patch.
    let elements = format!(
        "
  - id: spot
    kind: shape
    page: 0
    at: {{x: 500, y: 300}}
    width: 900
    height: 200
    shape: rect
    fill: \"#E8E8E8\"{}",
        text("#FFFFFF", (500.0, 300.0))
    );
    let found = inspect(&project("#101820", &elements));
    let low = with(&found, FindingCode::LowContrast);
    assert_eq!(low.len(), 1, "{found:#?}");
    assert!(low[0].message.contains("1.2:1"), "{}", low[0].message);
    let alone = inspect(&project("#101820", &text("#FFFFFF", (500.0, 300.0))));
    assert_eq!(with(&alone, FindingCode::LowContrast).len(), 0);
}

#[test]
fn text_near_and_on_a_device() {
    // Device top edge at about y 1200 - 1160/2 = 620; the text ends near y 330
    // (far), about 610 (near) or crosses the frame (630).
    let near = |y: f64| inspect(&project("#FFFFFF", &(text("#111111", (500.0, y)) + PHONE)));
    assert_eq!(with(&near(300.0), FindingCode::TextNearDevice).len(), 0);
    let close = near(588.0);
    let f = with(&close, FindingCode::TextNearDevice);
    assert_eq!(f.len(), 1, "{close:#?}");
    assert!(f[0].message.contains("within"), "{}", f[0].message);
    assert_eq!(f[0].level, Severity::Info);
    let crossing = near(640.0);
    let f = with(&crossing, FindingCode::TextNearDevice);
    assert_eq!(f.len(), 1, "{crossing:#?}");
    assert_eq!(f[0].level, Severity::Warn);
}

#[test]
fn text_on_a_card_over_the_device_is_fine() {
    let card = "
  - id: card
    kind: shape
    page: 0
    at: {x: 500, y: 640}
    width: 800
    height: 200
    radius: 30
    shape: rect
    fill: \"#FFFFFF\"";
    let found = inspect(&project(
        "#DDE3EA",
        &(PHONE.to_owned() + card + &text("#111111", (500.0, 640.0))),
    ));
    assert!(
        with(&found, FindingCode::TextNearDevice).is_empty(),
        "{found:#?}"
    );
}

#[test]
fn screen_header_cut_and_covered() {
    // A phone pushed up so the page top cuts through its status bar.
    let phone = "
  - id: phone
    kind: device
    page: 0
    at: {x: 500, y: 520}
    width: 560
    model: iphone-island";
    let found = inspect(&project("#FFFFFF", phone));
    assert_eq!(
        with(&found, FindingCode::ScreenHeaderCut).len(),
        1,
        "{found:#?}"
    );
    // A card over the top of the screen.
    let card = "
  - id: card
    kind: shape
    page: 0
    at: {x: 500, y: 720}
    width: 600
    height: 140
    shape: rect
    fill: \"#222222\"";
    let found = inspect(&project("#FFFFFF", &(PHONE.to_owned() + card)));
    let f = with(&found, FindingCode::ScreenHeaderCovered);
    assert_eq!(f.len(), 1, "{found:#?}");
    assert_eq!(
        f[0].element
            .as_ref()
            .map(vitrine_core::model::ElementId::as_str),
        Some("card")
    );
}

#[test]
fn seam_overlap_corner_and_store_blend() {
    // A card on page 0 reaching across the seam over a badge of page 1;
    // a square tile in the top-left corner; white pages.
    let elements = "
  - id: badge
    kind: shape
    page: 1
    at: {x: 80, y: 1000}
    width: 200
    height: 200
    shape: rect
    fill: \"#3366CC\"
  - id: card
    kind: shape
    page: 0
    at: {x: 1000, y: 1000}
    width: 400
    height: 300
    shape: rect
    fill: \"#CC3333\"
  - id: tile
    kind: shape
    page: 0
    at: {x: 110, y: 110}
    width: 200
    height: 200
    shape: rect
    fill: \"#33AA55\"";
    let found = inspect(&project("#FFFFFF", elements));
    let seam = with(&found, FindingCode::SeamOverlap);
    assert_eq!(seam.len(), 1, "{found:#?}");
    assert_eq!(
        seam[0]
            .element
            .as_ref()
            .map(vitrine_core::model::ElementId::as_str),
        Some("card")
    );
    let corner = with(&found, FindingCode::ElementInCorner);
    assert_eq!(corner.len(), 1, "{found:#?}");
    assert!(corner[0].message.contains("image 1 top-left"));
    let blend = with(&found, FindingCode::PageBlendsWithStore);
    assert_eq!(blend.len(), 1, "{found:#?}");
    assert!(blend[0].message.contains("light"));
}

#[test]
fn validate_runs_the_inspector_and_honours_allow() {
    let mut p = project("#FFFFFF", &text("#777777", (500.0, 300.0)));
    let ctx = ctx();
    let inspector = Inspector::new(&ctx);
    let mut o = ValidateOptions::new(&NoAssets);
    o.inspector = Some(&inspector);
    let r = validate(&p, &o);
    let f = r
        .with_code(FindingCode::LowContrast)
        .next()
        .expect("low_contrast");
    assert_eq!(
        f.target.as_ref().map(vitrine_core::model::TargetId::as_str),
        Some("t")
    );
    assert_eq!(f.lang.as_ref().map(Lang::as_str), Some("en"));
    p.canvases[0].elements[0].allow = vec!["low_contrast".into()];
    let r = validate(&p, &o);
    assert!(r.with_code(FindingCode::LowContrast).next().is_none());
}

#[test]
fn report_has_screens_gaps_and_pixel_overlaps() {
    let p = project(
        "#FFFFFF",
        &(PHONE.to_owned() + &text("#111111", (500.0, 900.0))),
    );
    let ctx = ctx();
    let scene = resolve(&p, "t", "en", &ctx.resolve_options(&NoAssets)).expect("resolve");
    let r = scene_report(&ctx, &scene, None);
    let g = r.gallery.as_ref().expect("gallery");
    assert!(g.gap_lu > 0.0 && g.radius_lu.x > 0.0);
    assert_eq!(g.measured, Some("no"));
    assert_eq!(r.seams.len(), 1);
    assert_eq!(r.seams[0].after_page, 0);
    let d = &r.devices[0];
    // The screen lies inside the frame and is narrower.
    assert!(d.screen.w < 560.0 && d.screen.w > 450.0, "{d:#?}");
    assert!(d.screen.x > 220.0 && d.screen.y > 620.0, "{d:#?}");
    assert!(d.screen.header > 0.0);
    // The text lies over the screen: covered_by and an overlap.
    assert_eq!(d.covered_by[0].id.as_str(), "title");
    let o = &r.overlaps[0];
    assert_eq!(
        o.text.as_ref().map(vitrine_core::model::ElementId::as_str),
        Some("title")
    );
    assert_eq!(o.other.as_str(), "phone");
    assert!(o.share_of_text.unwrap_or(0.0) > 0.9, "{o:#?}");
}
