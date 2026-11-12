//! Imported frames stored in the project (ADR 0027): embed → resolve from
//! the project with an empty app store → identical render; update after a
//! colour change; unembed; validation infos; templates never take them.
//! Synthetic bezels only (generated PNGs, never vendor artwork).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tiny_skia::Pixmap;
use vitrine_core::Project;
use vitrine_core::frame_import::embed::{self, EmbedAction};
use vitrine_core::frame_import::{
    FrameIndex, FrameOrigin, FrameStore, Package, Rules, import, synthetic,
};
use vitrine_core::layout::FontLibrary;
use vitrine_core::resolve::{DiagnosticCode, FsAssets, ResolvedKind, ResolvedScene, resolve};
use vitrine_core::validate::{FindingCode, ValidateOptions, validate};
use vitrine_render::frame_packs::frame_index;
use vitrine_render::scene::{RenderContext, RenderOptions};

const PROJECT: &str = r##"format: vitrine/1
name: stored
languages: {default: en, available: [en]}
canvases:
- id: s
  page: {aspect: 1000:2000, count: 1}
  backgrounds: ["#FFFFFF"]
  elements:
  - id: phone
    kind: device
    page: 0
    at: {x: 500, y: 1000}
    width: 600
    model: iphone-island
    frame: imported:apple
    color: COLOR
    screenshot: shot
targets:
- id: t
  canvas: s
  size: [500, 1000]
"##;

struct Setup {
    _dir: tempfile::TempDir,
    /// App-wide store with the synthetic package ("this computer").
    app: PathBuf,
    /// An empty app store ("the build server").
    empty: PathBuf,
    project: PathBuf,
}

fn write_project(dir: &Path, color: &str) {
    std::fs::write(dir.join("project.yaml"), PROJECT.replace("COLOR", color)).unwrap();
}

fn setup() -> Setup {
    let dir = tempfile::tempdir().unwrap();
    let pkg_dir = dir.path().join("package");
    synthetic::write_folder(&pkg_dir, &synthetic::default_specs()).unwrap();
    let app = dir.path().join("app/frames");
    let store = FrameStore::new(&app);
    let pkg = Package::open(&pkg_dir).unwrap();
    import(&store, &pkg, Rules::builtin("apple").unwrap(), true).unwrap();
    let project = dir.path().join("project");
    let shot_path = project.join("screenshots/en/shot.png");
    std::fs::create_dir_all(shot_path.parent().unwrap()).unwrap();
    let mut shot = Pixmap::new(330, 717).unwrap();
    shot.fill(tiny_skia::Color::from_rgba8(40, 200, 70, 255));
    shot.save_png(&shot_path).unwrap();
    write_project(&project, "black-titanium");
    let empty = dir.path().join("empty/frames");
    Setup {
        _dir: dir,
        app,
        empty,
        project,
    }
}

fn load(p: &Path) -> Project {
    Project::load(p).unwrap()
}

fn render(project_dir: &Path, index: Option<Arc<FrameIndex>>) -> (ResolvedScene, Vec<u8>) {
    let project = load(project_dir);
    let ctx = RenderContext::with_fonts(project_dir, FontLibrary::bundled()).with_frames(index);
    let fs = FsAssets::new(project_dir);
    let scene = resolve(&project, "t", "en", &ctx.resolve_options(&fs)).unwrap();
    let r = ctx.rasterize(&scene, 0, &RenderOptions::export());
    assert!(r.issues.is_empty(), "{:?}", r.issues);
    (scene, r.pixmap.data().to_vec())
}

fn imported(scene: &ResolvedScene) -> Option<&vitrine_core::resolve::ImportedFrame> {
    let ResolvedKind::Device(d) = &scene.elements[0].props else {
        panic!("not a device")
    };
    d.imported.as_ref()
}

fn codes(project_dir: &Path, index: Option<&FrameIndex>) -> Vec<FindingCode> {
    let project = load(project_dir);
    let fs = FsAssets::new(project_dir);
    let mut opts = ValidateOptions::new(&fs);
    opts.frames = index;
    validate(&project, &opts)
        .findings
        .iter()
        .map(|f| f.code)
        .collect()
}

