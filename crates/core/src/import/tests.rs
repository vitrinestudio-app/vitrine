//! Screenshot import planning and apply on generated folders.

use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::model::Lang;
use crate::ops::Op;

/// Phone canvas (keys home, planner, garden-plan) with an iPhone and an
/// iPad target; languages en (default) + de.
fn project() -> Project {
    let yaml = "format: vitrine/1
revision: 1
name: t
languages:
  default: en
  available: [en, de]
canvases:
- id: s
  page: {aspect: 1000:2000, count: 3}
  elements:
  - {id: d1, kind: device, page: 0, at: {x: 500, y: 1000}, width: 600, model: iphone-island, screenshot: home}
  - {id: d2, kind: device, page: 1, at: {x: 500, y: 1000}, width: 600, model: iphone-island, screenshot: planner}
  - {id: d3, kind: device, page: 2, at: {x: 500, y: 1000}, width: 600, model: iphone-island, screenshot: garden-plan}
targets:
- {id: phone, canvas: s, size: [1320, 2868], screenshots: ios-phone}
- {id: pad, canvas: s, size: [2064, 2752], screenshots: ios-tablet}
";
    crate::io::project_from_yaml(yaml).unwrap_or_else(|e| panic!("{e}"))
}

/// A file with a PNG header of `w × h` (enough for `imagesize`) plus
/// `tag` so files can differ in content.
fn png(path: &Path, w: u32, h: u32, tag: &str) {
    let mut b = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    b.extend_from_slice(&w.to_be_bytes());
    b.extend_from_slice(&h.to_be_bytes());
    b.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    b.extend_from_slice(tag.as_bytes());
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, b).unwrap();
}

struct Dirs {
    _tmp: tempfile::TempDir,
    project: PathBuf,
    source: PathBuf,
}

fn dirs() -> Dirs {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path().join("app");
    let source = tmp.path().join("src");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&source).unwrap();
    Dirs {
        _tmp: tmp,
        project,
        source,
    }
}

fn item<'a>(plan: &'a ImportPlan, source: &str) -> &'a PlannedFile {
    plan.items
        .iter()
        .find(|i| i.source == source)
        .unwrap_or_else(|| panic!("no item {source} in {:#?}", plan.items))
}

fn skipped<'a>(plan: &'a ImportPlan, source: &str) -> &'a SkippedFile {
    plan.skipped
        .iter()
        .find(|i| i.source == source)
        .unwrap_or_else(|| panic!("{source} not skipped: {:#?}", plan.skipped))
}

fn plan(d: &Dirs, p: &Project, rules: &ImportRules) -> ImportPlan {
    plan_screenshot_import(p, &d.project, &d.source, rules).unwrap()
}

fn fastlane_fixture(d: &Dirs) {
    let s = &d.source;
    png(
        &s.join("en-US/iPhone 16 Pro Max-01_home.png"),
        1320,
        2868,
        "en1",
    );
    png(
        &s.join("en-US/iPhone 16 Pro Max-02_planner.png"),
        1320,
        2868,
        "en2",
    );
    png(
        &s.join("en-US/iPhone 16 Pro Max-03_garden_plan.png"),
        1320,
        2868,
        "en3",
    );
    png(
        &s.join("en-US/iPhone 16 Pro Max-01_home_framed.png"),
        1500,
        3000,
        "f",
    );
    png(
        &s.join("en-US/iPad Pro (12.9-inch) (6th generation)-01_home.png"),
        2048,
        2732,
        "pad",
    );
    png(
        &s.join("de-DE/iPhone 16 Pro Max-01_home.png"),
        1320,
        2868,
        "de1",
    );
    png(&s.join("de-DE/Apple Watch-01_home.png"), 10, 90, "watch");
    png(
        &s.join("fr-FR/iPhone 16 Pro Max-01_home.png"),
        1320,
        2868,
        "fr",
    );
    fs::write(s.join("screenshots.html"), "<html/>").unwrap();
}

