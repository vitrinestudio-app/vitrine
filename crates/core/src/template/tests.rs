use std::io::Write;
use std::path::{Path, PathBuf};

use super::extract::{SaveOptions, extract};
use super::insert::{InsertOptions, StyleConflict, plan_insert};
use super::instantiate::{create_project, parse_langs};
use super::store::TemplateStore;
use super::*;
use crate::model::{ElementKind, Project};

fn example_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example")
}

fn example() -> Project {
    Project::load(example_dir()).unwrap()
}

fn save_opts(name: &str, canvases: Option<Vec<&str>>) -> SaveOptions {
    SaveOptions {
        name: name.into(),
        description: "A test".into(),
        author: "Tester".into(),
        tags: TemplateTags {
            style: vec!["Minimal".into()],
            use_: vec!["home-garden".into()],
            ..TemplateTags::default()
        },
        canvases: canvases.map(|c| c.into_iter().map(String::from).collect()),
        ..SaveOptions::default()
    }
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    out.into_inner()
}

#[test]
fn extract_whole_project_round_trips_through_zip() {
    let p = example();
    let (pkg, report) = extract(&p, &example_dir(), &save_opts("Fernwood set", None)).unwrap();
    assert_eq!(pkg.manifest.id, "fernwood-set");
    assert_eq!(pkg.manifest.tags.style, ["minimal"]);
    assert_eq!(pkg.manifest.tags.use_, ["home-garden"]);
    assert_eq!(pkg.manifest.samples.len(), 0);
    assert_eq!(pkg.manifest.license, LICENSE);
    assert_eq!(pkg.manifest.canvases.len(), p.canvases.len());
    assert!(report.placeholders > 0);
    // Placeholders only, never the project's screenshots.
    for s in pkg.screenshots() {
        let data = pkg.file(s).unwrap();
        let real = std::fs::read(example_dir().join(s)).ok();
        assert_ne!(real.as_deref(), Some(data), "{s} must be a placeholder");
    }
    assert!(pkg.file("assets/icon-good.svg").is_some());
    let project = pkg.project().unwrap();
    assert!(project.store.is_empty(), "store settings left out");
    assert_eq!(project.name, "Fernwood set");

    let bytes = pkg.to_zip_bytes().unwrap();
    let back = Package::from_zip_bytes(&bytes, Limits::default()).unwrap();
    assert_eq!(back, pkg);
}

#[test]
fn extract_selected_canvas_keeps_only_its_texts_and_targets() {
    let p = example();
    let (pkg, _) = extract(&p, &example_dir(), &save_opts("Mac", Some(vec!["mac"]))).unwrap();
    let t = pkg.project().unwrap();
    assert_eq!(t.canvases.len(), 1);
    assert!(t.targets.iter().all(|t| t.canvas.as_str() == "mac"));
    let keys = walk::string_keys(&t.canvases[0]);
    for map in t.strings.values() {
        assert!(map.keys().all(|k| keys.contains(k)));
    }
    assert!(matches!(
        extract(&p, &example_dir(), &save_opts("x", Some(vec!["nope"]))),
        Err(TemplateError::NotFound(_))
    ));
}

#[test]
fn new_project_from_template_maps_languages() {
    let p = example();
    let (pkg, _) = extract(&p, &example_dir(), &save_opts("Fernwood", None)).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("new.vitrine");
    let langs = parse_langs(&["de-AT".into(), "ja".into()]).unwrap();
    let created = create_project(&pkg, &dir, "Neu", &langs).unwrap();
    let np = created.project;
    assert_eq!(np.name, "Neu");
    assert_eq!(np.languages.default.as_str(), "de-AT");
    assert_eq!(np.canvases.len(), p.canvases.len());
    // de-AT takes the German texts, ja the template's default (en).
    let key = "plan.title";
    assert_eq!(np.string("de-AT", key), p.string("de", key));
    assert_eq!(np.string("ja", key), p.string("en", key));
    assert!(
        created
            .copied_texts
            .iter()
            .any(|(l, s)| l.as_str() == "ja" && s.as_str() == "en")
    );
    assert!(dir.join("assets/icon-good.svg").is_file());
    assert!(dir.join("screenshots/de-AT").is_dir());
    // Placeholders are not copied (validation keeps asking for screenshots).
    assert!(
        std::fs::read_dir(dir.join("screenshots/de-AT"))
            .unwrap()
            .next()
            .is_none()
    );
    // Not into a non-empty folder.
    assert!(matches!(
        create_project(&pkg, &dir, "Neu", &langs),
        Err(TemplateError::Conflict(_))
    ));
}

