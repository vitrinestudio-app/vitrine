//! Template gallery: list/search with tag filters, detail, download,
//! previews, sprites, the tag taxonomy, upload, abuse reports
//! (`docs/api/API.md`).

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde::Deserialize;
use serde_json::{Value, json};
use vitrine_core::template::tags::{self, taxonomy};
use vitrine_core::template::{Limits, Package, is_template_id};
use vitrine_render::template::{PREVIEW_HEIGHT, add_previews};

use super::{Client, St, limit, read_limit};
use crate::AppState;
use crate::auth;
use crate::db::{self, APPROVED, PENDING, TemplateRow};
use crate::error::{ApiError, ApiResult};
use crate::sprites::{self, CanvasSprites};

pub(super) fn routes(max_upload: usize) -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/templates", get(list))
        .route(
            "/v1/templates",
            post(upload).layer(DefaultBodyLimit::max(max_upload)),
        )
        .route("/v1/templates/{id}", get(detail))
        .route("/v1/templates/{id}/download", get(download))
        .route("/v1/templates/{id}/previews/{canvas}", get(preview))
        .route("/v1/templates/{id}/sprites/{file}", get(sprite))
        .route("/v1/templates/{id}/report", post(report))
        .route("/v1/tags", get(tag_list))
}

/// Largest `limit` of `GET /v1/templates` (the website loads the whole
/// gallery with a few requests).
pub(crate) const MAX_LIST_LIMIT: u32 = 200;

/// Facets that can filter the list.
const FILTER_FACETS: [&str; 4] = ["style", "color", "use", "devices"];

/// `GET /v1/templates` parameters: `q`, `tag`, `limit`, `offset` and the
/// facets (repeated and/or comma-separated: `style=bold&style=retro`,
/// `use=games,kids`).
fn parse_list_query(pairs: &[(String, String)]) -> ApiResult<(db::Filter, u32, u32)> {
    let mut filter = db::Filter::default();
    let (mut limit, mut offset) = (50u32, 0u32);
    let number = |k: &str, v: &str| {
        v.trim().parse::<u32>().map_err(|_| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_filter",
                format!("`{k}` must be a number"),
            )
        })
    };
    for (k, v) in pairs {
        match k.as_str() {
            "q" => filter.q = Some(v.clone()),
            "tag" => filter.tag = Some(v.trim().to_lowercase()),
            "limit" => limit = number(k, v)?,
            "offset" => offset = number(k, v)?,
            facet if FILTER_FACETS.contains(&facet) => {
                let Some(f) = taxonomy().facet(facet) else {
                    continue;
                };
                for value in v.split(',').map(|s| s.trim().to_lowercase()) {
                    if value.is_empty() {
                        continue;
                    }
                    if !f.has(&value) {
                        return Err(ApiError::new(
                            StatusCode::BAD_REQUEST,
                            "invalid_filter",
                            format!("unknown {facet} `{value}`; allowed: {}", f.allowed()),
                        ));
                    }
                    match filter.facets.iter_mut().find(|(n, _)| n == facet) {
                        Some((_, list)) => list.push(value),
                        None => filter.facets.push((facet.to_owned(), vec![value])),
                    }
                }
            }
            _ => {}
        }
    }
    Ok((filter, limit.clamp(1, MAX_LIST_LIMIT), offset))
}

pub(super) fn with_links(state: &AppState, t: &TemplateRow) -> Value {
    let base = format!("{}/v1/templates/{}", state.config.public_url, t.id);
    let mut v = serde_json::to_value(t).unwrap_or(Value::Null);
    v["download_url"] = json!(format!("{base}/download"));
    if let Some(list) = v["canvases"].as_array_mut() {
        for c in list {
            let Some(id) = c["id"].as_str().map(str::to_owned) else {
                continue;
            };
            c["preview_url"] = json!(format!("{base}/previews/{id}"));
            c["page_aspect"] = c["aspect"].clone();
            c["sprite"] = sprites::meta_of(&t.sprites, &id)
                .map_or(Value::Null, |m| sprites::api_object(&base, &m));
        }
    }
    if !v["tags"].is_object() {
        // Rows not migrated yet.
        v["tags"] = json!({ "style": [], "color": [], "use": [] });
    }
    v["official"] = json!(state.config.is_official(&t.uploader));
    v
}

async fn list(
    State(state): St,
    client: Client,
    Query(pairs): Query<Vec<(String, String)>>,
) -> ApiResult<Json<Value>> {
    read_limit(&state, &client)?;
    let (filter, limit, offset) = parse_list_query(&pairs)?;
    let rows = db::search(&state.db(), APPROVED, &filter, limit, offset)?;
    let items: Vec<Value> = rows.iter().map(|t| with_links(&state, t)).collect();
    Ok(Json(
        json!({ "templates": items, "limit": limit, "offset": offset }),
    ))
}