#[test]
fn fastlane_output_is_mapped() {
    let d = dirs();
    fastlane_fixture(&d);
    let p = project();
    let plan = plan(&d, &p, &ImportRules::default());
    assert_eq!(plan.format, SourceFormat::Fastlane);

    let home = item(&plan, "en-US/iPhone 16 Pro Max-01_home.png");
    assert_eq!(
        home.dest.as_deref(),
        Some("screenshots/en/ios-phone/home.png")
    );
    assert_eq!(home.device.as_deref(), Some("iPhone 16 Pro Max"));
    assert_eq!(home.action, Action::New);
    // en-US → en is the default subtag rule: certain.
    assert_eq!(home.confidence, Confidence::High, "{:?}", home.notes);

    let plan3 = item(&plan, "en-US/iPhone 16 Pro Max-03_garden_plan.png");
    assert_eq!(
        plan3.dest.as_deref(),
        Some("screenshots/en/ios-phone/garden-plan.png")
    );

    let pad = item(
        &plan,
        "en-US/iPad Pro (12.9-inch) (6th generation)-01_home.png",
    );
    assert_eq!(
        pad.dest.as_deref(),
        Some("screenshots/en/ios-tablet/home.png")
    );
    assert_eq!(
        item(&plan, "de-DE/iPhone 16 Pro Max-01_home.png")
            .dest
            .as_deref(),
        Some("screenshots/de/ios-phone/home.png")
    );

    // Unknown device size, unknown locale: unresolved, with reasons.
    let watch = item(&plan, "de-DE/Apple Watch-01_home.png");
    assert_eq!(watch.action, Action::Unresolved);
    assert!(
        watch.notes.iter().any(|n| n.contains("unknown device")),
        "{:?}",
        watch.notes
    );
    let fr = item(&plan, "fr-FR/iPhone 16 Pro Max-01_home.png");
    assert_eq!(fr.action, Action::Unresolved);
    assert!(
        fr.notes.iter().any(|n| n.contains("fr-FR")),
        "{:?}",
        fr.notes
    );

    assert_eq!(skipped(&plan, "screenshots.html").reason, "fastlane report");
    assert!(
        skipped(&plan, "en-US/iPhone 16 Pro Max-01_home_framed.png")
            .reason
            .contains("framed")
    );

    // Learned mapping: devices and locales (not the unresolved ones).
    assert_eq!(plan.mapping.devices["iPhone 16 Pro Max"], "ios-phone");
    assert_eq!(
        plan.mapping.devices["iPad Pro (12.9-inch) (6th generation)"],
        "ios-tablet"
    );
    assert!(!plan.mapping.devices.contains_key("Apple Watch"));
    assert_eq!(plan.mapping.locales["de-DE"], Lang::new("de"));
    assert!(!plan.mapping.locales.contains_key("fr-FR"));

    // Still missing afterwards: tablet planner/garden-plan in en; de
    // falls back to en for those.
    let missing = |lang: &str, variant: &str, key: &str| {
        plan.missing
            .iter()
            .find(|m| m.lang == lang && m.variant == variant && m.key == key)
            .map(|m| m.status)
    };
    assert_eq!(
        missing("en", "ios-tablet", "planner"),
        Some(MissingStatus::Missing)
    );
    assert_eq!(missing("en", "ios-phone", "home"), None);
    assert_eq!(missing("de", "ios-phone", "home"), None);
    assert_eq!(
        missing("de", "ios-phone", "planner"),
        Some(MissingStatus::Fallback)
    );
}

