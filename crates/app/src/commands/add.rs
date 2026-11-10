//! Adding new elements (`element_add`): the toolbar's "Add ▸ Device /
//! Text / Image / Shape / Background". The element is built here with
//! sensible defaults for the view (device model from the target's catalog
//! preset, a new translation key with placeholder text, the `headline`
//! style …) and added to the base strip as one undo step, centred on the
//! given page.

use std::collections::BTreeMap;
use std::path::Path;

use arrangia_core::Project;
use arrangia_core::catalog;
use arrangia_core::edit::unique_id;
use arrangia_core::model::{Element, ElementKind, Lang, StringKey, Target};
use arrangia_core::ops::Op;
use serde::Deserialize;
use serde_json::{Value, json};

use super::AppState;
use super::assets::{first_image_asset, image_dims};
use super::edit::{EditResult, Plan, ViewContext, double_option, edit, with_editor};
use crate::error::{CmdResult, CommandError};

/// What to add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AddKind {
    Device,
    Text,
    Image,
    Shape,
    Background,
}

view_args! {
    pub struct ElementAddArgs {
        pub kind: AddKind,
        /// Home page (default 0); the element is centred on it.
        #[serde(default)]
        pub page: u32,
        /// Device: catalog model (default: the target preset's device).
        #[serde(default)]
        pub model: Option<String>,
        /// Image: project-relative asset path (default: the first image
        /// in `assets/`).
        #[serde(default)]
        pub asset: Option<String>,
        /// Text: placeholder text (default `New text`).
        #[serde(default)]
        pub text: Option<String>,
    }
}

/// Placeholder text of a new text element.
const PLACEHOLDER: &str = "New text";
/// Largest share of the page a new device or image may cover.
const MAX_SHARE: f64 = 0.6;

/// Catalog device for `target`: its preset's device (preset = target id,
/// else a preset with the same screenshot folder), else the first device
/// of the strip, else the first phone of the catalog.
fn default_model(project: &Project, target: Option<&Target>, strip: &str) -> String {
    let preset = target.and_then(|t| {
        catalog::target_preset(t.id.as_str())
            .filter(|p| p.device.is_some())
            .or_else(|| {
                let folder = t.screenshots.as_deref()?;
                catalog::target_presets()
                    .iter()
                    .find(|p| p.device.is_some() && p.screenshots.as_deref() == Some(folder))
            })
    });
    if let Some(d) = preset.and_then(|p| p.device.clone()) {
        return d;
    }
    let mut existing = None;
    if let Some(s) = project.strip(strip) {
        s.walk_elements(&mut |e| {
            if existing.is_none()
                && let ElementKind::Device(d) = &e.kind
            {
                existing = Some(d.model.clone());
            }
        });
    }
    existing
        .or_else(|| {
            catalog::devices()
                .iter()
                .find(|d| d.class == catalog::DeviceClass::Phone)
                .map(|d| d.id.clone())
        })
        .unwrap_or_else(|| "iphone-island".to_owned())
}

/// Width (lu) so a box of aspect `w:h` fits into `MAX_SHARE` of the page.
fn fitting_width(w: f64, h: f64, page_h: f64) -> f64 {
    let max_w = 1000.0 * MAX_SHARE;
    let max_h = page_h * MAX_SHARE;
    if w <= 0.0 || h <= 0.0 {
        return max_w;
    }
    (max_w.min(max_h * w / h)).round()
}

/// A translation key based on `base` that no language uses yet.
fn unique_key(project: &Project, base: &str) -> String {
    let used = |k: &str| {
        project
            .strings
            .values()
            .any(|m| m.keys().any(|x| x.as_str() == k))
    };
    if !used(base) {
        return base.to_owned();
    }
    let mut n = 2u32;
    loop {
        let k = format!("{base}-{n}");
        if !used(&k) {
            return k;
        }
        n += 1;
    }
}

/// Where the new element goes and what the view knows.
struct Placement<'a> {
    project: &'a Project,
    dir: &'a Path,
    target: Option<&'a Target>,
    strip: &'a str,
    id: &'a str,
    page: u32,
    page_h: f64,
}

