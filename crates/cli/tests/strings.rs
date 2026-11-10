//! `arrangia strings export|import` on a copy of the example project.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn arrangia(project: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arrangia"))
        .arg("--project")
        .arg(project)
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("run arrangia")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        let target = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            if e.file_name() != "golden" {
                copy_dir(&e.path(), &target);
            }
        } else {
            std::fs::copy(e.path(), target).unwrap();
        }
    }
}

fn example_copy(tmp: &tempfile::TempDir) -> PathBuf {
    let dir = tmp.path().join("example");
    copy_dir(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example"),
        &dir,
    );
    dir
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn export_then_import_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let project = example_copy(&tmp);
    let out = tmp.path().join("out");
    std::fs::create_dir_all(&out).unwrap();
    for format in ["csv", "xliff12", "xliff20"] {
        let dir = out.join(format);
        std::fs::create_dir_all(&dir).unwrap();
        let o = arrangia(
            &project,
            &[
                "strings",
                "export",
                "--format",
                format,
                "--context",
                "--out",
                s(&dir),
            ],
        );
        assert_eq!(code(&o), 0, "{}", String::from_utf8_lossy(&o.stderr));
        assert!(stdout(&o).contains("wrote"), "{}", stdout(&o));
    }
    let csv = out.join("csv/Fernwood-translations.csv");
    let x12 = out.join("xliff12/Fernwood-de.xlf");
    let x20 = out.join("xliff20/Fernwood-de.xlf");
    let o = arrangia(
        &project,
        &["strings", "import", s(&csv), s(&x12), s(&x20), "--json"],
    );
    assert_eq!(code(&o), 0, "{}", stdout(&o));
    let v: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(v["applied"], false);
    assert_eq!(v["plan"]["changes"].as_array().unwrap().len(), 0);
    assert_eq!(v["plan"]["files"][1]["format"], "xliff12");
    assert_eq!(v["plan"]["files"][2]["format"], "xliff20");
    assert_eq!(v["plan"]["langs"][1]["unchanged"], 26);
}

#[test]
fn import_applies_and_reports_overflow() {
    let tmp = tempfile::tempdir().unwrap();
    let project = example_copy(&tmp);
    let csv = tmp.path().join("de.csv");
    std::fs::write(
        &csv,
        "\u{FEFF}key;de;notes\r\nplan.sub;Beete und Aufgaben;\r\nbeds.title;\"Plane jedes einzelne Beet ganz bequem **per Drag & Drop** auf deinem Tablet\";zu lang?\r\nnope;x;\r\n",
    )
    .unwrap();
    let before = std::fs::read_to_string(project.join("i18n/de.yaml")).unwrap();

    let o = arrangia(&project, &["strings", "import", s(&csv), "--dry-run"]);
    let out = stdout(&o);
    assert_eq!(code(&o), 1, "new overflow → exit 1\n{out}");
    assert!(out.contains("lang de: 0 new, 2 changed"), "{out}");
    assert!(out.contains("text_overflow"), "{out}");
    assert!(out.contains("unknown key `nope`"), "{out}");
    assert!(out.contains("dry run"), "{out}");
    assert_eq!(
        std::fs::read_to_string(project.join("i18n/de.yaml")).unwrap(),
        before
    );

    let o = arrangia(
        &project,
        &["strings", "import", s(&csv), "--no-layout-check"],
    );
    assert_eq!(code(&o), 0, "{}", stdout(&o));
    assert!(stdout(&o).contains("Imported: 2 changes applied"));
    let de = std::fs::read_to_string(project.join("i18n/de.yaml")).unwrap();
    assert!(de.contains("plan.sub: Beete und Aufgaben"), "{de}");

    // Second run: nothing left to do.
    let o = arrangia(
        &project,
        &["strings", "import", s(&csv), "--no-layout-check"],
    );
    assert!(stdout(&o).contains("nothing to change"), "{}", stdout(&o));
}

#[test]
fn xliff_per_language_names_and_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let project = example_copy(&tmp);
    let target = tmp.path().join("t/strings.xlf");
    let o = arrangia(
        &project,
        &[
            "strings",
            "export",
            "--out",
            s(&target),
            "--missing-only",
            "--json",
        ],
    );
    assert_eq!(code(&o), 0);
    let v: Value = serde_json::from_str(&stdout(&o)).unwrap();
    // One file per language (`strings-<lang>.xlf`): de and fr are complete,
    // fr-CA only overrides two texts (the rest falls back to fr).
    let keys: Vec<_> = v
        .as_array()
        .unwrap()
        .iter()
        .map(|f| {
            (
                f["file"]["lang"].as_str().unwrap(),
                f["file"]["keys"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(keys[..2], [("de", 0), ("fr", 0)], "{keys:?}");
    assert_eq!(keys[2].0, "fr-CA");
    assert!(keys[2].1 > 20, "{keys:?}");
    for lang in ["de", "fr", "fr-CA"] {
        assert!(tmp.path().join(format!("t/strings-{lang}.xlf")).is_file());
    }
    assert!(!target.exists());

    let o = arrangia(&project, &["strings", "export", "--lang", "xx"]);
    assert_eq!(code(&o), 2);
    let bad = tmp.path().join("bad.xlf");
    std::fs::write(&bad, "<xliff version=\"2.0\"><file").unwrap();
    let o = arrangia(&project, &["strings", "import", s(&bad)]);
    assert_eq!(code(&o), 2, "{}", stdout(&o));
}