#[test]
fn maps_override_detection() {
    let d = dirs();
    fastlane_fixture(&d);
    let mut p = project();
    // Project mapping (as saved in project.yaml) + rules on top.
    p.import
        .fastlane
        .devices
        .insert("Apple Watch".into(), "ios-phone".into());
    p.import
        .fastlane
        .locales
        .insert("fr-FR".into(), Lang::new("de"));
    let mut rules = ImportRules::default();
    rules.mapping.keys.insert("01_home".into(), "start".into());
    let plan = plan(&d, &p, &rules);
    let watch = item(&plan, "de-DE/Apple Watch-01_home.png");
    assert_eq!(
        watch.dest.as_deref(),
        Some("screenshots/de/ios-phone/start.png")
    );
    // fr-FR is mapped to de as well: duplicate destination with the
    // de-DE iPhone file; only one is copied.
    let fr = item(&plan, "fr-FR/iPhone 16 Pro Max-01_home.png");
    let de = item(&plan, "de-DE/iPhone 16 Pro Max-01_home.png");
    assert_eq!(fr.dest, de.dest);
    let actions = [fr.action, de.action, watch.action];
    assert_eq!(
        actions.iter().filter(|a| **a == Action::Duplicate).count(),
        2,
        "{actions:?}"
    );
    assert!(plan.has_conflicts());
    assert_eq!(plan.mapping.keys["01_home"], "start");
}

#[test]
fn overrides_fix_rows() {
    let d = dirs();
    fastlane_fixture(&d);
    let p = project();
    let mut rules = ImportRules::default();
    rules.overrides.insert(
        "de-DE/Apple Watch-01_home.png".into(),
        ItemOverride {
            variant: Some(String::new()),
            key: Some("watch".into()),
            ..ItemOverride::default()
        },
    );
    rules.overrides.insert(
        "fr-FR/iPhone 16 Pro Max-01_home.png".into(),
        ItemOverride {
            skip: true,
            ..ItemOverride::default()
        },
    );
    let plan = plan(&d, &p, &rules);
    let watch = item(&plan, "de-DE/Apple Watch-01_home.png");
    assert_eq!(watch.dest.as_deref(), Some("screenshots/de/watch.png"));
    assert!(watch.overridden);
    assert_eq!(watch.confidence, Confidence::High);
    assert_eq!(
        skipped(&plan, "fr-FR/iPhone 16 Pro Max-01_home.png").reason,
        "skipped by you"
    );
    // Other files share the screen name `01_home`: the row correction is
    // not turned into a key mapping for all of them.
    assert!(!plan.mapping.keys.contains_key("01_home"));
}

#[test]
fn vitrine_convention() {
    let d = dirs();
    let s = &d.source;
    png(&s.join("de/ios-phone/home.png"), 1206, 2622, "a");
    png(&s.join("en/planner.png"), 1206, 2622, "b");
    png(&s.join("en/custom-variant/home.png"), 10, 10, "c");
    png(&s.join("xx/home.png"), 10, 10, "d");
    png(&s.join("en/a/b/deep.png"), 10, 10, "e");
    fs::write(s.join("en/notes.txt"), "x").unwrap();
    let p = project();
    let plan = plan(&d, &p, &ImportRules::default());
    assert_eq!(plan.format, SourceFormat::Vitrine);
    assert_eq!(
        item(&plan, "de/ios-phone/home.png").dest.as_deref(),
        Some("screenshots/de/ios-phone/home.png")
    );
    let planner = item(&plan, "en/planner.png");
    assert_eq!(planner.dest.as_deref(), Some("screenshots/en/planner.png"));
    assert_eq!(planner.variant, None);
    assert_eq!(planner.confidence, Confidence::High);
    let custom = item(&plan, "en/custom-variant/home.png");
    assert_eq!(custom.confidence, Confidence::Medium);
    assert_eq!(item(&plan, "xx/home.png").action, Action::Unresolved);
    assert!(
        skipped(&plan, "en/notes.txt")
            .reason
            .contains("not a screenshot")
    );
    assert!(skipped(&plan, "en/a/b/deep.png").reason.contains("<lang>"));
}

