//! End-to-end tests of the `vitrine` binary: `new` → `validate` →
//! `render` → `export` in a temporary folder, exit codes and JSON output.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

fn vitrine(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vitrine"))
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .expect("run vitrine")
}

fn vitrine_in(project: &Path, args: &[&str]) -> Output {
    let mut all = vec!["--project", project.to_str().expect("utf-8 path")];
    all.extend_from_slice(args);
    vitrine(&all)
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
    let o = vitrine(&all);
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
    let project = vitrine_core::Project::load(&dir).unwrap();
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
    let o = vitrine(&["new", dir.to_str().unwrap()]);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("not empty"), "{}", stderr(&o));
}

#[test]
#[allow(clippy::too_many_lines)]
fn round_trip_new_validate_export() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(&tmp, &["--lang", "en", "--lang", "de"]);

    // Export mode: missing screenshots are errors, and the only findings.
    let o = vitrine_in(&dir, &["validate", "--format", "json"]);
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
    let o = vitrine_in(&dir, &["validate", "--preview"]);
    assert_eq!(code(&o), 0, "{}", stdout(&o));
    assert!(stdout(&o).contains("screenshot_missing"));
    assert!(stdout(&o).contains("ios-iphone-69 (de)"));
    let o = vitrine_in(&dir, &["validate", "--preview", "--strict"]);
    assert_eq!(code(&o), 1);

    // Filters.
    let o = vitrine_in(
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
    let o = vitrine_in(&dir, &["export"]);
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
    let o = vitrine_in(&dir, &["export", "--format", "json"]);
    let json: Value = serde_json::from_str(&stdout(&o)).unwrap();
    let files = json["files"].as_array().unwrap();
    assert_eq!(files.len(), 18);
    assert!(files.iter().all(|f| f["skipped"] == true));
    let o = vitrine_in(
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
    let o = vitrine_in(
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
    assert!(other.join("index.html").is_file());
    assert!(export.join("index.html").is_file());

    // fastlane layout: deliver/supply folders, the blog banner left out.
    let o = vitrine_in(
        &dir,
        &[
            "export",
            "--layout",
            "fastlane",
            "--lang",
            "de",
            "--no-html",
        ],
    );
    assert_eq!(code(&o), 0, "{}\n{}", stdout(&o), stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("no store slot"), "{out}");
    let fl = export.join("fastlane");
    assert_eq!(
        png_size(&fl.join("screenshots/de-DE/01_ios-iphone-69.png")),
        (1320, 2868)
    );
    assert_eq!(
        png_size(&fl.join("metadata/android/de-DE/images/phoneScreenshots/03_de-DE.png")),
        (1080, 2160)
    );
    assert!(!fl.join("index.html").exists());
    assert!(!fl.join("screenshots/de-DE/_gallery.png").exists());
}

#[test]
fn minimal_project_with_screenshot_validates() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(&tmp, &["--template", "minimal"]);
    let o = vitrine_in(&dir, &["validate"]);
    assert_eq!(code(&o), 1);
    std::fs::copy(
        repo_file("tests/fixtures/example/screenshots/en/ios-phone/home.png"),
        dir.join("screenshots/en/home.png"),
    )
    .unwrap();
    let o = vitrine_in(&dir, &["validate"]);
    assert_eq!(code(&o), 0, "{}", stdout(&o));
    assert!(stdout(&o).contains("OK:"), "{}", stdout(&o));
}

#[test]
fn render_writes_pngs_of_the_requested_size() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(&tmp, &[]);

    // Default output path, printed on stdout.
    let o = vitrine_in(&dir, &["render", "--width", "400"]);
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
    let o = vitrine_in(
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

    let o = vitrine_in(
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

    let o = vitrine_in(&dir, &["render", "--canvas", "--width", "600", "-o", f]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert_eq!(png_size(&file).0, 600);

    let o = vitrine_in(&dir, &["render", "--gallery", "--max-edge", "300", "-o", f]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    // Images 300 px high plus the gallery's margins.
    let h = png_size(&file).1;
    assert!((300..400).contains(&h), "{h}");

    // Out of range / unsupported selections are usage errors.
    let o = vitrine_in(&dir, &["render", "--page", "9"]);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("out of range"), "{}", stderr(&o));
    let o = vitrine_in(&dir, &["render", "--target", "blog-header", "--gallery"]);
    assert_eq!(code(&o), 2);
    let o = vitrine_in(&dir, &["render", "--target", "nope"]);
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
    let o = vitrine_in(tmp.path(), &["validate"]);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("no project.yaml"), "{}", stderr(&o));

    // YAML syntax error: message names the file and the line.
    let dir = new_project(&tmp, &[]);
    let yaml = dir.join("project.yaml");
    let text = std::fs::read_to_string(&yaml).unwrap();
    std::fs::write(&yaml, text.replace("canvases:", "canvases: [")).unwrap();
    let o = vitrine_in(&dir, &["export"]);
    assert_eq!(code(&o), 2);
    let err = stderr(&o);
    assert!(err.contains("project.yaml"), "{err}");
    assert!(err.contains("line"), "{err}");

    // Usage error (clap).
    let o = vitrine(&["validate", "--format", "xml"]);
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
    let o = vitrine_in(&root, &["validate", "--strict"]);
    assert_eq!(code(&o), 0, "{}\n{}", stdout(&o), stderr(&o));
    assert!(
        stdout(&o).contains("0 errors, 0 warnings"),
        "{}",
        stdout(&o)
    );

    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("export");
    let o = vitrine_in(
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
/// Photoshop files counted separately, `skip` rules reported as skipped on
/// purpose, `frames remove --bezel`.
#[test]
fn frames_import_keeps_all_sizes_and_shows_the_licence() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = tmp.path().join("Bezels");
    vitrine_core::frame_import::synthetic::write_folder(
        &pkg,
        &vitrine_core::frame_import::synthetic::default_specs(),
    )
    .unwrap();
    // Skipped on purpose by a `skip` rule (never decoded, content irrelevant).
    std::fs::write(pkg.join("iPhone Duo - Night Sky - Outer Open.png"), b"png").unwrap();
    let app = tmp.path().join("appdir");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_vitrine"))
            .args(args)
            .env("NO_COLOR", "1")
            .env("VITRINE_APP_DIR", &app)
            .output()
            .expect("run vitrine")
    };
    let pkg_s = pkg.to_str().unwrap();
    let o = run(&["frames", "import", pkg_s, "--dry-run"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("would import 7 frames"), "{out}");
    assert!(out.contains("2 Photoshop file(s) ignored"), "{out}");
    assert!(out.contains("1 other file(s) skipped"), "{out}");
    assert!(out.contains("1 skipped on purpose"), "{out}");
    assert!(out.contains("1 unmatched"), "{out}");
    assert!(
        out.contains(
            "skipped  iPhone Duo - Night Sky - Outer Open.png  (on purpose: marketing composite"
        ),
        "{out}"
    );
    assert!(out.contains("(no rule matched)"), "{out}");
    assert!(
        out.contains("subject to Apple's licence (License.rtf"),
        "{out}"
    );
    assert!(out.contains("vitrine frames embed"), "{out}");
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

/// `frames embed` stores the used imported frames in the project (ADR
/// 0027): a second, empty app folder ("the build server") validates
/// without `imported_frame_missing`; `frames unembed` removes them.
#[test]
fn frames_embed_for_ci_and_unembed() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = tmp.path().join("Bezels");
    vitrine_core::frame_import::synthetic::write_folder(
        &pkg,
        &vitrine_core::frame_import::synthetic::default_specs(),
    )
    .unwrap();
    let project = new_project(&tmp, &[]);
    let yaml = std::fs::read_to_string(project.join("project.yaml")).unwrap();
    std::fs::write(
        project.join("project.yaml"),
        yaml.replacen("styles:\n", "styles:\n  frame: imported:apple\n", 1),
    )
    .unwrap();
    let mac = tmp.path().join("mac");
    let ci = tmp.path().join("ci");
    let run = |app: &Path, args: &[&str]| {
        let mut all = vec!["--project", project.to_str().unwrap()];
        all.extend_from_slice(args);
        Command::new(env!("CARGO_BIN_EXE_vitrine"))
            .args(all)
            .env("NO_COLOR", "1")
            .env("VITRINE_APP_DIR", app)
            .output()
            .expect("run vitrine")
    };
    let o = run(&mac, &["frames", "import", pkg.to_str().unwrap()]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let o = run(&mac, &["validate", "--format", "json"]);
    assert!(
        stdout(&o).contains("imported_frame_not_stored"),
        "{}",
        stdout(&o)
    );
    let o = run(&mac, &["frames", "embed", "--dry-run"]);
    assert_eq!(code(&o), 0, "{}{}", stdout(&o), stderr(&o));
    let out = stdout(&o);
    assert!(out.contains("add "), "{out}");
    assert!(out.contains("(dry run)"), "{out}");
    assert!(out.contains("private repository"), "{out}");
    assert!(!project.join("frames").exists());
    let o = run(&mac, &["frames", "embed"]);
    assert_eq!(code(&o), 0, "{}{}", stdout(&o), stderr(&o));
    assert!(stdout(&o).contains("stored "), "{}", stdout(&o));
    assert!(project.join("frames/imported/README.md").is_file());
    let o = run(&mac, &["frames", "embed"]);
    assert!(stdout(&o).contains("up to date"), "{}", stdout(&o));

    let o = run(&ci, &["validate", "--format", "json"]);
    let out = stdout(&o);
    assert!(!out.contains("imported_frame_missing"), "{out}");
    assert!(out.contains("imported_frames_stored"), "{out}");

    let o = run(&ci, &["frames", "unembed"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stdout(&o).contains("removed "), "{}", stdout(&o));
    assert!(!project.join("frames").exists());
    let o = run(&ci, &["validate", "--format", "json"]);
    assert!(
        stdout(&o).contains("imported_frame_missing"),
        "{}",
        stdout(&o)
    );
}

/// Own frame packs (ADR 0024): `frames import` recognises a pack zip,
/// `--to project` puts it into the project, `frames list --packs`,
/// render with `frame: pack:<id>`, `frames remove --pack`.
#[test]
fn frame_pack_import_into_the_project_and_render() {
    let tmp = tempfile::tempdir().unwrap();
    let zip =
        vitrine_core::frame_import::synthetic::write_pack_zip(&tmp.path().join("clay.zip"), 4)
            .unwrap();
    let project = new_project(&tmp, &[]);
    let app = tmp.path().join("appdir");
    let run = |args: &[&str]| {
        let mut all = vec!["--project", project.to_str().unwrap()];
        all.extend_from_slice(args);
        Command::new(env!("CARGO_BIN_EXE_vitrine"))
            .args(all)
            .env("NO_COLOR", "1")
            .env("VITRINE_APP_DIR", &app)
            .output()
            .expect("run vitrine")
    };
    let zip_s = zip.to_str().unwrap();
    let o = run(&["frames", "import", zip_s, "--to", "project", "--dry-run"]);
    assert_eq!(code(&o), 0, "{}{}", stdout(&o), stderr(&o));
    let out = stdout(&o);
    assert!(
        out.contains("would import pack `clay-phone` (10 frames"),
        "{out}"
    );
    assert!(
        out.contains("use it with `frame: pack:clay-phone`"),
        "{out}"
    );
    assert!(!project.join("frames").exists());
    let o = run(&["frames", "import", zip_s, "--to", "project"]);
    assert_eq!(code(&o), 0, "{}{}", stdout(&o), stderr(&o));
    assert!(project.join("frames/clay-phone/frame.yaml").is_file());
    let o = run(&["frames", "list", "--packs", "--format", "json"]);
    let packs: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(packs[0]["origin"], "project");
    assert_eq!(packs[0]["frames"].as_array().unwrap().len(), 10);

    // Every device of the new project with the pack: no missing frame.
    let yaml = std::fs::read_to_string(project.join("project.yaml")).unwrap();
    std::fs::write(
        project.join("project.yaml"),
        yaml.replacen("styles:\n", "styles:\n  frame: pack:clay-phone\n", 1),
    )
    .unwrap();
    let o = run(&["validate", "--format", "json"]);
    assert!(
        !stdout(&o).contains("imported_frame_missing"),
        "{}",
        stdout(&o)
    );
    let o = run(&[
        "frames",
        "remove",
        "--pack",
        "clay-phone",
        "--from",
        "project",
    ]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let o = run(&["validate", "--format", "json"]);
    assert!(
        stdout(&o).contains("imported_frame_missing"),
        "{}",
        stdout(&o)
    );
    let o = run(&["frames", "remove", "--pack", "clay-phone"]);
    assert_eq!(code(&o), 2, "no app pack: {}", stderr(&o));
}

/// `vitrine fonts` lists the bundled families and the project's fonts.
#[test]
fn fonts_lists_bundled_and_project_families() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(&tmp, &[]);
    std::fs::create_dir_all(dir.join("fonts")).unwrap();
    std::fs::copy(
        repo_file("assets/fonts/lora/Lora[wght].ttf"),
        dir.join("fonts/Brand.ttf"),
    )
    .unwrap();
    let o = vitrine_in(&dir, &["fonts", "--format", "json"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let list: Value = serde_json::from_str(&stdout(&o)).unwrap();
    let list = list.as_array().unwrap();
    let bundled: Vec<&str> = list
        .iter()
        .filter(|f| f["source"] == "bundled")
        .map(|f| f["family"].as_str().unwrap())
        .collect();
    // 11 families plus the six script fonts (ADR 0028).
    assert_eq!(bundled.len(), 17);
    let arabic = list
        .iter()
        .find(|f| f["family"] == "Noto Sans Arabic")
        .unwrap();
    assert_eq!(arabic["category"], "script");
    assert_eq!(bundled[0], "Inter");
    assert!(bundled.contains(&"Playfair Display"));
    let mono = list
        .iter()
        .find(|f| f["family"] == "JetBrains Mono")
        .unwrap();
    assert_eq!(mono["category"], "monospace");
    assert_eq!(mono["weights"], serde_json::json!([100, 800]));
    let project = list.iter().find(|f| f["source"] == "project").unwrap();
    assert_eq!(project["family"], "Lora");
    assert_eq!(project["weights"], serde_json::json!([400, 700]));
    let o = vitrine_in(&dir, &["fonts"]);
    assert!(stdout(&o).contains("Caveat"), "{}", stdout(&o));
    // Nothing to store: the project uses no installed system font.
    let o = vitrine_in(
        &dir,
        &[
            "--no-system-fonts",
            "fonts",
            "embed",
            "--dry-run",
            "--format",
            "json",
        ],
    );
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    let plan: Value = serde_json::from_str(&stdout(&o)).unwrap();
    assert_eq!(plan["items"], serde_json::json!([]));
    assert_eq!(plan["applied"], false);
    assert!(!dir.join("fonts/system").exists());
    let o = vitrine_in(&dir, &["fonts", "unembed"]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(
        stdout(&o).contains("no system fonts stored"),
        "{}",
        stdout(&o)
    );
}

/// CI formats of `validate`: GitHub annotations pointing at `project.yaml`
/// lines, GitLab Code Quality JSON, `JUnit` XML written with `-o` (the text
/// report stays on stdout); exit codes unchanged.
#[test]
fn validate_ci_formats() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = new_project(&tmp, &["--lang", "en"]);
    let yaml_lines = std::fs::read_to_string(dir.join("project.yaml"))
        .unwrap()
        .lines()
        .count();

    let o = vitrine_in(&dir, &["validate", "--format", "github"]);
    assert_eq!(code(&o), 1, "{}", stdout(&o));
    let out = stdout(&o);
    let first = out.lines().next().unwrap();
    assert!(first.starts_with("::error file="), "{out}");
    assert!(first.contains("project.yaml,line="), "{out}");
    assert!(
        first.contains("title=vitrine%3A screenshot_missing"),
        "{out}"
    );
    let line: usize = first
        .split(",line=")
        .nth(1)
        .and_then(|r| r.split(',').next())
        .and_then(|n| n.parse().ok())
        .unwrap();
    assert!(line >= 1 && line <= yaml_lines, "{first}");
    assert!(out.contains("Invalid:"), "text report too: {out}");

    let o = vitrine_in(&dir, &["validate", "--format", "gitlab"]);
    assert_eq!(code(&o), 1);
    let cq: Value = serde_json::from_str(&stdout(&o)).expect("Code Quality JSON");
    let issues = cq.as_array().unwrap();
    assert_ne!(issues.len(), 0);
    assert!(issues.iter().all(|i| {
        i["severity"] == "critical"
            && i["check_name"] == "vitrine/screenshot_missing"
            && i["location"]["path"]
                .as_str()
                .unwrap()
                .ends_with("project.yaml")
    }));

    let xml = tmp.path().join("validate.xml");
    let o = vitrine_in(
        &dir,
        &["validate", "--format", "junit", "-o", xml.to_str().unwrap()],
    );
    assert_eq!(code(&o), 1);
    assert!(stdout(&o).contains("screenshot_missing"), "text on stdout");
    let x = std::fs::read_to_string(&xml).unwrap();
    assert!(x.starts_with("<?xml"), "{x}");
    assert!(x.contains("<testsuite name=\"ios-iphone-69\""), "{x}");
    assert!(x.contains("<failure type=\"screenshot_missing\""), "{x}");

    // Preview mode: warnings → skipped, exit 0; --strict → failures, exit 1.
    let o = vitrine_in(
        &dir,
        &[
            "validate",
            "--preview",
            "--format",
            "junit",
            "-o",
            xml.to_str().unwrap(),
        ],
    );
    assert_eq!(code(&o), 0);
    assert!(std::fs::read_to_string(&xml).unwrap().contains("<skipped "));
    let o = vitrine_in(
        &dir,
        &["validate", "--preview", "--strict", "--format", "github"],
    );
    assert_eq!(code(&o), 1);
    assert!(stdout(&o).starts_with("::warning file="), "{}", stdout(&o));

    // `-o` needs a report format.
    let o = vitrine_in(&dir, &["validate", "-o", xml.to_str().unwrap()]);
    assert_eq!(code(&o), 2);
}

/// `template tags`, `template save` with facet tags and sample screenshots,
/// the removed `--tag`, and a new project carrying the samples.
#[test]
fn template_tags_and_sample_screenshots() {
    let tmp = tempfile::tempdir().unwrap();
    let app = tmp.path().join("app");
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_vitrine"))
            .args(args)
            .env("NO_COLOR", "1")
            .env("VITRINE_APP_DIR", &app)
            .output()
            .expect("run vitrine")
    };
    let o = run(&["template", "tags"]);
    assert_eq!(code(&o), 0);
    assert!(stdout(&o).contains("health-fitness"), "{}", stdout(&o));
    let o = run(&["template", "tags", "--format", "json"]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["facets"][2]["id"], "use");

    let example = repo_file("tests/fixtures/example");
    let ex = example.to_str().unwrap();
    let o = run(&[
        "--project",
        ex,
        "template",
        "save",
        "--name",
        "X",
        "--tag",
        "dark",
    ]);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("--color dark"), "{}", stderr(&o));
    let o = run(&[
        "--project",
        ex,
        "template",
        "save",
        "--name",
        "X",
        "--style",
        "fancy",
    ]);
    assert_eq!(code(&o), 2);
    assert!(stderr(&o).contains("minimal"), "{}", stderr(&o));

    let file = tmp.path().join("samples.vitrine-template");
    let o = run(&[
        "--project",
        ex,
        "template",
        "save",
        "--name",
        "Samples",
        "--canvas",
        "phone",
        "--style",
        "minimal",
        "--color",
        "light,gradient",
        "--use",
        "travel",
        "--sample-screenshots",
        "-o",
        file.to_str().unwrap(),
    ]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stdout(&o).contains("packed as samples"), "{}", stdout(&o));

    let dir = tmp.path().join("p");
    let o = run(&[
        "new",
        dir.to_str().unwrap(),
        "--template",
        file.to_str().unwrap(),
    ]);
    assert_eq!(code(&o), 0, "{}", stderr(&o));
    assert!(stdout(&o).contains("sample screenshot"), "{}", stdout(&o));
    assert!(dir.join("screenshots/samples.yaml").is_file());
    let o = run(&[
        "--project",
        dir.to_str().unwrap(),
        "validate",
        "--format",
        "json",
    ]);
    assert!(stdout(&o).contains("sample_screenshot"), "{}", stdout(&o));
}