fn approved(state: &AppState, id: &str) -> ApiResult<TemplateRow> {
    if !is_template_id(id) {
        return Err(ApiError::not_found("template"));
    }
    match db::get(&state.db(), id)? {
        Some(t) if t.status == APPROVED => Ok(t),
        _ => Err(ApiError::not_found("template")),
    }
}

async fn detail(
    State(state): St,
    client: Client,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    read_limit(&state, &client)?;
    let t = approved(&state, &id)?;
    Ok(Json(with_links(&state, &t)))
}

async fn download(State(state): St, client: Client, Path(id): Path<String>) -> ApiResult<Response> {
    read_limit(&state, &client)?;
    let t = approved(&state, &id)?;
    let bytes = state
        .storage
        .package(&t.id)
        .map_err(|_| ApiError::not_found("template file"))?;
    db::count_download(&state.db(), &t.id)?;
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/zip"),
    );
    let disposition = format!("attachment; filename=\"{}.vitrine-template\"", t.id);
    if let Ok(v) = HeaderValue::from_str(&disposition) {
        headers.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok((headers, bytes).into_response())
}

async fn preview(
    State(state): St,
    client: Client,
    Path((id, canvas)): Path<(String, String)>,
) -> ApiResult<Response> {
    read_limit(&state, &client)?;
    let t = approved(&state, &id)?;
    let canvas = canvas.trim_end_matches(".png");
    let png = state
        .storage
        .preview(&t.id, canvas)
        .map_err(|_| ApiError::not_found("preview"))?;
    Ok(([(header::CONTENT_TYPE, "image/png")], png).into_response())
}

async fn sprite(
    State(state): St,
    client: Client,
    headers: HeaderMap,
    Path((id, file)): Path<(String, String)>,
    Query(q): Query<Vec<(String, String)>>,
) -> ApiResult<Response> {
    read_limit(&state, &client)?;
    let t = approved(&state, &id)?;
    let (canvas, double) =
        sprites::parse_file_name(&file).ok_or_else(|| ApiError::not_found("sprite"))?;
    let meta = sprites::meta_of(&t.sprites, canvas).ok_or_else(|| ApiError::not_found("sprite"))?;
    let tag = if double { &meta.etag_2x } else { &meta.etag };
    let etag = format!("\"{tag}\"");
    // A `?v=` naming the current content may be cached for good; plain
    // URLs revalidate after an hour.
    let versioned = q.iter().any(|(k, v)| k == "v" && v == tag);
    let cache = if versioned {
        "public, max-age=31536000, immutable"
    } else {
        "public, max-age=3600"
    };
    let mut out = HeaderMap::new();
    out.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    if let Ok(v) = HeaderValue::from_str(&etag) {
        out.insert(header::ETAG, v);
    }
    let matches = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.split(',').any(|e| e.trim() == etag || e.trim() == "*"));
    if matches {
        return Ok((StatusCode::NOT_MODIFIED, out).into_response());
    }
    let bytes = state
        .storage
        .sprite(&t.id, canvas, double)
        .map_err(|_| ApiError::not_found("sprite"))?;
    out.insert(header::CONTENT_TYPE, HeaderValue::from_static("image/webp"));
    Ok((out, bytes).into_response())
}

