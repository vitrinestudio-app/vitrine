//! Project tools: open, create, summary, the GUI's view and selection.

use std::collections::BTreeMap;
use std::path::PathBuf;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};
use vitrine_app::CommandError;
use vitrine_app::commands::AppState;
use vitrine_core::model::ElementKind;

use crate::output::ToolOutput;
use crate::server::{Host, VitrineMcp, cmd};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectOpenParams {
    /// Project folder (contains project.yaml). Absolute path recommended.
    pub dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectCreateParams {
    /// New folder (must not exist or be empty). Absolute path recommended.
    pub dir: PathBuf,
    /// Project/app name (default: the folder name).
    #[serde(default)]
    pub name: Option<String>,
    /// Template: built in (`phone` – three phone pages, iPhone 6.9" +
    /// 6.3" + Google Play + blog banner; `minimal` – one page), the id of an
    /// installed template (see `template_list`) or a `.vitrine-template`
    /// file path. Default `phone`.
    #[serde(default)]
    pub template: Option<String>,
    /// Languages as BCP 47 tags, the first is the default language, e.g.
    /// `["en", "de", "fr"]` (default: the app setting, usually `en`).
    #[serde(default)]
    pub langs: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// Translation key of every text element (by element id).
fn text_keys(app: &AppState) -> Result<BTreeMap<String, String>, CommandError> {
    let session = app.session()?;
    Ok(session.with_project(|p| {
        let mut out = BTreeMap::new();
        for s in &p.canvases {
            s.walk_elements(&mut |e| {
                if let ElementKind::Text(t) = &e.kind {
                    out.insert(e.id.to_string(), t.text.to_string());
                }
            });
        }
        out
    }))
}

/// Compact element list of a summary canvas.
fn compact_elements(elements: &Value, keys: &BTreeMap<String, String>) -> Vec<Value> {
    elements
        .as_array()
        .into_iter()
        .flatten()
        .map(|e| {
            let id = e["id"].as_str().unwrap_or_default();
            let mut c = json!({
                "id": id, "kind": e["kind"], "page": e["page"], "label": e["label"],
            });
            if let Some(k) = keys.get(id) {
                c["key"] = json!(k);
            }
            if let Some(p) = e.get("parent").filter(|p| !p.is_null()) {
                c["parent"] = p.clone();
            }
            if e["visible"] == false {
                c["visible"] = json!(false);
            }
            if e["z"].as_i64().is_some_and(|z| z != 0) {
                c["z"] = e["z"].clone();
            }
            c
        })
        .collect()
}

/// The compact project overview of `project_summary`.
pub fn compact_summary(app: &AppState) -> Result<Value, CommandError> {
    let s = cmd(app, "project_summary", Value::Null)?;
    let keys = text_keys(app)?;
    let canvases: Vec<Value> = s["canvases"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|st| {
            json!({
                "id": st["id"],
                "aspect": st["page"]["aspect"],
                "page_height_lu": st["page"]["height_lu"],
                "pages": st["page"]["ids"],
                "variants": st["variants"],
                "elements": compact_elements(&st["elements"], &keys),
            })
        })
        .collect();
    let targets: Vec<Value> = s["targets"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|t| {
            let mut c = json!({
                "id": t["id"], "kind": t["kind"], "canvas": t["canvas"],
                "size": format!("{}x{}", t["size"][0], t["size"][1]),
                "group": t["group"],
            });
            for k in ["variant", "gallery", "screenshots"] {
                if let Some(v) = t.get(k).filter(|v| !v.is_null()) {
                    c[k] = v.clone();
                }
            }
            if t["langs"] != s["languages"]["available"] {
                c["langs"] = t["langs"].clone();
            }
            c
        })
        .collect();
    Ok(json!({
        "dir": s["dir"],
        "name": s["name"],
        "revision": s["revision"],
        "languages": s["languages"],
        "canvases": canvases,
        "targets": targets,
        "styles": s["styles"],
    }))
}

fn summary_line(v: &Value) -> String {
    let n = |k: &str| v[k].as_array().map_or(0, Vec::len);
    let elements: usize = v["canvases"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|s| s["elements"].as_array().map_or(0, Vec::len))
        .sum();
    format!(
        "project `{}` (revision {}): {} canvas(s), {elements} element(s), {} target(s), languages {}",
        v["name"].as_str().unwrap_or_default(),
        v["revision"],
        n("canvases"),
        n("targets"),
        v["languages"]["available"]
    )
}

#[tool_router(router = project_tools, vis = "pub(crate)")]
impl VitrineMcp {
    #[tool(
        description = "Open a Vitrine Studio project folder (closes the current one) and return its compact summary (canvases, pages, elements, targets, languages, styles). Example: {\"dir\": \"/Users/me/Screens/MyApp\"}."
    )]
    async fn project_open(
        &self,
        Parameters(p): Parameters<ProjectOpenParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run_unsynced(&ctx, move |app| {
            cmd(app, "project_open", json!({ "dir": p.dir }))?;
            let s = compact_summary(app)?;
            Ok(ToolOutput::new(format!("opened {}", summary_line(&s)), s))
        })
        .await
    }

    #[tool(
        description = "Create a new project from a template (built in or installed, see template_list) in an empty or new folder, open it and return its summary. Refuses non-empty folders. Example: {\"dir\": \"/Users/me/Screens/MyApp\", \"name\": \"MyApp\", \"template\": \"phone\", \"langs\": [\"en\", \"de\"]}."
    )]
    async fn project_create(
        &self,
        Parameters(p): Parameters<ProjectCreateParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run_unsynced(&ctx, move |app| {
            let mut args =
                json!({ "dir": p.dir, "template": p.template.as_deref().unwrap_or("phone") });
            if let Some(n) = p.name {
                args["name"] = json!(n);
            }
            if !p.langs.is_empty() {
                args["langs"] = json!(p.langs);
            }
            cmd(app, "project_new", args)?;
            let s = compact_summary(app)?;
            Ok(ToolOutput::new(format!("created {}", summary_line(&s)), s))
        })
        .await
    }

    #[tool(
        description = "Compact overview of the open project: canvases (aspect, page height in lu, page ids, variants, elements with id/kind/home page/label and translation key), targets (id, kind, canvas, size WxH, group, variant), languages, style names, revision. Call it first and whenever ids are unclear.",
        annotations(read_only_hint = true)
    )]
    async fn project_summary(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let s = compact_summary(app)?;
            Ok(ToolOutput::new(summary_line(&s), s))
        })
        .await
    }

    #[tool(
        description = "What the user is looking at in the Vitrine Studio GUI: canvas, target, language, variant and selected element ids. Use it when the user says \"this page\", \"here\" or \"the selected text\". Headless (no GUI) the view is empty unless a GUI pushed one.",
        annotations(read_only_hint = true)
    )]
    async fn view_get(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let host = self.host();
        self.run(&ctx, move |app| {
            let mut v = cmd(app, "view_get", Value::Null)?;
            v["host"] = json!(match host {
                Host::Gui => "gui",
                Host::Headless => "headless",
            });
            let line = match (&v["target"], &v["lang"]) {
                (Value::String(t), Value::String(l)) => format!("view: target {t}, language {l}"),
                _ => "no view set (no GUI connected)".to_owned(),
            };
            Ok(ToolOutput::new(line, v))
        })
        .await
    }

    #[tool(
        description = "The elements selected in the Vitrine Studio GUI, each with its inspector data (base fields, overrides, effective values, text). Use it for requests like \"make this bigger\". Empty when nothing is selected or no GUI is connected.",
        annotations(read_only_hint = true)
    )]
    async fn selection_get(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let view = cmd(app, "view_get", Value::Null)?;
            let mut items = Vec::new();
            for id in view["selection"].as_array().into_iter().flatten() {
                let mut args = json!({ "id": id });
                for k in ["target", "lang", "variant"] {
                    if let Some(v) = view.get(k) {
                        args[k] = v.clone();
                    }
                }
                match cmd(app, "element_get", args) {
                    Ok(info) => items.push(crate::tools::elements::compact_info(&info)),
                    Err(e) => items.push(json!({ "id": id, "error": e.message })),
                }
            }
            let line = format!("{} selected element(s)", items.len());
            Ok(ToolOutput::new(
                line,
                json!({ "selection": view["selection"], "view": view, "elements": items }),
            ))
        })
        .await
    }
}
