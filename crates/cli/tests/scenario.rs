//! Phase 3 "done when" scenario (KONZEPT §12) on a copy of the example
//! project: three languages × three targets export correctly with their
//! fallbacks, and a text that does not fit is reported as an overflow.
//!
//! The targets are the cheap ones of the example (web slides, Open Graph
//! banner, blog header: 5 files per language) because the binary is a
//! debug build here; `crates/render/tests/golden.rs` covers every target
//! in en, de and fr at reduced scale.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn vitrine_in(project: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vitrine"))
        .arg("--project")
        .arg(project)
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("run vitrine")
}

fn text(o: &Output) -> String {
    format!(
        "{}\n{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn json(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap_or_else(|e| panic!("{e}: {}", text(o)))
}

fn example() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example")
}

/// Copy of the example project without golden images and build output.
fn example_copy(tmp: &tempfile::TempDir) -> PathBuf {
    fn copy(from: &Path, to: &Path) {
        fs::create_dir_all(to).unwrap();
        for e in fs::read_dir(from).unwrap().flatten() {
            let name = e.file_name();
            if ["golden", "export", ".cache"].contains(&name.to_string_lossy().as_ref()) {
                continue;
            }
            let p = e.path();
            if p.is_dir() {
                copy(&p, &to.join(&name));
            } else {
                fs::copy(&p, to.join(&name)).unwrap();
            }
        }
    }
    let dir = tmp.path().join("example");
    copy(&example(), &dir);
    dir
}

/// Findings of `validate --format json` with the given code.
fn findings<'a>(report: &'a Value, code: &str) -> Vec<&'a Value> {
    report["findings"]
        .as_array()
        .expect("findings")
        .iter()
        .filter(|f| f["code"] == code)
        .collect()
}

const LANGS: [&str; 3] = ["en", "de", "fr"];
const TARGETS: [(&str, u32, u32); 3] = [
    ("web-slides", 1920, 1080),
    ("open-graph", 1200, 630),
    ("blog-header", 2400, 1000),
];

#[test]
fn three_languages_by_three_targets_export_with_fallbacks() {
    let tmp = tempfile::tempdir().unwrap();
    let project = example_copy(&tmp);

    // The example is clean: fallbacks are deliberate (infos only).
    let o = vitrine_in(&project, &["validate", "--strict", "--format", "json"]);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let report = json(&o);
    assert_eq!(report["errors"], 0);
    assert_eq!(report["warnings"], 0);
    // fr has no web screenshots: the English ones are used.
    let shots = findings(&report, "screenshot_fallback");
    assert!(
        shots
            .iter()
            .any(|f| f["lang"] == "fr" && f["target"] == "web-slides"),
        "{shots:?}"
    );
    // fr has its own phone screens for the first three pages only.
    let fr_phone: Vec<_> = shots
        .iter()
        .filter(|f| f["lang"] == "fr" && f["target"] == "ios-iphone-69")
        .map(|f| f["element"].as_str().unwrap())
        .collect();
    assert_eq!(fr_phone, ["phone-harvest", "phone-calendar"]);
    // fr-CA only overrides two texts; the rest falls back to fr (prefix
    // chain, info), never to the default language (that would be a warning).
    let texts = findings(&report, "text_fallback");
    assert!(texts.len() > 20, "{texts:?}");
    assert!(
        texts.iter().all(|f| f["lang"] == "fr-CA"
            && f["level"] == "info"
            && f["message"].as_str().unwrap().contains("using `fr`")),
        "{texts:?}"
    );
    // German needs automatic hyphenation for one headline.
    assert!(
        findings(&report, "hyphenated")
            .iter()
            .any(|f| f["lang"] == "de" && f["element"] == "title-companions")
    );

    let out = tmp.path().join("export");
    let mut args = vec!["export", "--format", "json", "--out", out.to_str().unwrap()];
    for (t, _, _) in TARGETS {
        args.extend(["--target", t]);
    }
    for l in LANGS {
        args.extend(["--lang", l]);
    }
    let o = vitrine_in(&project, &args);
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
    let report = json(&o);
    assert_eq!(report["errors"].as_array().unwrap().len(), 0);
    let files = report["files"].as_array().unwrap();
    // Per language: 2 slides + gallery, 1 Open Graph banner, 1 blog header.
    assert_eq!(files.len(), LANGS.len() * 5, "{files:?}");
    for lang in LANGS {
        for (target, w, h) in TARGETS {
            let mine: Vec<_> = files
                .iter()
                .filter(|f| f["lang"] == lang && f["target"] == target)
                .collect();
            assert!(!mine.is_empty(), "{target}/{lang}: nothing exported");
            for f in mine {
                let path = f["path"].as_str().unwrap();
                let on_disk = path.split('/').fold(out.clone(), |p, s| p.join(s));
                let bytes = fs::metadata(&on_disk)
                    .unwrap_or_else(|e| panic!("{path}: {e}"))
                    .len();
                assert_eq!(f["bytes"].as_u64(), Some(bytes), "{path}");
                assert!(bytes > 10_000, "{path}: only {bytes} bytes");
                if f["kind"] != "gallery" {
                    assert_eq!(
                        (f["width"].as_u64(), f["height"].as_u64()),
                        (Some(w.into()), Some(h.into())),
                        "{path}"
                    );
                }
            }
        }
    }
    // The language versions differ (texts and screenshots are localised).
    let read =
        |lang: &str| fs::read(out.join("blog-header").join(lang).join("banner.jpg")).unwrap();
    assert_ne!(read("en"), read("de"));
    assert_ne!(read("en"), read("fr"));
    // The export reports the screenshot fallbacks it used.
    let issues = report["issues"].as_array().unwrap();
    assert!(
        issues.iter().any(|i| i["lang"] == "fr"
            && i["target"] == "web-slides"
            && i.to_string().contains("screenshot_fallback")),
        "{issues:?}"
    );
}

#[test]
fn too_long_translation_is_reported_as_overflow() {
    let tmp = tempfile::tempdir().unwrap();
    let project = example_copy(&tmp);
    let fr = project.join("i18n").join("fr.yaml");
    let yaml = fs::read_to_string(&fr).unwrap().replace("\r\n", "\n");
    let long = "plan.title: Votre potager, [accent]planifié en quelques minutes[/], \
                avec toutes vos planches, vos semis et vos récoltes au même endroit\n";
    let yaml: String = yaml
        .lines()
        .map(|l| {
            if l.starts_with("plan.title:") {
                long.to_owned()
            } else {
                format!("{l}\n")
            }
        })
        .collect();
    fs::write(&fr, yaml).unwrap();

    let o = vitrine_in(
        &project,
        &[
            "validate",
            "--target",
            "ios-iphone-69",
            "--lang",
            "fr",
            "--format",
            "json",
        ],
    );
    assert_eq!(o.status.code(), Some(1), "{}", text(&o));
    let report = json(&o);
    let overflow = findings(&report, "text_overflow");
    assert_eq!(overflow.len(), 1, "{overflow:?}");
    let f = overflow[0];
    assert_eq!(f["level"], "error");
    assert_eq!(f["element"], "title-plan");
    assert_eq!(f["lang"], "fr");
    assert!(f["message"].as_str().unwrap().contains("plan.title"));

    // Other languages are not affected.
    let o = vitrine_in(
        &project,
        &["validate", "--target", "ios-iphone-69", "--lang", "de"],
    );
    assert_eq!(o.status.code(), Some(0), "{}", text(&o));
}