#[test]
fn embed_renders_identically_without_the_app_store() {
    let s = setup();
    let on_mac = frame_index(Some(&s.app), Some(&s.project));
    let (scene_mac, px_mac) = render(&s.project, on_mac.clone());
    let f = imported(&scene_mac).expect("app frame");
    assert_eq!(f.origin, FrameOrigin::App);
    let c = codes(&s.project, on_mac.as_deref());
    assert!(c.contains(&FindingCode::ImportedFrameNotStored), "{c:?}");
    assert!(!c.contains(&FindingCode::ImportedFramesStored));

    // Dry run writes nothing.
    let project = load(&s.project);
    let fs = FsAssets::new(&s.project);
    let ix = on_mac.as_deref().unwrap();
    let plan = embed::plan(&project, &s.project, ix, &fs, false);
    assert_eq!(plan.items.len(), 1, "{:?}", plan.items);
    assert_eq!(plan.items[0].action, EmbedAction::Add);
    assert!(plan.bytes_after > 0 && plan.bytes_now == 0);
    assert!(!embed::embed_root(&s.project).exists());

    // Store, then render on the "build server" (empty app store).
    let done = embed::apply(&project, &s.project, ix, &fs, false).unwrap();
    assert!(done.applied && done.stored);
    let root = embed::embed_root(&s.project);
    assert!(root.join(embed::README_FILE).is_file());
    assert!(root.join(embed::OPTIONS_FILE).is_file());
    assert!(root.join("apple/index.yaml").is_file());
    let readme = std::fs::read_to_string(root.join(embed::README_FILE)).unwrap();
    assert!(readme.contains("private") && readme.contains(".gitignore"));
    // The package's licence file travels with the frames.
    assert_eq!(done.licenses.len(), 1, "{done:?}");
    assert!(root.join(&done.licenses[0]).is_file());

    let on_ci = frame_index(Some(&s.empty), Some(&s.project));
    let (scene_ci, px_ci) = render(&s.project, on_ci.clone());
    let f_ci = imported(&scene_ci).expect("project frame");
    assert_eq!(f_ci.origin, FrameOrigin::Project);
    assert_eq!(f_ci.bezel, f.bezel);
    assert_eq!(px_ci, px_mac, "project copy renders the same pixels");
    let c = codes(&s.project, on_ci.as_deref());
    assert!(c.contains(&FindingCode::ImportedFramesStored), "{c:?}");
    assert!(!c.contains(&FindingCode::ImportedFrameNotStored));
    assert!(!c.contains(&FindingCode::StoredFramesStale));

    // On the Mac the project copy wins; up to date.
    let both = frame_index(Some(&s.app), Some(&s.project));
    let (scene_both, _) = render(&s.project, both.clone());
    assert_eq!(imported(&scene_both).unwrap().origin, FrameOrigin::Project);
    let again = embed::plan(&project, &s.project, both.as_deref().unwrap(), &fs, false);
    assert!(again.up_to_date, "{:?}", again.items);
}

