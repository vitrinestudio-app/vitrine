//! Canvas management: create (empty, with a starter layout or as
//! a copy of another canvas, optionally with a first target), rename and
//! remove (with the targets showing it). Every command is one undo step;
//! the ops are built here, never in a front end.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::json;
use vitrine_core::Project;
use vitrine_core::catalog;
use vitrine_core::edit::{copy_canvas, fresh_element_id, unique_canvas_id, unique_string_key};
use vitrine_core::model::{
    Aspect, Canvas, CanvasId, Element, ElementId, Fill, Lang, PixelSize, Target, TargetId,
};
use vitrine_core::ops::Op;

use super::AppState;
use super::edit::EditResult;
use super::targets::{TargetAddArgs, preset_target, unique_target_id};
use crate::error::{CmdResult, CommandError, ErrorCode};

/// What a new canvas starts with.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasStart {
    /// Only the canvas background.
    #[default]
    Empty,
    /// A headline and a device on every page.
    Starter,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasAddArgs {
    /// Canvas id (default: made unique from `canvas`, or from the copied
    /// canvas's id).
    #[serde(default)]
    pub id: Option<String>,
    /// Copy this canvas (pages, backgrounds, elements with new ids,
    /// variants); `aspect`/`count`/`start` are then ignored.
    #[serde(default)]
    pub duplicate_of: Option<String>,
    /// With `duplicate_of`: give the copied texts their own translation
    /// keys (`<id>.<key>`, translations copied) instead of sharing them.
    #[serde(default)]
    pub copy_strings: bool,
    /// With `duplicate_of`: also copy the targets showing the original.
    #[serde(default)]
    pub copy_targets: bool,
    /// Page aspect `w:h` (default: the size of `target_preset`).
    #[serde(default)]
    pub aspect: Option<String>,
    /// Page count (default 3, or 1 for a non-phone/tablet aspect).
    #[serde(default)]
    pub count: Option<u32>,
    /// Explicit page ids.
    #[serde(default)]
    pub ids: Vec<String>,
    /// Canvas backgrounds (default: one light colour).
    #[serde(default)]
    pub backgrounds: Option<Vec<Fill>>,
    #[serde(default)]
    pub start: CanvasStart,
    /// Also add a target from this catalog preset showing the new canvas.
    #[serde(default)]
    pub target_preset: Option<String>,
    /// Size of that target (default: the preset's default size).
    #[serde(default)]
    pub target_size: Option<PixelSize>,
}

/// Result of [`canvas_add`].
#[derive(Debug, Clone, Serialize)]
pub struct CanvasAddResult {
    #[serde(flatten)]
    pub edit: EditResult,
    /// Id of the new canvas.
    pub canvas: String,
    /// Ids of the targets added with it.
    pub targets: Vec<String>,
}

/// Default canvas background of a new canvas.
const DEFAULT_BACKGROUND: &str = "#EEF1F6";
/// Placeholder of the starter headlines (default language).
const STARTER_TEXT: &str = "Your headline";
/// Colour of the starter headlines.
const STARTER_COLOR: &str = "#14213D";

fn default_backgrounds() -> CmdResult<Vec<Fill>> {
    serde_json::from_value(json!([DEFAULT_BACKGROUND]))
        .map_err(|e| CommandError::internal(format!("default background: {e}")))
}

fn canvas_of<'p>(p: &'p Project, id: &str) -> CmdResult<&'p Canvas> {
    p.canvas(id)
        .ok_or_else(|| CommandError::not_found(format!("unknown canvas `{id}`")))
}

fn new_canvas_id(p: &Project, id: Option<&str>, base: &str) -> CmdResult<CanvasId> {
    match id {
        Some(id) => {
            let id = CanvasId::parse(id.trim()).map_err(CommandError::invalid)?;
            if p.canvas(id.as_str()).is_some() {
                return Err(CommandError::invalid(format!(
                    "a canvas `{id}` already exists"
                )));
            }
            Ok(id)
        }
        None => Ok(unique_canvas_id(p, base)),
    }
}

