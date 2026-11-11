use std::fs;
use std::path::Path;

use super::*;
use crate::frame_import::synthetic;

fn pack(dir: &Path) -> PathBuf {
    let p = dir.join("clay-phone");
    synthetic::write_pack(&p, 4).unwrap();
    p
}

/// Loads a pack without SVG support (core has no renderer).
fn load(dir: &Path) -> PackInfo {
    load_pack(
        dir,
        "clay-phone",
        PackOrigin::App,
        &dir.join(COMPILED_DIR),
        None,
    )
}

#[test]
fn compiles_png_frames_and_reports_svg_without_renderer() {
    let tmp = tempfile::tempdir().unwrap();
    let info = load(&pack(tmp.path()));
    assert_eq!(info.name, "Clay phone");
    assert_eq!(info.license.as_deref(), Some("CC0-1.0"));
    // 4 PNG frames × 2 models; the SVG needs the renderer.
    assert_eq!(info.frames.len(), 8, "{:?}", info.errors);
    assert_eq!(info.errors.len(), 1);
    assert!(info.errors[0].contains("outline.svg"), "{:?}", info.errors);
    let sand = info
        .frames
        .iter()
        .find(|f| f.color == "sand" && f.orientation == Orientation::Portrait)
        .unwrap();
    assert_eq!(sand.model, "iphone-island");
    assert_eq!(sand.color_label, "Sand");
    assert_eq!(sand.apple_model, "Clay phone");
    // phone() at div 4: margin 25, screen 1206/4 × 2622/4.
    let s = sand.screen;
    assert!(s.x.abs_diff(25) <= 1 && s.y.abs_diff(25) <= 1, "{s:?}");
    assert!(s.w.abs_diff(301) <= 2 && s.h.abs_diff(655) <= 2, "{s:?}");
    assert!(sand.swatch.is_some());
    let land = info
        .frames
        .iter()
        .find(|f| f.color == "sand" && f.orientation == Orientation::Landscape)
        .unwrap();
    assert!(land.size.w > land.size.h);
    assert!(info.compiled.join(&sand.file).is_file());
    assert!(info.compiled.join(&sand.mask).is_file());
}

#[test]
fn painted_screen_is_cut_out() {
    let tmp = tempfile::tempdir().unwrap();
    let info = load(&pack(tmp.path()));
    let f = info.frames.iter().find(|f| f.color == "painted").unwrap();
    assert_eq!((f.screen.x, f.screen.y), (25, 25));
    assert!((f.radius - 40.0).abs() < 0.01, "{}", f.radius);
    let bezel = decode_png(&fs::read(info.compiled.join(&f.file)).unwrap()).unwrap();
    let mask = decode_png(&fs::read(info.compiled.join(&f.mask)).unwrap()).unwrap();
    let at = |img: &RgbaImage, x: u32, y: u32| {
        let i = (y * img.width + x) as usize * 4;
        [img.data[i], img.data[i + 3]]
    };
    let (cx, cy) = (bezel.width / 2, bezel.height / 2);
    assert_eq!(at(&bezel, cx, cy)[1], 0, "screen centre transparent");
    assert_eq!(at(&mask, cx, cy)[0], 255, "mask inside");
    assert_eq!(at(&mask, 5, cy)[0], 0, "mask outside");
    assert_eq!(at(&bezel, 10, cy)[1], 255, "frame stays opaque");
}

#[test]
fn compiled_files_are_reused_until_the_pack_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = pack(tmp.path());
    let first = load(&dir);
    let stamp_file = first.compiled.join(STAMP_FILE);
    let t0 = fs::metadata(&stamp_file).unwrap().modified().unwrap();
    let again = load(&dir);
    assert_eq!(again.frames, first.frames);
    assert_eq!(again.errors, first.errors);
    assert_eq!(fs::metadata(&stamp_file).unwrap().modified().unwrap(), t0);
    // Drop the SVG and the painted frame from the manifest: recompiled.
    let text = fs::read_to_string(dir.join(PACK_FILE)).unwrap();
    let cut = text.split("  # Painted").next().unwrap().to_owned();
    fs::write(dir.join(PACK_FILE), cut).unwrap();
    let changed = load(&dir);
    assert_eq!(changed.frames.len(), 6, "{:?}", changed.errors);
    assert!(changed.errors.is_empty());
}