/// `GET /v1/tags`: the taxonomy (facets, values, labels en/de, counts).
async fn tag_list(State(state): St, client: Client) -> ApiResult<Json<Value>> {
    read_limit(&state, &client)?;
    let facets: Vec<Value> = taxonomy()
        .facets
        .iter()
        .map(|f| {
            json!({
                "id": f.id,
                "label": f.label,
                "min": f.min,
                "max": f.max,
                "derived": f.derived,
                "values": f.values.iter().map(|v| json!({"id": v.id, "label": v.label})).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(Json(json!({ "facets": facets })))
}

/// Why an upload was refused: `(code, message)`.
type Refusal = (&'static str, String);

/// What the server made of an upload.
struct Checked {
    pkg: Package,
    sprites: Vec<CanvasSprites>,
    devices: Vec<String>,
    /// The manifest's canvases with per-canvas `devices`.
    canvases: Value,
    /// Sample screenshots dropped because the uploader did not confirm.
    samples_removed: usize,
}

/// Server-side check of an upload: the same checks as the app, complete
/// tags, screenshots replaced by generated placeholders (confirmed sample
/// screenshots kept), previews and sprites rendered here.
fn validate_upload(bytes: &[u8], max: usize, confirm_samples: bool) -> Result<Checked, Refusal> {
    let limits = Limits {
        max_package: max as u64,
        ..Limits::default()
    };
    let invalid = |e: String| ("invalid_template", e);
    let mut pkg = Package::from_zip_bytes(bytes, limits).map_err(|e| invalid(e.to_string()))?;
    let t = &pkg.manifest.tags;
    if !t.dropped.is_empty() {
        return Err((
            "invalid_tags",
            format!(
                "unknown tags {:?}: use the facets style, color and use (`vitrine template tags`)",
                t.dropped
            ),
        ));
    }
    t.check(true).map_err(|e| ("invalid_tags", e.0))?;
    let samples = pkg.manifest.samples.len();
    pkg.regenerate_placeholders(confirm_samples)
        .map_err(|e| invalid(e.to_string()))?;
    add_previews(&mut pkg, PREVIEW_HEIGHT)
        .map_err(|e| invalid(format!("cannot render previews: {e:#}")))?;
    let sprites =
        sprites::render(&pkg).map_err(|e| invalid(format!("cannot render sprites: {e:#}")))?;
    let project = pkg.project().map_err(|e| invalid(e.to_string()))?;
    let devices = tags::devices(&project);
    let canvases = crate::maintenance::canvases_json(&project, &pkg.manifest.canvases);
    Ok(Checked {
        pkg,
        sprites,
        devices,
        canvases,
        samples_removed: if confirm_samples { 0 } else { samples },
    })
}

async fn upload(
    State(state): St,
    client: Client,
    headers: HeaderMap,
    Query(q): Query<Vec<(String, String)>>,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let who = auth::uploader(&state, &headers)?;
    limit(&state, &client, "upload")?;
    let max = state.config.max_upload;
    let confirm = q
        .iter()
        .any(|(k, v)| k == "confirm_own_screenshots" && matches!(v.trim(), "1" | "true" | "yes"));
    let checked = {
        let _slot = state.render_slot().await;
        tokio::task::spawn_blocking(move || validate_upload(&body, max, confirm))
            .await
            .map_err(ApiError::internal)?
            .map_err(|(code, e)| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, code, e))?
    };
    let pkg = &checked.pkg;
    let m = &pkg.manifest;
    if let Some(existing) = db::get(&state.db(), &m.id)? {
        if !who.owns(&existing.uploader, existing.uploader_id) {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "id_taken",
                format!("the template id `{}` belongs to another uploader", m.id),
            ));
        }
        if existing.version == m.version {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "version_exists",
                format!(
                    "version {} of `{}` exists; raise the version",
                    m.version, m.id
                ),
            ));
        }
    }
    let size = state.storage.put(pkg).map_err(ApiError::internal)?;
    state
        .storage
        .put_sprites(&m.id, &checked.sprites)
        .map_err(ApiError::internal)?;
    let status = if state.config.auto_approve {
        APPROVED
    } else {
        PENDING
    };
    let now = db::now();
    let fonts: Vec<&str> = m.fonts.iter().map(|f| f.family.as_str()).collect();
    let row = TemplateRow {
        id: m.id.clone(),
        name: m.name.clone(),
        description: m.description.clone(),
        author: m.author.clone(),
        tags: db::tags_json(&m.tags),
        version: m.version.clone(),
        license: vitrine_core::template::LICENSE,
        uploader: who.login,
        uploader_id: who.github_id,
        status: status.to_owned(),
        canvases: checked.canvases.clone(),
        fonts: json!(fonts),
        size: i64::try_from(size).unwrap_or(i64::MAX),
        downloads: 0,
        created_at: now,
        updated_at: now,
        devices: json!(checked.devices),
        samples: i64::try_from(m.samples.len()).unwrap_or(0),
        sprites: sprites::metas_json(&checked.sprites),
    };
    db::upsert(&state.db(), &row)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": row.id,
            "version": row.version,
            "status": status,
            "samples": row.samples,
            "samples_removed": checked.samples_removed,
        })),
    ))
}

#[derive(Debug, Deserialize)]
struct ReportBody {
    /// `copyright`, `trademark`, `offensive`, `malware`, `other`.
    reason: String,
    #[serde(default)]
    message: String,
}

const REASONS: [&str; 5] = ["copyright", "trademark", "offensive", "malware", "other"];

async fn report(
    State(state): St,
    client: Client,
    Path(id): Path<String>,
    Json(body): Json<ReportBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    limit(&state, &client, "report")?;
    let t = approved(&state, &id)?;
    if !REASONS.contains(&body.reason.as_str()) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_reason",
            format!("reason must be one of {}", REASONS.join(", ")),
        ));
    }
    let message: String = body.message.chars().take(2000).collect();
    db::add_report(&state.db(), &t.id, &body.reason, &message)?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "received": true }))))
}