/// The new element as YAML-shaped JSON plus extra ops (a new string) and
/// its list index (`None` = top).
fn build(
    kind: AddKind,
    args: &ElementAddArgs,
    image_default: Option<&str>,
    at: &Placement<'_>,
) -> CmdResult<(Value, Vec<Op>, Option<usize>)> {
    let p = at.project;
    let (id, page, page_h) = (at.id, at.page, at.page_h);
    let center = json!({ "x": 500, "y": (page_h / 2.0).round() });
    let mut ops = Vec::new();
    let mut index = None;
    let element = match kind {
        AddKind::Device => {
            let model = args
                .model
                .clone()
                .unwrap_or_else(|| default_model(p, at.target, at.strip));
            let dev = catalog::device(&model)
                .ok_or_else(|| CommandError::not_found(format!("unknown device `{model}`")))?;
            json!({
                "id": id, "kind": "device", "page": page,
                "anchor_y": "center", "at": center,
                "width": fitting_width(dev.body.w, dev.body.h, page_h),
                "model": model, "screenshot": id, "shadow": "soft",
            })
        }
        AddKind::Text => {
            let key = unique_key(p, "text");
            ops.push(Op::SetString {
                lang: Lang::parse(p.languages.default.as_str()).map_err(CommandError::invalid)?,
                key: StringKey::parse(&key).map_err(CommandError::invalid)?,
                value: Some(args.text.clone().unwrap_or_else(|| PLACEHOLDER.to_owned())),
            });
            let style = if p.styles.text.contains_key("headline") {
                Some("headline".to_owned())
            } else {
                p.styles.text.keys().next().cloned()
            };
            let mut v = json!({
                "id": id, "kind": "text", "page": page,
                "anchor_y": "center", "at": center, "width": 800,
                "text": key, "align": "center",
            });
            if let Some(style) = style {
                v["style"] = json!(style);
            }
            v
        }
        AddKind::Image => {
            let asset = args
                .asset
                .clone()
                .or_else(|| image_default.map(ToOwned::to_owned))
                .unwrap_or_default();
            let (w, h) = image_dims(&at.dir.join(&asset)).unwrap_or((1, 1));
            let width = fitting_width(f64::from(w), f64::from(h), page_h).min(400.0);
            json!({
                "id": id, "kind": "image", "page": page,
                "anchor_y": "center", "at": center, "width": width,
                "height": (width * f64::from(h) / f64::from(w.max(1))).round(),
                "asset": asset,
            })
        }
        AddKind::Shape => {
            let fill = if p.styles.colors.contains_key("accent") {
                "@accent"
            } else {
                "#FF7A45"
            };
            json!({
                "id": id, "kind": "shape", "page": page,
                "anchor_y": "center", "at": center, "width": 400, "height": 400,
                "shape": "rect", "radius": 40, "fill": fill,
            })
        }
        AddKind::Background => {
            index = Some(0);
            json!({
                "id": id, "kind": "background", "page": page,
                "fill": {
                    "kind": "linear-gradient", "angle": 160,
                    "stops": [["#F4F7F0", 0], ["#DCEBD5", 1]],
                },
            })
        }
    };
    Ok((element, ops, index))
}

pub fn element_add(app: &AppState, args: ElementAddArgs) -> CmdResult<EditResult> {
    let kind = args.kind;
    let label_kind = match kind {
        AddKind::Device => "device",
        AddKind::Text => "text",
        AddKind::Image => "image",
        AddKind::Shape => "shape",
        AddKind::Background => "background",
    };
    let image_default = if kind == AddKind::Image && args.asset.is_none() {
        Some(first_image_asset(app.session()?.dir()).ok_or_else(|| {
            CommandError::invalid("the project has no images in assets/ yet; import one first")
        })?)
    } else {
        None
    };
    edit(app, &args.view(), format!("Add {label_kind}"), |s, p, c| {
        let strip = with_editor(s, p, c, |ed| Ok(ed.strip().clone()))?;
        if args.page >= strip.page.count {
            return Err(CommandError::invalid(format!(
                "page {} does not exist (the strip has {})",
                args.page, strip.page.count
            )));
        }
        let id = unique_id(p, label_kind);
        let placement = Placement {
            project: p,
            dir: s.dir(),
            target: p.target(c.target.as_str()),
            strip: strip.id.as_str(),
            id: id.as_str(),
            page: args.page,
            page_h: strip.page.aspect.page_height(),
        };
        let (element, mut ops, index) = build(kind, &args, image_default.as_deref(), &placement)?;
        let element: Element =
            serde_json::from_value(element).map_err(|e| CommandError::internal(e.to_string()))?;
        ops.push(Op::AddElement {
            strip: strip.id.clone(),
            parent: None,
            index,
            element,
            overrides: BTreeMap::new(),
        });
        Ok(Plan {
            ops,
            created: vec![id.to_string()],
        })
    })
}
