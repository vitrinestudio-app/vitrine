//! YAML round-trip tests on the sample project in `tests/data/sample`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use vitrine_core::Project;
use vitrine_core::io::{self, IoError};
use vitrine_core::model::{ElementKind, Lang, StringKey};

fn sample_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("data")
        .join("sample")
}

/// Reads a checked-in text file with LF line endings. The repository
/// forces LF (`.gitattributes`), but a checkout made with
/// `core.autocrlf=true` before that (or by other tools) has CRLF, while
/// the writer always emits LF; the byte-exact comparisons below are about
/// the YAML content, not the checkout's line endings.
fn read_lf(path: &Path) -> String {
    fs::read_to_string(path).unwrap().replace("\r\n", "\n")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

#[test]
fn canonical_file_is_reproduced_byte_for_byte() {
    let dir = sample_dir();
    let project = Project::load(&dir).unwrap();
    let original = read_lf(&dir.join("project.yaml"));
    assert_eq!(project.to_yaml().unwrap(), original);
    for (lang, strings) in &project.strings {
        let file = read_lf(&dir.join("i18n").join(format!("{lang}.yaml")));
        assert_eq!(io::to_yaml(strings).unwrap(), file, "i18n/{lang}.yaml");
    }
}

#[test]
fn load_save_load_is_equal_and_bumps_revision() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&sample_dir(), tmp.path());
    let mut project = Project::load(tmp.path()).unwrap();
    assert_eq!(project.revision, 7);
    project.save(tmp.path()).unwrap();
    assert_eq!(project.revision, 8);

    let reloaded = Project::load(tmp.path()).unwrap();
    assert_eq!(reloaded, project);

    // Only the revision line differs from the canonical sample.
    let saved = fs::read_to_string(tmp.path().join("project.yaml")).unwrap();
    let original = read_lf(&sample_dir().join("project.yaml"));
    assert_eq!(saved.replace("revision: 8", "revision: 7"), original);

    // No temp files are left behind.
    let leftovers: Vec<_> = fs::read_dir(tmp.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty());
}

#[test]
fn save_detects_conflicting_revision() {
    let tmp = tempfile::tempdir().unwrap();
    copy_dir(&sample_dir(), tmp.path());
    let mut a = Project::load(tmp.path()).unwrap();
    let mut b = Project::load(tmp.path()).unwrap();
    a.save(tmp.path()).unwrap();
    let err = b.save(tmp.path()).unwrap_err();
    assert!(
        matches!(
            err,
            IoError::Conflict {
                expected: 7,
                found: 8
            }
        ),
        "{err}"
    );
    assert_eq!(b.revision, 7, "a failed save does not change the revision");
}

#[test]
fn new_project_saves_into_empty_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("new.vitrine");
    let mut p = Project::new("demo", Lang::new("en"));
    p.strings
        .entry(Lang::new("en"))
        .or_default()
        .insert(StringKey::new("a.b"), "Hello".into());
    p.save(&dir).unwrap();
    assert_eq!(p.revision, 1);
    assert_eq!(
        fs::read_to_string(dir.join("project.yaml")).unwrap(),
        "format: vitrine/1\nrevision: 1\nname: demo\nlanguages:\n  default: en\n  available: [en]\n"
    );
    assert_eq!(
        fs::read_to_string(dir.join("i18n/en.yaml")).unwrap(),
        "a.b: Hello\n"
    );
    assert_eq!(Project::load(&dir).unwrap(), p);
}

#[test]
fn sample_model_content() {
    let p = Project::load(sample_dir()).unwrap();
    assert_eq!(p.canvases.len(), 2);
    let (canvas, phone) = p.element("hero-phone").unwrap();
    assert_eq!(canvas.id.as_str(), "phone");
    assert!(matches!(phone.kind, ElementKind::Device(_)));
    // Group children are found as well.
    assert!(p.element("badge-ring").is_some());
    assert_eq!(
        p.string("de", "hero.title"),
        Some("Plane deine Beete **in Minuten**")
    );
    let variant = &canvas.variants[&vitrine_core::model::VariantId::new("tablet-landscape")];
    assert_eq!(variant.overrides.len(), 3);
}

#[test]
fn tricky_strings_round_trip() {
    let mut map = BTreeMap::new();
    for (i, s) in [
        "**Bold** first",
        "[accent]Span[/] first",
        "_italic_ first",
        "Colon: inside",
        "Line one\\nline two",
        "Real\nnewline",
        "Soft\\-hyphen and Misch\u{00AD}kultur",
        "# not a comment",
        "yes",
        "no",
        "1320:2868",
        "  leading spaces",
        "\"quoted\"",
        "it's",
        "",
        "@name",
        "- dash",
    ]
    .into_iter()
    .enumerate()
    {
        map.insert(StringKey::new(&format!("k.{i:02}")), s.to_owned());
    }
    let yaml = io::to_yaml(&map).unwrap();
    let back = io::strings_from_yaml(&yaml).unwrap();
    assert_eq!(back, map, "{yaml}");
    assert_eq!(io::to_yaml(&back).unwrap(), yaml);
}

