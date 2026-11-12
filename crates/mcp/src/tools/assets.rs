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

use vitrine_app::CommandError;

use super::project::NoParams;
use crate::output::ToolOutput;
use crate::server::{VitrineMcp, cmd};

/// Kind of imported file.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// App screenshot shown in device elements →
    /// screenshots/<lang>/[<variant>/]<key>.png.
    Screenshot,
    /// Any image (PNG, JPEG, SVG …) for image elements and image
    /// backgrounds/fills → assets/<name>.<ext>. Alias: `image`.
    #[serde(alias = "image")]
    Asset,
    /// TTF/OTF font → fonts/.
    Font,
}

/// One file of a mapping-list import (`files`).
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImportFile {
    /// Absolute source file the user named.
    pub path: PathBuf,
    /// Default: the call's `kind`.
    #[serde(default)]
    pub kind: Option<AssetKind>,
    /// Screenshots: key (file name without extension in the project).
    #[serde(default)]
    pub key: Option<String>,
    /// Assets/fonts: file name without extension in the project (same as
    /// `key`).
    #[serde(default)]
    pub name: Option<String>,
    /// Screenshots: language folder (default: the call's, else the default
    /// language).
    #[serde(default)]
    pub lang: Option<String>,
    /// Screenshots: variant folder (default: the call's, else detected
    /// from the pixel size; `""` = directly in the language folder).
    #[serde(default)]
    pub variant: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssetsImportParams {
    /// Absolute source files the user named (all with the same kind, lang,
    /// variant). For per-file keys/names use `files`.
    #[serde(default)]
    pub paths: Vec<PathBuf>,
    /// `screenshot`, `asset` (any image; alias `image`) or `font`. Default
    /// for the entries of `files`.
    #[serde(default)]
    pub kind: Option<AssetKind>,
    /// Mapping list: one entry per file with its own kind, key/name, lang,
    /// variant, e.g. `[{"path": "/Users/me/shots/a.png", "kind":
    /// "screenshot", "key": "home", "lang": "de"}, {"path":
    /// "/Users/me/bg.jpg", "kind": "asset", "name": "hero-bg"}]`. Each file
    /// gets its own result (a failing file does not stop the others).
    #[serde(default)]
    pub files: Vec<ImportFile>,
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
    /// Only report where each file would go; copy nothing.
    #[serde(default)]
    pub dry_run: bool,
}

/// Source folder layout of a screenshot import.
#[derive(Debug, Clone, Copy, Default, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceFormat {
    /// Detect (fastlane names, language folders, else flat).
    #[default]
    Auto,
    /// `<lang>/[<variant>/]<key>.png` like the project.
    Vitrine,
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

/// One entry of a mapping-list import (the call's settings as defaults).
fn import_one(
    app: &vitrine_app::commands::AppState,
    p: &AssetsImportParams,
    f: &ImportFile,
) -> Result<Value, CommandError> {
    let kind = f.kind.or(p.kind).ok_or_else(|| {
        CommandError::invalid("`kind` missing (screenshot, asset or font) for this file")
    })?;
    let key = match (&f.key, &f.name) {
        (Some(_), Some(_)) => {
            return Err(CommandError::invalid("give `key` or `name`, not both"));
        }
        (Some(k), None) | (None, Some(k)) => Some(k.clone()),
        (None, None) => None,
    };
    let mut args = json!({
        "paths": [f.path], "kind": kind, "replace": p.replace, "dry_run": p.dry_run,
    });
    let lang = f.lang.clone().or_else(|| p.lang.clone());
    let variant = f.variant.clone().or_else(|| p.variant.clone());
    for (k, v) in [("lang", lang), ("variant", variant), ("key", key)] {
        if let Some(v) = v {
            args[k] = json!(v);
        }
    }
    cmd(app, "assets_import", args)
}

#[tool_router(router = asset_tools, vis = "pub(crate)")]
impl VitrineMcp {
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
        description = "Copy files the user named into the project: screenshots (screenshots/<lang>/[<variant>/]<key>.png, shown by device elements with that `screenshot` key), assets (kind `asset` = any image for image elements and image fills, alias `image` → assets/) or fonts (→ fonts/). Several files with their own key/name/lang/variant in ONE call: `files` (mapping list, per-file results, `dry_run` to preview). A whole folder of screenshots (fastlane, language folders): screenshots_import. Examples: {\"paths\": [\"/Users/me/Desktop/home.png\"], \"kind\": \"screenshot\", \"lang\": \"en\", \"key\": \"home\"}; {\"files\": [{\"path\": \"/Users/me/de/1.png\", \"kind\": \"screenshot\", \"lang\": \"de\", \"key\": \"home\"}, {\"path\": \"/Users/me/leaf.svg\", \"kind\": \"asset\", \"name\": \"leaf\"}], \"dry_run\": true}."
    )]
    async fn assets_import(
        &self,
        Parameters(p): Parameters<AssetsImportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            if p.paths.is_empty() == p.files.is_empty() {
                return Err(CommandError::invalid(
                    "give either `paths` (with `kind`) or `files` [{path, kind?, key?, name?, lang?, variant?}]",
                ));
            }
            if !p.paths.is_empty() {
                let kind = p.kind.ok_or_else(|| {
                    CommandError::invalid("`kind` is required: screenshot, asset (any image) or font")
                })?;
                let mut args = json!({
                    "paths": p.paths, "kind": kind, "replace": p.replace, "dry_run": p.dry_run,
                });
                for (k, v) in [("lang", p.lang), ("variant", p.variant), ("key", p.key)] {
                    if let Some(v) = v {
                        args[k] = json!(v);
                    }
                }
                let v = cmd(app, "assets_import", args)?;
                let n = v.as_array().map_or(0, Vec::len);
                let verb = if p.dry_run { "would import" } else { "imported" };
                return Ok(ToolOutput::new(
                    format!("{verb} {n} file(s)"),
                    json!({ "files": v, "dry_run": p.dry_run }),
                ));
            }
            let mut results = Vec::new();
            let mut ok = 0;
            for f in &p.files {
                let r = import_one(app, &p, f);
                match r {
                    Ok(v) => {
                        ok += 1;
                        let mut item = v.as_array().and_then(|a| a.first()).cloned().unwrap_or_default();
                        item["ok"] = json!(true);
                        results.push(item);
                    }
                    Err(e) => results.push(json!({
                        "source": f.path, "ok": false,
                        "error": {"code": crate::output::code_name(e.code), "message": e.message},
                    })),
                }
            }
            let failed = results.len() - ok;
            let verb = if p.dry_run { "would import" } else { "imported" };
            let summary = if failed > 0 {
                format!("{verb} {ok} file(s), {failed} failed")
            } else {
                format!("{verb} {ok} file(s)")
            };
            Ok(ToolOutput::new(
                summary,
                json!({ "files": results, "dry_run": p.dry_run }),
            ))
        })
        .await
    }

    #[tool(
        description = "Import a folder of app screenshots (fastlane output, language folders or flat) into screenshots/<lang>/<variant>/<key>.png: plans the mapping (device → variant, locale → language, screen → key; extra names via `mapping`) and copies unless dry_run. Run with dry_run first and show the user what would happen. Single files with their own key: assets_import {files: [...]}. Example: {\"dir\": \"/Users/me/app/fastlane/screenshots\", \"dry_run\": true}."
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
        description = "Device frames from image files: vendor frames installed app-wide (Apple Product Bezels imported by the user, `sources`; devices use `frame: imported:apple`) and own frame packs (`packs`: the project's `frames/<id>/` and app-wide ones; devices use `frame: pack:<id>`, colours are the pack's colour ids), plus the Apple bezels the user stored in the project for CI (`stored`, `frames/imported/`; they win over the app-wide ones; storing/removing them is the user's decision in the GUI or CLI because of Apple's licence). Models, colours, orientations. Read-only.",
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
            let packs = v["packs"].as_array().map_or(0, Vec::len);
            let stored: usize = v["stored"]
                .as_object()
                .into_iter()
                .flat_map(|m| m.values())
                .map(|f| f.as_array().map_or(0, Vec::len))
                .sum();
            Ok(ToolOutput::new(
                format!(
                    "{n} installed vendor frame(s), {packs} frame pack(s), {stored} stored in the project"
                ),
                v,
            ))
        })
        .await
    }
}