#[test]
fn insert_into_the_same_project_renames_everything_that_collides() {
    let p = example();
    let (pkg, _) = extract(
        &p,
        &example_dir(),
        &save_opts("Fernwood", Some(vec!["phone"])),
    )
    .unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let plan = plan_insert(&p, tmp.path(), &pkg, &InsertOptions::default()).unwrap();
    assert_eq!(plan.report.canvases[0].from, "phone");
    assert_eq!(plan.report.canvases[0].to, "phone-2");
    assert_ne!(plan.report.elements_renamed.len(), 0);
    assert!(plan.report.targets.iter().all(|r| r.from != r.to));
    // Equal styles and texts are shared, nothing to rename.
    assert_eq!(plan.report.styles_renamed.len(), 0);
    assert_ne!(plan.report.styles_shared.len(), 0);
    assert_eq!(plan.report.keys_renamed.len(), 0);
    // The asset files are new for an empty folder.
    assert!(plan.files.iter().any(|f| f.to == "assets/icon-good.svg"));
    let mut q = p.clone();
    q.apply_ops(plan.ops).unwrap();
    assert_eq!(q.canvases.len(), p.canvases.len() + 1);
    // Into the example folder itself: identical assets are reused.
    let plan = plan_insert(&p, &example_dir(), &pkg, &InsertOptions::default()).unwrap();
    assert!(plan.files.is_empty(), "{:?}", plan.files);
}

#[test]
fn style_and_text_conflicts_follow_the_rule() {
    let p = example();
    let (pkg, _) = extract(
        &p,
        &example_dir(),
        &save_opts("Fernwood", Some(vec!["phone"])),
    )
    .unwrap();
    // The project changes a colour, a text style and a text the template uses.
    let mut q = p.clone();
    let mut styles = q.styles.clone();
    let (color, _) = styles
        .colors
        .iter()
        .next()
        .map(|(k, v)| (k.clone(), *v))
        .unwrap();
    styles
        .colors
        .insert(color.clone(), crate::model::Rgba::parse("#123456").unwrap());
    q.apply(crate::ops::Op::SetStyles { styles }).unwrap();
    let lang = q.languages.default.clone();
    q.apply(crate::ops::Op::SetString {
        lang: lang.clone(),
        key: crate::model::StringKey::new("plan.title"),
        value: Some("Changed".into()),
    })
    .unwrap();

    let tmp = tempfile::tempdir().unwrap();
    let plan = plan_insert(&q, tmp.path(), &pkg, &InsertOptions::default()).unwrap();
    let renamed = &plan.report.styles_renamed;
    assert!(
        renamed
            .iter()
            .any(|s| s.name == color && s.to.as_deref() == Some(&format!("{color}-2")))
    );
    assert!(
        plan.report
            .keys_renamed
            .iter()
            .any(|r| r.from == "plan.title" && r.to == "fernwood.plan.title")
    );
    let mut r = q.clone();
    r.apply_ops(plan.ops).unwrap();
    assert_eq!(r.string(lang.as_str(), "plan.title"), Some("Changed"));
    // The span style `accent` uses the renamed colour, so it was renamed too
    // and the inserted text follows.
    assert_eq!(
        r.string(lang.as_str(), "fernwood.plan.title"),
        Some("Your garden, [accent-2]planned in minutes[/]")
    );
    assert!(r.styles.colors.contains_key(&format!("{color}-2")));

    // Rule "use the project's styles": no renamed colour.
    let opts = InsertOptions {
        styles: StyleConflict::UseProject,
        ..InsertOptions::default()
    };
    let plan = plan_insert(&q, tmp.path(), &pkg, &opts).unwrap();
    assert_eq!(plan.report.styles_renamed.len(), 0);
    assert!(
        plan.report
            .styles_from_project
            .iter()
            .any(|s| s.name == color)
    );
    let mut r = q.clone();
    r.apply_ops(plan.ops).unwrap();
    assert!(!r.styles.colors.contains_key(&format!("{color}-2")));
}

