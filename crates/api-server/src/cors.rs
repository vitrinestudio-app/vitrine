//! CORS for the public, read-only routes (`VITRINE_API_CORS_ORIGINS`): the
//! website (`https://www.vitrinestudio.app`) fetches the template list and
//! the tag taxonomy from the browser.
//!
//! Only `GET`/`HEAD` of `/health`, `/v1/templates…` and `/v1/tags` get
//! `Access-Control-Allow-Origin`, and only for an origin on the allow
//! list (echoed, never `*`, never credentials). A preflight (`OPTIONS` with
//! `Access-Control-Request-Method: GET|HEAD`) of those paths is answered
//! 204. Uploads, reports, login and moderation never get CORS headers.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;

/// Paths a browser on an allowed origin may read.
fn is_public_read(path: &str) -> bool {
    path == "/health"
        || path == "/v1/tags"
        || path == "/v1/templates"
        || (path.starts_with("/v1/templates/") && !path.ends_with("/report"))
}

/// The request's `Origin` when it is on the allow list.
fn allowed_origin(state: &AppState, headers: &HeaderMap) -> Option<HeaderValue> {
    let origin = headers.get(header::ORIGIN)?;
    let text = origin.to_str().ok()?;
    state
        .config
        .cors_origins
        .iter()
        .any(|o| o.eq_ignore_ascii_case(text))
        .then(|| origin.clone())
}

pub(crate) async fn layer(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Response {
    if state.config.cors_origins.is_empty() || !is_public_read(req.uri().path()) {
        return next.run(req).await;
    }
    let origin = allowed_origin(&state, req.headers());
    let method = req.method().clone();
    if method == Method::OPTIONS {
        let wanted = req
            .headers()
            .get(header::ACCESS_CONTROL_REQUEST_METHOD)
            .and_then(|v| v.to_str().ok());
        if let (Some(origin), Some("GET" | "HEAD")) = (origin, wanted) {
            let mut res = StatusCode::NO_CONTENT.into_response();
            let h = res.headers_mut();
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
            h.insert(
                header::ACCESS_CONTROL_ALLOW_METHODS,
                HeaderValue::from_static("GET, HEAD"),
            );
            h.insert(
                header::ACCESS_CONTROL_MAX_AGE,
                HeaderValue::from_static("86400"),
            );
            h.insert(header::VARY, HeaderValue::from_static("Origin"));
            return res;
        }
        return next.run(req).await;
    }
    let mut res = next.run(req).await;
    if method == Method::GET || method == Method::HEAD {
        let h = res.headers_mut();
        // The answer depends on the Origin: caches must keep them apart.
        h.append(header::VARY, HeaderValue::from_static("Origin"));
        if let Some(origin) = origin {
            h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, origin);
        }
    }
    res
}
