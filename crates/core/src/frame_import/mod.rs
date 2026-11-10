//! Import of vendor device frames (KONZEPT §4.2, ADR 0012): Apple Design
//! Resources "Product Bezels" downloaded by the user.
//!
//! ```text
//! Package::open(folder | .zip | .dmg)       package (PNG entries)
//!   → plan(store, package, rules, dry run)  match names (rules + user mapping),
//!                                           analyse each PNG (screen rect, mask),
//!                                           pick one file per model × colour × orientation
//!   → install                               app-wide FrameStore (never a project)
//! FrameIndex::load(root)                    what resolve / render use
//! ```
//!
//! Projects only say `frame: imported:apple` (plus model and colour);
//! resolve picks the installed frame or falls back to the parametric one
//! with an `imported_frame_missing` info.

mod analyze;
mod package;
mod rules;
mod store;
pub mod synthetic;

use serde::Serialize;

pub use analyze::{
    ALPHA_THRESHOLD, MASK_GROW, PxRect, RgbaImage, ScreenAnalysis, analyze, decode_png,
    encode_gray_png, encode_rgba_png,
};
pub use package::{Package, PackageEntry, PackageKind};
pub use rules::{FileMatch, Pattern, Rule, Rules, normalize_name, slug};
pub use store::{
    Assignment, FrameIndex, FrameSourceIndex, FrameStore, INDEX_FILE, InstalledFrame,
    USER_MAPPING_FILE, UserMapping, today,
};

use crate::model::{Orientation, PixelSize};

/// Folder of the frame store inside the app data folder.
pub const FRAMES_DIR: &str = "frames";

/// Environment variable overriding the app folder (GUI and CLI): app data
/// then lives in `$ARRANGIA_APP_DIR/data`, settings in `…/config`.
pub const APP_DIR_ENV: &str = "ARRANGIA_APP_DIR";

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
    /// Another file wins for the same model × colour × orientation.
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
    /// The user chose to ignore it (user mapping `null`).
    pub ignored: bool,
}

/// Result of [`plan`] / [`import`].
#[derive(Debug, Clone, Serialize)]
pub struct ImportPlan {
    pub source: String,
    pub package: String,
    pub kind: PackageKind,
    /// The rules are not yet verified against a real package.
    pub unverified_rules: bool,
    pub items: Vec<PlanItem>,
    pub unmatched: Vec<Unmatched>,
    /// Non-PNG files (PSDs, licences …).
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
                color_label: a.color.clone(),
                orientation: a.orientation,
                priority: i32::MAX,
                rule: None,
            }),
            Some(None) => {
                unmatched.push(Unmatched {
                    file: e.path.clone(),
                    name,
                    ignored: true,
                });
                continue;
            }
            None => rules.match_file(&e.name),
        };
        match m {
            Some(m) => matched.push((e, name, m)),
            None => unmatched.push(Unmatched {
                file: e.path.clone(),
                name,
                ignored: false,
            }),
        }
    }
    let mut analysed = analyse_all(package, &matched);
    pick_winners(&mut analysed, &installed);
    if apply {
        let package_name = package.display_name();
        let date = today();
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
                file: String::new(),
                mask: String::new(),
                size: a.size,
                screen: a.screen,
                radius: (a.radius * 10.0).round() / 10.0,
                source_file: entry.name.clone(),
                package: package_name.clone(),
                imported: date.clone(),
            };
            let mask_png = encode_gray_png(a.size.w, a.size.h, &a.mask)?;
            store.install(source, frame, &bytes, &mask_png)?;
        }
    }
    Ok(ImportPlan {
        source: source.to_owned(),
        package: package.display_name(),
        kind: package.kind,
        unverified_rules: !rules.verified,
        items: analysed,
        unmatched,
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

/// One file per model × colour × orientation: highest priority, then the
/// largest image, then the last name.
fn pick_winners(items: &mut [PlanItem], installed: &FrameSourceIndex) {
    let key = |i: &PlanItem| {
        (
            i.matched.model.clone(),
            i.matched.color.clone(),
            i.matched.orientation,
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
            let (model, color, o) = key(&items[a]);
            if installed
                .frames
                .iter()
                .any(|f| f.model == model && f.color == color && f.orientation == o)
            {
                Action::Replace
            } else {
                Action::Install
            }
        } else {
            Action::Superseded {
                by: items[best].name.clone(),
            }
        };
    }
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