#[test]
fn unsafe_archives_are_refused() {
    let p = example();
    let (pkg, _) = extract(
        &p,
        &example_dir(),
        &save_opts("Fernwood", Some(vec!["mac"])),
    )
    .unwrap();
    let manifest = manifest_to_yaml(&pkg.manifest).unwrap();
    let project = pkg.file("project.yaml").unwrap().to_vec();
    let base: Vec<(&str, &[u8])> = vec![
        ("template.yaml", manifest.as_bytes()),
        ("project.yaml", &project),
    ];
    let with = |extra: (&str, &[u8])| {
        let mut e = base.clone();
        e.push(extra);
        Package::from_zip_bytes(&zip_of(&e), Limits::default())
    };
    let err = |r: Result<Package, TemplateError>| r.unwrap_err().to_string();
    assert!(err(with(("../evil.png", b"\x89PNG\r\n\x1a\n"))).contains("traversal"));
    assert!(err(with(("assets/run.sh", b"#!/bin/sh"))).contains("not allowed"));
    assert!(err(with(("assets/x.png", b"MZ not a png"))).contains("does not match"));
    assert!(err(with(("assets/x.svg", b"<svg onload=\"alert(1)\"/>"))).contains("SVG"));
    assert!(err(with((".hidden/x.png", b"x"))).contains("hidden"));
    let small = Limits {
        max_file: 10,
        ..Limits::default()
    };
    assert!(
        Package::from_zip_bytes(&zip_of(&base), small)
            .unwrap_err()
            .to_string()
            .contains("larger than")
    );
    assert!(
        Package::from_zip_bytes(&zip_of(&[("project.yaml", &project)]), Limits::default())
            .unwrap_err()
            .to_string()
            .contains("template.yaml is missing")
    );
    assert!(Package::from_zip_bytes(b"not a zip", Limits::default()).is_err());
}