/// Headline + device per page (like the `minimal` project template, but
/// without depending on project styles other than an existing `headline`).
fn starter_elements(
    p: &Project,
    canvas: &Canvas,
    device: &str,
    ops: &mut Vec<Op>,
) -> CmdResult<Vec<Element>> {
    let page_h = canvas.page.aspect.page_height();
    let dev = catalog::device(device)
        .ok_or_else(|| CommandError::not_found(format!("unknown device `{device}`")))?;
    // The device fills at most 60 % of the page width and 55 % of its
    // height, centred a bit below the middle (the headline sits on top).
    let width = (600.0_f64)
        .min(page_h * 0.55 * dev.body.w / dev.body.h)
        .round();
    let lang = Lang::parse(p.languages.default.as_str()).map_err(CommandError::invalid)?;
    let style = p.styles.text.contains_key("headline");
    let mut taken = BTreeSet::new();
    let mut keys = BTreeSet::new();
    let mut out = Vec::new();
    for i in 0..canvas.page.count {
        let page_id = canvas.page.page_id(i);
        let key = unique_string_key(p, &format!("{}.{page_id}.title", canvas.id), &keys);
        keys.insert(key.to_string());
        ops.push(Op::SetString {
            lang: lang.clone(),
            key: key.clone(),
            value: Some(STARTER_TEXT.to_owned()),
        });
        let title: ElementId =
            fresh_element_id(p, &format!("{}-title-{}", canvas.id, i + 1), &mut taken);
        let device_id = fresh_element_id(p, &format!("{}-device-{}", canvas.id, i + 1), &mut taken);
        let mut text = json!({
            "id": title, "kind": "text", "page": i, "anchor_y": "top",
            "text": key, "box": { "top": (page_h * 0.07).round(), "left": 80, "right": 80 },
            "align": "center", "fit": { "shrink_to": 40, "max_lines": 3 },
            // Dark text on the light default background, whatever colour
            // the project's headline style has.
            "color": STARTER_COLOR,
        });
        if style {
            text["style"] = json!("headline");
        } else {
            text["font_size"] = json!(72);
        }
        let device = json!({
            "id": device_id, "kind": "device", "page": i, "anchor_y": "center",
            "at": { "x": 500, "y": (page_h * 0.6).round() }, "width": width,
            "model": device, "screenshot": page_id,
        });
        for v in [text, device] {
            out.push(
                serde_json::from_value(v)
                    .map_err(|e| CommandError::internal(format!("starter element: {e}")))?,
            );
        }
    }
    Ok(out)
}

/// Device of the starter layout: the preset's device, else the first
/// phone of the catalog.
fn starter_device(preset: Option<&str>) -> String {
    preset
        .and_then(catalog::target_preset)
        .and_then(|p| p.device.clone())
        .or_else(|| {
            catalog::devices()
                .iter()
                .find(|d| d.class == catalog::DeviceClass::Phone)
                .map(|d| d.id.clone())
        })
        .unwrap_or_else(|| "iphone-island".to_owned())
}

/// A new canvas from `args` (aspect, count, backgrounds, starter layout);
/// string ops of the starter texts go to `ops`.
fn fresh_canvas(p: &Project, args: &CanvasAddArgs, ops: &mut Vec<Op>) -> CmdResult<Canvas> {
    let preset = args
        .target_preset
        .as_deref()
        .map(|id| {
            catalog::target_preset(id)
                .ok_or_else(|| CommandError::not_found(format!("unknown preset `{id}`")))
        })
        .transpose()?;
    let aspect = match (&args.aspect, preset) {
        (Some(a), _) => Aspect::parse(a).map_err(CommandError::invalid)?,
        (None, Some(pr)) => {
            let size = args
                .target_size
                .or_else(|| pr.default_size())
                .ok_or_else(|| CommandError::invalid(format!("preset `{}` has no size", pr.id)))?;
            Aspect::new(size.w, size.h)
        }
        (None, None) => {
            return Err(CommandError::invalid(
                "give the page aspect (`aspect`) or a target preset",
            ));
        }
    };
    let count = args
        .count
        .unwrap_or(if aspect.h > aspect.w { 3 } else { 1 });
    if count == 0 || count > 99 {
        return Err(CommandError::invalid("a canvas has 1 to 99 pages"));
    }
    let id = new_canvas_id(p, args.id.as_deref(), "canvas")?;
    let mut canvas = Canvas::new(id, aspect, count);
    canvas.page.ids.clone_from(&args.ids);
    canvas.backgrounds = args
        .backgrounds
        .clone()
        .map_or_else(default_backgrounds, Ok)?;
    if args.start == CanvasStart::Starter {
        let device = starter_device(args.target_preset.as_deref());
        canvas.elements = starter_elements(p, &canvas, &device, ops)?;
    }
    Ok(canvas)
}

