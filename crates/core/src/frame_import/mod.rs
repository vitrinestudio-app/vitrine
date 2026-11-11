//! Import of vendor device frames (KONZEPT §4.2, ADR 0012): Apple Design
//! Resources "Product Bezels" downloaded by the user; own frame packs
//! (`frame.yaml` + PNG/SVG, ADR 0024) in [`pack`].
//!
//! ```text
//! Package::open(folder | .zip | .dmg)       package (PNG entries)
//!   → plan(store, package, rules, dry run)  match names (rules + user mapping),
//!                                           analyse each PNG (screen rect, mask),
//!                                           one file per model × colour × orientation ×
//!                                           vendor model (iPhone 18 Pro, … Pro Max)
//!   → install                               app-wide FrameStore (never a project)
//! FrameIndex::load(root)                    what resolve / render use
//! select_bezel(candidates, pin, sizes)      which vendor model a device shows
//! ```
//!
//! Projects only say `frame: imported:apple` (plus model and colour);
//! resolve picks the installed frame or falls back to the parametric one
//! with an `imported_frame_missing` info.

mod analyze;
mod colors;
pub mod pack;
mod package;
mod rules;
mod store;
pub mod synthetic;

use serde::Serialize;

pub use analyze::{
    ALPHA_THRESHOLD, MASK_GROW, PxRect, RgbaImage, ScreenAnalysis, analyze, body_swatch,
    decode_png, encode_gray_png, encode_rgba_png, hex_rgb,
};
pub use colors::{parse_hex, remap_color, set_default_frame_ops};
pub use package::{
    Package, PackageEntry, PackageError, PackageKind, attach_error, imageinfo_has_sla,
    likely_volume,
};
pub use rules::{FileMatch, Pattern, Rule, Rules, normalize_name, slug, vendor_model};
pub use store::{
    ANY_MODEL, Assignment, FrameIndex, FrameSourceIndex, FrameStore, INDEX_FILE, InstalledColor,
    InstalledFrame, RemoveFilter, USER_MAPPING_FILE, UserMapping, today,
};

use crate::model::{Orientation, PixelSize};

/// Folder of the frame store inside the app data folder.
pub const FRAMES_DIR: &str = "frames";

/// Environment variable overriding the app folder (GUI and CLI): app data
/// then lives in `$VITRINE_APP_DIR/data`, settings in `…/config`.
pub const APP_DIR_ENV: &str = "VITRINE_APP_DIR";

/// Screen aspect tolerance against the catalog (KONZEPT §4.3: ~2 %).
pub const ASPECT_TOLERANCE: f64 = 0.02;

/// What happens to a matched file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "action")]
pub enum Action {
    /// New frame.
    Install,
    /// Replaces an installed frame.
    Replace,
    /// Another file (`by`, its path in the package) wins for the same
    /// model × colour × orientation × vendor model (a duplicate).
    Superseded { by: String },
    /// The PNG could not be analysed (see `error`).
    Failed,
}

