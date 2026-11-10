//! End-to-end tests of the `arrangia` binary: `new` → `validate` →
//! `render` → `export` in a temporary folder, exit codes and JSON output.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn arrangia(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arrangia"))
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("run arrangia")
}

fn arrangia_in(project: &Path, args: &[&str]) -> Output {
    let mut all = vec!["--project", project.to_str().expect("utf-8 path")];
    all.extend_from_slice(args);
    arrangia(&all)
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn code(o: &Output) -> i32 {
    o.status.code().expect("exit code")
}

/// Width and height from a PNG's IHDR chunk.
fn png_size(path: &Path) -> (u32, u32) {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    assert_eq!(
        &bytes[..8],
        b"\x89PNG\r\n\x1a\n",
        "{} is not a PNG",
        path.display()
    );
    let be = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
    (be(16), be(20))
}

fn new_project(tmp: &tempfile::TempDir, args: &[&str]) -> PathBuf {
    let dir = tmp.path().join("app");
    let mut all = vec!["new", dir.to_str().unwrap()];
    all.extend_from_slice(args);
    let o = arrangia(&all);
    assert_eq!(code(&o), 0, "new failed: {}", stderr(&o));
    dir
}

fn repo_file(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

#[test]
fn new_creates_project_folder() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(
        &tmp,
        &["--name", "Grove: eco", "--lang", "en", "--lang", "de"],
    );
    for rel in [
        "project.yaml",
        "i18n/en.yaml",
        "i18n/de.yaml",
        ".gitignore",
        "screenshots/en",
        "assets",
        "fonts",
    ] {
        assert!(dir.join(rel).exists(), "{rel} missing");
    }
    let gitignore = std::fs::read_to_string(dir.join(".gitignore")).unwrap();
    assert!(gitignore.contains("export/") && gitignore.contains(".cache/"));
    let project = arrangia_core::Project::load(&dir).unwrap();
    assert_eq!(project.name, "Grove: eco");
    assert_eq!(project.languages.default.as_str(), "en");
    let targets: Vec<&str> = project.targets.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(targets, ["ios-iphone-69", "play-phone", "blog-header"]);
    assert!(
        std::fs::read_to_string(dir.join("i18n/de.yaml"))
            .unwrap()
            .contains("Deine App")
    );

    // Refuses to overwrite a non-empty folder.
    let o = arrangia(&["new", dir.to_str().unwrap()]);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("not empty"), "{}", stderr(&o));
}

#[test]
fn round_trip_new_validate_export() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(&tmp, &["--lang", "en", "--lang", "de"]);

    // Export mode: missing screenshots are errors, and the only findings.
    let o = arrangia_in(&dir, &["validate", "--format", "json"]);
    assert_eq!(code(&o), 1, "{}", stdout(&o));
    let json: Value = serde_json::from_str(&stdout(&o)).expect("JSON output");
    assert_eq!(json["ok"], false);
    let findings = json["findings"].as_array().unwrap();
    assert_ne!(findings.len(), 0);
    for f in findings {
        assert_eq!(f["code"], "screenshot_missing", "unexpected finding {f}");
    }
    assert_eq!(json["errors"].as_u64().unwrap() as usize, findings.len());

    // Preview mode: warnings only → exit 0, but 1 with --strict.
    let o = arrangia_in(&dir, &["validate", "--preview"]);
    assert_eq!(code(&o), 0, "{}", stdout(&o));
    assert!(stdout(&o).contains("screenshot_missing"));
    assert!(stdout(&o).contains("ios-iphone-69 (de)"));
    let o = arrangia_in(&dir, &["validate", "--preview", "--strict"]);
    assert_eq!(code(&o), 1);

    // Filters.
    let o = arrangia_in(
        &dir,
        &[
            "validate",
            "--format",
            "json",
            "--target",
            "play-phone",
            "--lang",
            "de",
        ],
    );
    let json: Value = serde_json::from_str(&stdout(&o)).unwrap();
    for f in json["findings"].as_array().unwrap() {
        assert_eq!(f["target"], "play-phone");
        assert_eq!(f["lang"], "de");
    }

    // Export with placeholders.
    let o = arrangia_in(&dir, &["export"]);
    assert_eq!(code(&o), 0, "{}\n{}", stdout(&o), stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("play-phone"), "{out}");
    assert!(out.contains("18 files written"), "{out}");
    let export = dir.join("export");
    assert_eq!(
        png_size(&export.join("ios-iphone-69/en/01_welcome.png")),
        (1320, 2868)
    );
    assert_eq!(
        png_size(&export.join("play-phone/de/03_start.png")),
        (1080, 2160)
    );
    assert!(export.join("ios-iphone-69/de/_gallery.png").is_file());
    assert!(export.join("blog-header/en/banner.jpg").is_file());

    // Incremental: nothing changed; --force renders again.
    let o = arrangia_in(&dir, &["export", "--format", "json"]);
    let json: Value = serde_json::from_str(&stdout(&o)).unwrap();
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 18);
    assert!(files.iter().all(|f| f["skipped"] == true));
    let o = arrangia_in(
        &dir,
        &[
            "export",
            "--force",
            "--no-gallery",
            "--target",
            "play-phone",
            "--lang",
            "en",
        ],
    );
    assert_eq!(code(&o), 0);
    assert!(stdout(&o).contains("3 files written"), "{}", stdout(&o));

    // Explicit output folder.
    let other = tmp.path().join("out");
    let o = arrangia_in(
        &dir,
        &[
            "export",
            "-q",
            "--target",
            "blog-header",
            "--out",
            other.to_str().unwrap(),
        ],
    );
    assert_eq!(code(&o), 0);
    assert!(stdout(&o).is_empty(), "{}", stdout(&o));
    assert!(other.join("blog-header/de/banner.jpg").is_file());
}