/// Adds a canvas as one undo step: empty, with a starter layout
/// or as a copy of another canvas, optionally with a first target.
pub fn canvas_add(app: &AppState, args: CanvasAddArgs) -> CmdResult<CanvasAddResult> {
    let session = app.session()?;
    let p = session.snapshot();
    let mut ops = Vec::new();
    let mut targets: Vec<Target> = Vec::new();
    let mut taken_targets = BTreeSet::new();

    let canvas = if let Some(src_id) = &args.duplicate_of {
        let src = canvas_of(&p, src_id)?;
        let id = new_canvas_id(&p, args.id.as_deref(), src.id.as_str())?;
        let copy = copy_canvas(&p, src, id.clone(), args.copy_strings);
        ops.extend(copy.string_ops);
        if args.copy_targets {
            for t in p.targets.iter().filter(|t| t.canvas == src.id) {
                let mut t = t.clone();
                let new_id = unique_target_id(&p, &format!("{}-{id}", t.id), &taken_targets);
                taken_targets.insert(new_id.clone());
                t.id = TargetId::parse(&new_id).map_err(CommandError::invalid)?;
                t.canvas = id.clone();
                // `visible_in` of the copied elements still names the
                // original targets; the copies show everything.
                targets.push(t);
            }
        }
        copy.canvas
    } else {
        fresh_canvas(&p, &args, &mut ops)?
    };

    if let Some(preset) = &args.target_preset {
        let target = preset_target(
            &p,
            &TargetAddArgs {
                preset: preset.clone(),
                canvas: canvas.id.to_string(),
                id: None,
                size: args.target_size,
                orientation: None,
            },
            canvas.page.count,
            &taken_targets,
        )?;
        targets.push(target);
    }

    let id = canvas.id.to_string();
    let target_ids: Vec<String> = targets.iter().map(|t| t.id.to_string()).collect();
    ops.insert(
        0,
        Op::AddCanvas {
            canvas,
            index: None,
        },
    );
    ops.extend(targets.into_iter().map(|target| Op::AddTarget {
        target,
        index: None,
    }));
    let label = match &args.duplicate_of {
        Some(src) => format!("Duplicate canvas {src} as {id}"),
        None => format!("Add canvas {id}"),
    };
    let result = session.apply_ops(ops, Some(label))?;
    Ok(CanvasAddResult {
        edit: EditResult {
            result,
            created: vec![id.clone()],
        },
        canvas: id,
        targets: target_ids,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasRenameArgs {
    pub canvas: String,
    pub to: String,
}

/// Renames a canvas; targets showing it follow.
pub fn canvas_rename(app: &AppState, args: CanvasRenameArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let p = session.snapshot();
    let from = canvas_of(&p, &args.canvas)?.id.clone();
    let to = CanvasId::parse(args.to.trim()).map_err(CommandError::invalid)?;
    if to == from {
        return Ok(EditResult {
            result: session.unchanged(),
            created: Vec::new(),
        });
    }
    let result = session.apply_ops(
        vec![Op::RenameCanvas {
            from: from.clone(),
            to: to.clone(),
        }],
        Some(format!("Rename canvas {from} to {to}")),
    )?;
    Ok(EditResult {
        result,
        created: vec![to.to_string()],
    })
}

/// Removes a canvas with its elements and the targets showing it (one
/// undo step). Refused for the last canvas and while an element of another
/// canvas limits its visibility to one of those targets.
pub fn canvas_remove(app: &AppState, args: super::inspect::CanvasArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let p = session.snapshot();
    let s = canvas_of(&p, &args.canvas)?;
    if p.canvases.len() <= 1 {
        return Err(CommandError::new(
            ErrorCode::OpRejected,
            "the last canvas cannot be removed",
        ));
    }
    // Elements first (their `visible_in` may name the canvas's targets),
    // then the targets, then the canvas itself.
    let mut ops = p.clip_to_release_canvas_ops(&s.id);
    ops.extend(
        s.elements
            .iter()
            .rev()
            .map(|e| Op::RemoveElement { id: e.id.clone() }),
    );
    ops.extend(
        p.targets
            .iter()
            .filter(|t| t.canvas == s.id)
            .map(|t| Op::RemoveTarget { id: t.id.clone() }),
    );
    ops.push(Op::RemoveCanvas { id: s.id.clone() });
    let result = session.apply_ops(ops, Some(format!("Remove canvas {}", s.id)))?;
    Ok(EditResult {
        result,
        created: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::commands::dispatch;
    use crate::commands::targets::tests::{call, opened};

    fn canvases(app: &crate::AppState) -> Vec<String> {
        let s = call(app, "project_summary", Value::Null);
        s["canvases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x["id"].as_str().unwrap().to_owned())
            .collect()
    }

    #[test]
    fn add_empty_and_starter_with_target() {
        let (_tmp, app) = opened();
        let r = call(
            &app,
            "canvas_add",
            json!({"aspect": "2064:2752", "count": 2}),
        );
        assert_eq!(r["canvas"], "canvas");
        assert_eq!(r["targets"], json!([]));
        let s = call(&app, "canvas_get", json!({"canvas": "canvas"}));
        assert_eq!(s["count"], 2);
        assert_eq!(s["backgrounds"].as_array().unwrap().len(), 1);

        let r = call(
            &app,
            "canvas_add",
            json!({"id": "tablet", "start": "starter", "target_preset": "ios-ipad-13"}),
        );
        assert_eq!(r["canvas"], "tablet");
        let target = r["targets"][0].as_str().unwrap().to_owned();
        let t = call(&app, "target_get", json!({"id": target}));
        assert_eq!(t["target"]["canvas"], "tablet");
        let s = call(&app, "canvas_get", json!({"canvas": "tablet"}));
        // Portrait aspect from the preset → 3 pages, headline + device each.
        assert_eq!(s["count"], 3);
        let o = call(&app, "overlay", json!({"target": target}));
        assert_eq!(o["elements"].as_array().unwrap().len(), 6);

        // One undo step removes canvas, target and strings.
        call(&app, "undo", Value::Null);
        assert!(!canvases(&app).contains(&"tablet".to_owned()));
        assert!(dispatch(&app, "target_get", json!({"id": target})).is_err());

        // Duplicate id and missing aspect are refused.
        assert!(dispatch(&app, "canvas_add", json!({"id": "phone", "aspect": "1:2"})).is_err());
        assert!(dispatch(&app, "canvas_add", json!({})).is_err());
    }

    #[test]
    fn duplicate_rename_remove() {
        let (_tmp, app) = opened();
        let r = call(
            &app,
            "canvas_add",
            json!({"duplicate_of": "phone", "id": "phone-copy", "copy_targets": true}),
        );
        assert_eq!(r["canvas"], "phone-copy");
        let targets = r["targets"].as_array().unwrap().clone();
        assert_ne!(targets, Vec::<Value>::new());
        let copy = call(&app, "canvas_get", json!({"canvas": "phone-copy"}));
        let orig = call(&app, "canvas_get", json!({"canvas": "phone"}));
        assert_eq!(copy["count"], orig["count"]);
        assert_eq!(
            copy["variants"].as_array().unwrap().len(),
            orig["variants"].as_array().unwrap().len()
        );
        assert_eq!(copy["targets"].as_array().unwrap().len(), targets.len());

        let r = call(
            &app,
            "canvas_rename",
            json!({"canvas": "phone-copy", "to": "tablet"}),
        );
        assert_eq!(r["created"], json!(["tablet"]));
        let t = call(&app, "target_get", json!({"id": targets[0]}));
        assert_eq!(t["target"]["canvas"], "tablet");

        call(&app, "canvas_remove", json!({"canvas": "tablet"}));
        assert!(!canvases(&app).contains(&"tablet".to_owned()));
        assert!(dispatch(&app, "target_get", json!({"id": targets[0]})).is_err());
        call(&app, "undo", Value::Null);
        assert!(canvases(&app).contains(&"tablet".to_owned()));
        call(&app, "target_get", json!({"id": targets[0]}));
    }

    #[test]
    fn variant_from_copy_with_targets_and_detaching_remove() {
        let (_tmp, app) = opened();
        let r = call(
            &app,
            "canvas_variant",
            json!({"canvas": "phone", "change": {"action": "add", "id": "android-2", "from": "android", "targets": ["ios-iphone-63"]}}),
        );
        assert_eq!(r["created"], json!(["android-2"]));
        let s = call(&app, "canvas_get", json!({"canvas": "phone"}));
        let v = |id: &str| {
            s["variants"]
                .as_array()
                .unwrap()
                .iter()
                .find(|x| x["id"] == id)
                .unwrap()
                .clone()
        };
        assert_eq!(v("android-2")["overrides"], v("android")["overrides"]);
        assert_eq!(v("android-2")["targets"], json!(["ios-iphone-63"]));

        // Removing a variant in use detaches its targets (one undo step).
        call(
            &app,
            "canvas_variant",
            json!({"canvas": "phone", "change": {"action": "remove", "id": "android"}}),
        );
        let t = call(&app, "target_get", json!({"id": "play-phone"}));
        assert!(t["target"].get("variant").is_none());
        call(&app, "undo", Value::Null);
        let t = call(&app, "target_get", json!({"id": "play-phone"}));
        assert_eq!(t["target"]["variant"], "android");

        // Targets of another canvas are refused.
        let other = call(&app, "canvas_get", json!({"canvas": "mac"}))["targets"][0].clone();
        assert!(
            dispatch(
                &app,
                "canvas_variant",
                json!({"canvas": "phone", "change": {"action": "add", "id": "x", "targets": [other]}}),
            )
            .is_err()
        );
    }
}
