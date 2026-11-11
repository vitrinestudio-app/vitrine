//! Authentication.
//!
//! - **Uploaders**: GitHub OAuth (planned, ADR 0023). The app would use the
//!   device flow (`POST /v1/auth/github/device` → user code, then
//!   `POST /v1/auth/github/token` polling), the website the web flow
//!   (`GET /v1/auth/github/login` → GitHub → `/v1/auth/github/callback`);
//!   the server exchanges the code, reads the GitHub login and issues its
//!   own session token (random, stored hashed, revocable). TODO: implement
//!   behind the `github-oauth` feature; today those routes answer 501 and
//!   upload tokens come from `GALLERY_UPLOAD_TOKENS`.
//! - **Admin**: `GALLERY_ADMIN_TOKEN` as bearer token.

use axum::http::HeaderMap;

use crate::AppState;
use crate::error::ApiError;

fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
}

/// Constant-time comparison (tokens are short; avoids early exit).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// The uploader's login for the request's bearer token.
pub fn uploader(state: &AppState, headers: &HeaderMap) -> Result<String, ApiError> {
    let token = bearer(headers).ok_or_else(ApiError::unauthorized)?;
    state
        .config
        .upload_tokens
        .iter()
        .find(|(t, _)| same(t, token))
        .map(|(_, login)| login.clone())
        .ok_or_else(ApiError::unauthorized)
}

/// Checks the admin token.
pub fn admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = &state.config.admin_token else {
        return Err(ApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            "moderation_disabled",
            "moderation is disabled (GALLERY_ADMIN_TOKEN is not set)",
        ));
    };
    match bearer(headers) {
        Some(t) if same(t, expected) => Ok(()),
        _ => Err(ApiError::unauthorized()),
    }
}

#[cfg(feature = "github-oauth")]
pub mod github {
    //! TODO(github-oauth): device and web flow (see the module docs). Stubs.

    use axum::Router;
    use axum::http::StatusCode;
    use axum::routing::{get, post};

    use crate::error::ApiError;

    async fn not_yet() -> ApiError {
        ApiError::new(
            StatusCode::NOT_IMPLEMENTED,
            "not_implemented",
            "GitHub login is not implemented yet",
        )
    }

    pub fn routes<S: Clone + Send + Sync + 'static>() -> Router<S> {
        Router::new()
            .route("/v1/auth/github/device", post(not_yet))
            .route("/v1/auth/github/token", post(not_yet))
            .route("/v1/auth/github/login", get(not_yet))
            .route("/v1/auth/github/callback", get(not_yet))
    }
}
