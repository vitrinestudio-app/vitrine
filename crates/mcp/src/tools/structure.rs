//! Canvases, variants, targets and languages.

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::output::ToolOutput;
use crate::params::{edit_line, edit_summary};
use crate::server::{VitrineMcp, cmd};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanvasAddParams {
    /// New canvas id, e.g. `tablet` (default: `canvas`, made unique).
    #[serde(default)]
    pub id: Option<String>,
    /// Page aspect `w:h`, usually the export size of its main target, e.g.
    /// `1320:2868` (iPhone 6.9" portrait) or `2064:2752` (iPad 13").
    /// Optional with `target_preset` (then the preset's size).
    #[serde(default)]
    pub aspect: Option<String>,
    /// Number of pages (default 3 for portrait pages, else 1).
    #[serde(default)]
    pub count: Option<u32>,
    /// Page ids (optional, e.g. `["intro", "plan"]`; default `page-01` …).
    #[serde(default)]
    pub ids: Vec<String>,
    /// Canvas backgrounds bottom to top (Fill objects: `"#0B3324"`, or
    /// `{"kind": "linear-gradient", "angle": 115, "stops": [["#0B3324", 0],
    /// ["#1D5C68", 1]]}`). Default: one light grey.
    #[serde(default)]
    pub backgrounds: Vec<Value>,
    /// `empty` (default) or `starter` (a headline + device on every page,
    /// texts with placeholder strings in the default language).
    #[serde(default)]
    pub start: Option<String>,
    /// Also add a target from this catalog preset showing the new canvas
    /// (same undo step), e.g. `ios-ipad-13`.
    #[serde(default)]
    pub target_preset: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanvasDuplicateParams {
    /// Canvas to copy.
    pub canvas: String,
    /// Id of the copy (default: the original's id made unique, `phone-2`).
    #[serde(default)]
    pub id: Option<String>,
    /// Give the copied texts their own translation keys (`<id>.<key>`,
    /// translations copied) instead of sharing them with the original.
    #[serde(default)]
    pub copy_strings: bool,
    /// Also copy the targets showing the original (ids `<target>-<id>`).
    #[serde(default)]
    pub copy_targets: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanvasRenameParams {
    pub canvas: String,
    /// New id.
    pub to: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanvasIdParams {
    pub canvas: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanvasUpdateParams {
    /// Canvas id.
    pub canvas: String,
    /// New page aspect `w:h`.
    #[serde(default)]
    pub aspect: Option<String>,
    /// New page count (refused while a removed page is still an element's
    /// home page).
    #[serde(default)]
    pub count: Option<u32>,
    /// Page ids (`[]` = generated ids).
    #[serde(default)]
    pub ids: Option<Vec<String>>,
    /// Canvas backgrounds, bottom to top (replaces the list; see `canvas_add`).
    #[serde(default)]
    pub backgrounds: Option<Vec<Value>>,
}

/// What to do with a variant.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VariantAction {
    /// Add a variant: empty (optionally with `content_scale`) or a copy of
    /// `from`; `targets` show it from then on.
    Add,
    /// Remove it; targets showing it fall back to the base layout.
    Remove,
    /// Rename it to `to`; targets follow.
    Rename,
    /// Set `content_scale` (missing/null = remove).
    Scale,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CanvasVariantParams {
    pub canvas: String,
    pub action: VariantAction,
    /// Variant id, e.g. `tablet-landscape`.
    pub id: String,
    /// New id (`rename`).
    #[serde(default)]
    pub to: Option<String>,
    /// Factor for devices/images in this variant, e.g. 0.75 (`add`,
    /// `scale`).
    #[serde(default)]
    pub content_scale: Option<f64>,
    /// `add`: copy this variant's overrides and content scale.
    #[serde(default)]
    pub from: Option<String>,
    /// `add`: targets of the canvas that show the new variant.
    #[serde(default)]
    pub targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetAddParams {
    /// Preset id from `catalog_targets`, e.g. `ios-iphone-69`, `ios-ipad-13`,
    /// `play-phone`.
    pub preset: String,
    /// Canvas the target shows.
    pub canvas: String,
    /// Target id (default: the preset id, made unique).
    #[serde(default)]
    pub id: Option<String>,
    /// Export size `WxH` in px, e.g. `1290x2796` (default: the preset's
    /// default size).
    #[serde(default)]
    pub size: Option<String>,
    /// `portrait` or `landscape` (picks the preset's first size in that
    /// orientation).
    #[serde(default)]
    pub orientation: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetUpdateParams {
    /// Target id.
    pub id: String,
    /// Fields to change, YAML names (`null` resets an optional field), e.g.
    /// `{"size": [2064, 2752], "group": 2, "variant": "tablet-landscape",
    /// "langs": ["en", "de"], "screenshots": "ipad-13"}`.
    #[serde(default)]
    pub patch: Map<String, Value>,
    /// New id (renames the target; references follow).
    #[serde(default)]
    pub rename: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdParams {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LanguageParams {
    /// BCP 47 language tag, e.g. `fr`, `pt-BR`.
    pub lang: String,
}

/// `"1290x2796"` → `[1290, 2796]`.
pub fn parse_size(s: &str) -> Result<Value, vitrine_app::CommandError> {
    let bad = || vitrine_app::CommandError::invalid(format!("size `{s}` is not `WxH`"));
    let (w, h) = s.split_once(['x', 'X', '×']).ok_or_else(bad)?;
    let w: u32 = w.trim().parse().map_err(|_| bad())?;
    let h: u32 = h.trim().parse().map_err(|_| bad())?;
    Ok(json!([w, h]))
}

fn edited(r: &Value) -> ToolOutput {
    let s = edit_summary(r);
    ToolOutput::new(edit_line(&s), s)
}

#[tool_router(router = structure_tools, vis = "pub(crate)")]
impl VitrineMcp {
    #[tool(
        description = "Add a canvas with N pages of one aspect, empty or with a starter layout (`start: \"starter\"`: headline + device per page), optionally with a first target from a catalog preset (one undo step). Example: {\"id\": \"tablet\", \"target_preset\": \"ios-ipad-13\", \"start\": \"starter\"} or {\"id\": \"tablet\", \"aspect\": \"2064:2752\", \"count\": 3, \"backgrounds\": [\"#0B3324\"]}. Without `target_preset` add a target showing it (target_add) before adding elements."
    )]
    async fn canvas_add(
        &self,
        Parameters(p): Parameters<CanvasAddParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({ "ids": p.ids });
            for (k, v) in [
                ("id", p.id.map(Value::from)),
                ("aspect", p.aspect.map(Value::from)),
                ("count", p.count.map(Value::from)),
                ("start", p.start.map(Value::from)),
                ("target_preset", p.target_preset.map(Value::from)),
            ] {
                if let Some(v) = v {
                    args[k] = v;
                }
            }
            if !p.backgrounds.is_empty() {
                args["backgrounds"] = json!(p.backgrounds);
            }
            Ok(edited(&cmd(app, "canvas_add", args)?))
        })
        .await
    }

    #[tool(
        description = "Duplicate a canvas: pages, backgrounds, elements (new unique ids), variants with their overrides; optionally the targets showing it and own copies of the texts' translations (one undo step). Example: {\"canvas\": \"phone\", \"id\": \"tablet\", \"copy_targets\": true}."
    )]
    async fn canvas_duplicate(
        &self,
        Parameters(p): Parameters<CanvasDuplicateParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({
                "duplicate_of": p.canvas,
                "copy_strings": p.copy_strings,
                "copy_targets": p.copy_targets,
            });
            if let Some(id) = p.id {
                args["id"] = json!(id);
            }
            Ok(edited(&cmd(app, "canvas_add", args)?))
        })
        .await
    }

    #[tool(
        description = "Rename a canvas; targets showing it follow. Example: {\"canvas\": \"main\", \"to\": \"phone\"}."
    )]
    async fn canvas_rename(
        &self,
        Parameters(p): Parameters<CanvasRenameParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            Ok(edited(&cmd(
                app,
                "canvas_rename",
                json!({ "canvas": p.canvas, "to": p.to }),
            )?))
        })
        .await
    }

    #[tool(
        description = "Remove a canvas with its elements, variants and the targets showing it (one undo step, refused for the last canvas). Tell the user which targets go with it. Example: {\"canvas\": \"tablet\"}."
    )]
    async fn canvas_remove(
        &self,
        Parameters(p): Parameters<CanvasIdParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            Ok(edited(&cmd(
                app,
                "canvas_remove",
                json!({ "canvas": p.canvas }),
            )?))
        })
        .await
    }

    #[tool(
        description = "Change a canvas: page aspect, page count, page ids, canvas backgrounds (one undo step). Example: {\"canvas\": \"phone\", \"count\": 5, \"backgrounds\": [{\"kind\": \"linear-gradient\", \"angle\": 160, \"stops\": [[\"#F4F7F0\", 0], [\"#DCEBD5\", 1]]}]}."
    )]
    async fn canvas_update(
        &self,
        Parameters(p): Parameters<CanvasUpdateParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({ "canvas": p.canvas });
            if let Some(a) = p.aspect {
                args["aspect"] = json!(a);
            }
            if let Some(c) = p.count {
                args["count"] = json!(c);
            }
            if let Some(i) = p.ids {
                args["ids"] = json!(i);
            }
            if let Some(b) = p.backgrounds {
                args["backgrounds"] = json!(b);
            }
            Ok(edited(&cmd(app, "canvas_patch", args)?))
        })
        .await
    }

    #[tool(
        description = "Add (empty, or a copy `from` another variant), remove, rename or scale a variant of a canvas. A variant holds per-format overrides (e.g. `tablet-landscape` for iPad targets with group 2); `targets` on add (or target_update {patch: {variant: …}}) make targets show it; edit it with scope `variant`. Removing detaches its targets (they show the base layout again). Example: {\"canvas\": \"phone\", \"action\": \"add\", \"id\": \"tablet-landscape\", \"content_scale\": 0.75, \"targets\": [\"ios-ipad-13\"]}."
    )]
    async fn canvas_variant(
        &self,
        Parameters(p): Parameters<CanvasVariantParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let change = match p.action {
                VariantAction::Add => json!({
                    "action": "add", "id": p.id, "content_scale": p.content_scale,
                    "from": p.from, "targets": p.targets,
                }),
                VariantAction::Remove => json!({ "action": "remove", "id": p.id }),
                VariantAction::Rename => json!({
                    "action": "rename", "id": p.id,
                    "to": p.to.ok_or_else(|| vitrine_app::CommandError::invalid("`rename` needs `to`"))?,
                }),
                VariantAction::Scale => {
                    json!({ "action": "scale", "id": p.id, "content_scale": p.content_scale })
                }
            };
            Ok(edited(&cmd(
                app,
                "canvas_variant",
                json!({ "canvas": p.canvas, "change": change }),
            )?))
        })
        .await
    }

    #[tool(
        description = "Add an export target from a catalog preset (store size, slot, gallery gaps, device). Example: {\"preset\": \"ios-ipad-13\", \"canvas\": \"phone\"}. For iPad landscape from a phone canvas set group 2 + a variant afterwards with target_update."
    )]
    async fn target_add(
        &self,
        Parameters(p): Parameters<TargetAddParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({ "preset": p.preset, "canvas": p.canvas });
            if let Some(id) = p.id {
                args["id"] = json!(id);
            }
            if let Some(s) = p.size {
                args["size"] = parse_size(&s)?;
            }
            if let Some(o) = p.orientation {
                args["orientation"] = json!(o);
            }
            Ok(edited(&cmd(app, "target_add_from_preset", args)?))
        })
        .await
    }

    #[tool(
        description = "Change a target's fields (size, group, variant, langs, screenshots, gallery, device_map …) and/or rename it. Example: {\"id\": \"ios-ipad-13\", \"patch\": {\"group\": 2, \"variant\": \"tablet-landscape\"}}."
    )]
    async fn target_update(
        &self,
        Parameters(p): Parameters<TargetUpdateParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({ "id": p.id, "patch": p.patch });
            if let Some(r) = p.rename {
                args["rename"] = json!(r);
            }
            Ok(edited(&cmd(app, "target_patch", args)?))
        })
        .await
    }

    #[tool(
        description = "Remove a target (its exports are not deleted). Example: {\"id\": \"blog-header\"}."
    )]
    async fn target_remove(
        &self,
        Parameters(p): Parameters<IdParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            Ok(edited(&cmd(app, "target_remove", json!({ "id": p.id }))?))
        })
        .await
    }

    #[tool(
        description = "Add a language to the project (translations start empty and fall back to the default language). Example: {\"lang\": \"fr\"}; then strings_missing {lang: \"fr\"} and strings_set."
    )]
    async fn language_add(
        &self,
        Parameters(p): Parameters<LanguageParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            Ok(edited(&cmd(
                app,
                "language_add",
                json!({ "lang": p.lang }),
            )?))
        })
        .await
    }

    #[tool(
        description = "Remove a language with its translations and language overrides (refused for the default language). Undoable."
    )]
    async fn language_remove(
        &self,
        Parameters(p): Parameters<LanguageParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            Ok(edited(&cmd(
                app,
                "language_remove",
                json!({ "lang": p.lang }),
            )?))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_parse() {
        assert_eq!(parse_size("1290x2796").unwrap(), json!([1290, 2796]));
        assert_eq!(parse_size("10 × 20").unwrap(), json!([10, 20]));
        assert!(parse_size("1290").is_err());
        assert!(parse_size("ax2").is_err());
    }
}
