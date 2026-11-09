//! Golden-image tests on the example project (`tests/fixtures/example`).
//!
//! Every output image of every target is rendered in English at
//! [`SCALE`] of the target size, plus German for the iPhone set and the
//! blog header, and store gallery previews [`GALLERY_HEIGHT`] px high. Each
//! render is compared with its reference PNG in
//! `tests/fixtures/example/golden/<target>/<lang>/`:
//!
//! - a pixel differs when any RGB channel differs by more than
//!   [`CHANNEL_TOLERANCE`];
//! - a case fails when more than [`MAX_DIFF_SHARE`] of its pixels differ
//!   (or the size differs). The tolerance absorbs last-bit differences
//!   between platforms (libm, SIMD resampling paths); layout or drawing
//!   changes exceed it.
//!
//! On failure `*.actual.png` and `*.diff.png` are written next to the
//! reference (ignored by Git) and a summary is printed.
//!
//! `ARRANGIA_UPDATE_GOLDEN=1 cargo test -p arrangia-render --test golden`
//! rewrites references that are missing or outside the tolerance and
//! removes stale ones; review the image diff before committing.
//!
//! Only the bundled fonts are used (the fixture has no `fonts/` folder),
//! so the renders do not depend on the machine.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use arrangia_core::Project;
use arrangia_core::model::ImageFormat;
use arrangia_core::resolve::{FsAssets, ResolveOptions, resolve};
use arrangia_core::validate::{Severity, ValidateOptions, validate};
use arrangia_render::export::{gallery, image_file_name};
use arrangia_render::raster::{save_png_rgb, to_rgb};
use arrangia_render::scene::{Quality, RenderContext, RenderOptions, Scale};
use rayon::prelude::*;
use tiny_skia::Pixmap;

/// Render scale relative to the target size.
const SCALE: f64 = 0.25;
/// Height of the gallery previews in px.
const GALLERY_HEIGHT: u32 = 320;
/// Largest per-channel difference (0–255) that still counts as equal.
const CHANNEL_TOLERANCE: u8 = 8;
/// Largest share of differing pixels per image.
const MAX_DIFF_SHARE: f64 = 0.005;
/// Background the renders are flattened onto (as in the export).
const WHITE: [u8; 3] = [255, 255, 255];

/// Extra languages per target (English is always rendered).
const EXTRA_LANGS: &[(&str, &str)] = &[("ios-iphone-69", "de"), ("blog-header", "de")];

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("repository root")
        .join("tests")
        .join("fixtures")
        .join("example")
}

fn golden_dir() -> PathBuf {
    fixture().join("golden")
}

fn update_mode() -> bool {
    std::env::var_os("ARRANGIA_UPDATE_GOLDEN").is_some_and(|v| !v.is_empty() && v != "0")
}

/// One rendered image with its reference path relative to `golden/`.
struct Case {
    rel: Vec<String>,
    pixmap: Pixmap,
}

impl Case {
    fn path(&self, root: &Path) -> PathBuf {
        self.rel.iter().fold(root.to_path_buf(), |p, s| p.join(s))
    }
    fn name(&self) -> String {
        self.rel.join("/")
    }
}

fn render_all(project: &Project, root: &Path) -> Vec<Case> {
    let (ctx, _) = RenderContext::new(root);
    let assets = FsAssets::new(root);
    let mut jobs: Vec<(String, String)> = project
        .targets
        .iter()
        .map(|t| (t.id.as_str().to_owned(), "en".to_owned()))
        .collect();
    jobs.extend(
        EXTRA_LANGS
            .iter()
            .map(|(t, l)| ((*t).to_owned(), (*l).to_owned())),
    );
    let mut cases: Vec<Case> = jobs
        .par_iter()
        .flat_map_iter(|(target, lang)| {
            let scene = resolve(project, target, lang, &ResolveOptions::new(&assets))
                .unwrap_or_else(|e| panic!("resolve {target}/{lang}: {e}"));
            let prepared = ctx.prepare(&scene);
            let opts = RenderOptions {
                scale: Scale::Factor(SCALE),
                quality: Quality::Export,
            };
            let mut out = Vec::new();
            for i in 0..scene.frame.images.len() {
                let r = ctx.rasterize_prepared(&prepared, i, &opts);
                let file = image_file_name(&scene, i as u32, ImageFormat::Png);
                out.push(Case {
                    rel: vec![target.clone(), lang.clone(), file],
                    pixmap: r.pixmap,
                });
            }
            if scene.frame.images.len() > 1 {
                let (pm, _) = gallery(&ctx, &prepared, GALLERY_HEIGHT);
                out.push(Case {
                    rel: vec![target.clone(), lang.clone(), "_gallery.png".to_owned()],
                    pixmap: pm,
                });
            }
            out
        })
        .collect();
    cases.sort_by(|a, b| a.rel.cmp(&b.rel));
    cases
}

/// Comparison result of one case.
enum Outcome {
    Match,
    Missing,
    SizeMismatch((u32, u32), (u32, u32)),
    Differs { share: f64, max: u8 },
}