#[test]
fn symlinks_are_refused() {
    let mut out = std::io::Cursor::new(Vec::new());
    {
        let mut zip = zip::ZipWriter::new(&mut out);
        zip.add_symlink(
            "assets/link.png",
            "/etc/passwd",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.finish().unwrap();
    }
    let e = Package::from_zip_bytes(&out.into_inner(), Limits::default()).unwrap_err();
    assert!(e.to_string().contains("symbolic links"), "{e}");
}

#[test]
fn manifest_rules() {
    let m = builtin::package("phone").unwrap().manifest;
    assert!(m.check().is_ok());
    let bad = |f: &dyn Fn(&mut Manifest)| {
        let mut x = m.clone();
        f(&mut x);
        x.check().is_err()
    };
    assert!(bad(&|m| m.license = "CC-BY-4.0".into()));
    assert!(bad(&|m| m.id = "Bad Id".into()));
    assert!(bad(&|m| m.format = "vitrine-template/9".into()));
    assert!(bad(&|m| m.canvases.clear()));
    assert_eq!(slug("My Bold Set!"), "my-bold-set");
    assert_eq!(slug("###"), "template");
}

#[test]
fn store_install_list_find_remove() {
    let tmp = tempfile::tempdir().unwrap();
    let store = TemplateStore::new(tmp.path().join("templates"));
    let p = example();
    let (pkg, _) = extract(
        &p,
        &example_dir(),
        &save_opts("Fernwood", Some(vec!["mac"])),
    )
    .unwrap();
    store.install(&pkg, false).unwrap();
    assert!(matches!(
        store.install(&pkg, false),
        Err(TemplateError::Conflict(_))
    ));
    store.install(&pkg, true).unwrap();
    let list = store.list();
    assert!(
        list.iter()
            .any(|t| t.id == "phone" && t.source == TemplateSource::Builtin)
    );
    assert!(
        list.iter()
            .any(|t| t.id == "fernwood" && t.source == TemplateSource::Installed)
    );
    assert_eq!(store.get("fernwood").unwrap(), pkg);
    assert_eq!(store.find("minimal").unwrap().manifest.id, "minimal");
    let file = tmp.path().join("x.vitrine-template");
    pkg.write_zip(&file).unwrap();
    assert_eq!(store.find(file.to_str().unwrap()).unwrap(), pkg);
    store.remove("fernwood").unwrap();
    assert!(store.get("fernwood").is_err());
    assert!(store.remove("phone").is_err());
    let mut builtin_named = pkg.clone();
    builtin_named.manifest.id = "phone".into();
    assert!(store.install(&builtin_named, true).is_err());
}

#[test]
fn ofl_fonts_are_shipped_with_a_license_and_others_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    let pkg = builtin::package("minimal").unwrap();
    let mut project = create_project(&pkg, &dir, "Fonts", &parse_langs(&[]).unwrap())
        .unwrap()
        .project;
    // A text style using a project font (a copy of bundled Inter, OFL).
    let inter = include_bytes!("../../../../assets/fonts/Inter-Regular.ttf");
    std::fs::write(dir.join("fonts/Inter-Regular.ttf"), inter).unwrap();
    let mut styles = project.styles.clone();
    styles.text.insert(
        "body".into(),
        crate::model::TextStyle {
            font: Some("Inter".into()),
            ..Default::default()
        },
    );
    project.apply(crate::ops::Op::SetStyles { styles }).unwrap();
    let (tpl, report) = extract(&project, &dir, &save_opts("With font", None)).unwrap();
    assert_eq!(tpl.manifest.fonts.len(), 1);
    let f = &tpl.manifest.fonts[0];
    assert_eq!(f.license_file, "fonts/Inter-Regular-OFL.txt");
    assert_eq!(report.licenses_generated, ["fonts/Inter-Regular.ttf"]);
    assert!(fonts::is_ofl_text(
        std::str::from_utf8(tpl.file(&f.license_file).unwrap()).unwrap()
    ));

    // The same font with its licence declaration removed is refused.
    let mut patched = inter.to_vec();
    for (from, to) in [
        ("Open Font License", "Xxxx Xxxx Xxxxxxx"),
        ("OFL", "XXX"),
        ("openfontlicense", "xxxxxxxxxxxxxxx"),
    ] {
        let utf16 = |s: &str| {
            s.encode_utf16()
                .flat_map(u16::to_be_bytes)
                .collect::<Vec<u8>>()
        };
        let (a, b) = (utf16(from), utf16(to));
        let mut i = 0;
        while i + a.len() <= patched.len() {
            if patched[i..i + a.len()] == a[..] {
                patched[i..i + a.len()].copy_from_slice(&b);
            }
            i += 1;
        }
    }
    std::fs::write(dir.join("fonts/Inter-Regular.ttf"), &patched).unwrap();
    let e = extract(&project, &dir, &save_opts("With font", None)).unwrap_err();
    assert!(matches!(e, TemplateError::FontLicense { .. }), "{e}");
    assert!(e.to_string().contains("--skip-unlicensed-fonts"), "{e}");

    // Saved without it (ADR 0028): left out, listed as required. Renamed
    // to a family that is not bundled ("Brand").
    let utf16 = |s: &str| {
        s.encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<u8>>()
    };
    let (from, to) = (utf16("Inter"), utf16("Brand"));
    let mut pos = 0;
    while pos + from.len() <= patched.len() {
        if patched[pos..pos + from.len()] == from[..] {
            patched[pos..pos + from.len()].copy_from_slice(&to);
        }
        pos += 1;
    }
    std::fs::write(dir.join("fonts/Inter-Regular.ttf"), &patched).unwrap();
    let mut styles = project.styles.clone();
    styles.text.get_mut("body").unwrap().font = Some("Brand".into());
    styles.text.insert(
        "sys".into(),
        crate::model::TextStyle {
            font: Some("Some System Font".into()),
            ..Default::default()
        },
    );
    project.apply(crate::ops::Op::SetStyles { styles }).unwrap();
    let mut opts = save_opts("Without font", None);
    opts.skip_unlicensed_fonts = true;
    let (tpl, report) = extract(&project, &dir, &opts).unwrap();
    assert!(tpl.manifest.fonts.is_empty(), "{:?}", tpl.manifest.fonts);
    assert!(tpl.paths().all(|p| !p.starts_with("fonts/")));
    assert_eq!(report.fonts_skipped.len(), 1);
    assert_eq!(report.fonts_skipped[0].0, "fonts/Inter-Regular.ttf");
    assert_eq!(
        tpl.manifest.fonts_required,
        [
            RequiredFont {
                family: "Brand".into(),
                reason: RequiredReason::License
            },
            RequiredFont {
                family: "Some System Font".into(),
                reason: RequiredReason::NotInProject
            }
        ]
    );
    tpl.check().unwrap();
    let missing = unavailable_fonts(
        &tpl.manifest.fonts_required,
        &crate::layout::FontLibrary::bundled(),
        &crate::layout::system::SystemFontIndex::default(),
    );
    assert_eq!(missing, ["Brand", "Some System Font"]);
}