/// A file that maps to a frame.
#[derive(Debug, Clone, Serialize)]
pub struct PlanItem {
    /// Path inside the package.
    pub file: String,
    /// Normalised file name (the key of user assignments).
    pub name: String,
    #[serde(flatten)]
    pub matched: FileMatch,
    #[serde(flatten)]
    pub action: Action,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<PixelSize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub screen: Option<PxRect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Sanity-check findings (screen aspect vs the catalog …).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

/// A PNG no rule matched.
#[derive(Debug, Clone, Serialize)]
pub struct Unmatched {
    pub file: String,
    pub name: String,
    /// The user chose to ignore it (user mapping `null`), or a `skip`
    /// rule of the source excludes it (`reason`).
    pub ignored: bool,
    /// Why the source's rules skip it (`skip:` in the rule file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Result of [`plan`] / [`import`].
#[derive(Debug, Clone, Serialize)]
pub struct ImportPlan {
    pub source: String,
    pub package: String,
    pub kind: PackageKind,
    /// Some files matched rules that are not yet verified against a real
    /// package (or nothing matched and the rules are not all verified).
    pub unverified_rules: bool,
    pub items: Vec<PlanItem>,
    pub unmatched: Vec<Unmatched>,
    /// Photoshop files (ignored).
    pub psd: usize,
    /// Licence files of the package (absolute path for folders,
    /// `<package>/<file>` for archives and disk images): the frames are
    /// subject to them.
    pub licenses: Vec<String>,
    /// Other non-PNG files.
    pub skipped: usize,
    /// `false` = dry run.
    pub applied: bool,
}

impl ImportPlan {
    /// Items that are (or would be) installed.
    pub fn installs(&self) -> impl Iterator<Item = &PlanItem> {
        self.items
            .iter()
            .filter(|i| matches!(i.action, Action::Install | Action::Replace))
    }
}

/// Plans (`apply == false`) or performs the import of `package` into
/// `store` with `rules` and the store's user mapping.
#[allow(clippy::too_many_lines)]
pub fn import(
    store: &FrameStore,
    package: &Package,
    rules: &Rules,
    apply: bool,
) -> Result<ImportPlan, String> {
    let source = rules.source.as_str();
    let mapping = store.user_mapping(source)?;
    let installed = store.source_index(source)?;
    let mut unmatched = Vec::new();
    let mut matched = Vec::new();
    for e in &package.entries {
        let name = normalize_name(&e.name);
        let m = match mapping.files.get(&name) {
            Some(Some(a)) => Some(FileMatch {
                model: a.model.clone(),
                color: rules.color_id(&a.color),
                apple_model: vendor_model(&name, &a.color),
                color_label: a.color.clone(),
                orientation: a.orientation,
                priority: i32::MAX,
                rule: None,
                verified: true,
            }),
            Some(None) => {
                unmatched.push(Unmatched {
                    file: e.path.clone(),
                    name,
                    ignored: true,
                    reason: None,
                });
                continue;
            }
            None => {
                if let Some(reason) = rules.skip_reason(&e.name) {
                    unmatched.push(Unmatched {
                        file: e.path.clone(),
                        name,
                        ignored: true,
                        reason: Some(reason.to_owned()),
                    });
                    continue;
                }
                rules.match_file(&e.name)
            }
        };
        match m {
            Some(m) => matched.push((e, name, m)),
            None => unmatched.push(Unmatched {
                file: e.path.clone(),
                name,
                ignored: false,
                reason: None,
            }),
        }
    }
    let mut analysed = analyse_all(package, &matched);
    pick_winners(&mut analysed, &installed);
    if apply {
        let package_name = package.display_name();
        let date = today();
        let license = package
            .licenses
            .first()
            .map(|l| l.rsplit('/').next().unwrap_or(l).to_owned());
        for item in analysed
            .iter_mut()
            .filter(|i| matches!(i.action, Action::Install | Action::Replace))
        {
            // Analysed again one at a time: keeping every decoded bezel and
            // mask of a whole package in memory would need gigabytes.
            let Some(entry) = package.entries.iter().find(|e| e.path == item.file) else {
                continue;
            };
            let bytes = package.read(entry)?;
            let a = decode_png(&bytes).and_then(|img| analyze(&img))?;
            let frame = InstalledFrame {
                model: item.matched.model.clone(),
                color: item.matched.color.clone(),
                color_label: item.matched.color_label.clone(),
                orientation: item.matched.orientation,
                apple_model: item.matched.apple_model.clone(),
                file: String::new(),
                mask: String::new(),
                size: a.size,
                screen: a.screen,
                radius: (a.radius * 10.0).round() / 10.0,
                priority: item.matched.priority.clamp(-1000, 1000),
                source_file: entry.name.clone(),
                package: package_name.clone(),
                license: license.clone(),
                imported: date.clone(),
                swatch: Some(hex_rgb(a.swatch)),
            };
            let mask_png = encode_gray_png(a.size.w, a.size.h, &a.mask)?;
            store.install(source, frame, &bytes, &mask_png)?;
        }
    }
    Ok(ImportPlan {
        source: source.to_owned(),
        package: package.display_name(),
        kind: package.kind,
        unverified_rules: !rules.verified
            && (analysed.iter().any(|i| !i.matched.verified)
                || (analysed.is_empty() && !unmatched.is_empty())),
        items: analysed,
        unmatched,
        psd: package.psd,
        licenses: package.license_display(),
        skipped: package.skipped,
        applied: apply,
    })
}

/// Reads and analyses the matched files (in parallel, a few threads).
fn analyse_all(package: &Package, matched: &[(&PackageEntry, String, FileMatch)]) -> Vec<PlanItem> {
    let threads = std::thread::available_parallelism()
        .map_or(2, std::num::NonZero::get)
        .clamp(1, 8);
    let chunk = matched.len().div_ceil(threads).max(1);
    let mut out: Vec<PlanItem> = Vec::with_capacity(matched.len());
    std::thread::scope(|s| {
        let handles: Vec<_> = matched
            .chunks(chunk)
            .map(|part| {
                s.spawn(move || {
                    part.iter()
                        .map(|(e, name, m)| analyse_one(package, e, name, m))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for h in handles {
            out.extend(h.join().expect("analysis thread"));
        }
    });
    out
}

fn analyse_one(package: &Package, e: &PackageEntry, name: &str, m: &FileMatch) -> PlanItem {
    let mut item = PlanItem {
        file: e.path.clone(),
        name: name.to_owned(),
        matched: m.clone(),
        action: Action::Install,
        size: None,
        screen: None,
        radius: None,
        error: None,
        warnings: Vec::new(),
    };
    let result = package
        .read(e)
        .and_then(|bytes| decode_png(&bytes))
        .and_then(|img| analyze(&img));
    match result {
        Ok(a) => {
            item.size = Some(a.size);
            item.screen = Some(a.screen);
            item.radius = Some((a.radius * 10.0).round() / 10.0);
            item.warnings = sanity(m, &a);
        }
        Err(err) => {
            item.action = Action::Failed;
            item.error = Some(err);
        }
    }
    item
}

/// Screen aspect vs the catalog model (±[`ASPECT_TOLERANCE`]); unknown
/// models and orientations.
fn sanity(m: &FileMatch, a: &ScreenAnalysis) -> Vec<String> {
    let mut out = Vec::new();
    let Some(dev) = crate::catalog::device(&m.model) else {
        out.push(format!("unknown model `{}`", m.model));
        return out;
    };
    if !dev.supports(m.orientation) {
        out.push(format!(
            "`{}` does not support {:?} orientation",
            m.model, m.orientation
        ));
    }
    if let Some(aspect) = dev.screenshot.aspect {
        let mut expected = f64::from(aspect.w) / f64::from(aspect.h);
        if m.orientation != dev.natural {
            expected = 1.0 / expected;
        }
        let found = f64::from(a.screen.w) / f64::from(a.screen.h);
        let off = (found / expected - 1.0).abs();
        if off > ASPECT_TOLERANCE {
            out.push(format!(
                "screen {}×{} px (aspect {found:.4}) differs by {:.1} % from `{}` ({expected:.4}); wrong model?",
                a.screen.w,
                a.screen.h,
                off * 100.0,
                m.model
            ));
        }
    }
    out
}

/// One file per model × colour × orientation × vendor model (duplicates
/// only): highest priority, then the largest image, then the last name.
fn pick_winners(items: &mut [PlanItem], installed: &FrameSourceIndex) {
    let key = |i: &PlanItem| {
        (
            i.matched.model.clone(),
            i.matched.color.clone(),
            i.matched.orientation,
            slug(&i.matched.apple_model),
        )
    };
    let rank = |i: &PlanItem| {
        (
            i.matched.priority,
            i.size.map_or(0, |s| u64::from(s.w) * u64::from(s.h)),
            i.name.clone(),
        )
    };
    let n = items.len();
    for a in 0..n {
        if items[a].action == Action::Failed {
            continue;
        }
        let best = (0..n)
            .filter(|&b| items[b].action != Action::Failed && key(&items[b]) == key(&items[a]))
            .max_by(|&x, &y| rank(&items[x]).cmp(&rank(&items[y])))
            .unwrap_or(a);
        items[a].action = if best == a {
            let (model, color, o, vendor) = key(&items[a]);
            if installed.frames.iter().any(|f| {
                f.model == model
                    && f.color == color
                    && f.orientation == o
                    && slug(&f.apple_model) == vendor
            }) {
                Action::Replace
            } else {
                Action::Install
            }
        } else {
            Action::Superseded {
                by: items[best].file.clone(),
            }
        };
    }
}

/// Why [`select_bezel`] chose a bezel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BezelChoice {
    /// The element names it (`bezel:`).
    Pinned,
    /// Its screen has the target's page size (e.g. 1206×2622 →
    /// iPhone 18 Pro for the 6.3" App Store slot).
    Target,
    /// Its screen has the screenshot's pixel size.
    Screenshot,
    /// Its screen aspect is closest to the screenshot (or page) aspect.
    Aspect,
    /// Nothing matched: the largest bezel (fallback).
    Largest,
}

/// Picks one of the installed bezels `candidates` (one catalog model ×
/// colour × orientation, several vendor models):
///
/// 1. `pin` (the element's `bezel:`), if installed;
/// 2. screen size == `target_px` (the target's page size, either
///    orientation): the bezel of the device the store slot stands for;
/// 3. screen size == `screenshot_px` (either orientation);
/// 4. screen aspect within [`ASPECT_TOLERANCE`] of the screenshot aspect
///    (else the page aspect), closest first;
/// 5. the largest screen.
///
/// Ties: higher rule priority, larger screen, then the vendor model name.
/// An unknown `pin` falls through to the automatic choice.
pub fn select_bezel<'f>(
    candidates: &[&'f InstalledFrame],
    pin: Option<&str>,
    target_px: Option<PixelSize>,
    screenshot_px: Option<PixelSize>,
) -> Option<(&'f InstalledFrame, BezelChoice)> {
    if candidates.is_empty() {
        return None;
    }
    if let Some(p) = pin
        && let Some(f) = candidates.iter().find(|f| f.is_bezel(p))
    {
        return Some((f, BezelChoice::Pinned));
    }
    let best = |pool: &mut dyn Iterator<Item = &'f InstalledFrame>| {
        pool.max_by(|a, b| {
            (
                a.priority,
                a.screen_area(),
                std::cmp::Reverse(&a.apple_model),
            )
                .cmp(&(
                    b.priority,
                    b.screen_area(),
                    std::cmp::Reverse(&b.apple_model),
                ))
        })
    };
    let same = |f: &InstalledFrame, px: PixelSize| {
        (f.screen.w, f.screen.h) == (px.w, px.h) || (f.screen.w, f.screen.h) == (px.h, px.w)
    };
    for (px, choice) in [
        (target_px, BezelChoice::Target),
        (screenshot_px, BezelChoice::Screenshot),
    ] {
        if let Some(px) = px.filter(|p| p.w > 0 && p.h > 0)
            && let Some(f) = best(&mut candidates.iter().copied().filter(|f| same(f, px)))
        {
            return Some((f, choice));
        }
    }
    if let Some(px) = screenshot_px.or(target_px).filter(|p| p.w > 0 && p.h > 0) {
        let want = f64::from(px.w) / f64::from(px.h);
        let off = |f: &InstalledFrame| {
            let a = f64::from(f.screen.w) / f64::from(f.screen.h.max(1));
            // Either orientation: a landscape bezel for a portrait shot is
            // still a match of the device, the aspect check warns anyway.
            (a / want - 1.0).abs().min((a * want - 1.0).abs())
        };
        let close: Vec<_> = candidates
            .iter()
            .copied()
            .filter(|f| off(f) <= ASPECT_TOLERANCE)
            .collect();
        if !close.is_empty() {
            let min = close.iter().map(|f| off(f)).fold(f64::INFINITY, f64::min);
            // Within 0.1 % of the best counts as equal (Pro vs Pro Max).
            if let Some(f) = best(&mut close.into_iter().filter(|f| off(f) <= min + 0.001)) {
                return Some((f, BezelChoice::Aspect));
            }
        }
    }
    best(&mut candidates.iter().copied()).map(|f| (f, BezelChoice::Largest))
}

/// Orientation name used in file names and the store.
pub fn orientation_name(o: Orientation) -> &'static str {
    match o {
        Orientation::Portrait => "portrait",
        Orientation::Landscape => "landscape",
    }
}

#[cfg(test)]
mod tests;
