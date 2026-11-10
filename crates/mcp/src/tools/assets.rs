//! Assets: list, import files, import screenshot folders, installed frames
//! (read-only).

use std::collections::BTreeMap;
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
use crate::server::{ArrangiaMcp, cmd};

/// Kind of imported file.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// App screenshot → screenshots/<lang>/[<variant>/]<key>.png.
    Screenshot,
    /// Image/SVG for image elements and backgrounds → assets/.
    Asset,
    /// TTF/OTF font → fonts/.
    Font,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetsImportParams {
    /// Absolute source files the user named.
    pub paths: Vec<PathBuf>,
    pub kind: AssetKind,
    /// Screenshots: language folder (default: the default language).
    #[serde(default)]
    pub lang: Option<String>,
    /// Screenshots: variant folder, e.g. `ios-phone` (default: detected
    /// from the pixel size; `""` = directly in the language folder).
    #[serde(default)]
    pub variant: Option<String>,
    /// Screenshots: key (file name without extension) when importing one
    /// file; device elements show the screenshot with their `screenshot`
    /// key (default: the sanitized file name).
    #[serde(default)]
    pub key: Option<String>,
    /// Replace an existing file with the same name (default: add `-2`).
    #[serde(default)]
    pub replace: bool,
}

/// Source folder layout of a screenshot import.
#[derive(Debug, Clone, Copy, Default, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    /// Detect (fastlane names, language folders, else flat).
    #[default]
    Auto,
    /// `<lang>/[<variant>/]<key>.png` like the project.
    Arrangia,
    /// fastlane snapshot output (`de-DE/iPhone 16 Pro Max-01_home.png`).
    Fastlane,
    /// All files in one folder.
    Flat,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportMapping {
    /// fastlane device name → screenshot variant, e.g. `{"iPhone 16 Pro
    /// Max": "ios-phone"}`.
    #[serde(default)]
    pub devices: BTreeMap<String, String>,
    /// Locale folder → project language, e.g. `{"de-DE": "de"}`.
    #[serde(default)]
    pub locales: BTreeMap<String, String>,
    /// Screen name in the file name → screenshot key, e.g. `{"01_home":
    /// "home"}`.
    #[serde(default)]
    pub keys: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScreenshotsImportParams {
    /// Source folder (absolute) the user named.
    pub dir: PathBuf,
    /// Folder layout (default `auto`).
    #[serde(default)]
    pub format: SourceFormat,
    /// Extra mappings on top of the project's.
    #[serde(default)]
    pub mapping: ImportMapping,
    /// Only plan (what would be copied where, what stays missing).
    #[serde(default)]
    pub dry_run: bool,
    /// Store the effective mapping in project.yaml for the next import.
    #[serde(default)]
    pub save_mapping: bool,
}

#[tool_router(router = asset_tools, vis = "pub(crate)")]
impl ArrangiaMcp {
    #[tool(
        description = "Files of the project: screenshots (path, language, variant, key, size), assets, fonts, which device elements use each screenshot key, which elements use each asset, and the screenshot variants the targets need.",
        annotations(read_only_hint = true)
    )]
    async fn assets_list(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(app, "assets_list", Value::Null)?;
            let n = |k: &str| v[k].as_array().map_or(0, Vec::len);
            Ok(ToolOutput::new(
                format!(
                    "{} screenshot(s), {} asset(s), {} font(s)",
                    n("screenshots"),
                    n("assets"),
                    n("fonts")
                ),
                v,
            ))
        })
        .await
    }

    #[tool(
        description = "Copy files the user named into the project: screenshots (language/variant/key), image assets or fonts. Example: {\"paths\": [\"/Users/me/Desktop/home.png\"], \"kind\": \"screenshot\", \"lang\": \"en\", \"key\": \"home\"}."
    )]
    async fn assets_import(
        &self,
        Parameters(p): Parameters<AssetsImportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mut args = json!({ "paths": p.paths, "kind": p.kind, "replace": p.replace });
            for (k, v) in [("lang", p.lang), ("variant", p.variant), ("key", p.key)] {
                if let Some(v) = v {
                    args[k] = json!(v);
                }
            }
            let v = cmd(app, "assets_import", args)?;
            let n = v.as_array().map_or(0, Vec::len);
            Ok(ToolOutput::new(
                format!("imported {n} file(s)"),
                json!({ "files": v }),
            ))
        })
        .await
    }

    #[tool(
        description = "Import a folder of app screenshots (fastlane output, language folders or flat) into screenshots/<lang>/<variant>/<key>.png: plans the mapping (device → variant, locale → language, screen → key) and copies unless dry_run. Run with dry_run first and show the user what would happen. Example: {\"dir\": \"/Users/me/app/fastlane/screenshots\", \"dry_run\": true}."
    )]
    async fn screenshots_import(
        &self,
        Parameters(p): Parameters<ScreenshotsImportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let mapping = json!({
                "devices": p.mapping.devices, "locales": p.mapping.locales, "keys": p.mapping.keys,
            });
            let format = serde_json::to_value(p.format).unwrap_or_default();
            if p.dry_run {
                let plan = cmd(
                    app,
                    "import_plan",
                    json!({ "dir": p.dir, "format": format, "mapping": mapping }),
                )?;
                let n = plan["items"].as_array().map_or(0, Vec::len);
                return Ok(ToolOutput::new(
                    format!("plan: {n} file(s) (dry run)"),
                    plan,
                ));
            }
            let r = cmd(
                app,
                "import_apply",
                json!({
                    "dir": p.dir, "format": format, "mapping": mapping,
                    "save_mapping": p.save_mapping,
                }),
            )?;
            let copied = r["report"]["copied"].as_array().map_or(0, Vec::len);
            Ok(ToolOutput::new(format!("copied {copied} file(s)"), r))
        })
        .await
    }

    #[tool(
        description = "Vendor device frames installed app-wide (Apple Product Bezels imported by the user in the GUI): models, colours, orientations. Read-only; device elements use them with frame `imported:apple`.",
        annotations(read_only_hint = true)
    )]
    async fn frames_list(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(app, "frames_list", Value::Null)?;
            let n: usize = v["sources"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|s| s["frames"].as_array().map_or(0, Vec::len))
                .sum();
            Ok(ToolOutput::new(format!("{n} installed frame(s)"), v))
        })
        .await
    }
}
