//! Export layouts (standard, fastlane), stale-file removal and the
//! `index.html` overview.

use std::path::Path;

use vitrine_core::Project;
use vitrine_core::io::project_from_yaml;
use vitrine_core::layout::FontLibrary;
use vitrine_render::export::{ExportLayout, ExportOptions, FileKind, export};
use vitrine_render::scene::RenderContext;

const PROJECT: &str = r##"
format: vitrine/1
name: Layout <Test>
languages: {default: en, available: [en, de]}
canvases:
- id: s
  page: {aspect: 1320:2868, count: 2, ids: [hero, plan]}
  backgrounds: ["#336699"]
targets:
- id: phone
  canvas: s
  size: [1320, 2868]
  store: {apple: {display_type: APP_IPHONE_67}}
- id: phone-old
  canvas: s
  size: [1290, 2796]
  store: {apple: {display_type: APP_IPHONE_67}}
  langs: [en]
- id: play
  canvas: s
  size: [1080, 2160]
  store: {google: {image_type: phoneScreenshots}}
- id: fg
  kind: banner
  canvas: s
  size: [1024, 500]
  format: jpeg
  store: {google: {image_type: featureGraphic}}
  langs: [en]
- id: web
  canvas: s
  size: [660, 1434]
  langs: [en]
"##;

fn project() -> Project {
    project_from_yaml(PROJECT).unwrap()
}

fn ctx(root: &Path) -> RenderContext {
    RenderContext::with_fonts(root, FontLibrary::bundled())
}

#[test]
fn fastlane_layout_places_files_for_deliver_and_supply() {
    let dir = tempfile::tempdir().unwrap();
    let p = project();
    let out = dir.path().join("fastlane");
    let report = export(
        &p,
        &ctx(dir.path()),
        &ExportOptions {
            out_dir: Some(out.clone()),
            layout: ExportLayout::Fastlane,
            html: true,
            ..ExportOptions::default()
        },
    );
    let paths: Vec<&str> = report.files.iter().map(|f| f.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "metadata/android/de-DE/images/phoneScreenshots/01_de-DE.png",
            "metadata/android/de-DE/images/phoneScreenshots/02_de-DE.png",
            "metadata/android/en-US/images/featureGraphic.jpg",
            "metadata/android/en-US/images/phoneScreenshots/01_en-US.png",
            "metadata/android/en-US/images/phoneScreenshots/02_en-US.png",
            "screenshots/de-DE/01_phone.png",
            "screenshots/de-DE/02_phone.png",
            "screenshots/en-US/01_phone.png",
            "screenshots/en-US/02_phone.png",
        ]
    );
    assert!(report.files.iter().all(|f| f.kind != FileKind::Gallery));
    assert_eq!(
        report.files[0].locale.as_deref(),
        Some("de-DE"),
        "{:?}",
        report.files[0]
    );
    // phone-old is 6.9" too: deliver would mix it into APP_IPHONE_67.
    assert_eq!(report.errors.len(), 1, "{:?}", report.errors);
    assert!(
        report.errors[0].contains("phone-old"),
        "{:?}",
        report.errors
    );
    assert!(
        report
            .notes
            .iter()
            .any(|n| n.target == "web" && n.message.contains("no store slot")),
        "{:?}",
        report.notes
    );
    assert!(!out.join("screenshots/en-US/01_phone-old.png").exists());

    // The overview lists every file with relative paths, nothing external.
    let html = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert_eq!(report.index.as_deref(), Some("index.html"));
    for p in &paths {
        assert!(html.contains(&format!("src=\"{p}\"")), "{p} missing");
    }
    assert!(html.contains("Layout &lt;Test&gt;"));
    assert!(html.contains("1320×2868"));
    assert!(html.contains("prefers-color-scheme: dark"));
    assert!(!html.contains("http://") && !html.contains("https://"));
    assert!(html.find("id=\"lang-en\"") < html.find("id=\"lang-de\""));
}

#[test]
fn reexport_removes_stale_files_and_index_lists_all_targets() {
    let dir = tempfile::tempdir().unwrap();
    let mut p = project();
    let out = dir.path().join("export");
    let ctx = ctx(dir.path());
    let opts = |targets: &[&str]| ExportOptions {
        out_dir: Some(out.clone()),
        targets: targets.iter().map(|t| (*t).to_owned()).collect(),
        langs: vec!["en".into()],
        html: true,
        ..ExportOptions::default()
    };
    let first = export(&p, &ctx, &opts(&["web"]));
    assert!(first.errors.is_empty(), "{:?}", first.errors);
    assert!(out.join("web/en/02_plan.png").is_file());
    // A second call for another target (as the GUI batches): the index
    // still lists the first target's files.
    let second = export(&p, &ctx, &opts(&["phone"]));
    assert!(second.errors.is_empty(), "{:?}", second.errors);
    let html = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(html.contains("src=\"web/en/01_hero.png\""), "{html}");
    assert!(html.contains("src=\"phone/en/02_plan.png\""));
    assert!(!html.contains("_gallery.png"));

    // One page fewer: the old page 2 is removed, other targets stay.
    p.canvases[0].page.count = 1;
    let third = export(&p, &ctx, &opts(&["web"]));
    assert!(third.errors.is_empty(), "{:?}", third.errors);
    assert_eq!(third.removed, ["web/en/02_plan.png"]);
    assert!(!out.join("web/en/02_plan.png").exists());
    assert!(out.join("web/en/01_hero.png").is_file());
    assert!(out.join("phone/en/02_plan.png").is_file());
    let html = std::fs::read_to_string(out.join("index.html")).unwrap();
    assert!(!html.contains("web/en/02_plan.png"));
    assert!(html.contains("phone/en/02_plan.png"));

    // A file the manifest does not list is never touched.
    std::fs::write(out.join("web/en/notes.txt"), "mine").unwrap();
    export(&p, &ctx, &opts(&["web"]));
    assert!(out.join("web/en/notes.txt").is_file());
}

#[test]
fn fastlane_conflict_with_an_earlier_export() {
    let dir = tempfile::tempdir().unwrap();
    let p = project();
    let out = dir.path().join("fl");
    let ctx = ctx(dir.path());
    let opts = |t: &str| ExportOptions {
        out_dir: Some(out.clone()),
        layout: ExportLayout::Fastlane,
        targets: vec![t.to_owned()],
        langs: vec!["en".into()],
        ..ExportOptions::default()
    };
    let a = export(&p, &ctx, &opts("phone-old"));
    assert!(a.errors.is_empty(), "{:?}", a.errors);
    let b = export(&p, &ctx, &opts("phone"));
    assert_eq!(b.errors.len(), 1, "{:?}", b.errors);
    assert!(b.files.is_empty());
    assert!(out.join("screenshots/en-US/01_phone-old.png").is_file());
    assert!(!out.join("index.html").exists());
}
