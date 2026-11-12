//! Templates (phase 6.3, ADR 0023): `template_list`, `template_save`,
//! `template_insert`, `template_import`. New projects from a template:
//! `project_create {template}`. Tags come from the fixed taxonomy
//! (`crates/core/data/template-tags.yaml`): the schema lists the allowed
//! values of each facet as an enum.

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

/// A tag value of one facet; its JSON schema is a string enum of the
/// taxonomy's values.
macro_rules! facet_tag {
    ($name:ident, $facet:literal, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl JsonSchema for $name {
            fn schema_name() -> std::borrow::Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                let values: Vec<&str> = vitrine_core::template::tags::taxonomy()
                    .facet($facet)
                    .map(|f| f.values.iter().map(|v| v.id.as_str()).collect())
                    .unwrap_or_default();
                schemars::json_schema!({ "type": "string", "enum": values, "description": $doc })
            }
        }
    };
}

facet_tag!(StyleTag, "style", "Style tag of a template.");
facet_tag!(ColorTag, "color", "Colour tag of a template.");
facet_tag!(UseTag, "use", "App category tag of a template.");

fn tag_values<T: Clone + Into<String>>(v: &[T]) -> Vec<String> {
    v.iter().cloned().map(Into::into).collect()
}

impl From<StyleTag> for String {
    fn from(t: StyleTag) -> Self {
        t.0
    }
}
impl From<ColorTag> for String {
    fn from(t: ColorTag) -> Self {
        t.0
    }
}
impl From<UseTag> for String {
    fn from(t: UseTag) -> Self {
        t.0
    }
}

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
    /// Style tags (1–2 for the gallery).
    #[serde(default)]
    pub style: Vec<StyleTag>,
    /// Colour tags (1–2 for the gallery).
    #[serde(default)]
    pub color: Vec<ColorTag>,
    /// App category tags (1–3 for the gallery).
    #[serde(default, rename = "use")]
    pub use_: Vec<UseTag>,
    /// Pack the project's screenshots used by the canvases as sample
    /// screenshots (only when the user says they are their own app
    /// screenshots and may be published as CC0), re-encoded as JPEG
    /// quality 60 (PNGs with transparency stay PNG); default placeholders.
    #[serde(default)]
    pub sample_screenshots: bool,
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
        description = "Save canvases of the open project (default: all) as a template in the app-wide store: the canvases with their targets and variants, styles, their texts in every language, image assets and OFL fonts (licence files kept or generated; other fonts are refused). Screenshots are replaced by generated placeholders unless `sample_screenshots` is true (only when the user confirms they are their own; they become samples in new projects and block store uploads there); store settings are left out. Tags only from the fixed facets style (1-2), color (1-2), use (1-3) – the gallery needs all three. Licence: CC0. Example: {\"name\": \"Bold gradient\", \"canvases\": [\"phone\"], \"style\": [\"bold\"], \"color\": [\"gradient\"], \"use\": [\"productivity\"]}."
    )]
    async fn template_save(
        &self,
        Parameters(p): Parameters<TemplateSaveParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({
                "name": p.name, "replace": p.replace,
                "tags": {
                    "style": tag_values(&p.style),
                    "color": tag_values(&p.color),
                    "use": tag_values(&p.use_),
                },
                "sample_screenshots": p.sample_screenshots,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_schema_lists_the_tag_values() {
        let schema = serde_json::to_string(&schemars::schema_for!(TemplateSaveParams)).unwrap();
        for v in [
            "\"minimal\"",
            "\"pastel\"",
            "\"health-fitness\"",
            "\"sample_screenshots\"",
        ] {
            assert!(schema.contains(v), "{v} missing in {schema}");
        }
        let p: TemplateSaveParams =
            serde_json::from_value(json!({"name": "X", "use": ["travel"]})).unwrap();
        assert_eq!(tag_values(&p.use_), ["travel"]);
    }
}
