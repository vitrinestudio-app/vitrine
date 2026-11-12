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
    /// Longer edge of the image in px (default 1200, 64..4096). Gallery:
    /// the image height is half of it; a gallery wider than 4096 px
    /// (8192 px with `inline: false`) is rendered lower instead.
    #[serde(default = "default_max_edge")]
    pub max_edge: u32,
    /// view gallery: page colour around the images – `light` (default:
    /// the store's light page), `dark` (the store's dark mode) or
    /// `#RRGGBB`. Use it to check that the pages stand out from the store.
    #[serde(default)]
    pub background: Option<String>,
    /// `png` (default, lossless) or `jpeg` (much smaller for photos and
    /// wide galleries; quality 85).
    #[serde(default)]
    pub format: Option<PreviewFormat>,
    /// Also write the image to this file (relative to the project folder,
    /// or absolute; folders are created), e.g. `preview/gallery.png` for a
    /// README.
    #[serde(default)]
    pub out_file: Option<PathBuf>,
    /// Return the image in the result (default true). `false` with
    /// `out_file`: only write the file (saves transport for big images).
    #[serde(default = "yes")]
    pub inline: bool,
}

fn yes() -> bool {
    true
}

/// Image encoding of `render_preview`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, serde::Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreviewFormat {
    #[default]
    Png,
    Jpeg,
}

/// Widest inline gallery preview (px); files may be twice as wide.
pub const MAX_INLINE_GALLERY_WIDTH: u32 = 4096;

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
    /// `false`: write only the `_gallery.png` previews, no page images
    /// (fast check of whole sets; earlier page files are kept). Default true.
    #[serde(default)]
    pub pages: Option<bool>,
    /// Seconds to wait for the export (default 45, 0..=600). A longer
    /// export keeps running in the background: the result then has
    /// `status: running` and a `job` id for `export_status`.
    #[serde(default)]
    pub wait_seconds: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ExportStatusParams {
    /// Job id from `export` (default: the last export).
    #[serde(default)]
    pub job: Option<String>,
    /// Seconds to wait for the export to finish (default 30, 0..=600).
    #[serde(default)]
    pub wait_seconds: Option<u32>,
    /// Stop the running export (files already written stay).
    #[serde(default)]
    pub cancel: bool,
}

/// Default wait of `export` (s): below the usual client timeouts.
pub const EXPORT_WAIT_SECONDS: u32 = 45;
/// Default wait of `export_status` (s).
pub const EXPORT_STATUS_WAIT_SECONDS: u32 = 30;

