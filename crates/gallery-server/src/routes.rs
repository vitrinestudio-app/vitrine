//! HTTP routes (API sketch: `docs/gallery/API.md`).

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{ConnectInfo, DefaultBodyLimit, FromRequestParts, Path, Query, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};
use vitrine_core::template::{Limits, Package, is_template_id};
use vitrine_render::template::{PREVIEW_HEIGHT, add_previews};

use crate::AppState;
use crate::auth;
use crate::db::{self, APPROVED, PENDING, REJECTED, TemplateRow};
use crate::error::{ApiError, ApiResult};

type St = State<Arc<AppState>>;

/// All routes.
pub fn router(state: Arc<AppState>) -> Router {
    let max = state.config.max_upload;
    let r = Router::new()
        .route("/health", get(health))
        .route("/v1/templates", get(list))
        .route(
            "/v1/templates",
            post(upload).layer(DefaultBodyLimit::max(max)),
        )
        .route("/v1/templates/{id}", get(detail))
        .route("/v1/templates/{id}/download", get(download))
        .route("/v1/templates/{id}/previews/{canvas}", get(preview))
        .route("/v1/templates/{id}/report", post(report))
        .route("/v1/admin/queue", get(admin_queue))
        .route("/v1/admin/templates/{id}/{action}", post(admin_action));
    #[cfg(feature = "github-oauth")]
    let r = r.merge(auth::github::routes());
    r.with_state(state)
}

/// Rate-limit key of the client: `X-Forwarded-For` (first hop) behind a
/// trusted proxy, else the peer address.
struct Client(String);

impl FromRequestParts<Arc<AppState>> for Client {
    type Rejection = std::convert::Infallible;

    #[allow(clippy::unused_async_trait_impl)]
    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let forwarded = state
            .config
            .trust_proxy
            .then(|| parts.headers.get("x-forwarded-for")?.to_str().ok())
            .flatten()
            .and_then(|v| v.split(',').next())
            .map(|v| v.trim().to_owned());
        let peer = parts
            .extensions
            .get::<ConnectInfo<SocketAddr>>()
            .map(|c| c.0.ip().to_string());
        Ok(Self(
            forwarded.or(peer).unwrap_or_else(|| "unknown".to_owned()),
        ))
    }
}

fn limit(state: &AppState, client: &Client, what: &str) -> ApiResult<()> {
    if state.limiter.allow(&format!("{what}:{}", client.0)) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many requests; try again later",
        ))
    }
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    q: Option<String>,
    tag: Option<String>,
    limit: Option<u32>,
    offset: Option<u32>,
}

fn with_links(state: &AppState, t: &TemplateRow) -> Value {
    let base = format!("{}/v1/templates/{}", state.config.public_url, t.id);
    let mut v = serde_json::to_value(t).unwrap_or(Value::Null);
    v["download_url"] = json!(format!("{base}/download"));
    if let Some(list) = v["canvases"].as_array_mut() {
        for c in list {
            if let Some(id) = c["id"].as_str() {
                c["preview_url"] = json!(format!("{base}/previews/{id}"));
            }
        }
    }
    if let Some(o) = v.as_object_mut() {
        o.remove("uploader");
    }
    v["uploader"] = json!(t.uploader);
    v
}

async fn list(State(state): St, Query(q): Query<ListQuery>) -> ApiResult<Json<Value>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 100);
    let offset = q.offset.unwrap_or(0);
    let rows = db::search(
        &state.db(),
        APPROVED,
        q.q.as_deref().filter(|s| !s.is_empty()),
        q.tag.as_deref().filter(|s| !s.is_empty()),
        limit,
        offset,
    )?;
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

async fn detail(State(state): St, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let t = approved(&state, &id)?;
    Ok(Json(with_links(&state, &t)))
}

async fn download(State(state): St, Path(id): Path<String>) -> ApiResult<Response> {
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
    Path((id, canvas)): Path<(String, String)>,
) -> ApiResult<Response> {
    let t = approved(&state, &id)?;
    let canvas = canvas.trim_end_matches(".png");
    let png = state
        .storage
        .preview(&t.id, canvas)
        .map_err(|_| ApiError::not_found("preview"))?;
    Ok(([(header::CONTENT_TYPE, "image/png")], png).into_response())
}

/// Server-side check of an upload: the same checks as the app, screenshots
/// replaced by generated placeholders, previews rendered here.
fn validate_upload(bytes: &[u8], max: usize) -> Result<Package, String> {
    let limits = Limits {
        max_package: max as u64,
        ..Limits::default()
    };
    let mut pkg = Package::from_zip_bytes(bytes, limits).map_err(|e| e.to_string())?;
    pkg.regenerate_placeholders().map_err(|e| e.to_string())?;
    add_previews(&mut pkg, PREVIEW_HEIGHT).map_err(|e| format!("cannot render previews: {e:#}"))?;
    Ok(pkg)
}

async fn upload(
    State(state): St,
    client: Client,
    headers: HeaderMap,
    body: Bytes,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let login = auth::uploader(&state, &headers)?;
    limit(&state, &client, "upload")?;
    let max = state.config.max_upload;
    let pkg = tokio::task::spawn_blocking(move || validate_upload(&body, max))
        .await
        .map_err(ApiError::internal)?
        .map_err(|e| ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, "invalid_template", e))?;
    let m = &pkg.manifest;
    if let Some(existing) = db::get(&state.db(), &m.id)? {
        if existing.uploader != login {
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
    let size = state.storage.put(&pkg).map_err(ApiError::internal)?;
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
        tags: json!(m.tags),
        version: m.version.clone(),
        license: vitrine_core::template::LICENSE,
        uploader: login,
        status: status.to_owned(),
        canvases: serde_json::to_value(&m.canvases).unwrap_or(Value::Null),
        fonts: json!(fonts),
        size: i64::try_from(size).unwrap_or(i64::MAX),
        downloads: 0,
        created_at: now,
        updated_at: now,
    };
    db::upsert(&state.db(), &row)?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "id": row.id, "version": row.version, "status": status })),
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

async fn admin_queue(State(state): St, headers: HeaderMap) -> ApiResult<Json<Value>> {
    auth::admin(&state, &headers)?;
    let db = state.db();
    let pending = db::search(&db, PENDING, None, None, 100, 0)?;
    let reports = db::open_reports(&db)?;
    Ok(Json(json!({ "pending": pending, "reports": reports })))
}

#[derive(Debug, Default, Deserialize)]
struct ModerationBody {
    #[serde(default)]
    note: String,
}

async fn admin_action(
    State(state): St,
    headers: HeaderMap,
    Path((id, action)): Path<(String, String)>,
    body: Option<Json<ModerationBody>>,
) -> ApiResult<Json<Value>> {
    auth::admin(&state, &headers)?;
    let status = match action.as_str() {
        "approve" => APPROVED,
        "reject" => REJECTED,
        _ => return Err(ApiError::not_found("action")),
    };
    let note = body.map(|b| b.0.note).unwrap_or_default();
    let db = state.db();
    if !db::set_status(&db, &id, status, &note)? {
        return Err(ApiError::not_found("template"));
    }
    db::resolve_reports(&db, &id)?;
    Ok(Json(json!({ "id": id, "status": status })))
}