#[test]
fn duplicates_keep_the_file_already_in_the_project() {
    let d = dirs();
    let p = project();
    png(&d.source.join("en/ios-phone/home.png"), 1320, 2868, "same");
    png(&d.source.join("en/home.png"), 1320, 2868, "other");
    png(
        &d.project.join("screenshots/en/ios-phone/home.png"),
        1320,
        2868,
        "same",
    );
    let mut rules = ImportRules::default();
    rules.overrides.insert(
        "en/home.png".to_owned(),
        ItemOverride {
            variant: Some("ios-phone".into()),
            ..ItemOverride::default()
        },
    );
    let plan = plan(&d, &p, &rules);
    // `en/home.png` sorts first and is as certain, but must not replace
    // the identical file already in the project.
    assert_eq!(
        item(&plan, "en/ios-phone/home.png").action,
        Action::Unchanged
    );
    assert_eq!(item(&plan, "en/home.png").action, Action::Duplicate);
    assert_eq!(plan.to_copy(), 0);
}

#[test]
fn project_screenshots_folder_as_source() {
    let d = dirs();
    png(
        &d.source.join("screenshots/en/ios-phone/home.png"),
        1320,
        2868,
        "x",
    );
    let plan = plan(&d, &project(), &ImportRules::default());
    assert_eq!(plan.format, SourceFormat::Vitrine);
    assert_eq!(
        plan.items[0].dest.as_deref(),
        Some("screenshots/en/ios-phone/home.png")
    );
}

#[test]
fn flat_folder_by_name_and_size() {
    let d = dirs();
    let s = &d.source;
    png(&s.join("home_de.png"), 1320, 2868, "a");
    png(&s.join("ios-tablet-planner-en.png"), 100, 100, "b");
    png(&s.join("02 Garden Plan.png"), 2048, 2732, "c");
    png(&s.join("random.jpeg"), 10, 90, "d");
    let p = project();
    let plan = plan(&d, &p, &ImportRules::default());
    assert_eq!(plan.format, SourceFormat::Flat);
    assert_eq!(
        item(&plan, "home_de.png").dest.as_deref(),
        Some("screenshots/de/ios-phone/home.png")
    );
    assert_eq!(item(&plan, "home_de.png").confidence, Confidence::High);
    assert_eq!(
        item(&plan, "ios-tablet-planner-en.png").dest.as_deref(),
        Some("screenshots/en/ios-tablet/planner.png")
    );
    let garden = item(&plan, "02 Garden Plan.png");
    assert_eq!(
        garden.dest.as_deref(),
        Some("screenshots/en/ios-tablet/garden-plan.png")
    );
    // No language in the name → default language, low confidence.
    assert_eq!(garden.confidence, Confidence::Low);
    let random = item(&plan, "random.jpeg");
    assert_eq!(random.dest.as_deref(), Some("screenshots/en/random.jpg"));
    assert_eq!(random.confidence, Confidence::Low);

    // The same files given as a list (drag & drop).
    let files: Vec<PathBuf> = ["home_de.png", "random.jpeg"]
        .iter()
        .map(|f| s.join(f))
        .collect();
    let plan = plan_files(&p, &d.project, &files, &ImportRules::default());
    assert_eq!(plan.items.len(), 2);
    assert_eq!(
        item(&plan, "home_de.png").dest.as_deref(),
        Some("screenshots/de/ios-phone/home.png")
    );
}

