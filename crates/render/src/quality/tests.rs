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
    project_with(background, elements, "")
}

/// [`project`] with extra top-level YAML (`styles:` …) before the canvases.
fn project_with(background: &str, elements: &str, top: &str) -> Project {
    let yaml = format!(
        "format: vitrine/1
revision: 1
name: t
languages:
  default: en
  available: [en]
{top}canvases:
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

/// A text rotated like a trading-card name: a black frame, a coloured card
/// on it and a dark name near the card's top edge, all turned by `rot`
/// around the card centre; `wbox` is the text box width.
fn card_name(rot: f64, wbox: f64, y: f64) -> String {
    let py = 600.0 - y;
    format!(
        "
  - id: frame
    kind: shape
    page: 0
    at: {{x: 500, y: 600}}
    width: 900
    height: 600
    rotation: {rot}
    shape: rect
    fill: \"#000000\"
  - id: card
    kind: shape
    page: 0
    at: {{x: 500, y: 600}}
    width: 760
    height: 360
    rotation: {rot}
    shape: rect
    fill: \"#E8B100\"
  - id: title
    kind: text
    page: 0
    at: {{x: 500, y: {y}}}
    width: {wbox}
    rotation: {rot}
    pivot: {{x: 0, y: {py}}}
    align: center
    text: title
    color: \"#1A1A1A\"
    font_size: 90"
    )
}

/// Worst contrast ratio `low_contrast` measures for text `title` (the
/// thresholds are raised so every text reports its ratio).
fn measured_ratio(p: &Project) -> f64 {
    let th = Thresholds {
        min_contrast: 30.0,
        min_contrast_large: 30.0,
        ..Thresholds::default()
    };
    let ctx = ctx();
    let scene = resolve(p, "t", "en", &ctx.resolve_options(&NoAssets)).expect("resolve");
    let found = Inspector::new(&ctx).inspect(&scene, &th, true);
    let f = with(&found, FindingCode::LowContrast);
    let m = &f.first().expect("low_contrast").message;
    m.split("contrast of ")
        .nth(1)
        .and_then(|r| r.split(':').next())
        .and_then(|r| r.parse().ok())
        .unwrap_or_else(|| panic!("{m}"))
}

#[test]
fn low_contrast_of_rotated_text_measures_only_around_the_glyphs() {
    // Regression (MCP test round 2, trading-card): rotated names over a
    // card next to a black frame. The ratio must not depend on the
    // rotation or on how far the (empty) text box reaches over the frame.
    for wbox in [740.0, 900.0] {
        let straight = measured_ratio(&project("#FFFFFF", &card_name(0.0, wbox, 520.0)));
        assert!(straight > 7.0, "{straight}");
        for rot in [-7.0, 3.0, 7.0] {
            let r = measured_ratio(&project("#FFFFFF", &card_name(rot, wbox, 520.0)));
            assert!(
                (r - straight).abs() < 0.3,
                "{rot}° box {wbox}: {r} vs {straight}"
            );
        }
        // With the default thresholds: no finding.
        let found = inspect(&project("#FFFFFF", &card_name(7.0, wbox, 520.0)));
        assert!(
            with(&found, FindingCode::LowContrast).is_empty(),
            "{found:#?}"
        );
    }
    // A wrapped second line moves the glyphs onto the frame: a real finding
    // that names the element behind them.
    let found = inspect(&project("#FFFFFF", &card_name(7.0, 600.0, 462.0)));
    let low = with(&found, FindingCode::LowContrast);
    assert_eq!(low.len(), 1, "{found:#?}");
    assert!(
        low[0].message.contains("mostly `frame`"),
        "{}",
        low[0].message
    );
}

#[test]
fn low_contrast_names_the_texts_own_shadow() {
    // A hard red shadow behind red-ish text on white: the worst pixels are
    // the text's own shadow.
    let elements = "
  - id: title
    kind: text
    page: 0
    at: {x: 500, y: 300}
    width: 800
    text: title
    color: \"#E2384A\"
    font_size: 90
    shadow: hard";
    let found = inspect(&project_with(
        "#FFFFFF",
        elements,
        "styles:\n  shadows:\n    hard:\n      layers:\n      - {color: \"#C8303F\", x: 8, y: 8, blur: 0}\n",
    ));
    let low = with(&found, FindingCode::LowContrast);
    assert_eq!(low.len(), 1, "{found:#?}");
    assert!(low[0].message.contains("own shadow"), "{}", low[0].message);
}

#[test]
fn device_cut_at_the_side_edges_only() {
    let device = |x: f64, y: f64, rot: f64, clip: &str| {
        format!(
            "
  - id: phone
    kind: device
    page: 0
    at: {{x: {x}, y: {y}}}
    width: 560
    rotation: {rot}
    model: iphone-island{clip}"
        )
    };
    let cut = |x, y, rot, clip| {
        let found = inspect(&project("#FFFFFF", &device(x, y, rot, clip)));
        with(&found, FindingCode::DeviceCut)
            .into_iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    // Straight and inside: fine; tilted -12° near the left canvas edge: the
    // rotated corner sticks out (half rotated width ≈ 274 + 121 lu).
    let f = cut(420.0, 1000.0, 0.0, "");
    assert!(f.is_empty(), "{f:#?}");
    let f = cut(330.0, 1000.0, -12.0, "");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert_eq!(f[0].level, Severity::Info);
    assert!(
        f[0].message.contains("left edge of the canvas"),
        "{}",
        f[0].message
    );
    // Right edge of page 1 with `clip: page`: cut; without clip the device
    // runs on into page 2 (not cut).
    let f = cut(800.0, 1000.0, 10.0, "\n    clip: page");
    assert_eq!(f.len(), 1, "{f:#?}");
    assert!(
        f[0].message.contains("right edge of page 1"),
        "{}",
        f[0].message
    );
    let f = cut(800.0, 1000.0, 10.0, "");
    assert!(f.is_empty(), "{f:#?}");
    // Cut at the bottom (deliberate crop): not reported.
    let f = cut(500.0, 1900.0, 0.0, "\n    clip: page");
    assert!(f.is_empty(), "{f:#?}");
    // Side buttons only: below the 8 lu threshold.
    let f = cut(283.0, 1000.0, 0.0, "");
    assert!(f.is_empty(), "{f:#?}");
}

#[test]
fn screen_covered_below_the_top_bar() {
    // A card over the middle of the screen (~20 % of it), and a small
    // badge (< 15 %).
    let elements = "
  - id: card
    kind: shape
    page: 0
    at: {x: 500, y: 1200}
    width: 500
    height: 230
    shape: rect
    fill: \"#222222\"
  - id: badge
    kind: shape
    page: 0
    at: {x: 500, y: 1550}
    width: 120
    height: 120
    shape: ellipse
    fill: \"#CC3333\"";
    let found = inspect(&project("#FFFFFF", &(PHONE.to_owned() + elements)));
    let f = with(&found, FindingCode::ScreenCovered);
    assert_eq!(f.len(), 1, "{found:#?}");
    assert_eq!(
        f[0].element
            .as_ref()
            .map(vitrine_core::model::ElementId::as_str),
        Some("card")
    );
    assert!(
        with(&found, FindingCode::ScreenHeaderCovered).is_empty(),
        "{found:#?}"
    );
    // A higher project threshold keeps it quiet.
    let p = project("#FFFFFF", &(PHONE.to_owned() + elements));
    let ctx = ctx();
    let scene = resolve(&p, "t", "en", &ctx.resolve_options(&NoAssets)).expect("resolve");
    let th = Thresholds {
        screen_cover: 0.5,
        ..Thresholds::default()
    };
    let found = Inspector::new(&ctx).inspect(&scene, &th, true);
    assert!(
        with(&found, FindingCode::ScreenCovered).is_empty(),
        "{found:#?}"
    );
}

#[test]
fn element_in_corner_ignores_corners_hidden_by_an_overlay() {
    // A square tile in the top-left corner, hidden by an opaque rounded
    // panel drawn above it.
    let tile = "
  - id: tile
    kind: shape
    page: 0
    at: {x: 110, y: 110}
    width: 200
    height: 200
    shape: rect
    fill: \"#33AA55\"";
    let found = inspect(&project("#FFFFFF", tile));
    assert_eq!(with(&found, FindingCode::ElementInCorner).len(), 1);
    let overlay = "
  - id: panel
    kind: shape
    page: 0
    at: {x: 250, y: 250}
    width: 500
    height: 500
    shape: rect
    fill: \"#FFFFFF\"";
    let found = inspect(&project("#FFFFFF", &(tile.to_owned() + overlay)));
    let corner = with(&found, FindingCode::ElementInCorner);
    assert!(
        corner
            .iter()
            .all(|f| f.element.as_ref().is_none_or(|e| e.as_str() != "tile")),
        "{found:#?}"
    );
}

#[test]
fn screen_header_cut_names_the_cause() {
    let phone = "
  - id: phone
    kind: device
    page: 0
    at: {x: 500, y: 520}
    width: 560
    model: iphone-island";
    let found = inspect(&project("#FFFFFF", phone));
    let f = with(&found, FindingCode::ScreenHeaderCut);
    assert_eq!(f.len(), 1, "{found:#?}");
    assert!(
        f[0].message.starts_with("the top edge of page 1 cuts"),
        "{}",
        f[0].message
    );
}