fn compare(actual: &[u8], (aw, ah): (u32, u32), reference: &Path) -> (Outcome, Option<Vec<u8>>) {
    let Ok(img) = image::open(reference) else {
        return (Outcome::Missing, None);
    };
    let img = img.to_rgb8();
    if img.dimensions() != (aw, ah) {
        return (Outcome::SizeMismatch(img.dimensions(), (aw, ah)), None);
    }
    let expected = img.as_raw();
    let mut differing = 0usize;
    let mut max = 0u8;
    let mut diff = Vec::with_capacity(actual.len());
    for (a, e) in actual
        .as_chunks::<3>()
        .0
        .iter()
        .zip(expected.as_chunks::<3>().0)
    {
        let d = (0..3).map(|c| a[c].abs_diff(e[c])).max().unwrap_or(0);
        max = max.max(d);
        if d > CHANNEL_TOLERANCE {
            differing += 1;
            diff.extend_from_slice(&[255, 0, 0]);
        } else {
            // Faded grey copy of the reference for orientation.
            let g = ((u16::from(e[0]) + u16::from(e[1]) + u16::from(e[2])) / 3) as u8;
            let g = 255 - (255 - g) / 4;
            diff.extend_from_slice(&[g, g, g]);
        }
    }
    let share = differing as f64 / (f64::from(aw) * f64::from(ah));
    if share > MAX_DIFF_SHARE {
        (Outcome::Differs { share, max }, Some(diff))
    } else {
        (Outcome::Match, None)
    }
}

fn write_rgb(path: &Path, w: u32, h: u32, rgb: Vec<u8>) {
    let img = image::RgbImage::from_raw(w, h, rgb).expect("rgb buffer");
    img.save(path)
        .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

/// Reference files (relative, `/`-separated) below `golden/`.
fn reference_files(dir: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            if e.path().is_dir() {
                walk(&e.path(), &rel, out);
            } else if Path::new(&name).extension().is_some_and(|x| x == "png")
                && !name.ends_with(".actual.png")
                && !name.ends_with(".diff.png")
            {
                out.insert(rel);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(dir, "", &mut out);
    out
}

#[test]
fn example_project_matches_golden_images() {
    let root = fixture();
    let project = Project::load(&root).expect("load the example project");
    let golden = golden_dir();
    let update = update_mode();
    let cases = render_all(&project, &root);
    assert!(cases.len() > 30, "only {} cases rendered", cases.len());

    let mut failures = Vec::new();
    let mut updated = Vec::new();
    for case in &cases {
        let path = case.path(&golden);
        let (w, h) = (case.pixmap.width(), case.pixmap.height());
        let rgb = to_rgb(&case.pixmap, WHITE);
        let (outcome, diff) = compare(&rgb, (w, h), &path);
        let actual = path.with_extension("actual.png");
        let diff_path = path.with_extension("diff.png");
        // Leftovers of an earlier failed run.
        let _ = std::fs::remove_file(&actual);
        let _ = std::fs::remove_file(&diff_path);
        if matches!(outcome, Outcome::Match) {
            continue;
        }
        if update {
            save_png_rgb(&case.pixmap, &path, WHITE)
                .unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
            updated.push(case.name());
            continue;
        }
        let msg = match outcome {
            Outcome::Match => unreachable!(),
            Outcome::Missing => "reference missing".to_owned(),
            Outcome::SizeMismatch(r, a) => {
                format!("size {}x{}, reference {}x{}", a.0, a.1, r.0, r.1)
            }
            Outcome::Differs { share, max } => format!(
                "{:.2} % of the pixels differ by more than {CHANNEL_TOLERANCE} (max {max}); limit {:.2} %",
                share * 100.0,
                MAX_DIFF_SHARE * 100.0
            ),
        };
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).expect("golden folder");
        }
        save_png_rgb(&case.pixmap, &actual, WHITE).expect("write actual");
        if let Some(diff) = diff {
            write_rgb(&diff_path, w, h, diff);
        }
        failures.push(format!("  {}: {msg}", case.name()));
    }

    // References no case produces any more.
    let produced: BTreeSet<String> = cases.iter().map(Case::name).collect();
    let stale: Vec<String> = reference_files(&golden)
        .difference(&produced)
        .cloned()
        .collect();
    if update {
        for s in &stale {
            let p = s.split('/').fold(golden.clone(), |p, part| p.join(part));
            std::fs::remove_file(&p).unwrap_or_else(|e| panic!("remove {}: {e}", p.display()));
        }
        if !updated.is_empty() || !stale.is_empty() {
            eprintln!(
                "golden: updated {} reference(s), removed {} stale: {:?} {:?}",
                updated.len(),
                stale.len(),
                updated,
                stale
            );
        }
        return;
    }
    for s in &stale {
        failures.push(format!("  {s}: stale reference (no longer rendered)"));
    }
    assert!(
        failures.is_empty(),
        "{} of {} golden images do not match (actual/diff images written next to the references in {}; \
         run with ARRANGIA_UPDATE_GOLDEN=1 to accept):\n{}",
        failures.len(),
        cases.len(),
        golden.display(),
        failures.join("\n")
    );
}

#[test]
fn example_project_validates_without_errors_or_warnings() {
    let root = fixture();
    let project = Project::load(&root).expect("load the example project");
    let assets = FsAssets::new(&root);
    let report = validate(&project, &ValidateOptions::new(&assets));
    let problems: Vec<String> = report
        .findings
        .iter()
        .filter(|f| f.level != Severity::Info)
        .map(|f| format!("{:?} {:?}: {}", f.level, f.code, f.message))
        .collect();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    // The fixture exercises the screenshot fallback chain on purpose.
    assert!(
        report
            .findings
            .iter()
            .any(|f| f.code == arrangia_core::validate::FindingCode::ScreenshotFallback)
    );
}
