//! Catalog tools: devices, target presets, store galleries, fonts.

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

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DevicesParams {
    /// Only this device class: `phone`, `tablet`, `laptop`, `desktop`,
    /// `browser`, `none` (default: all).
    #[serde(default)]
    pub class: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TargetsParams {
    /// Only presets of this store or platform: `apple`, `google`, `none`
    /// (web/banners), or `ios`, `ipados`, `macos`, `android`, `web`
    /// (default: all).
    #[serde(default)]
    pub store: Option<String>,
}

fn matches(v: &Value, key: &str, want: Option<&str>) -> bool {
    want.is_none_or(|w| v[key].as_str().is_some_and(|s| s.eq_ignore_ascii_case(w)))
}

#[tool_router(router = catalog_tools, vis = "pub(crate)")]
impl VitrineMcp {
    #[tool(
        description = "Device models for device elements (`model`): id, label, class, orientations, colour variants (`color`), display aspect and typical screenshot sizes.",
        annotations(read_only_hint = true)
    )]
    async fn catalog_devices(
        &self,
        Parameters(p): Parameters<DevicesParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let all = cmd(app, "catalog_devices", Value::Null)?;
            let list: Vec<Value> = all
                .as_array()
                .into_iter()
                .flatten()
                .filter(|d| matches(d, "class", p.class.as_deref()))
                .map(|d| {
                    json!({
                        "id": d["id"], "label": d["label"], "class": d["class"],
                        "orientations": d["orientations"], "colors": d["colors"],
                        "default_color": d["default_color"],
                        "display_aspect": d["display_aspect"],
                        "screenshot_sizes": d["screenshot_sizes"],
                    })
                })
                .collect();
            Ok(ToolOutput::new(
                format!("{} device(s)", list.len()),
                json!({ "devices": list }),
            ))
        })
        .await
    }

    #[tool(
        description = "Target presets for target_add: id, label, store, platform, kind, portrait/landscape export sizes, default size, store slot, gallery profile, preset device, requirement (required/optional). Example: catalog_targets {\"store\": \"apple\"}.",
        annotations(read_only_hint = true)
    )]
    async fn catalog_targets(
        &self,
        Parameters(p): Parameters<TargetsParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let all = cmd(app, "catalog_targets", Value::Null)?;
            let list: Vec<Value> = all
                .as_array()
                .into_iter()
                .flatten()
                .filter(|t| {
                    matches(t, "store", p.store.as_deref())
                        || matches(t, "platform", p.store.as_deref())
                })
                .cloned()
                .collect();
            Ok(ToolOutput::new(
                format!("{} target preset(s)", list.len()),
                json!({ "presets": list }),
            ))
        })
        .await
    }

    #[tool(
        description = "Store upload configuration (read-only): the app-wide store accounts (id, kind apple/google, label, Apple issuer/key id, Google client email – never any key material) and, with a project open, its store mapping (`store:` in project.yaml: Apple app id / bundle id / platform, Google package name, account references) with the effective store locale of every project language (explicit, exact or built-in default) and account references that do not resolve. Accounts and the mapping are set by the user in the GUI; there is no tool to import keys.",
        annotations(read_only_hint = true)
    )]
    async fn store_config_get(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(app, "store_config_get", Value::Null)?;
            let n = v["accounts"].as_array().map_or(0, Vec::len);
            let mapped: Vec<&str> = ["apple", "google"]
                .into_iter()
                .filter(|s| !v["project"]["store"][*s].is_null())
                .collect();
            let text = if v["project"].is_null() {
                format!("{n} store account(s); no project open")
            } else if mapped.is_empty() {
                format!("{n} store account(s); the project is not mapped to a store")
            } else {
                format!(
                    "{n} store account(s); project mapped to {}",
                    mapped.join(", ")
                )
            };
            Ok(ToolOutput::new(text, v))
        })
        .await
    }

    #[tool(
        description = "Store gallery profiles (gap between images, corner radius, background) used for gallery previews and to cut content in gaps.",
        annotations(read_only_hint = true)
    )]
    async fn catalog_galleries(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(app, "catalog_galleries", Value::Null)?;
            let n = v.as_array().map_or(0, Vec::len);
            Ok(ToolOutput::new(
                format!("{n} gallery profile(s)"),
                json!({ "galleries": v }),
            ))
        })
        .await
    }

    #[tool(
        description = "Font families usable in text styles: bundled (always available, no files needed: Inter, Inter Display, Montserrat, Nunito, Oswald – sans serif; Playfair Display, Lora – serif; Bebas Neue – all-caps display; Caveat, Dancing Script – handwriting; JetBrains Mono – monospace; `bundled_info` gives category, weight range (variable fonts: any weight in it) and whether a real italic exists), from the project's fonts/ folder, and system fallbacks (`system`: families loaded for characters no other font covers; `system_enabled`: the app setting).",
        annotations(read_only_hint = true)
    )]
    async fn catalog_fonts(
        &self,
        Parameters(_): Parameters<NoParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(app, "catalog_fonts", Value::Null)?;
            Ok(ToolOutput::new("font families", v))
        })
        .await
    }
}