/// `wait_seconds` checked (0..=600) with a default.
fn wait_of(v: Option<u32>, default: u32) -> Result<std::time::Duration, CommandError> {
    let s = v.unwrap_or(default);
    if s > 600 {
        return Err(CommandError::invalid("wait_seconds must be 0..600"));
    }
    Ok(std::time::Duration::from_secs(u64::from(s)))
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

/// Area of the intersection of two canvas rects.
fn overlap_area(r1: &Value, r2: &Value) -> f64 {
    let get = |r: &Value, k: &str| r[k].as_f64().unwrap_or(0.0);
    let span = |pos: &str, len: &str| {
        (get(r1, pos) + get(r1, len)).min(get(r2, pos) + get(r2, len))
            - get(r1, pos).max(get(r2, pos))
    };
    let (width, height) = (span("x", "w"), span("y", "h"));
    if width > 0.0 && height > 0.0 {
        width * height
    } else {
        0.0
    }
}

/// Layout report from the overlay of a view: page-relative boxes, text
/// metrics, gaps and overlaps of texts with other elements.
pub fn layout_report(overlay: &Value, page_filter: Option<u32>) -> Value {
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
    let all: Vec<&Value> = overlay["elements"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let mut elements = Vec::new();
    for e in &all {
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
        elements.push(item);
    }
    // Texts overlapping other visible elements (not their own groups).
    let mut overlaps = Vec::new();
    for (i, a) in all.iter().enumerate() {
        let Some(at) = a.get("text").filter(|t| t.is_object()) else {
            continue;
        };
        for b in all.iter().skip(i + 1).chain(all.iter().take(i)) {
            if b["kind"] == "background" || b["kind"] == "group" || b["id"] == a["id"] {
                continue;
            }
            let b_is_text = b.get("text").is_some_and(Value::is_object);
            if b_is_text && b["id"].as_str() < a["id"].as_str() {
                continue; // each text pair once
            }
            let br = if b_is_text {
                &b["text"]["content"]
            } else {
                &b["rect"]
            };
            let area = overlap_area(&at["content"], br);
            if area <= 1.0 {
                continue;
            }
            if page_filter.is_some_and(|f| {
                a["page"].as_u64() != Some(u64::from(f)) && b["page"].as_u64() != Some(u64::from(f))
            }) {
                continue;
            }
            let text_area = at["content"]["w"].as_f64().unwrap_or(0.0)
                * at["content"]["h"].as_f64().unwrap_or(0.0);
            overlaps.push(json!({
                "text": a["id"], "other": b["id"], "other_kind": b["kind"],
                "area_lu2": area.round(),
                "share_of_text": if text_area > 0.0 { json!(((area / text_area) * 100.0).round() / 100.0) } else { Value::Null },
            }));
        }
    }
    json!({
        "target": overlay["target"], "lang": overlay["lang"], "variant": overlay["variant"],
        "pages": pages.iter().map(|p| json!({
            "index": p["index"], "id": p["id"], "image": p["image"],
            "size": {"w": r1(&p["rect"]["w"]), "h": r1(&p["rect"]["h"])},
        })).collect::<Vec<_>>(),
        "elements": elements,
        "overlaps": overlaps,
    })
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
        description = "Render a PNG preview with the export renderer and return it as an image – look at it after every visual change. view: image (one output image as exported; pick `image` index or `page`), canvas (whole canvas with page borders and hatched gaps), gallery (all images with store gaps and corner radius; background: dark shows the store's dark mode). format: jpeg for smaller transfers; out_file also writes the image into the project (inline: false = only the file). Also returns size, the number of output images and errors/warnings of this target × language. Example: {\"target\": \"ios-iphone-69\", \"lang\": \"de\", \"page\": 2}.",
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
            if !p.inline && p.out_file.is_none() {
                return Err(CommandError::invalid(
                    "inline: false needs out_file (else there is nothing to return)",
                ));
            }
            if p.background.is_some() && p.view != PreviewView::Gallery {
                return Err(CommandError::invalid(
                    "background is only used with view: gallery",
                ));
            }
            let format = p.format.unwrap_or_default();
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
                    args["format"] = json!(format);
                    args["image"] = json!(index);
                    args["max_edge"] = json!(p.max_edge);
                    image_cmd(app, "render_image", args)?
                }
                PreviewView::Canvas => {
                    let mut args = base.clone();
                    args["format"] = json!(format);
                    args["max_edge"] = json!(p.max_edge);
                    args["overlay"] = json!(true);
                    image_cmd(app, "render_canvas", args)?
                }
                PreviewView::Gallery => {
                    let mut args = base.clone();
                    args["format"] = json!(format);
                    let height = (p.max_edge / 2).max(64);
                    args["height"] = json!(height);
                    args["max_width"] = json!(if p.inline {
                        MAX_INLINE_GALLERY_WIDTH
                    } else {
                        2 * MAX_INLINE_GALLERY_WIDTH
                    });
                    if let Some(bg) = &p.background {
                        args["background"] = json!(bg);
                        info["background"] = json!(bg);
                    }
                    let img = image_cmd(app, "render_gallery", args)?;
                    if img.height + 1 < height {
                        info["note"] = json!(format!(
                            "the gallery is wide: rendered {} px high instead of {height} to stay within {} px width; look at single images (view: image) for details",
                            img.height, img.width
                        ));
                    }
                    img
                }
            };
            info["width"] = json!(img.width);
            info["height"] = json!(img.height);
            info["format"] = json!(format);
            if let Some(out) = &p.out_file {
                let dir = app.session()?.dir().to_owned();
                let path = if out.is_absolute() {
                    out.clone()
                } else {
                    dir.join(out)
                };
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        CommandError::io(format!("{}: {e}", parent.display()))
                    })?;
                }
                std::fs::write(&path, &img.data)
                    .map_err(|e| CommandError::io(format!("{}: {e}", path.display())))?;
                info["file"] = json!(path);
                info["bytes"] = json!(img.data.len());
            }
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
            let out = ToolOutput::new(line, info);
            Ok(if p.inline {
                out.with_image(img.data.clone(), img.format.content_type())
            } else {
                out
            })
        })
        .await
    }

    #[tool(
        description = "Exact layout of a target × language: per element its home page and box (lu relative to the page's top-left), rotation, `in_gap` (cut from page exports); per text the chosen font size, line count, content box, overflow, auto-hyphenated words and emergency breaks; plus texts overlapping other elements. Use it to fix overflows and collisions precisely.",
        annotations(read_only_hint = true)
    )]
    async fn layout_report(
        &self,
        Parameters(p): Parameters<LayoutReportParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        self.run(&ctx, move |app| {
            let (target, lang) = view_of(app, p.target, p.lang)?;
            let overlay = cmd(app, "overlay", json!({ "target": target, "lang": lang }))?;
            let r = layout_report(&overlay, p.page);
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
        description = "Check the project for every target × language: errors (block export: missing screenshots, wrong sizes, store rules), warnings (text overflow, emergency breaks, content in gaps), infos (fallbacks). Use preview: true while still editing.",
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
                json!({ "targets": p.targets, "langs": p.langs, "preview": p.preview }),
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
        description = "Export the store files (pages, page groups, banners, gallery previews) for targets × languages into <project>/export (incremental: unchanged files are skipped; files of the same targets × languages it no longer produces are removed; writes an index.html overview unless html: false). layout: \"fastlane\" writes fastlane-ready folders instead (<project>/export/fastlane: screenshots/<asc-locale>/ for deliver, metadata/android/<play-locale>/images/<type>/ for supply; targets without a store slot are left out, upload-set conflicts are errors, see `notes`). pages: false writes only the _gallery.png previews (quick check of whole sets). Waits at most wait_seconds (default 45): a longer export continues in the background and the result says status: running with a `job` id – then call export_status {job} until it is finished. Sends progress notifications when the client asks for them. Returns the folder, counts, every file (path, size, skipped) and issues. Example: {\"targets\": [\"ios-iphone-69\"], \"langs\": [\"en\", \"de\"]}."
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
        let jobs = self.exports().clone();
        let relay = self.progress().cloned();
        let owned = self.app().clone();
        let result = self
            .run(&ctx, move |_| {
                let wait = wait_of(p.wait_seconds, EXPORT_WAIT_SECONDS)?;
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
                if let Some(pages) = p.pages {
                    args["pages"] = json!(pages);
                }
                // On its own thread: a long export outlives this call.
                let actor = vitrine_app::Actor::current();
                let job = jobs.start(relay.as_ref(), move || {
                    actor.scope(|| export_output(&owned, args))
                })?;
                match job.wait(wait) {
                    Some(r) => crate::jobs::finished_output(&job, r),
                    None => Ok(job.running_output()),
                }
            })
            .await;
        let _ = done_tx.send(());
        if let Some(h) = forward {
            // Progress notifications go out before the result.
            let _ = h.await;
        }
        result
    }

    #[tool(
        description = "State of a background export (started by `export` when it took longer than its wait_seconds): progress (done/total files) while it runs, the full export result once finished. Waits up to wait_seconds (default 30) for the end. cancel: true stops it. Example: {\"job\": \"export-1\"}.",
        annotations(read_only_hint = true)
    )]
    async fn export_status(
        &self,
        Parameters(p): Parameters<ExportStatusParams>,
        ctx: RequestContext<RoleServer>,
    ) -> CallToolResult {
        let jobs = self.exports().clone();
        self.run_unsynced(&ctx, move |app| {
            let wait = wait_of(p.wait_seconds, EXPORT_STATUS_WAIT_SECONDS)?;
            let Some(job) = jobs.get(p.job.as_deref()) else {
                return Err(CommandError::not_found(match &p.job {
                    Some(j) => format!("no export `{j}` (only the last export is kept)"),
                    None => "no export was started in this session".to_owned(),
                }));
            };
            if p.cancel && !job.is_finished() {
                cmd(app, "export_cancel", Value::Null)?;
            }
            match job.wait(wait) {
                Some(r) => crate::jobs::finished_output(&job, r),
                None => Ok(job.running_output()),
            }
        })
        .await
    }
}

/// Runs the app's `export` command and shapes its report for the model.
fn export_output(app: &AppState, args: Value) -> Result<ToolOutput, CommandError> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_is_page_relative_and_finds_overlaps() {
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
        let r = layout_report(&overlay, None);
        let title = &r["elements"][2];
        assert_eq!(title["box"]["x"], 80.0);
        assert_eq!(title["text"]["font_size"], 80.0);
        assert_eq!(title["text"]["content"]["x"], 100.0);
        assert_eq!(r["elements"][1]["rotation"], 5.0);
        let o = r["overlaps"].as_array().unwrap();
        assert_eq!(o.len(), 1);
        assert_eq!(o[0]["other"], "dev");
        assert_eq!(o[0]["area_lu2"], 600.0 * 40.0);
        assert_eq!(
            layout_report(&overlay, Some(0))["elements"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn overlap_area_of_disjoint_rects_is_zero() {
        let a = json!({"x": 0, "y": 0, "w": 10, "h": 10});
        let b = json!({"x": 20, "y": 0, "w": 10, "h": 10});
        assert!(overlap_area(&a, &b).abs() < f64::EPSILON);
    }
}
