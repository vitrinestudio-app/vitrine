//! Targets editor: full target model, add from a catalog preset, patch
//! (incl. rename) and remove. The UI sends field values; the ops are built
//! here (`AddTarget`, `UpdateTarget`, `RenameTarget`, `RemoveTarget`).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use vitrine_core::catalog;
use vitrine_core::model::{
    BannerFit, CanvasId, GalleryRef, Gaps, Orientation, PixelSize, Region, Target, TargetId,
    TargetKind, VariantId,
};
use vitrine_core::ops::{Op, TargetPatch};

use super::AppState;
use super::edit::EditResult;
use super::project::check_target;
use crate::error::{CmdResult, CommandError};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetIdArgs {
    pub id: String,
}

/// A target as stored in `project.yaml` (all fields).
#[derive(Debug, Clone, Serialize)]
pub struct TargetModel {
    pub target: Target,
    /// Page count of the target's canvas (for banner page ranges).
    pub pages: u32,
    /// Variants of the target's canvas.
    pub variants: Vec<String>,
}

pub fn target_get(app: &AppState, args: TargetIdArgs) -> CmdResult<TargetModel> {
    let session = app.session()?;
    session.with_project(|p| {
        let t = p
            .target(&args.id)
            .ok_or_else(|| CommandError::not_found(format!("unknown target `{}`", args.id)))?;
        let canvas = p.canvas(t.canvas.as_str());
        Ok(TargetModel {
            target: t.clone(),
            pages: canvas.map_or(0, |s| s.page.count),
            variants: canvas
                .map(|s| s.variants.keys().map(ToString::to_string).collect())
                .unwrap_or_default(),
        })
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetAddArgs {
    /// Catalog preset id (`catalog_targets`).
    pub preset: String,
    /// Canvas the target shows.
    pub canvas: String,
    /// Target id (default: the preset id, made unique).
    #[serde(default)]
    pub id: Option<String>,
    /// Export size (default: the preset's default size, or its first size
    /// in `orientation`).
    #[serde(default)]
    pub size: Option<PixelSize>,
    #[serde(default)]
    pub orientation: Option<Orientation>,
}

/// `base`, or `base-2`, `base-3` … if taken (by the project or `taken`).
pub(super) fn unique_target_id(
    project: &vitrine_core::Project,
    base: &str,
    taken: &BTreeSet<String>,
) -> String {
    let free = |id: &str| project.target(id).is_none() && !taken.contains(id);
    if free(base) {
        return base.to_owned();
    }
    // At most one id per existing or taken target is in use.
    (2..=project.targets.len() + taken.len() + 2)
        .map(|n| format!("{base}-{n}"))
        .find(|id| free(id))
        .unwrap_or_else(|| base.to_owned())
}

/// A target from a catalog preset (size, kind, gallery profile, screenshot
/// variant and store slot of the preset) showing `canvas` with
/// `page_count` pages; the id defaults to the preset id made unique
/// against the project and `taken`.
pub(super) fn preset_target(
    project: &vitrine_core::Project,
    args: &TargetAddArgs,
    page_count: u32,
    taken: &BTreeSet<String>,
) -> CmdResult<Target> {
    let preset = catalog::target_preset(&args.preset)
        .ok_or_else(|| CommandError::not_found(format!("unknown preset `{}`", args.preset)))?;
    let id = match &args.id {
        Some(id) => id.clone(),
        None => unique_target_id(project, &preset.id, taken),
    };
    let size = args
        .size
        .or_else(|| match args.orientation {
            Some(Orientation::Landscape) => preset.sizes.landscape.first().copied(),
            Some(Orientation::Portrait) => preset.sizes.portrait.first().copied(),
            None => None,
        })
        .or_else(|| preset.default_size())
        .ok_or_else(|| CommandError::invalid(format!("preset `{}` has no size", preset.id)))?;
    let mut target = Target::new(
        TargetId::parse(&id).map_err(CommandError::invalid)?,
        CanvasId::parse(&args.canvas).map_err(CommandError::invalid)?,
        size,
    );
    target.kind = preset.kind;
    target.screenshots.clone_from(&preset.screenshots);
    target.gallery = preset.gallery.as_deref().map(GalleryRef::preset);
    target.store.clone_from(&preset.slot);
    target.screenshot_only = preset.screenshot_only;
    target.safe_area = preset.safe_area;
    if target.kind == TargetKind::Banner {
        target.gallery = None;
        target.fit = preset.fit;
        // Page ranges are clamped to the canvas (a 2-page canvas gets [0, 1]).
        let last = page_count.saturating_sub(1);
        target.region = preset.region.map(|r| match r {
            Region::Pages([a, b]) => Region::Pages([a.min(last), b.min(last)]),
            other @ Region::Rect(_) => other,
        });
    }
    Ok(target)
}

/// Adds a target from a catalog preset (see [`preset_target`]).
pub fn target_add_from_preset(app: &AppState, args: TargetAddArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let project = session.snapshot();
    let canvas = project
        .canvas(&args.canvas)
        .ok_or_else(|| CommandError::not_found(format!("unknown canvas `{}`", args.canvas)))?;
    let target = preset_target(&project, &args, canvas.page.count, &BTreeSet::new())?;
    let id = target.id.to_string();
    let result = session.apply_ops(
        vec![Op::AddTarget {
            target,
            index: None,
        }],
        Some(format!("Add target {id}")),
    )?;
    Ok(EditResult {
        result,
        created: vec![id],
    })
}

/// Store banners offered by "Banner from this canvas" (phase 6.2): the
/// Google Play feature graphic and the App Store creative assets.
pub const BANNER_PRESETS: [&str; 4] = [
    "play-feature-graphic",
    "ios-header",
    "ios-search-results",
    "ios-creative-universal",
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerOptionsArgs {
    pub canvas: String,
    /// First page of the proposed regions (default 0).
    #[serde(default)]
    pub page: Option<u32>,
}

/// One proposal of [`canvas_banner_options`].
#[derive(Debug, Clone, Serialize)]
pub struct BannerOption {
    pub preset: String,
    pub label: String,
    /// `apple` or `google`.
    pub store: String,
    pub size: PixelSize,
    /// Proposed page range: as many pages from `page` as match the banner's
    /// aspect best.
    pub pages: [u32; 2],
    /// Targets of this canvas that already use this store slot.
    pub existing: Vec<String>,
}

/// How many pages of `page_aspect` (w/h per page) side by side come
/// closest to `banner` (w/h), at most `count`.
fn pages_for(page_aspect: f64, banner: f64, count: u32) -> u32 {
    if page_aspect <= 0.0 {
        return 1;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a small page count"
    )]
    let n = (banner / page_aspect).round().max(1.0) as u32;
    n.clamp(1, count.max(1))
}

fn page_aspect(canvas: &vitrine_core::model::Canvas) -> f64 {
    f64::from(canvas.page.aspect.w) / f64::from(canvas.page.aspect.h.max(1))
}

fn aspect_of(size: PixelSize) -> f64 {
    f64::from(size.w) / f64::from(size.h.max(1))
}

/// Proposals for "Banner from this canvas": the store banner presets with
/// a page range of `canvas` that fits each one.
pub fn canvas_banner_options(
    app: &AppState,
    args: BannerOptionsArgs,
) -> CmdResult<Vec<BannerOption>> {
    let session = app.session()?;
    session.with_project(|p| {
        let canvas = p
            .canvas(&args.canvas)
            .ok_or_else(|| CommandError::not_found(format!("unknown canvas `{}`", args.canvas)))?;
        let count = canvas.page.count.max(1);
        let first = args.page.unwrap_or(0).min(count - 1);
        Ok(BANNER_PRESETS
            .iter()
            .filter_map(|id| catalog::target_preset(id))
            .filter_map(|preset| {
                let size = preset.default_size()?;
                let n = pages_for(page_aspect(canvas), aspect_of(size), count);
                let start = first.min(count - n);
                let existing = p
                    .targets
                    .iter()
                    .filter(|t| {
                        t.canvas.as_str() == args.canvas
                            && t.store.is_some()
                            && t.store == preset.slot
                    })
                    .map(|t| t.id.to_string())
                    .collect();
                Some(BannerOption {
                    preset: preset.id.clone(),
                    label: preset.label.clone(),
                    store: match preset.store {
                        catalog::Store::Apple => "apple",
                        catalog::Store::Google => "google",
                        catalog::Store::None => "none",
                    }
                    .to_owned(),
                    size,
                    pages: [start, start + n - 1],
                    existing,
                })
            })
            .collect())
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BannerFromCanvasArgs {
    pub canvas: String,
    /// A banner preset (`play-feature-graphic`, `ios-header`,
    /// `ios-search-results`, `ios-creative-universal`, or any other banner
    /// preset of `catalog_targets`).
    pub preset: String,
    /// Page range of the region (default: the proposal of
    /// [`canvas_banner_options`] from `page`).
    #[serde(default)]
    pub pages: Option<[u32; 2]>,
    /// First page for the default range (default 0).
    #[serde(default)]
    pub page: Option<u32>,
    /// A free rectangle (or page range) instead of `pages`.
    #[serde(default)]
    pub region: Option<Region>,
    /// `cover` (default) or `contain`.
    #[serde(default)]
    pub fit: Option<BannerFit>,
    /// Variant (default: that of the store's screenshot target on this
    /// canvas).
    #[serde(default)]
    pub variant: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub size: Option<PixelSize>,
}

/// "Banner from this canvas": adds a banner target for a store banner
/// preset showing a region of `canvas` (seamless, `fit: cover` by
/// default). Screenshot variant, device map and variant are taken from a
/// screenshot target of the same store on this canvas (a Play banner shows
/// Android devices like the Play screenshots). One undo step.
pub fn target_add_banner(app: &AppState, args: BannerFromCanvasArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let project = session.snapshot();
    let canvas = project
        .canvas(&args.canvas)
        .ok_or_else(|| CommandError::not_found(format!("unknown canvas `{}`", args.canvas)))?;
    let preset = catalog::target_preset(&args.preset)
        .ok_or_else(|| CommandError::not_found(format!("unknown preset `{}`", args.preset)))?;
    if preset.kind != TargetKind::Banner {
        return Err(CommandError::invalid(format!(
            "preset `{}` is not a banner",
            preset.id
        )));
    }
    let add = TargetAddArgs {
        preset: args.preset.clone(),
        canvas: args.canvas.clone(),
        id: args.id.clone(),
        size: args.size,
        orientation: None,
    };
    let mut target = preset_target(&project, &add, canvas.page.count, &BTreeSet::new())?;
    let count = canvas.page.count.max(1);
    let region = match (args.region, args.pages) {
        (Some(r), _) => r,
        (None, Some([a, b])) => {
            if a > b || b >= count {
                return Err(CommandError::invalid(format!(
                    "pages [{a}, {b}] are outside canvas `{}` ({count} pages)",
                    args.canvas
                )));
            }
            Region::Pages([a, b])
        }
        (None, None) => {
            let n = pages_for(page_aspect(canvas), aspect_of(target.size), count);
            let start = args.page.unwrap_or(0).min(count - n);
            Region::Pages([start, start + n - 1])
        }
    };
    target.region = Some(region);
    target.fit = Some(args.fit.unwrap_or(BannerFit::Cover));
    target.gaps = Some(Gaps::None);
    // The store's look: screenshots, device map and variant of a sibling.
    let same_store = |t: &&Target| {
        t.canvas.as_str() == args.canvas
            && t.kind == TargetKind::Pages
            && t.store.as_ref().is_some_and(|s| match preset.store {
                catalog::Store::Apple => s.apple.is_some(),
                catalog::Store::Google => s.google.is_some(),
                catalog::Store::None => false,
            })
    };
    if let Some(sibling) = project.targets.iter().find(same_store) {
        if target.screenshots.is_none() {
            target.screenshots.clone_from(&sibling.screenshots);
        }
        target.device_map.clone_from(&sibling.device_map);
        if args.variant.is_none() {
            target.variant.clone_from(&sibling.variant);
        }
    }
    if let Some(v) = &args.variant {
        target.variant = Some(VariantId::parse(v).map_err(CommandError::invalid)?);
    }
    let id = target.id.to_string();
    let result = session.apply_ops(
        vec![Op::AddTarget {
            target,
            index: None,
        }],
        Some(format!("Add banner {id}")),
    )?;
    Ok(EditResult {
        result,
        created: vec![id],
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetPatchArgs {
    pub id: String,
    /// Fields to change (`null` resets an optional field).
    #[serde(default)]
    pub patch: TargetPatch,
    /// New id (renames the target; references follow).
    #[serde(default)]
    pub rename: Option<String>,
}

/// Changes fields of a target and/or renames it, as one undo step.
pub fn target_patch(app: &AppState, args: TargetPatchArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    check_target(&session.snapshot(), &args.id)?;
    let mut ops = Vec::new();
    if args.patch != TargetPatch::default() {
        ops.push(Op::UpdateTarget {
            id: TargetId::new(&args.id),
            patch: args.patch,
        });
    }
    let mut created = Vec::new();
    if let Some(to) = args.rename.filter(|to| *to != args.id) {
        ops.push(Op::RenameTarget {
            from: TargetId::new(&args.id),
            to: TargetId::parse(&to).map_err(CommandError::invalid)?,
        });
        created.push(to);
    }
    let result = if ops.is_empty() {
        session.unchanged()
    } else {
        session.apply_ops(ops, Some(format!("Edit target {}", args.id)))?
    };
    Ok(EditResult { result, created })
}

pub fn target_remove(app: &AppState, args: TargetIdArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    check_target(&session.snapshot(), &args.id)?;
    let result = session.apply_ops(
        vec![Op::RemoveTarget {
            id: TargetId::new(&args.id),
        }],
        Some(format!("Remove target {}", args.id)),
    )?;
    Ok(EditResult {
        result,
        created: Vec::new(),
    })
}

#[cfg(test)]
pub(super) mod tests {
    use std::sync::Arc;

    use serde_json::{Value, json};

    use crate::commands::{AppState, Reply, dispatch};
    use crate::dialogs::NoDialogs;
    use crate::events::RecordingSink;
    use crate::settings::AppPaths;
    use crate::testutil::example_copy;

    /// App with the example project open (temp copy, no watcher).
    pub(in crate::commands) fn opened() -> (tempfile::TempDir, AppState) {
        let (tmp, dir) = example_copy();
        let app = AppState::new(
            AppPaths::in_dir(&tmp.path().join("app")),
            Arc::new(RecordingSink::default()),
            Arc::new(NoDialogs),
        )
        .without_watcher();
        call(&app, "project_open", json!({ "dir": dir }));
        (tmp, app)
    }

    pub(in crate::commands) fn call(app: &AppState, name: &str, args: Value) -> Value {
        match dispatch(app, name, args) {
            Ok(Reply::Json(v)) => v,
            Ok(Reply::Image(_)) => panic!("{name} returned an image"),
            Err(e) => panic!("{name} failed: {e:?}"),
        }
    }

    #[test]
    fn add_from_preset_patch_rename_remove() {
        let (_tmp, app) = opened();
        let r = call(
            &app,
            "target_add_from_preset",
            json!({"preset": "ios-iphone-69", "canvas": "phone"}),
        );
        // `ios-iphone-69` exists in the example: the id is made unique.
        assert_eq!(r["created"], json!(["ios-iphone-69-2"]));
        let t = call(&app, "target_get", json!({"id": "ios-iphone-69-2"}));
        assert_eq!(t["target"]["size"], json!([1320, 2868]));
        assert_eq!(t["target"]["gallery"], "appstore-iphone");
        assert_eq!(t["target"]["screenshots"], "ios-phone");
        assert!(t["target"]["store"]["apple"]["display_type"].is_string());
        assert_eq!(t["pages"], 6);

        let r = call(
            &app,
            "target_add_from_preset",
            json!({"preset": "ios-iphone-69", "canvas": "phone", "id": "land", "orientation": "landscape"}),
        );
        assert_eq!(r["created"], json!(["land"]));
        let t = call(&app, "target_get", json!({"id": "land"}));
        assert_eq!(t["target"]["size"], json!([2868, 1320]));

        // patch + rename as one undo step
        let r = call(
            &app,
            "target_patch",
            json!({"id": "land", "patch": {"format": "jpeg", "quality": 80, "gallery": null, "langs": ["de"]}, "rename": "landscape"}),
        );
        assert_eq!(r["applied"], true);
        assert_eq!(r["created"], json!(["landscape"]));
        let t = call(&app, "target_get", json!({"id": "landscape"}));
        assert_eq!(t["target"]["format"], "jpeg");
        assert_eq!(t["target"]["quality"], 80);
        assert!(t["target"].get("gallery").is_none());
        assert_eq!(t["target"]["langs"], json!(["de"]));
        call(&app, "undo", Value::Null);
        call(&app, "target_get", json!({"id": "land"}));

        // nothing to do
        let r = call(&app, "target_patch", json!({"id": "land"}));
        assert_eq!(r["applied"], false);

        // invalid values are rejected by the op
        let e = dispatch(
            &app,
            "target_patch",
            json!({"id": "land", "patch": {"quality": 0}}),
        )
        .unwrap_err();
        assert_eq!(e.code, crate::ErrorCode::OpRejected);

        call(&app, "target_remove", json!({"id": "land"}));
        assert!(dispatch(&app, "target_get", json!({"id": "land"})).is_err());
        assert!(
            dispatch(
                &app,
                "target_add_from_preset",
                json!({"preset": "nope", "canvas": "phone"})
            )
            .is_err()
        );
    }

    #[test]
    fn creative_and_watch_presets() {
        let (_tmp, app) = opened();
        call(
            &app,
            "target_add_from_preset",
            json!({"preset": "ios-creative-universal", "canvas": "phone"}),
        );
        let t = call(&app, "target_get", json!({"id": "ios-creative-universal"}));
        assert_eq!(t["target"]["kind"], "banner");
        assert_eq!(t["target"]["size"], json!([5244, 2950]));
        assert_eq!(t["target"]["region"], json!({"pages": [0, 3]}));
        assert_eq!(t["target"]["fit"], "cover");
        assert_eq!(t["target"]["store"]["apple"]["placement"], "universal");
        call(
            &app,
            "target_add_from_preset",
            json!({"preset": "watch-ultra", "canvas": "phone"}),
        );
        let t = call(&app, "target_get", json!({"id": "watch-ultra"}));
        assert_eq!(t["target"]["screenshot_only"], true);
        assert_eq!(t["target"]["screenshots"], "watch");
        // The UI's patch switches it off and sets a safe area.
        call(
            &app,
            "target_patch",
            json!({"id": "watch-ultra", "patch": {"screenshot_only": false, "safe_area": {"top": 5, "right": 0, "bottom": 5, "left": 0}}}),
        );
        let t = call(&app, "target_get", json!({"id": "watch-ultra"}));
        assert!(t["target"].get("screenshot_only").is_none());
        assert_eq!(t["target"]["safe_area"]["top"], 5.0);
        let devices = call(&app, "catalog_devices", Value::Null);
        let duo = devices
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["id"] == "iphone-duo")
            .unwrap();
        assert_eq!(duo["fold"]["other"], "iphone-duo-folded");
    }

    #[test]
    fn banner_presets_have_no_gallery() {
        let (_tmp, app) = opened();
        let presets = call(&app, "catalog_targets", Value::Null);
        let banner = presets
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["kind"] == "banner")
            .expect("a banner preset");
        let r = call(
            &app,
            "target_add_from_preset",
            json!({"preset": banner["id"], "canvas": "web"}),
        );
        let id = r["created"][0].as_str().unwrap();
        let t = call(&app, "target_get", json!({"id": id}));
        assert_eq!(t["target"]["kind"], "banner");
        assert!(t["target"].get("gallery").is_none());
    }

    #[test]
    fn banner_from_canvas_options_and_add() {
        let (_tmp, app) = opened();
        let opts = call(&app, "canvas_banner_options", json!({"canvas": "phone"}));
        let by = |id: &str| {
            opts.as_array()
                .unwrap()
                .iter()
                .find(|o| o["preset"] == id)
                .unwrap()
                .clone()
        };
        // Phone pages are 1320:2868 (0.46): 1024x500 (2.05) ≈ 4 pages,
        // 21:9 ≈ 5, 3:2 ≈ 3, 16:9 ≈ 4.
        assert_eq!(by("play-feature-graphic")["pages"], json!([0, 3]));
        assert_eq!(
            by("play-feature-graphic")["existing"],
            json!(["play-feature-graphic"])
        );
        assert_eq!(by("ios-header")["pages"], json!([0, 4]));
        assert_eq!(by("ios-search-results")["pages"], json!([0, 2]));
        assert_eq!(by("ios-creative-universal")["pages"], json!([0, 3]));
        assert_eq!(by("ios-header")["store"], "apple");
        // From page 4 of 6: clamped so the range fits.
        let late = call(
            &app,
            "canvas_banner_options",
            json!({"canvas": "phone", "page": 4}),
        );
        assert_eq!(late[0]["pages"], json!([2, 5]));

        // Play: Android look from the Play screenshot target.
        let r = call(
            &app,
            "target_add_banner",
            json!({"canvas": "phone", "preset": "play-feature-graphic"}),
        );
        let id = r["created"][0].as_str().unwrap();
        assert_eq!(id, "play-feature-graphic-2");
        let t = &call(&app, "target_get", json!({"id": id}))["target"];
        assert_eq!(t["kind"], "banner");
        assert_eq!(t["size"], json!([1024, 500]));
        assert_eq!(t["region"], json!({"pages": [0, 3]}));
        assert_eq!(t["fit"], "cover");
        assert_eq!(t["gaps"], "none");
        assert_eq!(
            t["store"],
            json!({"google": {"image_type": "featureGraphic"}})
        );
        assert_eq!(t["screenshots"], "android-phone");
        assert_eq!(
            t["device_map"],
            json!({"iphone-island": "android-punchhole"})
        );
        assert_eq!(t["variant"], "android");
        // Apple header with an explicit range and variant.
        let r = call(
            &app,
            "target_add_banner",
            json!({"canvas": "phone", "preset": "ios-header", "pages": [1, 5], "variant": "banner"}),
        );
        let t = &call(&app, "target_get", json!({"id": r["created"][0]}))["target"];
        assert_eq!(t["region"], json!({"pages": [1, 5]}));
        assert_eq!(t["variant"], "banner");
        assert_eq!(t["screenshots"], "ios-phone");
        assert_eq!(t["store"]["apple"]["placement"], "header");
        // One undo step each.
        call(&app, "undo", Value::Null);
        assert!(
            dispatch(&app, "target_get", json!({"id": r["created"][0]})).is_err(),
            "undone"
        );
        // Not a banner / outside the canvas.
        for bad in [
            json!({"canvas": "phone", "preset": "play-phone"}),
            json!({"canvas": "phone", "preset": "ios-header", "pages": [3, 9]}),
        ] {
            assert!(dispatch(&app, "target_add_banner", bad).is_err());
        }
    }
}
