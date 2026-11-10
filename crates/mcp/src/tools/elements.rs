//! Element tools: list, get, add, update (patch), remove, move, resize,
//! rotate, reorder, group, rename and raw ops.
//!
//! Gestures go through the app's `edit_*` commands (core edit intents,
//! ADR 0007): coordinates given relative to a page are converted to canvas
//! coordinates with the overlay data of the target.

use std::collections::BTreeMap;

use arrangia_app::CommandError;
use arrangia_app::commands::AppState;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::project::compact_summary;
use crate::output::ToolOutput;
use crate::params::{EditCtx, edit_line, edit_params, edit_summary, merged};
use crate::server::{ArrangiaMcp, cmd};

/// Patchable element fields (YAML names), documented for the model (a
/// macro so `concat!` can put it into tool descriptions).
macro_rules! patch_fields_doc {
    () => {
        " Fields (YAML names; `null` resets to the default). \
All kinds: page (home page, 0-based), lock_page, at {x, y} (centre, lu relative to the home page), \
width, height, rotation (degrees, clockwise), pivot {x, y}, anchor_y (top|bottom|center|stretch), \
opacity (0..1), z (layer), clip (none|page), visible, visible_in {targets, langs}. \
device: model, orientation (portrait|landscape), frame (stylized|detailed|imported:apple), bezel, \
color, fold (open|folded, foldables), chrome (none|browser), url, appearance (light|dark), screenshot (key), fit (cover|contain|stretch), shadow. \
text: text (translation key), style, box {top, left, right, bottom} (margins to the page), align \
(left|center|right), valign (top|center|bottom), fit {shrink_to, max_lines}, color, font_size, \
stroke {color, width}, glow {color, radius}, shadow, hyphenate. \
image: asset, fit, shadow. shape: shape (rect|ellipse|line), radius, fill, stroke, shadow. \
background: fill (\"#RRGGBB\" or {kind: linear-gradient|radial-gradient|image, …})."
    };
}

/// [`patch_fields_doc`] as a constant (tests).
pub const PATCH_FIELDS_DOC: &str = patch_fields_doc!();

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementListParams {
    /// Only this strip (default: all).
    #[serde(default)]
    pub strip: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementGetParams {
    /// Element id.
    pub id: String,
    /// Target of the view (default: the first target).
    #[serde(default)]
    pub target: Option<String>,
    /// Language for texts and language overrides (default: the default
    /// language).
    #[serde(default)]
    pub lang: Option<String>,
    /// Show this variant's override (default: the target's own variant).
    #[serde(default)]
    pub variant: Option<String>,
}

/// Kind of a new element.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AddKind {
    Device,
    Text,
    Image,
    Shape,
    Background,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ElementAddParams {
    /// What to add.
    pub kind: AddKind,
    /// Home page, 0-based (default 0). The element is centred on it.
    #[serde(default)]
    pub page: u32,
    /// Strip to add to (default: the strip of `target`, else the first
    /// strip with a target).
    #[serde(default)]
    pub strip: Option<String>,
    /// Target used for defaults (device model, page height). Default: the
    /// first target showing `strip`.
    #[serde(default)]
    pub target: Option<String>,
    /// Element id (default: `device`, `text-2` …).
    #[serde(default)]
    pub id: Option<String>,
    /// device: catalog model (default: the target preset's device).
    #[serde(default)]
    pub model: Option<String>,
    /// image: project-relative asset path, e.g. `assets/badge.png`
    /// (default: the first image in assets/).
    #[serde(default)]
    pub asset: Option<String>,
    /// text: the text in the default language (default `New text`; markup
    /// allowed). Ignored when `key` already has a translation.
    #[serde(default)]
    pub text: Option<String>,
    /// text: translation key, e.g. `hero.title` (default `text`, `text-2` …).
    #[serde(default)]
    pub key: Option<String>,
    /// Extra YAML fields over the defaults, e.g. `{"at": {"x": 500, "y":
    /// 400}, "width": 600, "style": "sub", "box": {"top": 160, "left": 80,
    /// "right": 80}, "anchor_y": "top"}` (see `element_update` for fields).
    #[serde(default)]
    pub fields: Map<String, Value>,
}

/// A translation set with an element patch.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StringValue {
    pub lang: String,
    pub key: String,
    /// The text (markup allowed); null removes it.
    #[serde(default)]
    pub value: Option<String>,
}

