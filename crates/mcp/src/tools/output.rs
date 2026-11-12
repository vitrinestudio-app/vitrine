//! Output: `render_preview` (PNG image content), `layout_report`, validate,
//! export (with progress notifications).

use std::path::PathBuf;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ProgressNotificationParam};
use rmcp::service::RequestContext;
use rmcp::{RoleServer, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use vitrine_app::CommandError;
use vitrine_app::commands::AppState;

use crate::output::ToolOutput;
use crate::server::{VitrineMcp, cmd, image_cmd};

/// What `render_preview` shows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewView {
    /// One output image exactly as exported (a page, a page group or a
    /// banner) – default.
    #[default]
    Image,
    /// The whole canvas with page borders, hatched gaps and page labels.
    Canvas,
    /// All output images of the target side by side with the store's gaps.
    Gallery,
}

/// Default longer edge of previews (px).
pub const DEFAULT_MAX_EDGE: u32 = 1200;

fn default_max_edge() -> u32 {
    DEFAULT_MAX_EDGE
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RenderPreviewParams {
    /// Target (default: the one the user looks at in the GUI, else the
    /// first target).
    #[serde(default)]
    pub target: Option<String>,
    /// Language (default: the GUI's language, else the default language).
    #[serde(default)]
    pub lang: Option<String>,
    /// `image` (default), `canvas` or `gallery`.
    #[serde(default)]
    pub view: PreviewView,
    /// view image: output image index, 0-based (default 0).
    #[serde(default)]
    pub image: Option<u32>,
    /// view image: the output image containing this page (0-based)
    /// instead of `image`.
    #[serde(default)]
    pub page: Option<u32>,
    /// Show this variant instead of the target's own (`""` = base).
    #[serde(default)]
    pub variant: Option<String>,
    /// Longer edge of the PNG in px (default 1200; gallery: image height
    /// is half of it).
    #[serde(default = "default_max_edge")]
    pub max_edge: u32,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LayoutReportParams {
    /// Target (default: the one the user looks at in the GUI, else the
    /// first target).
    #[serde(default)]
    pub target: Option<String>,
    /// Language (default: the GUI's language, else the default language).
    #[serde(default)]
    pub lang: Option<String>,
    /// Only elements with this home page (0-based).
    #[serde(default)]
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ValidateParams {
    /// Only these targets (default: all).
    #[serde(default)]
    pub targets: Option<Vec<String>>,
    /// Only these languages (default: each target's languages).
    #[serde(default)]
    pub langs: Option<Vec<String>>,
    /// Editing mode: missing screenshots are warnings, not errors.
    #[serde(default)]
    pub preview: bool,
    /// Run the rendered design checks (contrast, text near devices,
    /// screens, seams, corners, store background); default true.
    #[serde(default)]
    pub pixel_checks: Option<bool>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportParams {
    /// Only these targets (default: all).
    #[serde(default)]
    pub targets: Vec<String>,
    /// Only these languages (default: each target's languages).
    #[serde(default)]
    pub langs: Vec<String>,
    /// Render every file again (default: only changed ones).
    #[serde(default)]
    pub force: bool,
    /// Export folder (default `<project>/export`).
    #[serde(default)]
    pub out_dir: Option<PathBuf>,
    /// Also write `_gallery.png` previews (default true; never in the
    /// fastlane layout).
    #[serde(default)]
    pub gallery: Option<bool>,
    /// Folder structure (default `standard`: `<target>/<lang>/…`).
    /// `fastlane`: folders `fastlane deliver` / `supply` upload as they are
    /// (default folder `<project>/export/fastlane`).
    #[serde(default)]
    pub layout: Option<ExportLayout>,
    /// Write the `index.html` overview of all exported files (default true).
    #[serde(default)]
    pub html: Option<bool>,
}

/// Export folder structure.
#[derive(Debug, Clone, Copy, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportLayout {
    Standard,
    Fastlane,
}

/// Target and language of a preview (defaults filled in).
fn view_of(
    app: &AppState,
    target: Option<String>,
    lang: Option<String>,
) -> Result<(String, String), CommandError> {
    let session = app.session()?;
    session.with_project(|p| {
        let target = match target {
            Some(t) => {
                if p.target(&t).is_none() {
                    return Err(CommandError::not_found(format!("unknown target `{t}`")));
                }
                t
            }
            None => crate::params::gui_target(p, None)
                .or_else(|| p.targets.first().map(|t| t.id.to_string()))
                .ok_or_else(|| {
                    CommandError::invalid("the project has no targets; add one with target_add")
                })?,
        };
        let lang = lang
            .or_else(|| crate::params::gui_lang(p))
            .unwrap_or_else(|| p.languages.default.to_string());
        Ok((target, lang))
    })
}

fn r1(v: &Value) -> Value {
    v.as_f64()
        .map_or(Value::Null, |f| json!((f * 10.0).round() / 10.0))
}

fn rect_rel(r: &Value, ox: f64, oy: f64) -> Value {
    let f = |k: &str| r[k].as_f64().unwrap_or(0.0);
    json!({
        "x": r1(&json!(f("x") - ox)), "y": r1(&json!(f("y") - oy)),
        "w": r1(&r["w"]), "h": r1(&r["h"]),
    })
}

/// Layout report from the overlay of a view plus its rendered geometry
/// (`scene_report`): page-relative boxes, text metrics, devices with their
/// screens, gallery gap and seams, pixel overlaps.
pub fn layout_report(overlay: &Value, scene: &Value, page_filter: Option<u32>) -> Value {
    let pages = overlay["pages"].as_array().cloned().unwrap_or_default();
    let origin = |page: u64| {
        pages
            .iter()
            .find(|p| p["index"].as_u64() == Some(page))
            .map_or((0.0, 0.0), |p| {
                (
                    p["rect"]["x"].as_f64().unwrap_or(0.0),
                    p["rect"]["y"].as_f64().unwrap_or(0.0),
                )
            })
    };
    let devices: Vec<&Value> = scene["devices"].as_array().into_iter().flatten().collect();
    let mut elements = Vec::new();
    for e in overlay["elements"].as_array().into_iter().flatten() {
        let page = e["page"].as_u64().unwrap_or(0);
        if page_filter.is_some_and(|f| u64::from(f) != page) {
            continue;
        }
        let (ox, oy) = origin(page);
        let mut item = json!({
            "id": e["id"], "kind": e["kind"], "page": page,
            "box": rect_rel(&e["rect"], ox, oy),
        });
        if e["rotation"].as_f64().is_some_and(|r| r.abs() > 1e-6) {
            item["rotation"] = r1(&e["rotation"]);
        }
        if e["in_gap"] == true {
            item["in_gap"] = json!(true);
        }
        if let Some(t) = e.get("text").filter(|t| t.is_object()) {
            let mut text = json!({
                "font_size": r1(&t["font_size"]),
                "lines": t["lines"],
                "content": rect_rel(&t["content"], ox, oy),
                "overflow": t["overflow"],
            });
            for k in ["hyphenated", "emergency_breaks"] {
                if t[k].as_array().is_some_and(|a| !a.is_empty()) {
                    text[k] = t[k].clone();
                }
            }
            item["text"] = text;
        }
        if let Some(d) = devices.iter().find(|d| d["id"] == e["id"]) {
            item["model"] = d["model"].clone();
            item["frame"] = d["frame"].clone();
            item["screen"] = d["screen"].clone();
            if d["covered_by"].as_array().is_some_and(|a| !a.is_empty()) {
                item["screen_covered_by"] = d["covered_by"].clone();
            }
        }
        elements.push(item);
    }
    let mut out = json!({
        "target": overlay["target"], "lang": overlay["lang"], "variant": overlay["variant"],
        "pages": pages.iter().map(|p| json!({
            "index": p["index"], "id": p["id"], "image": p["image"],
            "size": {"w": r1(&p["rect"]["w"]), "h": r1(&p["rect"]["h"])},
        })).collect::<Vec<_>>(),
        "elements": elements,
        "overlaps": scene["overlaps"].as_array().cloned().unwrap_or_default(),
        "seams": scene["seams"].as_array().cloned().unwrap_or_default(),
    });
    if scene["gallery"].is_object() {
        out["gallery"] = scene["gallery"].clone();
    }
    out
}

/// Errors and warnings of `target` × `lang` (editing mode).
fn issues(app: &AppState, target: &str, lang: &str) -> Result<Value, CommandError> {
    let v = cmd(
        app,
        "validate",
        json!({ "targets": [target], "langs": [lang], "preview": true }),
    )?;
    let list: Vec<Value> = v["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["level"] != "info")
        .map(|f| {
            let mut c = Map::new();
            for k in ["level", "code", "element", "page", "message"] {
                if let Some(x) = f.get(k).filter(|x| !x.is_null()) {
                    c.insert(k.into(), x.clone());
                }
            }
            Value::Object(c)
        })
        .collect();
    Ok(json!(list))
}

#[tool_router(router = output_tools, vis = "pub(crate)")]
impl VitrineMcp {
    #[tool(
        description = "Render a PNG preview with the export renderer and return it as an image – look at it after every visual change. view: image (one output image as exported; pick `image` index or `page`), canvas (whole canvas with page borders and hatched gaps), gallery (all images with store gaps). Also returns size, the number of output images and errors/warnings of this target × language. Example: {\"target\": \"ios-iphone-69\", \"lang\": \"de\", \"page\": 2}.",
        annotations(read_only_hint = true)
    )]
    async fn render_preview(
        &self,
        Parameters(p): Parameters<RenderPreviewParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            if !(64..=4096).contains(&p.max_edge) {
                return Err(CommandError::invalid("max_edge must be 64..4096"));
            }
            let (target, lang) = view_of(app, p.target.clone(), p.lang.clone())?;
            let mut base = json!({ "target": target, "lang": lang });
            match p.variant.as_deref() {
                Some("") => base["variant"] = Value::Null,
                Some(v) => base["variant"] = json!(v),
                None => {}
            }
            let overlay = cmd(app, "overlay", base.clone())?;
            let images = overlay["images"].as_array().map_or(0, Vec::len);
            let mut info =
                json!({ "view": p.view, "target": target, "lang": lang, "images": images });
            let img = match p.view {
                PreviewView::Image => {
                    let index = match (p.page, p.image) {
                        (Some(_), Some(_)) => {
                            return Err(CommandError::invalid("give `image` or `page`, not both"));
                        }
                        (Some(page), None) => overlay["pages"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find(|x| x["index"].as_u64() == Some(u64::from(page)))
                            .ok_or_else(|| {
                                CommandError::invalid(format!("page {page} does not exist"))
                            })?["image"]
                            .as_u64()
                            .ok_or_else(|| {
                                CommandError::invalid(format!(
                                    "page {page} is not part of an output image of `{target}`"
                                ))
                            })?,
                        (None, i) => u64::from(i.unwrap_or(0)),
                    };
                    if index >= images as u64 {
                        return Err(CommandError::invalid(format!(
                            "image {index} does not exist (`{target}` has {images}, 0-based)"
                        )));
                    }
                    info["image"] = json!(index);
                    let mut args = base.clone();
                    args["image"] = json!(index);
                    args["max_edge"] = json!(p.max_edge);
                    image_cmd(app, "render_image", args)?
                }
                PreviewView::Canvas => {
                    let mut args = base.clone();
                    args["max_edge"] = json!(p.max_edge);
                    args["overlay"] = json!(true);
                    image_cmd(app, "render_canvas", args)?
                }
                PreviewView::Gallery => {
                    let mut args = base.clone();
                    args["height"] = json!((p.max_edge / 2).max(64));
                    image_cmd(app, "render_gallery", args)?
                }
            };
            info["width"] = json!(img.width);
            info["height"] = json!(img.height);
            let found = issues(app, &target, &lang)?;
            let n = found.as_array().map_or(0, Vec::len);
            info["issues"] = found;
            let line = format!(
                "{} of {target} ({lang}), {}x{} px, {n} issue(s)",
                match p.view {
                    PreviewView::Image =>
                        format!("image {}/{images}", info["image"].as_u64().unwrap_or(0) + 1),
                    PreviewView::Canvas => "canvas".to_owned(),
                    PreviewView::Gallery => "gallery".to_owned(),
                },
                img.width,
                img.height
            );
            Ok(ToolOutput::new(line, info).with_png(img.data.clone()))
        })
        .await
    }

    #[tool(
        description = "Exact layout of a target × language: per element its home page and box (lu relative to the page's top-left), rotation, `in_gap` (cut from page exports); per text the chosen font size, line count, content box, overflow, auto-hyphenated words and emergency breaks; per device its rotated `frame` corners and `screen` (unrotated box x/y/w/h, rotated `quad`, corner radius, `header` = height of the status bar / title zone) plus `screen_covered_by` (shares of screen and header); `gallery` (store gap and corner radius in lu/px, `measured`), `seams` (gap per page border, `next_page_offset` to express a canvas point on the next page); `overlaps` measured on drawn pixels (texts with other elements; `seam: true` = an element reaching across a page border over one of the next page). Use it to place cards relative to screens and to fix collisions precisely.",
        annotations(read_only_hint = true)
    )]
    async fn layout_report(
        &self,
        Parameters(p): Parameters<LayoutReportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let (target, lang) = view_of(app, p.target, p.lang)?;
            let view = json!({ "target": target, "lang": lang });
            let overlay = cmd(app, "overlay", view.clone())?;
            let mut args = view;
            if let Some(page) = p.page {
                args["page"] = json!(page);
            }
            let scene = cmd(app, "scene_report", args)?;
            let r = layout_report(&overlay, &scene, p.page);
            let texts = r["elements"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|e| e.get("text").is_some())
                .count();
            let overflow = r["elements"]
                .as_array()
                .into_iter()
                .flatten()
                .filter(|e| e["text"]["overflow"] == true)
                .count();
            let line = format!(
                "{target} ({lang}): {} element(s), {texts} text(s), {overflow} overflowing, {} overlap(s)",
                r["elements"].as_array().map_or(0, Vec::len),
                r["overlaps"].as_array().map_or(0, Vec::len),
            );
            Ok(ToolOutput::new(line, r))
        })
        .await
    }

    #[tool(
        description = "Check the project for every target × language: errors (block export: missing screenshots, wrong sizes, store rules), warnings (text overflow, emergency breaks, content in gaps, low text contrast, text touching devices), infos (fallbacks, text near devices, screen top bars covered or cut, seam overlaps, rounded store corners, small devices, pages blending with the store background). Rendered design checks run by default (pixel_checks: false = layout only, faster). Use preview: true while still editing. Intended findings: element field `allow: [code]`, project `checks.off`.",
        annotations(read_only_hint = true)
    )]
    async fn validate(
        &self,
        Parameters(p): Parameters<ValidateParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let v = cmd(
                app,
                "validate",
                json!({
                    "targets": p.targets, "langs": p.langs, "preview": p.preview,
                    "pixel_checks": p.pixel_checks,
                }),
            )?;
            let line = format!(
                "{} error(s), {} warning(s), {} info(s)",
                v["errors"], v["warnings"], v["infos"]
            );
            Ok(ToolOutput::new(line, v))
        })
        .await
    }

    #[tool(
        description = "Export the store files (pages, page groups, banners, gallery previews) for targets × languages into <project>/export (incremental: unchanged files are skipped; files of the same targets × languages it no longer produces are removed; writes an index.html overview unless html: false). layout: \"fastlane\" writes fastlane-ready folders instead (<project>/export/fastlane: screenshots/<asc-locale>/ for deliver, metadata/android/<play-locale>/images/<type>/ for supply; targets without a store slot are left out, upload-set conflicts are errors, see `notes`). Sends progress notifications when the client asks for them. Returns the folder, counts, every file (path, size, skipped) and issues. Example: {\"targets\": [\"ios-iphone-69\"], \"langs\": [\"en\", \"de\"]}."
    )]
    async fn export(
        &self,
        Parameters(p): Parameters<ExportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let (done_tx, done_rx) = tokio::sync::oneshot::channel::<()>();
        let forward = match (ctx.meta.get_progress_token(), self.progress()) {
            (Some(token), Some(relay)) => {
                let mut rx = relay.subscribe();
                let peer = ctx.peer.clone();
                Some(tokio::spawn(async move {
                    let send = |v: Value| {
                        let mut n = ProgressNotificationParam::new(
                            token.clone(),
                            v["done"].as_f64().unwrap_or(0.0),
                        );
                        if let Some(total) = v["total"].as_f64() {
                            n = n.with_total(total);
                        }
                        if let Some(path) = v["file"]["path"].as_str() {
                            n = n.with_message(path.to_owned());
                        }
                        let peer = peer.clone();
                        async move { peer.notify_progress(n).await.is_ok() }
                    };
                    let mut done_rx = done_rx;
                    loop {
                        tokio::select! {
                            v = rx.recv() => match v {
                                Some(v) => if !send(v).await { return },
                                None => return,
                            },
                            _ = &mut done_rx => break,
                        }
                    }
                    // The export finished: send what is still queued.
                    rx.close();
                    while let Some(v) = rx.recv().await {
                        if !send(v).await {
                            return;
                        }
                    }
                }))
            }
            _ => None,
        };
        let result = self
            .run(&ctx, move |app| {
                let mut args = json!({
                    "targets": p.targets, "langs": p.langs, "force": p.force,
                });
                if let Some(o) = p.out_dir {
                    args["out_dir"] = json!(o);
                }
                if let Some(g) = p.gallery {
                    args["gallery"] = json!(g);
                }
                if let Some(l) = p.layout {
                    args["layout"] = json!(l);
                }
                if let Some(h) = p.html {
                    args["html"] = json!(h);
                }
                let r = cmd(app, "export", args)?;
                let files: Vec<Value> = r["files"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .map(|f| {
                        json!({
                            "path": f["path"], "target": f["target"], "lang": f["lang"],
                            "size": format!("{}x{}", f["width"], f["height"]),
                            "skipped": f["skipped"],
                        })
                    })
                    .collect();
                let written = files.iter().filter(|f| f["skipped"] == false).count();
                let errors = r["errors"].as_array().map_or(0, Vec::len);
                let line = format!(
                    "exported {} file(s) to {} ({written} written, {} unchanged, {errors} error(s))",
                    files.len(),
                    r["out_dir"].as_str().unwrap_or_default(),
                    files.len() - written,
                );
                Ok(ToolOutput::new(
                    line,
                    json!({
                        "out_dir": r["out_dir"], "files": files, "written": written,
                        "issues": r["issues"], "errors": r["errors"], "cancelled": r["cancelled"],
                        "total_ms": r["total_ms"], "layout": r["layout"], "notes": r["notes"],
                        "removed": r["removed"], "index": r["index"],
                    }),
                ))
            })
            .await;
        let _ = done_tx.send(());
        if let Some(h) = forward {
            // Progress notifications go out before the result.
            let _ = h.await;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_is_page_relative_and_merges_scene_geometry() {
        let overlay = json!({
            "target": "t", "lang": "en", "variant": null,
            "pages": [
                {"index": 0, "id": "a", "image": 0, "rect": {"x": 0, "y": 0, "w": 1000, "h": 2000}},
                {"index": 1, "id": "b", "image": 1, "rect": {"x": 1100, "y": 0, "w": 1000, "h": 2000}},
            ],
            "elements": [
                {"id": "bg", "kind": "background", "page": 0, "rect": {"x": 0, "y": 0, "w": 2100, "h": 2000}, "rotation": 0, "in_gap": false},
                {"id": "dev", "kind": "device", "page": 1, "rect": {"x": 1300, "y": 500, "w": 600, "h": 1200}, "rotation": 5, "in_gap": false},
                {"id": "title", "kind": "text", "page": 1, "rect": {"x": 1180, "y": 100, "w": 840, "h": 500}, "rotation": 0, "in_gap": false,
                 "text": {"font_size": 80.04, "lines": 2, "overflow": false, "content": {"x": 1200, "y": 120, "w": 800, "h": 420}}},
            ],
        });
        let scene = json!({
            "gallery": {"gap_lu": 100.0, "gap_px": 132.0, "radius_lu": {"x": 125.0, "y": 125.0}, "measured": "no"},
            "seams": [{"after_page": 0, "x": 1000.0, "gap_lu": 100.0, "next_page_offset": -1100.0}],
            "devices": [{"id": "dev", "page": 1, "model": "iphone-island", "rotation": 5.0,
                         "frame": [], "screen": {"x": 230.0, "y": 520.0, "w": 540.0, "h": 1160.0}, "covered_by": []}],
            "overlaps": [{"text": "title", "other": "dev", "other_kind": "device", "area_lu2": 9000.0, "share_of_text": 0.05}],
        });
        let r = layout_report(&overlay, &scene, None);
        let title = &r["elements"][2];
        assert_eq!(title["box"]["x"], 80.0);
        assert_eq!(title["text"]["font_size"], 80.0);
        assert_eq!(title["text"]["content"]["x"], 100.0);
        assert_eq!(r["elements"][1]["rotation"], 5.0);
        assert_eq!(r["elements"][1]["screen"]["w"], 540.0);
        assert!(r["elements"][1].get("screen_covered_by").is_none());
        assert_eq!(r["overlaps"][0]["other"], "dev");
        assert_eq!(r["gallery"]["gap_lu"], 100.0);
        assert_eq!(r["seams"][0]["next_page_offset"], -1100.0);
        assert_eq!(
            layout_report(&overlay, &scene, Some(0))["elements"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