#[test]
fn snapshot_sample_yaml() {
    let p = Project::load(sample_dir()).unwrap();
    insta::assert_snapshot!("sample_project_yaml", p.to_yaml().unwrap());
}

fn load_err(yaml: &str) -> String {
    let tmp = tempfile::tempdir().unwrap();
    fs::write(tmp.path().join("project.yaml"), yaml).unwrap();
    Project::load(tmp.path()).unwrap_err().to_string()
}

const HEAD: &str = "format: vitrine/1\nname: x\nlanguages: { default: en, available: [en] }\n";

#[test]
fn errors_name_file_line_and_field() {
    let err = load_err(&format!(
        "{HEAD}canvases:\n  - id: s\n    page: {{ aspect: \"1:2\", count: 1 }}\n    elemnts: []\n"
    ));
    assert!(err.contains("project.yaml"), "{err}");
    assert!(err.contains("unknown field `elemnts`"), "{err}");
    assert!(err.contains("line 7"), "{err}");

    let err = load_err(&format!(
        "{HEAD}canvases:\n  - id: s\n    page: {{ aspect: \"1:2\", count: 1 }}\n    elements:\n      - {{ id: t, kind: text, text: a, model: x }}\n"
    ));
    assert!(
        err.contains("field `model` is not allowed on text elements"),
        "{err}"
    );

    let err = load_err(&format!(
        "{HEAD}canvases:\n  - id: s\n    page: {{ aspect: \"1:2\", count: 1 }}\n    backgrounds: [\"red\"]\n"
    ));
    assert!(err.contains("invalid colour `red`"), "{err}");

    let err = load_err("format: vitrine/9\nname: x\nlanguages: { default: en, available: [en] }\n");
    assert!(err.contains("unsupported format"), "{err}");

    let err =
        load_err("format: arrangia/1\nname: x\nlanguages: { default: en, available: [en] }\n");
    assert!(
        err.contains("project from Arrangia") && err.contains("`vitrine/1`"),
        "{err}"
    );

    // Before 2026-11-11 the canvas was called "strip": say what to rename.
    let old = "strips:\n  - id: s\n    page: { aspect: \"1:2\", count: 1 }\ntargets:\n  - id: t\n    strip: s\n    size: [100, 200]\n";
    let err = load_err(&format!("{HEAD}{old}"));
    assert!(
        err.contains("`strips:` to `canvases:`")
            && err.contains("`strip:` to `canvas:`")
            && !err.contains("format:"),
        "{err}"
    );
    let err = load_err(&format!(
        "format: arrangia/1\nname: x\nlanguages: {{ default: en, available: [en] }}\n{old}"
    ));
    assert!(
        err.contains("`canvases:`") && err.contains("`format:` to `vitrine/1`"),
        "{err}"
    );

    let err = load_err(&format!(
        "{HEAD}canvases:\n  - id: s\n    page: {{ aspect: \"1:2\", count: 1 }}\n    elements:\n      - {{ id: a, kind: text, text: a }}\n      - {{ id: a, kind: text, text: b }}\n"
    ));
    assert!(err.contains("duplicate element id `a`"), "{err}");

    let err = load_err("format: vitrine/1\nname: x\nlanguages: { default: de, available: [en] }\n");
    assert!(err.contains("default language `de`"), "{err}");
}

#[test]
fn yaml_12_booleans_only() {
    // `y`/`no` are strings, not booleans (YAML 1.2 core schema).
    let p = io::project_from_yaml(&format!(
        "{HEAD}canvases:\n  - id: s\n    page: {{ aspect: \"1:2\", count: 1 }}\n    elements:\n      - {{ id: d, kind: device, model: no, at: {{ x: 1, y: 2 }} }}\n"
    ))
    .unwrap();
    let ElementKind::Device(d) = &p.canvases[0].elements[0].kind else {
        panic!()
    };
    assert_eq!(d.model, "no");
    assert!(io::project_from_yaml(&format!(
        "{HEAD}canvases:\n  - id: s\n    page: {{ aspect: \"1:2\", count: 1 }}\n    elements:\n      - {{ id: d, kind: text, text: a, visible: yes }}\n"
    ))
    .is_err());
}

#[test]
fn concept_example_loads() {
    // The `project.yaml` example of KONZEPT §3.7, verbatim (with comments).
    let yaml = include_str!("data/konzept-3.7.yaml");
    let p = io::project_from_yaml(yaml).unwrap();
    assert_eq!(p.targets.len(), 4);
    assert_eq!(p.canvases[0].elements.len(), 2);
    let again = io::project_from_yaml(&p.to_yaml().unwrap()).unwrap();
    assert_eq!(again, p);
}