edit_params! {
    pub struct ElementUpdateParams {
        /// The element (or `ids` to patch several the same way; ids of
        /// several strips are fine).
        #[serde(default)]
        pub id: Option<String>,
        #[serde(default)]
        pub ids: Vec<String>,
        /// Fields to set (YAML names). With scope `variant` or `lang` they
        /// go into that override. `null`: in scope `base` resets the field
        /// to its default; in scope `variant`/`lang` removes the field from
        /// the override (back to the base value).
        #[serde(default)]
        pub patch: Map<String, Value>,
        /// Translations to set in the same undo step.
        #[serde(default)]
        pub strings: Vec<StringValue>,
        /// More patches in the SAME undo step, each with its own ids,
        /// scope, lang, variant (missing: those of this call), e.g. several
        /// language overrides at once.
        #[serde(default)]
        pub updates: Vec<UpdateEntry>,
    }
}

/// One entry of `element_update.updates`.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateEntry {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub ids: Vec<String>,
    /// Fields to set (`null` as in the call's `patch`).
    #[serde(default)]
    pub patch: Map<String, Value>,
    #[serde(default)]
    pub strings: Vec<StringValue>,
    /// `base` | `variant` | `lang` (default: the call's scope).
    #[serde(default)]
    pub scope: Option<crate::params::Scope>,
    /// Default: the call's variant.
    #[serde(default)]
    pub variant: Option<String>,
    /// Default: the call's lang (required for scope `lang`).
    #[serde(default)]
    pub lang: Option<String>,
    /// Default: the call's target.
    #[serde(default)]
    pub target: Option<String>,
}

/// Which override `element_reset_override` resets.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OverrideScope {
    /// A variant override (`variants.<variant>.overrides.<id>`).
    Variant,
    /// A language override of a text element (`langs.<lang>`).
    Lang,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ResetOverrideParams {
    /// The element (or `ids`; several strips are fine).
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub ids: Vec<String>,
    /// `variant` or `lang`.
    pub scope: OverrideScope,
    /// scope `variant`: the variant (default: the variant of `target`, or
    /// of the first target showing the element's strip).
    #[serde(default)]
    pub variant: Option<String>,
    /// scope `lang`: the language whose override is reset (required).
    #[serde(default)]
    pub lang: Option<String>,
    /// Fields (YAML names) to reset, e.g. `["at", "font_size"]`; omit to
    /// remove the whole override.
    #[serde(default)]
    pub fields: Option<Vec<String>>,
    /// Target whose variant is meant (scope `variant` without `variant`).
    #[serde(default)]
    pub target: Option<String>,
}

edit_params! {
    pub struct ElementIdsParams {
        /// Element ids (groups move/delete with their children).
        pub ids: Vec<String>,
    }
}

edit_params! {
    pub struct ElementMoveParams {
        /// The element (or `ids` with dx/dy).
        #[serde(default)]
        pub id: Option<String>,
        #[serde(default)]
        pub ids: Vec<String>,
        /// New centre x in lu, relative to the left edge of page `page`.
        #[serde(default)]
        pub x: Option<f64>,
        /// New centre y in lu, relative to the top edge of page `page`.
        #[serde(default)]
        pub y: Option<f64>,
        /// Page that x/y refer to (default: the element's home page). The
        /// home page follows the centre unless `keep_page`.
        #[serde(default)]
        pub page: Option<u32>,
        /// Relative move in lu instead of x/y (right/down positive).
        #[serde(default)]
        pub dx: Option<f64>,
        #[serde(default)]
        pub dy: Option<f64>,
        /// Keep the home page even when the centre lands on another page.
        #[serde(default)]
        pub keep_page: bool,
    }
}

/// The point of an element that stays in place when resizing.
#[derive(Debug, Clone, Copy, Default, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Fixed {
    #[default]
    Center,
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
}

edit_params! {
    pub struct ElementResizeParams {
        pub id: String,
        /// New width in lu (devices: width only, the height follows).
        #[serde(default)]
        pub width: Option<f64>,
        /// New height in lu.
        #[serde(default)]
        pub height: Option<f64>,
        /// Point that stays in place (default `center`; `n` keeps the top
        /// edge centre).
        #[serde(default)]
        pub fixed: Fixed,
    }
}