#[test]
fn existing_files_and_apply() {
    let d = dirs();
    fastlane_fixture(&d);
    let p = project();
    // Same content already in the project / different content.
    png(
        &d.project.join("screenshots/en/ios-phone/home.png"),
        1320,
        2868,
        "en1",
    );
    png(
        &d.project.join("screenshots/en/ios-phone/planner.png"),
        1320,
        2868,
        "old",
    );
    let first = plan(&d, &p, &ImportRules::default());
    assert_eq!(
        item(&first, "en-US/iPhone 16 Pro Max-01_home.png").action,
        Action::Unchanged
    );
    let planner = item(&first, "en-US/iPhone 16 Pro Max-02_planner.png");
    assert_eq!(planner.action, Action::Overwrite);
    assert!(planner.notes.iter().any(|n| n.contains("replaces")));
    assert_eq!(first.to_copy(), 4);

    let report = apply(&first, &d.project);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert_eq!(report.copied.len(), 4, "{report:?}");
    assert_eq!(report.unchanged, ["screenshots/en/ios-phone/home.png"]);
    assert_eq!(report.skipped, 2);
    assert!(
        d.project
            .join("screenshots/en/ios-tablet/home.png")
            .is_file()
    );
    assert_eq!(
        fs::read(d.project.join("screenshots/en/ios-phone/planner.png")).unwrap(),
        fs::read(d.source.join("en-US/iPhone 16 Pro Max-02_planner.png")).unwrap()
    );
    // Sources are copied, not moved.
    assert!(
        d.source
            .join("en-US/iPhone 16 Pro Max-02_planner.png")
            .is_file()
    );
    // No temp files left behind.
    let leftovers: Vec<_> = fs::read_dir(d.project.join("screenshots/en/ios-phone"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
        .collect();
    assert!(leftovers.is_empty());

    // Planning again: everything unchanged; applying again copies nothing.
    let again = plan(&d, &p, &ImportRules::default());
    assert_eq!(again.to_copy(), 0);
    assert_eq!(apply(&again, &d.project).copied, Vec::<String>::new());
}

#[test]
fn explicit_format_and_missing_source() {
    let d = dirs();
    png(&d.source.join("en/home.png"), 1320, 2868, "x");
    let rules = ImportRules {
        format: SourceFormat::Fastlane,
        ..ImportRules::default()
    };
    let plan = plan(&d, &project(), &rules);
    assert_eq!(plan.format, SourceFormat::Fastlane);
    assert_eq!(plan.items.len(), 0);
    assert!(skipped(&plan, "en/home.png").reason.contains("Device"));
    assert!(matches!(
        plan_screenshot_import(&project(), &d.project, &d.source.join("nope"), &rules),
        Err(ImportError::NotADir(_))
    ));
    assert_eq!("flat".parse::<SourceFormat>(), Ok(SourceFormat::Flat));
    assert!("x".parse::<SourceFormat>().is_err());
}

#[test]
fn import_settings_round_trip_and_op() {
    let mut p = project();
    let mut import = crate::model::ImportSettings::default();
    import
        .fastlane
        .devices
        .insert("iPhone 16 Pro Max".into(), "ios-phone".into());
    import.fastlane.devices.insert(
        "iPad Pro (12.9-inch) (6th generation)".into(),
        "ios-tablet".into(),
    );
    import
        .fastlane
        .locales
        .insert("de-DE".into(), Lang::new("de"));
    import.fastlane.keys.insert("01_home".into(), "home".into());
    let inverse = p
        .apply(Op::SetImport {
            import: import.clone(),
        })
        .unwrap();
    assert_eq!(p.import, import);
    let yaml = p.to_yaml().unwrap();
    assert!(yaml.contains("import:\n  fastlane:\n"), "{yaml}");
    let back = crate::io::project_from_yaml(&yaml).unwrap();
    assert_eq!(back.import, import);
    assert_eq!(back.to_yaml().unwrap(), yaml);
    p.apply(inverse).unwrap();
    assert!(p.import.is_empty());
    assert!(!p.to_yaml().unwrap().contains("import:"));

    let mut bad = crate::model::ImportSettings::default();
    bad.fastlane
        .devices
        .insert("x".into(), "not a variant".into());
    assert!(p.apply(Op::SetImport { import: bad }).is_err());
}

/// [`project`] plus Google Play phone and tablet targets and Hebrew.
fn project_with_android() -> Project {
    let mut p = project();
    let yaml = p.to_yaml().unwrap().replace(
        "targets:\n",
        "targets:\n- {id: play, canvas: s, size: [1080, 2160], screenshots: android-phone}\n- {id: play-tab, canvas: s, size: [1600, 2560], screenshots: android-tablet}\n",
    );
    p = crate::io::project_from_yaml(&yaml).unwrap_or_else(|e| panic!("{e}"));
    p.languages.available.push(Lang::new("he"));
    p
}

#[test]
#[allow(clippy::too_many_lines)]
fn whole_fastlane_folder_snapshot_and_screengrab() {
    let d = dirs();
    let p = project_with_android();
    let s = d.source.join("fastlane");
    let shots = s.join("screenshots");
    fs::create_dir_all(&shots).unwrap();
    fs::write(
        shots.join("Framefile.json"),
        r##"{"default": {"background": "./background.jpg", "title": {"color": "#ffffff"}}, "data": [{"filter": "01Home", "background": "../bg/home.png"}]}"##,
    )
    .unwrap();
    png(&shots.join("background.jpg"), 1320, 2868, "bg");
    png(&s.join("bg/home.png"), 1320, 2868, "bg2");
    fs::write(shots.join("screenshots.html"), "<html></html>").unwrap();
    png(
        &shots.join("en-US/iPhone 15 Pro Max-01Home.png"),
        1290,
        2796,
        "a",
    );
    png(
        &shots.join("en-US/iPhone 15 Pro Max-01Home_framed.png"),
        1500,
        3000,
        "f",
    );
    fs::write(
        shots.join("en-US/title.strings"),
        "\n\"01Home\" = \"Plan\";\n",
    )
    .unwrap();
    png(
        &shots.join("de-DE/iPad Pro 13-inch (M4)-02Planner.png"),
        2064,
        2752,
        "b",
    );
    fs::write(shots.join("de-DE/keyword.strings"), "\n").unwrap();
    png(
        &shots.join("iMessage/en-US/iPhone 15 Pro Max-01Home.png"),
        1290,
        2796,
        "c",
    );
    fs::create_dir_all(s.join("metadata/en-US")).unwrap();
    fs::write(s.join("metadata/en-US/description.txt"), "x").unwrap();
    png(&s.join("metadata/app_icon.png"), 1024, 1024, "icon");
    let android = s.join("metadata/android");
    fs::create_dir_all(android.join("en-US")).unwrap();
    fs::write(android.join("en-US/title.txt"), "x").unwrap();
    png(
        &android.join("en-US/images/featureGraphic.png"),
        1024,
        500,
        "fg",
    );
    png(
        &android.join("en-US/images/phoneScreenshots/home_1731234567890.png"),
        1080,
        2160,
        "d",
    );
    png(
        &android.join("de-DE/images/tenInchScreenshots/garden_plan_1731234567891.png"),
        1600,
        2560,
        "e",
    );
    png(
        &android.join("iw-IL/images/phoneScreenshots/planner.png"),
        1080,
        2160,
        "h",
    );

    let plan = plan(&d, &p, &ImportRules::default());
    assert_eq!(plan.format, SourceFormat::Fastlane);
    let it = item(&plan, "en-US/iPhone 15 Pro Max-01Home.png");
    assert_eq!(
        it.dest.as_deref(),
        Some("screenshots/en/ios-phone/home.png"),
        "{it:#?}"
    );
    assert_eq!(it.device.as_deref(), Some("iPhone 15 Pro Max"));
    let it = item(&plan, "de-DE/iPad Pro 13-inch (M4)-02Planner.png");
    assert_eq!(
        it.dest.as_deref(),
        Some("screenshots/de/ios-tablet/planner.png"),
        "{it:#?}"
    );
    let it = item(
        &plan,
        "metadata/android/en-US/images/phoneScreenshots/home_1731234567890.png",
    );
    assert_eq!(
        it.dest.as_deref(),
        Some("screenshots/en/android-phone/home.png"),
        "{it:#?}"
    );
    assert_eq!(it.confidence, Confidence::High, "{it:#?}");
    let it = item(
        &plan,
        "metadata/android/de-DE/images/tenInchScreenshots/garden_plan_1731234567891.png",
    );
    assert_eq!(
        it.dest.as_deref(),
        Some("screenshots/de/android-tablet/garden-plan.png"),
        "{it:#?}"
    );
    // Play's legacy Hebrew code → project language `he` (store locale map).
    let it = item(
        &plan,
        "metadata/android/iw-IL/images/phoneScreenshots/planner.png",
    );
    assert_eq!(
        it.dest.as_deref(),
        Some("screenshots/he/android-phone/planner.png"),
        "{it:#?}"
    );

    let reason = |src: &str| skipped(&plan, src).reason.clone();
    assert_eq!(reason("screenshots.html"), "fastlane report");
    assert_eq!(reason("Framefile.json"), "frameit configuration");
    assert!(reason("background.jpg").contains("frameit background"));
    assert!(reason("bg/home.png").contains("frameit background"));
    assert_eq!(reason("en-US/title.strings"), "frameit configuration");
    assert_eq!(reason("de-DE/keyword.strings"), "frameit configuration");
    assert!(reason("en-US/iPhone 15 Pro Max-01Home_framed.png").contains("framed"));
    assert!(reason("iMessage/en-US/iPhone 15 Pro Max-01Home.png").contains("iMessage"));
    assert!(reason("metadata/en-US/description.txt").contains("metadata"));
    assert!(reason("metadata/app_icon.png").contains("metadata"));
    assert!(reason("metadata/android/en-US/title.txt").contains("metadata"));
    assert!(reason("metadata/android/en-US/images/featureGraphic.png").contains("featureGraphic"));
    assert_eq!(plan.count(Action::Unresolved), 0, "{:#?}", plan.items);
}

#[test]
fn screengrab_folder_picked_directly() {
    let d = dirs();
    let p = project_with_android();
    // The user picks fastlane/metadata/android.
    png(
        &d.source
            .join("en-US/images/phoneScreenshots/01_home_1731234567890.png"),
        1080,
        2160,
        "a",
    );
    png(
        &d.source
            .join("de-DE/images/sevenInchScreenshots/planner.png"),
        1200,
        1920,
        "b",
    );
    let plan1 = plan(&d, &p, &ImportRules::default());
    assert_eq!(plan1.format, SourceFormat::Fastlane);
    assert_eq!(
        item(
            &plan1,
            "en-US/images/phoneScreenshots/01_home_1731234567890.png"
        )
        .dest
        .as_deref(),
        Some("screenshots/en/android-phone/home.png")
    );
    assert_eq!(
        item(&plan1, "de-DE/images/sevenInchScreenshots/planner.png")
            .dest
            .as_deref(),
        Some("screenshots/de/android-tablet/planner.png")
    );
    // A device map entry for the folder name wins over the built-in one.
    let mut rules = ImportRules::default();
    rules
        .mapping
        .devices
        .insert("sevenInchScreenshots".into(), "android-phone".into());
    let plan2 = plan(&d, &p, &rules);
    assert_eq!(
        item(&plan2, "de-DE/images/sevenInchScreenshots/planner.png")
            .dest
            .as_deref(),
        Some("screenshots/de/android-phone/planner.png")
    );
}

#[test]
fn screengrab_timestamps() {
    assert_eq!(strip_timestamp("home_1731234567890", 10), "home");
    assert_eq!(strip_timestamp("home_12", 10), "home_12");
    assert_eq!(strip_timestamp("_1731234567890", 10), "_1731234567890");
    assert_eq!(strip_timestamp("home", 10), "home");
}

/// One `fastlane/` folder that holds snapshot/screengrab captures and the
/// images an earlier `vitrine export --layout fastlane --out fastlane`
/// wrote next to them (ADR 0032).
fn shared_fastlane_folder(d: &Dirs, manifest: bool) -> PathBuf {
    let s = d.source.join("fastlane");
    png(
        &s.join("screenshots/en-US/iPhone 15 Pro Max-01Home.png"),
        1320,
        2868,
        "raw",
    );
    png(
        &s.join("screenshots/en-US/01_phone.png"),
        1320,
        2868,
        "own1",
    );
    png(
        &s.join("screenshots/en-US/02_phone.png"),
        1320,
        2868,
        "own2",
    );
    let shots = s.join("metadata/android/en-US/images/phoneScreenshots");
    png(&shots.join("home_1731234567890.png"), 1080, 2160, "raw2");
    png(&shots.join("01_en-US.png"), 1080, 2160, "own3");
    if manifest {
        fs::write(
            s.join(EXPORT_MANIFEST),
            r#"{
  "screenshots/en-US/01_phone.png": {"hash": "00000000000000ae", "target": "phone", "lang": "en"},
  "screenshots/en-US/02_phone.png": "00000000000000af",
  "metadata/android/en-US/images/phoneScreenshots/01_en-US.png": {"hash": "1", "target": "play", "lang": "en"},
  "../outside.png": "2",
  "screenshots/en-US/gone.png": "3"
}"#,
        )
        .unwrap();
    }
    s
}

#[test]
fn own_exports_listed_in_the_manifest_are_skipped() {
    let d = dirs();
    let p = project_with_android();
    let s = shared_fastlane_folder(&d, true);
    let plan = plan(&d, &p, &ImportRules::default());
    assert_eq!(plan.format, SourceFormat::Fastlane);
    for own in [
        "en-US/01_phone.png",
        "en-US/02_phone.png",
        "metadata/android/en-US/images/phoneScreenshots/01_en-US.png",
    ] {
        assert_eq!(skipped(&plan, own).reason, SKIP_OWN_EXPORT);
    }
    assert_eq!(
        item(&plan, "en-US/iPhone 15 Pro Max-01Home.png")
            .dest
            .as_deref(),
        Some("screenshots/en/ios-phone/home.png")
    );
    assert_eq!(
        item(
            &plan,
            "metadata/android/en-US/images/phoneScreenshots/home_1731234567890.png"
        )
        .dest
        .as_deref(),
        Some("screenshots/en/android-phone/home.png")
    );
    assert_eq!(plan.items.len(), 2, "{:#?}", plan.items);
    assert_eq!(plan.count(Action::Unresolved), 0, "{:#?}", plan.items);

    // The manifest one level above a picked `fastlane/screenshots` counts.
    let picked = plan_screenshot_import(
        &p,
        &d.project,
        &s.join("screenshots"),
        &ImportRules::default(),
    )
    .unwrap();
    assert_eq!(
        skipped(&picked, "en-US/01_phone.png").reason,
        SKIP_OWN_EXPORT
    );
    assert_eq!(picked.items.len(), 1, "{:#?}", picked.items);
}

#[test]
fn exports_without_manifest_are_skipped_by_name() {
    let d = dirs();
    let p = project_with_android();
    shared_fastlane_folder(&d, false);
    let plan = plan(&d, &p, &ImportRules::default());
    for own in [
        "en-US/01_phone.png",
        "en-US/02_phone.png",
        "metadata/android/en-US/images/phoneScreenshots/01_en-US.png",
    ] {
        assert_eq!(skipped(&plan, own).reason, SKIP_EXPORT_NAME);
    }
    assert_eq!(plan.items.len(), 2, "{:#?}", plan.items);
    assert_eq!(plan.count(Action::Unresolved), 0, "{:#?}", plan.items);
}

#[test]
fn export_name_rule() {
    let p = project_with_android();
    let shot: Vec<String> = vec!["en-US".into(), "x.png".into()];
    let snap = Route::Snapshot;
    assert!(looks_exported(&p, &snap, &shot, "01_phone"));
    assert!(looks_exported(&p, &snap, &shot, "10_pad_app_ipad_pro_129"));
    assert!(!looks_exported(&p, &snap, &shot, "01_phones"));
    assert!(!looks_exported(&p, &snap, &shot, "01_home"));
    assert!(!looks_exported(&p, &snap, &shot, "1_phone"));
    assert!(!looks_exported(&p, &snap, &shot, "iPhone 15-01_phone"));
    let grab = Route::Screengrab {
        locale: "de-DE".into(),
        kind: "android-phone".into(),
    };
    assert!(looks_exported(&p, &grab, &shot, "03_de-DE"));
    assert!(!looks_exported(&p, &grab, &shot, "03_en-US"));
    assert!(!looks_exported(&p, &grab, &shot, "01_home_1731234567890"));
}