#[test]
fn update_after_colour_change_and_unembed() {
    let s = setup();
    let fs = FsAssets::new(&s.project);
    let ix = frame_index(Some(&s.app), Some(&s.project)).unwrap();
    embed::apply(&load(&s.project), &s.project, &ix, &fs, false).unwrap();

    // The device switches to Desert Titanium (only the Pro Max has it).
    write_project(&s.project, "desert-titanium");
    let ix = frame_index(Some(&s.app), Some(&s.project)).unwrap();
    let c = codes(&s.project, Some(&ix));
    assert!(c.contains(&FindingCode::StoredFramesStale), "{c:?}");
    assert!(c.contains(&FindingCode::ImportedFrameNotStored), "{c:?}");
    let plan = embed::plan(&load(&s.project), &s.project, &ix, &fs, false);
    let actions: Vec<_> = plan
        .items
        .iter()
        .map(|i| (i.color.as_str(), i.action))
        .collect();
    assert!(
        actions.contains(&("desert-titanium", EmbedAction::Add)),
        "{actions:?}"
    );
    assert!(
        actions.contains(&("black-titanium", EmbedAction::Remove)),
        "{actions:?}"
    );
    embed::apply(&load(&s.project), &s.project, &ix, &fs, false).unwrap();
    let stored = FrameStore::new(embed::embed_root(&s.project))
        .source_index("apple")
        .unwrap();
    assert_eq!(stored.frames.len(), 1);
    assert_eq!(stored.frames[0].color, "desert-titanium");
    let on_ci = frame_index(Some(&s.empty), Some(&s.project));
    let (scene, _) = render(&s.project, on_ci);
    assert_eq!(imported(&scene).unwrap().color, "desert-titanium");

    // Unembed: back to the parametric frame on the build server.
    let freed = embed::remove(&s.project).unwrap();
    assert!(freed > 0);
    assert!(!s.project.join("frames").exists());
    let on_ci = frame_index(Some(&s.empty), Some(&s.project));
    assert!(on_ci.is_none());
    let (scene, _) = render(&s.project, None);
    assert!(imported(&scene).is_none());
    assert!(
        scene
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::ImportedFrameMissing)
    );
}

#[test]
fn all_sizes_keeps_every_vendor_model_of_the_slot() {
    let s = setup();
    let fs = FsAssets::new(&s.project);
    let ix = frame_index(Some(&s.app), Some(&s.project)).unwrap();
    let plan = embed::apply(&load(&s.project), &s.project, &ix, &fs, true).unwrap();
    let mut bezels: Vec<_> = plan.items.iter().map(|i| i.bezel.as_str()).collect();
    bezels.sort_unstable();
    assert_eq!(bezels, ["iPhone 16 Pro", "iPhone 16 Pro Max"]);
    assert!(embed::read_options(&s.project).all_sizes);
    // The extra size is not reported as unused.
    let ix = frame_index(Some(&s.app), Some(&s.project)).unwrap();
    let c = codes(&s.project, Some(&ix));
    assert!(!c.contains(&FindingCode::StoredFramesStale), "{c:?}");
    // Back to used-only: the extra size goes.
    let plan = embed::apply(&load(&s.project), &s.project, &ix, &fs, false).unwrap();
    assert_eq!(
        plan.items
            .iter()
            .filter(|i| i.action == EmbedAction::Remove)
            .count(),
        1
    );
}

#[test]
fn templates_never_contain_stored_frames() {
    let s = setup();
    let fs = FsAssets::new(&s.project);
    let ix = frame_index(Some(&s.app), Some(&s.project)).unwrap();
    embed::apply(&load(&s.project), &s.project, &ix, &fs, false).unwrap();
    let opts = vitrine_core::template::extract::SaveOptions {
        name: "Stored".into(),
        ..Default::default()
    };
    let (pkg, _) =
        vitrine_core::template::extract::extract(&load(&s.project), &s.project, &opts).unwrap();
    assert!(
        pkg.paths().all(|p| !p.starts_with("frames/")),
        "{:?}",
        pkg.paths().collect::<Vec<_>>()
    );
    // An image element pointing into the stored frames is refused.
    let bezel = FrameStore::new(embed::embed_root(&s.project))
        .source_index("apple")
        .unwrap()
        .frames[0]
        .file
        .clone();
    let yaml = std::fs::read_to_string(s.project.join("project.yaml"))
        .unwrap()
        .replace(
            "    screenshot: shot\n",
            &format!(
                "    screenshot: shot\n  - id: logo\n    kind: image\n    page: 0\n    at: {{x: 500, y: 200}}\n    width: 100\n    asset: frames/imported/apple/{bezel}\n"
            ),
        );
    std::fs::write(s.project.join("project.yaml"), yaml).unwrap();
    let err =
        vitrine_core::template::extract::extract(&load(&s.project), &s.project, &opts).unwrap_err();
    assert!(err.to_string().contains("vendor device frame"), "{err}");
    // And a package cannot carry such files.
    let mut pkg = pkg;
    assert!(
        pkg.put("frames/imported/apple/index.yaml", b"frames: []\n".to_vec())
            .is_err()
    );
}
