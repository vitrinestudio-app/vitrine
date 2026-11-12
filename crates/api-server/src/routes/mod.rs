//! HTTP routes, one module per API area (`docs/api/API.md`).

mod admin;
mod auth;
mod templates;

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, FromRequestParts, State};
use axum::http::StatusCode;
use axum::http::request::Parts;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::AppState;
use crate::error::{ApiError, ApiResult};

pub(crate) type St = State<Arc<AppState>>;

/// All routes.
pub fn router(state: Arc<AppState>) -> Router {
    let max = state.config.max_upload;
    Router::new()
        .route("/health", get(health))
        .merge(templates::routes(max))
        .merge(admin::routes())
        .merge(auth::routes())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::cors::layer,
        ))
        .with_state(state)
}

/// Rate-limit key of the client: `X-Forwarded-For` (first hop) behind a
/// trusted proxy, else the peer address.
pub(crate) struct Client(String);

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

pub(crate) fn limit(state: &AppState, client: &Client, what: &str) -> ApiResult<()> {
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

fn check(limiter: &crate::ratelimit::RateLimiter, client: &Client) -> ApiResult<()> {
    if limiter.allow(&client.0) {
        Ok(())
    } else {
        Err(ApiError::new(
            StatusCode::TOO_MANY_REQUESTS,
            "rate_limited",
            "too many requests; try again later",
        ))
    }
}

/// Rate limit of public reads (browsing stays open without login, but a
/// single client cannot hammer the server).
pub(crate) fn read_limit(state: &AppState, client: &Client) -> ApiResult<()> {
    check(&state.read_limiter, client)
}

/// Rate limit of login starts (each one makes the server call GitHub).
pub(crate) fn login_limit(state: &AppState, client: &Client) -> ApiResult<()> {
    check(&state.login_limiter, client)
}

/// Rate limit of device-flow polls.
pub(crate) fn poll_limit(state: &AppState, client: &Client) -> ApiResult<()> {
    check(&state.poll_limiter, client)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}