edit_params! {
    pub struct ElementRotateParams {
        /// The element (or `ids` to rotate several to the same angle;
        /// several strips are fine).
        #[serde(default)]
        pub id: Option<String>,
        #[serde(default)]
        pub ids: Vec<String>,
        /// Absolute rotation in degrees, positive = clockwise.
        pub degrees: f64,
    }
}

/// Z order change.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Order {
    Front,
    Back,
    Forward,
    Backward,
}

edit_params! {
    pub struct ElementReorderParams {
        pub ids: Vec<String>,
        /// `front`, `back`, `forward` (one step up), `backward`.
        pub order: Order,
    }
}

edit_params! {
    pub struct GroupParams {
        /// Siblings to group.
        pub ids: Vec<String>,
        /// Group id (default `group`, `group-2` …).
        #[serde(default)]
        pub group_id: Option<String>,
    }
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenameParams {
    pub id: String,
    /// New id (letters, digits, `-`, `_`, `.`).
    pub to: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ApplyOpsParams {
    /// Ops applied atomically as one undo step, e.g. `[{"op":
    /// "update_element", "id": "title", "patch": {"font_size": 80}},
    /// {"op": "set_lang_override", "element": "title", "lang": "de",
    /// "patch": {"font_size": 64}}, {"op": "set_string", "lang": "de",
    /// "key": "hero.title", "value": "Hallo"}]`. Every op and field:
    /// `docs_get {topic: "ops"}`.
    pub ops: Vec<Value>,
    /// Undo label (default: derived from the ops).
    #[serde(default)]
    pub label: Option<String>,
}

/// Compact inspector data of an element (`element_get`).
pub fn compact_info(info: &Value) -> Value {
    let mut out = json!({
        "id": info["id"], "kind": info["kind"], "strip": info["strip"],
        "effective": info["effective"],
    });
    for k in [
        "parent",
        "variant",
        "override",
        "lang_override",
        "lang_override_langs",
        "text",
    ] {
        if let Some(v) = info
            .get(k)
            .filter(|v| !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty()))
        {
            out[k] = v.clone();
        }
    }
    if info["children"].as_array().is_some_and(|c| !c.is_empty()) {
        out["children"] = info["children"].clone();
    }
    if out.get("override").is_some() || out.get("lang_override").is_some() {
        out["base"] = info["base"].clone();
    }
    out
}

/// Translations as the app's `strings` entries.
fn string_values(strings: &[StringValue]) -> Vec<Value> {
    strings
        .iter()
        .map(|s| json!({ "lang": s.lang, "key": s.key, "value": s.value }))
        .collect()
}

fn edited(r: &Value) -> ToolOutput {
    let s = edit_summary(r);
    ToolOutput::new(edit_line(&s), s)
}

/// `id` and `ids` together, without duplicates.
fn all_ids(id: Option<String>, ids: Vec<String>) -> Result<Vec<String>, CommandError> {
    let mut out: Vec<String> = id.into_iter().chain(ids).collect();
    let mut seen = std::collections::BTreeSet::new();
    out.retain(|i| seen.insert(i.clone()));
    if out.is_empty() {
        return Err(CommandError::invalid("give `id` or `ids`"));
    }
    Ok(out)
}

/// Canvas offset (top-left, lu) of page `page` and the home page of
/// element `id` in the view's target.
fn page_origin(
    app: &AppState,
    view: &Map<String, Value>,
    id: &str,
    page: Option<u32>,
) -> Result<(f64, f64), CommandError> {
    let overlay = cmd(app, "overlay", Value::Object(view.clone()))?;
    let home = overlay["elements"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|e| e["id"] == id)
        .and_then(|e| e["page"].as_u64())
        .or_else(|| {
            // Not drawn in this view (hidden): fall back to the model.
            app.session()
                .ok()
                .and_then(|s| s.with_project(|p| p.element(id).map(|(_, e)| u64::from(e.page))))
        })
        .ok_or_else(|| CommandError::not_found(format!("unknown element `{id}`")))?;
    let index = page.map_or(home, u64::from);
    let pages = overlay["pages"].as_array().cloned().unwrap_or_default();
    let p = pages
        .iter()
        .find(|p| p["index"].as_u64() == Some(index))
        .ok_or_else(|| {
            CommandError::invalid(format!(
                "page {index} does not exist (the strip has {} pages, 0-based)",
                pages.len()
            ))
        })?;
    Ok((
        p["rect"]["x"].as_f64().unwrap_or(0.0),
        p["rect"]["y"].as_f64().unwrap_or(0.0),
    ))
}

impl EditCtx {
    /// View JSON for an edit of the first of `ids`.
    fn view_for(&self, app: &AppState, ids: &[String]) -> Result<Map<String, Value>, CommandError> {
        self.view(app, ids.first().map(String::as_str), None)
    }
}

#[tool_router(router = element_tools, vis = "pub(crate)")]
impl ArrangiaMcp {
    #[tool(
        description = "List elements in painting order (bottom first): id, kind, home page, label (text in the default language, device model …), translation key, parent group.",
        annotations(read_only_hint = true)
    )]
    async fn element_list(
        &self,
        Parameters(p): Parameters<ElementListParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let s = compact_summary(app)?;
            let mut strips: BTreeMap<String, Value> = BTreeMap::new();
            for st in s["strips"].as_array().into_iter().flatten() {
                let id = st["id"].as_str().unwrap_or_default().to_owned();
                if p.strip.as_ref().is_none_or(|w| *w == id) {
                    strips.insert(id, st["elements"].clone());
                }
            }
            if let Some(w) = &p.strip
                && strips.is_empty()
            {
                return Err(CommandError::not_found(format!("unknown strip `{w}`")));
            }
            let n: usize = strips
                .values()
                .map(|v| v.as_array().map_or(0, Vec::len))
                .sum();
            Ok(ToolOutput::new(
                format!("{n} element(s)"),
                json!({ "strips": strips }),
            ))
        })
        .await
    }

    #[tool(
        description = "One element with all its fields: `effective` (what the view shows), plus base, the variant `override` and the `lang_override` when present, and for texts the translation in `lang` with its fallback.",
        annotations(read_only_hint = true)
    )]
    async fn element_get(
        &self,
        Parameters(p): Parameters<ElementGetParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({ "id": p.id });
            if let Some(t) = p.target {
                args["target"] = json!(t);
            }
            if let Some(l) = p.lang {
                args["lang"] = json!(l);
            }
            if let Some(v) = p.variant {
                args["variant"] = json!(v);
            }
            let info = cmd(app, "element_get", args)?;
            let c = compact_info(&info);
            Ok(ToolOutput::new(
                format!(
                    "{} `{}`",
                    c["kind"].as_str().unwrap_or("element"),
                    c["id"].as_str().unwrap_or_default()
                ),
                c,
            ))
        })
        .await
    }

    #[tool(
        description = "Add an element (one undo step) with sensible defaults, centred on page `page`: device (model from the target preset, screenshot key = id), text (new translation key, `headline` style), image, shape, background. Override defaults with `fields`. Examples: {\"kind\": \"text\", \"page\": 0, \"id\": \"title\", \"key\": \"hero.title\", \"text\": \"Plan your garden **in minutes**\", \"fields\": {\"box\": {\"top\": 160, \"left\": 80, \"right\": 80}, \"anchor_y\": \"top\", \"at\": null, \"width\": null}}; {\"kind\": \"device\", \"page\": 1, \"fields\": {\"anchor_y\": \"bottom\", \"at\": {\"x\": 500, \"y\": 1400}, \"width\": 720}}."
    )]
    async fn element_add(
        &self,
        Parameters(p): Parameters<ElementAddParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let project = app.session()?.snapshot();
            let target =
                crate::params::target_for(&project, p.target.as_deref(), p.strip.as_deref())?;
            if let Some(s) = &p.strip
                && project.strip(s).is_none()
            {
                return Err(CommandError::not_found(format!("unknown strip `{s}`")));
            }
            let kind = serde_json::to_value(p.kind).unwrap_or_default();
            let mut args = json!({
                "target": target, "variant": null, "kind": kind, "page": p.page,
            });
            for (k, v) in [
                ("id", p.id),
                ("model", p.model),
                ("asset", p.asset),
                ("text", p.text),
                ("key", p.key),
            ] {
                if let Some(v) = v {
                    args[k] = json!(v);
                }
            }
            if !p.fields.is_empty() {
                args["fields"] = Value::Object(p.fields);
            }
            Ok(edited(&cmd(app, "element_add", args)?))
        })
        .await
    }

    #[tool(
        description = concat!(
            "Change element fields (one undo step), optionally with translations. Scope: base (default, everywhere), variant (override for a format), lang (override for one language; text elements). `null` in scope variant/lang removes the field from the override (back to the base value); element_reset_override removes whole overrides. `ids` may span several strips; `updates` puts several differently scoped patches into one undo step, e.g. {\"updates\": [{\"id\": \"title\", \"scope\": \"lang\", \"lang\": \"de\", \"patch\": {\"font_size\": 64}}, {\"id\": \"title\", \"scope\": \"lang\", \"lang\": \"fr\", \"patch\": {\"font_size\": 66}}]}. All fields with types and defaults: docs_get {topic: \"elements\"}. Examples: {\"id\": \"title\", \"patch\": {\"style\": \"sub\", \"fit\": {\"shrink_to\": 60, \"max_lines\": 2}}}; {\"id\": \"title\", \"scope\": \"lang\", \"lang\": \"de\", \"patch\": {\"box\": {\"top\": 160, \"left\": 40, \"right\": 40}}}; {\"id\": \"phone-1\", \"scope\": \"variant\", \"target\": \"ios-ipad-13\", \"patch\": {\"width\": 560}}.",
            patch_fields_doc!()
        )
    )]
    async fn element_update(
        &self,
        Parameters(p): Parameters<ElementUpdateParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let call = p.edit();
            let ids = all_ids(p.id.clone(), p.ids.clone()).unwrap_or_default();
            if ids.is_empty() && p.updates.is_empty() {
                return Err(CommandError::invalid(
                    "give `id`, `ids` or `updates` [{id, patch, scope?, lang?, variant?}]",
                ));
            }
            let mut updates = Vec::new();
            let mut first = ids.clone();
            for u in &p.updates {
                let u_ids = all_ids(u.id.clone(), u.ids.clone())?;
                let ctx = EditCtx {
                    target: u.target.clone().or_else(|| call.target.clone()),
                    scope: u.scope.unwrap_or(call.scope),
                    variant: u.variant.clone().or_else(|| call.variant.clone()),
                    lang: u.lang.clone().or_else(|| call.lang.clone()),
                };
                let view = ctx.view_for(app, &u_ids)?;
                if first.is_empty() {
                    first.clone_from(&u_ids);
                }
                updates.push(merged(
                    view,
                    json!({ "ids": u_ids, "patch": u.patch, "strings": string_values(&u.strings) }),
                ));
            }
            // The call's own view (the first update's target when it has
            // no ids of its own; `lang` scope needs no `lang` then).
            let mut own = call.clone();
            if ids.is_empty() {
                own.scope = crate::params::Scope::Base;
            }
            let view = own.view_for(app, &first)?;
            let args = merged(
                view,
                json!({
                    "ids": ids, "patch": p.patch, "strings": string_values(&p.strings),
                    "updates": updates,
                }),
            );
            Ok(edited(&cmd(app, "element_patch", args)?))
        })
        .await
    }

    #[tool(
        description = "Delete elements (groups with their children) in one undo step; ids may span several strips. Translations stay. Example: {\"ids\": [\"badge\", \"arrow\"]}."
    )]
    async fn element_remove(
        &self,
        Parameters(p): Parameters<ElementIdsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let view = p.edit().view_for(app, &p.ids)?;
            Ok(edited(&cmd(
                app,
                "edit_remove",
                merged(view, json!({ "ids": p.ids })),
            )?))
        })
        .await
    }

    #[tool(
        description = "Move an element: x/y = new centre in lu relative to the top-left corner of page `page` (default: its home page; a page is 1000 lu wide), or dx/dy relative (several `ids`). The home page follows the centre. Converts to the element's own anchor/margins (core edit intent). Examples: {\"id\": \"phone-plan\", \"x\": 500, \"y\": 1500}; {\"ids\": [\"a\", \"b\"], \"dy\": -40}."
    )]
    async fn element_move(
        &self,
        Parameters(p): Parameters<ElementMoveParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let ids = all_ids(p.id.clone(), p.ids.clone())?;
            let view = p.edit().view_for(app, &ids)?;
            if p.dx.is_some() || p.dy.is_some() {
                if p.x.is_some() || p.y.is_some() {
                    return Err(CommandError::invalid("give either x/y or dx/dy"));
                }
                let args = merged(
                    view,
                    json!({
                        "ids": ids, "dx": p.dx.unwrap_or(0.0), "dy": p.dy.unwrap_or(0.0),
                        "keep_page": p.keep_page,
                    }),
                );
                return Ok(edited(&cmd(app, "edit_move_by", args)?));
            }
            let [id] = ids.as_slice() else {
                return Err(CommandError::invalid(
                    "x/y moves one element; use dx/dy for several",
                ));
            };
            let (Some(x), Some(y)) = (p.x, p.y) else {
                return Err(CommandError::invalid("give x and y (or dx/dy)"));
            };
            let (ox, oy) = page_origin(app, &view, id, p.page)?;
            let args = merged(
                view,
                json!({ "id": id, "x": ox + x, "y": oy + y, "keep_page": p.keep_page }),
            );
            Ok(edited(&cmd(app, "edit_move", args)?))
        })
        .await
    }

    #[tool(
        description = "Resize an element to width/height in lu (devices: width only; text boxes with margins change their margins). `fixed` is the point that stays in place (default center). Example: {\"id\": \"phone-plan\", \"width\": 800, \"fixed\": \"s\"}."
    )]
    async fn element_resize(
        &self,
        Parameters(p): Parameters<ElementResizeParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            if p.width.is_none() && p.height.is_none() {
                return Err(CommandError::invalid("give width and/or height"));
            }
            let view = p.edit().view(app, Some(&p.id), None)?;
            let args = merged(
                view,
                json!({ "id": p.id, "width": p.width, "height": p.height, "fixed": p.fixed }),
            );
            Ok(edited(&cmd(app, "edit_resize", args)?))
        })
        .await
    }

    #[tool(
        description = "Rotate elements to an absolute angle in degrees (positive = clockwise) around their pivot (default: centre); several `ids` (also of several strips) in one undo step. Example: {\"id\": \"phone-beds\", \"degrees\": -8}."
    )]
    async fn element_rotate(
        &self,
        Parameters(p): Parameters<ElementRotateParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let ids = all_ids(p.id.clone(), p.ids.clone())?;
            let view = p.edit().view_for(app, &ids)?;
            let args = match ids.as_slice() {
                [id] => merged(view, json!({ "id": id, "degrees": p.degrees })),
                _ => merged(view, json!({ "ids": ids, "degrees": p.degrees })),
            };
            Ok(edited(&cmd(app, "edit_rotate", args)?))
        })
        .await
    }

    #[tool(
        description = "Change painting order within the element list: front, back, forward, backward (z stays). Ids may span several strips (one undo step). Example: {\"ids\": [\"title-plan\", \"ipad-title-plan\"], \"order\": \"front\"}."
    )]
    async fn element_reorder(
        &self,
        Parameters(p): Parameters<ElementReorderParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let view = p.edit().view_for(app, &p.ids)?;
            let args = merged(view, json!({ "ids": p.ids, "order": p.order }));
            Ok(edited(&cmd(app, "edit_reorder", args)?))
        })
        .await
    }

    #[tool(
        description = "Group sibling elements (they then move/rotate together); returns the group id in `created`. Example: {\"ids\": [\"badge\", \"badge-text\"], \"group_id\": \"badge-group\"}."
    )]
    async fn element_group(
        &self,
        Parameters(p): Parameters<GroupParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let view = p.edit().view_for(app, &p.ids)?;
            let mut extra = json!({ "ids": p.ids });
            if let Some(g) = p.group_id {
                extra["group_id"] = json!(g);
            }
            Ok(edited(&cmd(app, "edit_group", merged(view, extra))?))
        })
        .await
    }

    #[tool(
        description = "Dissolve groups; the children keep their place. Example: {\"ids\": [\"badge-group\"]}."
    )]
    async fn element_ungroup(
        &self,
        Parameters(p): Parameters<ElementIdsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let view = p.edit().view_for(app, &p.ids)?;
            Ok(edited(&cmd(
                app,
                "edit_ungroup",
                merged(view, json!({ "ids": p.ids })),
            )?))
        })
        .await
    }

    #[tool(
        description = "Rename an element id (variant overrides follow). Example: {\"id\": \"text-2\", \"to\": \"title-beds\"}."
    )]
    async fn element_rename(
        &self,
        Parameters(p): Parameters<RenameParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            Ok(edited(&cmd(
                app,
                "edit_rename",
                json!({ "id": p.id, "to": p.to }),
            )?))
        })
        .await
    }

    #[tool(
        description = "Remove variant or language overrides (back to the base value) in one undo step: whole overrides or only `fields`. Ids may span several strips. Examples: {\"id\": \"title\", \"scope\": \"lang\", \"lang\": \"en\"}; {\"ids\": [\"title\", \"sub\"], \"scope\": \"lang\", \"lang\": \"de\", \"fields\": [\"box\", \"font_size\"]}; {\"id\": \"phone-1\", \"scope\": \"variant\", \"variant\": \"tablet-landscape\", \"fields\": [\"at\"]}."
    )]
    async fn element_reset_override(
        &self,
        Parameters(p): Parameters<ResetOverrideParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let ids = all_ids(p.id.clone(), p.ids.clone())?;
            let target = if let Some(t) = p.target {
                Some(t)
            } else {
                let project = app.session()?.snapshot();
                let strip = crate::params::strip_of_element(&project, &ids[0]);
                crate::params::gui_target(&project, strip.as_deref())
            };
            let mut args = json!({ "ids": ids, "scope": p.scope });
            for (k, v) in [("variant", p.variant), ("lang", p.lang), ("target", target)] {
                if let Some(v) = v {
                    args[k] = json!(v);
                }
            }
            if let Some(f) = p.fields {
                args["fields"] = json!(f);
            }
            Ok(edited(&cmd(app, "element_reset_override", args)?))
        })
        .await
    }

    #[tool(
        description = "Power tool: apply raw model ops atomically as ONE undo step (all or nothing). JSON form {\"op\": \"<name>\", …}. update_element always writes the base element; overrides use set_lang_override / reset_lang_override_field / set_variant_override / reset_override_field. Every op and field: call docs_get {topic: \"ops\"} (also resource arrangia://docs/ops). Prefer the specific tools; use this for multi-part changes that must be one step."
    )]
    async fn apply_ops(
        &self,
        Parameters(p): Parameters<ApplyOpsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            // Store mapping (app ids, accounts) is read-only over MCP
            // (Phase 5.1): the user sets it in the GUI.
            if p.ops
                .iter()
                .any(|o| o.get("op").and_then(Value::as_str) == Some("set_store"))
            {
                return Err(CommandError::invalid(
                    "set_store is not available over MCP: the user edits the store mapping in \
                     the GUI (Project settings → Stores); read it with store_config_get",
                ));
            }
            let mut args = json!({ "ops": p.ops });
            if let Some(l) = p.label {
                args["label"] = json!(l);
            }
            Ok(edited(&cmd(app, "apply_ops", args)?))
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use arrangia_core::model::ElementPatch;

    use super::*;

    #[test]
    fn patch_doc_names_every_field() {
        for f in ElementPatch::field_names() {
            assert!(
                PATCH_FIELDS_DOC.contains(&format!("{f} "))
                    || PATCH_FIELDS_DOC.contains(&format!("{f},"))
                    || PATCH_FIELDS_DOC.contains(&format!("{f}."))
                    || PATCH_FIELDS_DOC.contains(&format!("{f} (")),
                "field `{f}` is not documented"
            );
        }
    }

    #[test]
    fn ids_are_merged_and_required() {
        assert_eq!(
            all_ids(Some("a".into()), vec!["b".into(), "a".into()]).unwrap(),
            ["a", "b"]
        );
        assert!(all_ids(None, Vec::new()).is_err());
    }

    #[test]
    fn info_is_compacted() {
        let info = json!({
            "id": "t", "kind": "text", "strip": "s", "parent": null, "children": [],
            "variant": null, "base": {"a": 1}, "override": null, "effective": {"a": 1},
            "overridden": [], "lang_override": {"font_size": 3},
            "lang_override_langs": ["de"], "text": {"key": "k"},
        });
        let c = compact_info(&info);
        assert_eq!(c["lang_override"]["font_size"], 3);
        assert_eq!(c["base"]["a"], 1);
        assert!(c.get("parent").is_none());
        assert!(c.get("children").is_none());
    }
}
