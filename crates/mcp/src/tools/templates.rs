//! Templates (phase 6.3, ADR 0023): `template_list`, `template_save`,
//! `template_insert`, `template_import`. New projects from a template:
//! `project_create {template}`.

use std::path::PathBuf;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use super::project::NoParams;
use crate::output::ToolOutput;
use crate::server::{VitrineMcp, cmd};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateSaveParams {
    /// Template name, e.g. "Bold gradient".
    pub name: String,
    /// Template id (lowercase, digits, `-`; default: from the name).
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Canvas ids to save (default: all canvases of the project).
    #[serde(default)]
    pub canvases: Vec<String>,
    /// Replace an installed template with the same id.
    #[serde(default)]
    pub replace: bool,
    /// Also write a `.vitrine-template` file here (absolute path).
    #[serde(default)]
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateInsertParams {
    /// Template id (see `template_list`) or a `.vitrine-template` file.
    pub template: String,
    /// Canvases of the template to insert (default: all).
    #[serde(default)]
    pub canvases: Vec<String>,
    /// Same style name with a different definition: `rename` (default,
    /// the template's becomes e.g. `accent-2`) or `use_project`.
    #[serde(default)]
    pub styles: Option<String>,
    /// Only report what would happen.
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TemplateImportParams {
    /// `.vitrine-template` file (absolute path).
    pub path: PathBuf,
    #[serde(default)]
    pub replace: bool,
}

fn opt_list(v: &[String]) -> Value {
    if v.is_empty() { Value::Null } else { json!(v) }
}

#[tool_router(router = template_tools, vis = "pub(crate)")]
impl VitrineMcp {
    #[tool(
        description = "List templates: built in (`phone`, `minimal`) and installed in the app-wide store (saved, imported or downloaded). Each has id, name, description, tags, version, licence (always CC0-1.0), canvases (id, aspect, pages, targets) and fonts.",
        annotations(read_only_hint = true)
    )]
    async fn template_list(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run_unsynced(&ctx, move |app| {
            let list = cmd(app, "template_list", Value::Null)?;
            let n = list.as_array().map_or(0, Vec::len);
            Ok(ToolOutput::new(
                format!("{n} template(s)"),
                json!({ "templates": list }),
            ))
        })
        .await
    }

    #[tool(
        description = "Save canvases of the open project (default: all) as a template in the app-wide store: the canvases with their targets and variants, styles, their texts in every language, image assets and OFL fonts (licence files kept or generated; other fonts are refused). Screenshots are replaced by generated placeholders; store settings are left out. Licence: CC0. Example: {\"name\": \"Bold gradient\", \"canvases\": [\"phone\"], \"tags\": [\"gradient\"]}."
    )]
    async fn template_save(
        &self,
        Parameters(p): Parameters<TemplateSaveParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({
                "name": p.name, "tags": p.tags, "replace": p.replace,
                "canvases": opt_list(&p.canvases),
            });
            for (k, v) in [
                ("id", p.id),
                ("description", p.description),
                ("author", p.author),
            ] {
                if let Some(v) = v {
                    args[k] = json!(v);
                }
            }
            if let Some(f) = p.file {
                args["file"] = json!(f);
            }
            let r = cmd(app, "template_save", args)?;
            Ok(ToolOutput::new(
                format!(
                    "saved template `{}`",
                    r["template"]["id"].as_str().unwrap_or("?")
                ),
                r,
            ))
        })
        .await
    }

    #[tool(
        description = "Insert canvases of a template into the open project as one undo step. Colliding canvas/target/element ids get the next free id (`phone-2`); styles with the same name and a different definition are renamed (`accent-2`) or, with styles=use_project, taken from the project; text keys the project uses with other texts become `<template id>.<key>`; texts are mapped to the project's languages. Use dry_run first to see the report."
    )]
    async fn template_insert(
        &self,
        Parameters(p): Parameters<TemplateInsertParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({ "template": p.template, "canvases": opt_list(&p.canvases) });
            if let Some(s) = p.styles {
                args["styles"] = json!(s);
            }
            if p.dry_run {
                let r = cmd(app, "template_insert_plan", args)?;
                return Ok(ToolOutput::new(
                    "dry run: nothing changed",
                    json!({ "report": r }),
                ));
            }
            let r = cmd(app, "template_insert", args)?;
            let names: Vec<&str> = r["report"]["canvases"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c["to"].as_str())
                .collect();
            Ok(ToolOutput::new(format!("inserted {}", names.join(", ")), r))
        })
        .await
    }

    #[tool(
        description = "Install a `.vitrine-template` file into the app-wide store (checked first: size limits, safe paths, only expected file types, OFL fonts, valid project)."
    )]
    async fn template_import(
        &self,
        Parameters(p): Parameters<TemplateImportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run_unsynced(&ctx, move |app| {
            let r = cmd(
                app,
                "template_import",
                json!({ "path": p.path, "replace": p.replace }),
            )?;
            Ok(ToolOutput::new(
                format!("installed template `{}`", r["id"].as_str().unwrap_or("?")),
                r,
            ))
        })
        .await
    }
}