#[test]
fn manifest_problems_are_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = pack(tmp.path());
    let base = fs::read_to_string(dir.join(PACK_FILE)).unwrap();
    for (from, to, want) in [
        ("vitrine-frame/1", "vitrine-frame/9", "not supported"),
        (
            "file: clay-ink-portrait.png",
            "file: ../escape.png",
            "not a file inside",
        ),
        (
            "models: [iphone-island, android-punchhole]",
            "models: [no-such-phone]",
            "unknown device model",
        ),
        (
            "    raster: 1200",
            "    raster: 1200\n    colour: x",
            "unknown field",
        ),
    ] {
        fs::write(dir.join(PACK_FILE), base.replace(from, to)).unwrap();
        let info = load(&dir);
        assert!(
            info.errors.iter().any(|e| e.contains(want)),
            "{want}: {:?}",
            info.errors
        );
    }
    // Opaque screen without `screen:` explains what to do.
    fs::write(
        dir.join(PACK_FILE),
        base.replace("    screen: {", "    xscreen: {")
            .replace("    cut_screen: true\n", ""),
    )
    .unwrap();
    let text = fs::read_to_string(dir.join(PACK_FILE)).unwrap();
    let text = text
        .lines()
        .filter(|l| !l.starts_with("    xscreen") && !l.starts_with("    radius"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(dir.join(PACK_FILE), text).unwrap();
    let info = load(&dir);
    assert!(
        info.errors
            .iter()
            .any(|e| e.contains("painted.png") && e.contains("cut_screen")),
        "{:?}",
        info.errors
    );
}

#[test]
fn index_sources_and_any_model_packs() {
    let tmp = tempfile::tempdir().unwrap();
    let app_root = tmp.path().join("frames");
    let project = tmp.path().join("project");
    synthetic::write_pack(&app_root.join(PACKS_DIR).join("clay-phone"), 4).unwrap();
    // A project pack without models (any device), same id as the app pack
    // under another name: the project one wins.
    let proj_pack = project.join(PROJECT_FRAMES_DIR).join("clay-phone");
    synthetic::write_pack(&proj_pack, 4).unwrap();
    let text = fs::read_to_string(proj_pack.join(PACK_FILE)).unwrap();
    fs::write(
        proj_pack.join(PACK_FILE),
        text.replace("models: [iphone-island, android-punchhole]\n", "")
            .replace("name: Clay phone", "name: Project clay"),
    )
    .unwrap();
    let packs = load_packs(Some(&app_root), Some(&project), None);
    assert_eq!(packs.len(), 2);
    assert_eq!(packs[0].origin, PackOrigin::Project);
    assert!(packs[0].shadows_app_pack);
    assert!(packs[0].compiled.starts_with(project.join(".cache")));
    let mut index = FrameIndex::load(&app_root);
    add_packs(&mut index, &packs);
    let key = "pack:clay-phone";
    assert_eq!(index.source_dir(key), packs[0].compiled);
    // Any model: an iPad element may use it.
    let c = index.candidates(key, "ipad-modern", Some("ink"), None, Orientation::Portrait);
    assert_eq!(c.len(), 1);
    assert_eq!(c[0].model, ANY_MODEL);
    assert_eq!(c[0].package, "Project clay");
    // Unknown colour → the first installed colour.
    let c = index.candidates(
        key,
        "iphone-island",
        Some("nope"),
        None,
        Orientation::Portrait,
    );
    assert_eq!(c.len(), 1);
    let colors = index.installed_colors(key, "iphone-island");
    assert_eq!(
        colors.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["ink", "painted", "sand"]
    );
    // App packs are not in the index's own source folders.
    assert!(FrameIndex::load(&app_root).is_empty());
}

#[test]
fn import_zip_and_folder_dry_run_apply_replace_remove() {
    let tmp = tempfile::tempdir().unwrap();
    let zip = synthetic::write_pack_zip(&tmp.path().join("clay.zip"), 4).unwrap();
    assert!(is_pack(&zip));
    let packs_dir = tmp.path().join("frames").join(PACKS_DIR);
    let dry = import_pack(&zip, &packs_dir, PackTarget::App, None, false, None).unwrap();
    assert_eq!(dry.id, "clay-phone");
    assert!(!dry.applied && !dry.replaces);
    assert_eq!(dry.files, 7);
    assert_eq!(dry.pack.frames.len(), 8);
    assert!(!packs_dir.exists(), "dry run writes nothing");

    let done = import_pack(&zip, &packs_dir, PackTarget::App, None, true, None).unwrap();
    assert!(done.applied);
    assert!(packs_dir.join("clay-phone").join(PACK_FILE).is_file());
    assert!(!packs_dir.join("clay-phone").join("__MACOSX").exists());
    assert!(done.pack.compiled.join(INDEX_FILE).is_file());
    let again = import_pack(&zip, &packs_dir, PackTarget::App, None, true, None).unwrap();
    assert!(again.replaces);

    // A folder into a project, with an explicit id.
    let folder = pack(tmp.path());
    assert!(is_pack(&folder));
    assert!(!is_pack(tmp.path()) || find_manifest_dir(tmp.path()).is_some());
    let project = tmp.path().join("project");
    let into = project.join(PROJECT_FRAMES_DIR);
    let p = import_pack(
        &folder,
        &into,
        PackTarget::Project,
        Some("clay"),
        true,
        None,
    )
    .unwrap();
    assert_eq!(p.pack.origin, PackOrigin::Project);
    assert_eq!(p.pack.compiled, project_compiled_dir(&project, "clay"));
    assert!(
        import_pack(
            &folder,
            &into,
            PackTarget::Project,
            Some("../x"),
            true,
            None
        )
        .is_err()
    );

    assert!(remove_pack(&into, "clay", Some(&project)).unwrap());
    assert!(!into.join("clay").exists());
    assert!(!project_compiled_dir(&project, "clay").exists());
    assert!(!remove_pack(&into, "clay", Some(&project)).unwrap());
    // Not a pack.
    let empty = tmp.path().join("empty");
    fs::create_dir_all(&empty).unwrap();
    assert!(!is_pack(&empty));
    assert!(import_pack(&empty, &packs_dir, PackTarget::App, None, false, None).is_err());
}

#[test]
fn rounded_mask_is_antialiased() {
    let m = rounded_rect_mask(40, 40, (5.0, 5.0, 30.0, 30.0), 10.0);
    let at = |x: u32, y: u32| m[(y * 40 + x) as usize];
    assert_eq!(at(20, 20), 255);
    assert_eq!(at(2, 20), 0);
    assert_eq!(at(5, 5), 0, "corner cut");
    assert!(at(5, 20) > 100, "edge pixel");
}