#[test]
fn minimal_project_with_screenshot_validates() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(&tmp, &["--template", "minimal"]);
    let o = arrangia_in(&dir, &["validate"]);
    assert_eq!(code(&o), 1);
    std::fs::copy(
        repo_file("assets/spike/phone-home.png"),
        dir.join("screenshots/en/home.png"),
    )
    .unwrap();
    let o = arrangia_in(&dir, &["validate"]);
    assert_eq!(code(&o), 0, "{}", stdout(&o));
    assert!(stdout(&o).contains("OK:"), "{}", stdout(&o));
}

#[test]
fn render_writes_pngs_of_the_requested_size() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(&tmp, &[]);

    // Default output path, printed on stdout.
    let o = arrangia_in(&dir, &["render", "--width", "400"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let path = PathBuf::from(stdout(&o).trim());
    assert!(
        path.ends_with(".cache/render/ios-iphone-69_en_01.png"),
        "{path:?}"
    );
    let (w, h) = png_size(&path);
    assert_eq!(w, 400);
    assert_eq!(h, (400.0_f64 * 2868.0 / 1320.0).round() as u32);

    let file = tmp.path().join("p.png");
    let f = file.to_str().unwrap();
    let o = arrangia_in(
        &dir,
        &[
            "render",
            "--target",
            "play-phone",
            "--page",
            "2",
            "--max-edge",
            "500",
            "-o",
            f,
        ],
    );
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(png_size(&file), (250, 500));

    let o = arrangia_in(
        &dir,
        &[
            "render",
            "--target",
            "blog-header",
            "--scale",
            "0.5",
            "-o",
            f,
        ],
    );
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(png_size(&file), (1200, 500));

    let o = arrangia_in(&dir, &["render", "--canvas", "--width", "600", "-o", f]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(png_size(&file).0, 600);

    let o = arrangia_in(&dir, &["render", "--gallery", "--max-edge", "300", "-o", f]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    // Images 300 px high plus the gallery's margins.
    let h = png_size(&file).1;
    assert!((300..400).contains(&h), "{h}");

    // Out of range / unsupported selections are usage errors.
    let o = arrangia_in(&dir, &["render", "--page", "9"]);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("out of range"), "{}", stderr(&o));
    let o = arrangia_in(&dir, &["render", "--target", "blog-header", "--gallery"]);
    assert_eq!(code(&o), 2);
    let o = arrangia_in(&dir, &["render", "--target", "nope"]);
    assert_eq!(code(&o), 2);
    assert!(
        stderr(&o).contains("unknown target `nope`"),
        "{}",
        stderr(&o)
    );
}

#[test]
fn failures_exit_with_code_2() {
    let tmp = tempfile::tempdir().unwrap();
    // No project.
    let o = arrangia_in(tmp.path(), &["validate"]);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("no project.yaml"), "{}", stderr(&o));

    // YAML syntax error: message names the file and the line.
    let dir = new_project(&tmp, &[]);
    let yaml = dir.join("project.yaml");
    let text = std::fs::read_to_string(&yaml).unwrap();
    std::fs::write(&yaml, text.replace("strips:", "strips: [")).unwrap();
    let o = arrangia_in(&dir, &["export"]);
    assert_eq!(code(&o), 2);
    let err = stderr(&o);
    assert!(err.contains("project.yaml"), "{err}");
    assert!(err.contains("line"), "{err}");

    // Usage error (clap).
    let o = arrangia(&["validate", "--format", "xml"]);
    assert_eq!(code(&o), 2);
}

/// The example project (`tests/fixtures/example`) is clean (no errors or
/// warnings) and exports. A full export (8 targets × 4 languages, 136 files)
/// takes about two CPU minutes in a debug build, so this exports the web
/// targets only (browser frame, notebook with browser chrome, JPEG,
/// gallery preview); `crates/render/tests/golden.rs` renders every
/// target at reduced scale.
#[test]
fn example_project_validates_and_exports() {
    let root = repo_file("tests/fixtures/example");
    let o = arrangia_in(&root, &["validate", "--strict"]);
    assert_eq!(code(&o), 0, "{}\n{}", stdout(&o), stderr(&o));
    assert!(
        stdout(&o).contains("0 errors, 0 warnings"),
        "{}",
        stdout(&o)
    );

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("export");
    let o = arrangia_in(
        &root,
        &[
            "export",
            "--target",
            "web-slides",
            "--target",
            "open-graph",
            "--out",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(code(&o), 0, "{}\n{}", stdout(&o), stderr(&o));
    assert!(stdout(&o).contains("16 files written"), "{}", stdout(&o));
    for lang in ["en", "de", "fr", "fr-CA"] {
        let dir = out.join("web-slides").join(lang);
        for file in ["01_plots.jpg", "02_seeds.jpg"] {
            assert!(dir.join(file).is_file(), "{lang}/{file} missing");
        }
        assert!(png_size(&dir.join("_gallery.png")).1 > 0);
        assert!(
            out.join("open-graph")
                .join(lang)
                .join("banner.jpg")
                .is_file()
        );
    }
}

/// `frames import` of the synthetic bezel package into a temporary app
/// folder (never the real one): both iPhone sizes, licence notice,
/// Photoshop files counted separately, `frames remove --bezel`.
#[test]
fn frames_import_keeps_all_sizes_and_shows_the_licence() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = tmp.path().join("Bezels");
    arrangia_core::frame_import::synthetic::write_folder(
        &pkg,
        &arrangia_core::frame_import::synthetic::default_specs(),
    )
    .unwrap();
    let app = tmp.path().join("appdir");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_arrangia"))
            .args(args)
            .env("NO_COLOR", "1")
            .env("ARRANGIA_APP_DIR", &app)
            .output()
            .expect("run arrangia")
    };
    let pkg_s = pkg.to_str().unwrap();
    let o = run(&["frames", "import", pkg_s, "--dry-run"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("would import 7 frames"), "{out}");
    assert!(out.contains("2 Photoshop file(s) ignored"), "{out}");
    assert!(out.contains("1 other file(s) skipped"), "{out}");
    assert!(
        out.contains("subject to Apple's licence (License.rtf"),
        "{out}"
    );
    assert!(out.contains("never in projects"), "{out}");
    assert!(out.contains("/ iPhone 16 Pro "), "{out}");
    assert!(!app.exists(), "a dry run writes nothing");

    let o = run(&["frames", "import", pkg_s]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let o = run(&["frames", "list", "--format", "json"]);
    let list: Value = serde_json::from_slice(&o.stdout).unwrap();
    let models: Vec<&str> = list
        .as_array()
        .unwrap()
        .iter()
        .filter(|f| f["model"] == "iphone-island" && f["orientation"] == "portrait")
        .filter(|f| f["color"] == "black-titanium")
        .map(|f| f["apple_model"].as_str().unwrap())
        .collect();
    assert_eq!(models, ["iPhone 16 Pro", "iPhone 16 Pro Max"]);
    let o = run(&["frames", "remove", "--bezel", "iPhone 16 Pro"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stdout(&o).contains("removed 2 frames"), "{}", stdout(&o));
}
