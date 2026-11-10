//! `arrangia import screenshots` end to end: a fastlane-style folder made
//! from the example project's screenshots, imported into a copy of the
//! example project without screenshots.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn arrangia_in(project: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arrangia"))
        .arg("--project")
        .arg(project)
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("run arrangia")
}

fn out(o: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example")
}

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            copy_dir(&p, &to.join(e.file_name()));
        } else {
            fs::copy(&p, to.join(e.file_name())).unwrap();
        }
    }
}

/// Project copy without screenshots + fastlane folder with the example's
/// iPhone screenshots (`en-US`, `de-DE`), a tablet only matched by its
/// aspect, an unknown device, a report and a framed file.
fn setup(tmp: &Path) -> (PathBuf, PathBuf) {
    let project = tmp.join("app");
    fs::create_dir_all(&project).unwrap();
    fs::copy(example().join("project.yaml"), project.join("project.yaml")).unwrap();
    copy_dir(&example().join("i18n"), &project.join("i18n"));

    let src = tmp.join("fastlane");
    let shots = example().join("screenshots");
    for (lang, locale) in [("en", "en-US"), ("de", "de-DE")] {
        let dir = src.join(locale);
        fs::create_dir_all(&dir).unwrap();
        for (i, key) in ["home", "planner", "harvest"].iter().enumerate() {
            fs::copy(
                shots
                    .join(lang)
                    .join("ios-phone")
                    .join(format!("{key}.png")),
                dir.join(format!("iPhone 17 Pro-{:02}_{key}.png", i + 1)),
            )
            .unwrap();
        }
    }
    fs::copy(
        shots.join("en/ios-tablet/home.png"),
        src.join("en-US/Mystery Pad-01_home.png"),
    )
    .unwrap();
    fs::copy(
        shots.join("en/ios-phone/home.png"),
        src.join("en-US/iPhone 17 Pro-01_home_framed.png"),
    )
    .unwrap();
    // A device no catalog size or aspect matches (PNG header only).
    let mut watch = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
    watch.extend_from_slice(&10u32.to_be_bytes());
    watch.extend_from_slice(&90u32.to_be_bytes());
    watch.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    fs::write(src.join("en-US/Apple Watch (46mm)-09_watch.png"), watch).unwrap();
    fs::write(src.join("screenshots.html"), "<html></html>").unwrap();
    (project, src)
}

#[test]
fn imports_fastlane_output() {
    let tmp = tempfile::tempdir().unwrap();
    let (project, src) = setup(tmp.path());
    let src_s = src.to_str().unwrap();

    // Dry run: JSON plan, one unresolved device → exit 1, nothing copied.
    let o = arrangia_in(
        &project,
        &[
            "import",
            "screenshots",
            src_s,
            "--dry-run",
            "--format-output",
            "json",
        ],
    );
    assert_eq!(o.status.code(), Some(1), "{}", out(&o));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["plan"]["format"], "fastlane");
    assert_eq!(v["ok"], false);
    let items = v["plan"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 8, "{items:#?}");
    let dest = |source: &str| {
        items
            .iter()
            .find(|i| i["source"] == source)
            .and_then(|i| i["dest"].as_str())
            .map(str::to_owned)
    };
    assert_eq!(
        dest("de-DE/iPhone 17 Pro-01_home.png").as_deref(),
        Some("screenshots/de/ios-phone/home.png")
    );
    assert_eq!(
        dest("en-US/Mystery Pad-01_home.png").as_deref(),
        Some("screenshots/en/ios-tablet/home.png")
    );
    assert_eq!(dest("en-US/Apple Watch (46mm)-09_watch.png"), None);
    assert_eq!(v["plan"]["skipped"].as_array().unwrap().len(), 2);
    assert!(!project.join("screenshots").exists());

    // Text plan with a device map for the unknown device: exit 0, copied,
    // mapping saved.
    let o = arrangia_in(
        &project,
        &[
            "import",
            "screenshots",
            src_s,
            "--device-map",
            "Apple Watch (46mm)=ios-phone",
            "--save-mapping",
        ],
    );
    let text = out(&o);
    assert_eq!(o.status.code(), Some(0), "{text}");
    assert!(text.contains("de · ios-phone"), "{text}");
    assert!(text.contains("8 new"), "{text}");
    assert!(text.contains("copied 8 files"), "{text}");
    assert!(text.contains("missing"), "{text}");
    for f in ["en/ios-tablet/home.png", "en/ios-phone/watch.png"] {
        assert!(project.join("screenshots").join(f).is_file(), "{f}");
    }
    assert_eq!(
        fs::read(project.join("screenshots/de/ios-phone/planner.png")).unwrap(),
        fs::read(example().join("screenshots/de/ios-phone/planner.png")).unwrap()
    );
    let yaml = fs::read_to_string(project.join("project.yaml")).unwrap();
    assert!(yaml.contains("import:"), "{yaml}");
    assert!(yaml.contains("Apple Watch (46mm): ios-phone"), "{yaml}");
    assert!(yaml.contains("Mystery Pad: ios-tablet"), "{yaml}");
    assert!(yaml.contains("de-DE: de"), "{yaml}");

    // Again, with the saved mapping: everything unchanged.
    let o = arrangia_in(&project, &["import", "screenshots", src_s]);
    assert_eq!(o.status.code(), Some(0), "{}", out(&o));
    assert!(out(&o).contains("8 unchanged"), "{}", out(&o));
    assert!(out(&o).contains("copied 0 files"), "{}", out(&o));

    // Missing source folder: exit 2.
    let o = arrangia_in(&project, &["import", "screenshots", "/nonexistent/shots"]);
    assert_eq!(o.status.code(), Some(2), "{}", out(&o));
    // Bad mapping syntax: exit 2.
    let o = arrangia_in(
        &project,
        &["import", "screenshots", src_s, "--device-map", "nope"],
    );
    assert_eq!(o.status.code(), Some(2), "{}", out(&o));
}
