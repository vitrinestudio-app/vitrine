//! Targets editor: full target model, add from a catalog preset, patch
//! (incl. rename) and remove. The UI sends field values; the ops are built
//! here (`AddTarget`, `UpdateTarget`, `RenameTarget`, `RemoveTarget`).

use arrangia_core::catalog;
use arrangia_core::model::{
    GalleryRef, Orientation, PixelSize, StripId, Target, TargetId, TargetKind,
};
use arrangia_core::ops::{Op, TargetPatch};
use serde::{Deserialize, Serialize};

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
    /// Page count of the target's strip (for banner page ranges).
    pub pages: u32,
    /// Variants of the target's strip.
    pub variants: Vec<String>,
}

pub fn target_get(app: &AppState, args: TargetIdArgs) -> CmdResult<TargetModel> {
    let session = app.session()?;
    session.with_project(|p| {
        let t = p
            .target(&args.id)
            .ok_or_else(|| CommandError::not_found(format!("unknown target `{}`", args.id)))?;
        let strip = p.strip(t.strip.as_str());
        Ok(TargetModel {
            target: t.clone(),
            pages: strip.map_or(0, |s| s.page.count),
            variants: strip
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
    /// Strip the target shows.
    pub strip: String,
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

/// `base`, or `base-2`, `base-3` … if taken.
fn unique_target_id(project: &arrangia_core::Project, base: &str) -> String {
    if project.target(base).is_none() {
        return base.to_owned();
    }
    // At most one id per existing target is taken.
    (2..=project.targets.len() + 2)
        .map(|n| format!("{base}-{n}"))
        .find(|id| project.target(id).is_none())
        .unwrap_or_else(|| base.to_owned())
}

/// Adds a target from a catalog preset (size, kind, gallery profile,
/// screenshot variant and store slot of the preset).
pub fn target_add_from_preset(app: &AppState, args: TargetAddArgs) -> CmdResult<EditResult> {
    let preset = catalog::target_preset(&args.preset)
        .ok_or_else(|| CommandError::not_found(format!("unknown preset `{}`", args.preset)))?;
    let session = app.session()?;
    let project = session.snapshot();
    if project.strip(&args.strip).is_none() {
        return Err(CommandError::not_found(format!(
            "unknown strip `{}`",
            args.strip
        )));
    }
    let id = match args.id {
        Some(id) => id,
        None => unique_target_id(&project, &preset.id),
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
        StripId::parse(&args.strip).map_err(CommandError::invalid)?,
        size,
    );
    target.kind = preset.kind;
    target.screenshots.clone_from(&preset.screenshots);
    target.gallery = preset.gallery.as_deref().map(GalleryRef::preset);
    target.store.clone_from(&preset.slot);
    if target.kind == TargetKind::Banner {
        target.gallery = None;
    }
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
            json!({"preset": "ios-iphone-69", "strip": "phone"}),
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
            json!({"preset": "ios-iphone-69", "strip": "phone", "id": "land", "orientation": "landscape"}),
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
                json!({"preset": "nope", "strip": "phone"})
            )
            .is_err()
        );
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
            json!({"preset": banner["id"], "strip": "web"}),
        );
        let id = r["created"][0].as_str().unwrap();
        let t = call(&app, "target_get", json!({"id": id}));
        assert_eq!(t["target"]["kind"], "banner");
        assert!(t["target"].get("gallery").is_none());
    }
}