#[test]
fn server_side_placeholders_replace_uploaded_screenshots() {
    let p = example();
    let (mut pkg, _) = extract(
        &p,
        &example_dir(),
        &save_opts("Fernwood", Some(vec!["phone"])),
    )
    .unwrap();
    let shot = pkg.screenshots().next().unwrap().to_owned();
    let real = std::fs::read(example_dir().join("screenshots/en/ios-phone/planner.png")).unwrap();
    pkg.put(&shot, real.clone()).unwrap();
    pkg.regenerate_placeholders(false).unwrap();
    assert_ne!(pkg.file(&shot), Some(real.as_slice()));
    pkg.check().unwrap();
}

fn with_samples(canvases: Option<Vec<&str>>) -> Package {
    let mut opts = save_opts("Fernwood samples", canvases);
    opts.sample_screenshots = true;
    let (pkg, report) = extract(&example(), &example_dir(), &opts).unwrap();
    assert_ne!(report.samples.len(), 0);
    assert_eq!(report.samples, pkg.manifest.samples);
    pkg
}

#[test]
fn sample_screenshots_are_packed_and_listed() {
    let pkg = with_samples(Some(vec!["phone"]));
    // The project's own files, byte for byte.
    for s in &pkg.manifest.samples {
        let real = std::fs::read(example_dir().join(s)).unwrap();
        assert_eq!(pkg.file(s), Some(real.as_slice()), "{s}");
    }
    assert!(
        pkg.manifest
            .samples
            .contains(&"screenshots/en/ios-phone/planner.png".to_owned()),
        "{:?}",
        pkg.manifest.samples
    );
    let bytes = pkg.to_zip_bytes().unwrap();
    let back = Package::from_zip_bytes(&bytes, Limits::default()).unwrap();
    assert_eq!(back.manifest.samples, pkg.manifest.samples);
    let yaml = manifest_to_yaml(&pkg.manifest).unwrap();
    assert!(yaml.contains("samples:"), "{yaml}");

    // The server keeps confirmed samples and drops unconfirmed ones.
    let mut kept = pkg.clone();
    kept.regenerate_placeholders(true).unwrap();
    assert_eq!(kept.manifest.samples, pkg.manifest.samples);
    for s in &pkg.manifest.samples {
        assert_eq!(kept.file(s), pkg.file(s));
    }
    kept.check().unwrap();
    let mut dropped = pkg.clone();
    dropped.regenerate_placeholders(false).unwrap();
    assert_eq!(dropped.manifest.samples.len(), 0);
    assert!(
        dropped.screenshots().all(
            |s| !pkg.manifest.samples.contains(&s.to_owned()) || dropped.file(s) != pkg.file(s)
        )
    );
    dropped.check().unwrap();
}

#[test]
fn listed_samples_must_exist_and_be_screenshots() {
    let mut pkg = with_samples(Some(vec!["phone"]));
    pkg.manifest.samples.push("assets/icon-good.svg".into());
    assert!(
        pkg.check()
            .unwrap_err()
            .to_string()
            .contains("not a PNG/JPEG")
    );
    pkg.manifest.samples.pop();
    pkg.manifest.samples.push("screenshots/en/none.png".into());
    assert!(pkg.check().unwrap_err().to_string().contains("missing"));
}

#[test]
fn new_project_copies_and_marks_samples() {
    let pkg = with_samples(Some(vec!["phone"]));
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    let langs = parse_langs(&["de".into(), "en".into()]).unwrap();
    let created = create_project(&pkg, &dir, "P", &langs).unwrap();
    assert_ne!(created.samples.len(), 0);
    let marks = crate::samples::SampleMarks::load(&dir);
    assert_eq!(marks.template, pkg.manifest.id);
    for s in &created.samples {
        assert!(marks.is_sample(&dir, s), "{s}");
    }
    // English samples stay English; the default language `de` gets the
    // samples of the template language serving it.
    assert!(
        created
            .samples
            .iter()
            .any(|s| s.starts_with("screenshots/en/"))
    );
    assert!(
        created
            .samples
            .iter()
            .any(|s| s.starts_with("screenshots/de/"))
    );
    // Validation reports them as info.
    let fs = crate::resolve::FsAssets::new(&dir);
    let report = crate::validate::validate(
        &created.project,
        &crate::validate::ValidateOptions::new(&fs),
    );
    let found: Vec<_> = report
        .with_code(crate::validate::FindingCode::SampleScreenshot)
        .collect();
    assert_ne!(found.len(), 0);
    assert!(
        found
            .iter()
            .all(|f| f.level == crate::validate::Severity::Info)
    );
    // Replacing a file un-marks it.
    let first = &created.samples[0];
    std::fs::write(dir.join(first), b"\x89PNG\r\n\x1a\nmine").unwrap();
    assert!(!crate::samples::SampleMarks::load(&dir).is_sample(&dir, first));
}

#[test]
fn insert_copies_samples_without_replacing_screenshots() {
    let pkg = with_samples(Some(vec!["phone"]));
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("p");
    let langs = parse_langs(&["en".into()]).unwrap();
    let minimal = builtin::package("minimal").unwrap();
    let created = create_project(&minimal, &dir, "P", &langs).unwrap();
    // The project already has its own `planner` screenshot.
    std::fs::create_dir_all(dir.join("screenshots/en/ios-phone")).unwrap();
    std::fs::write(dir.join("screenshots/en/ios-phone/planner.jpg"), b"mine").unwrap();
    let plan = plan_insert(&created.project, &dir, &pkg, &InsertOptions::default()).unwrap();
    assert_ne!(plan.report.samples.len(), 0);
    assert!(
        !plan
            .report
            .samples
            .contains(&"screenshots/en/ios-phone/planner.png".to_owned())
    );
    for f in &plan.files {
        let dest = dir.join(&f.to);
        std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
        std::fs::write(dest, pkg.file(&f.from).unwrap()).unwrap();
    }
    insert::mark_samples(&dir, &pkg, &plan).unwrap();
    let marks = crate::samples::SampleMarks::load(&dir);
    assert_eq!(marks.files.len(), plan.report.samples.len());
}

#[test]
fn manifest_tags_are_checked() {
    let mut pkg = builtin::package("phone").unwrap();
    pkg.manifest.tags.style = vec!["fancy".into()];
    let e = pkg.check().unwrap_err().to_string();
    assert!(e.contains("unknown style tag `fancy`"), "{e}");
    assert!(e.contains("minimal, bold"), "{e}");
}

#[test]
fn template_texts_keep_span_styles_and_text_elements() {
    let pkg = builtin::package("phone").unwrap();
    let project = pkg.project().unwrap();
    let mut texts = 0;
    project.canvases[0].walk_elements(&mut |e| {
        if matches!(e.kind, ElementKind::Text(_)) {
            texts += 1;
        }
    });
    assert_eq!(texts, 3);
}
