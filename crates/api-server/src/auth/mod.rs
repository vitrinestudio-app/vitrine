//! Authentication (ADR 0025).
//!
//! - **Uploaders** sign in with GitHub ([`github`]): the app/CLI uses the
//!   device flow, a website the web flow. The server reads the GitHub
//!   identity once (login + numeric id), discards the GitHub access token
//!   and issues its own session token ([`session`]: random, stored as a
//!   SHA-256 hash, expires, revocable with `POST /v1/auth/logout`).
//! - **Static upload tokens** (`VITRINE_API_UPLOAD_TOKENS`) issued by the
//!   owner for accounts that cannot use OAuth (the org `vitrinestudio-app`)
//!   and testers.
//! - **Admin**: `VITRINE_API_ADMIN_TOKEN` as bearer token.

pub mod github;
pub mod session;

use axum::http::HeaderMap;
use axum::http::StatusCode;

use crate::AppState;
use crate::error::ApiError;

pub fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::trim)
        .filter(|t| !t.is_empty())
}

/// Constant-time comparison (tokens are short; avoids early exit).
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Who is uploading.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Uploader {
    /// GitHub login (shown as the template's uploader).
    pub login: String,
    /// Numeric GitHub user id (stable across renames); `None` for static
    /// tokens configured without one.
    pub github_id: Option<i64>,
    /// Signed in with a session token (revocable), not a static token.
    pub session: bool,
}

impl Uploader {
    /// May this uploader update a template owned by `login`/`id`? The
    /// numeric GitHub ids decide when both are known (a renamed account
    /// keeps its templates), else the logins (case-insensitive).
    pub fn owns(&self, login: &str, id: Option<i64>) -> bool {
        match (self.github_id, id) {
            (Some(a), Some(b)) => a == b,
            _ => self.login.eq_ignore_ascii_case(login),
        }
    }
}

/// The uploader for the request's bearer token: a static token or a live
/// session.
pub fn uploader(state: &AppState, headers: &HeaderMap) -> Result<Uploader, ApiError> {
    let token = bearer(headers).ok_or_else(ApiError::unauthorized)?;
    if let Some(t) = state
        .config
        .upload_tokens
        .iter()
        .find(|t| same(&t.token, token))
    {
        return Ok(Uploader {
            login: t.login.clone(),
            github_id: t.github_id,
            session: false,
        });
    }
    let s = session::lookup(&state.db(), token)?.ok_or_else(ApiError::unauthorized)?;
    Ok(Uploader {
        login: s.login,
        github_id: Some(s.github_id),
        session: true,
    })
}

/// Checks the admin token.
pub fn admin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(expected) = &state.config.admin_token else {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "moderation_disabled",
            "moderation is disabled (VITRINE_API_ADMIN_TOKEN is not set)",
        ));
    };
    match bearer(headers) {
        Some(t) if same(t, expected) => Ok(()),
        _ => Err(ApiError::unauthorized()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_prefers_the_github_id() {
        let me = Uploader {
            login: "new-name".into(),
            github_id: Some(7),
            session: true,
        };
        assert!(me.owns("old-name", Some(7)));
        assert!(!me.owns("new-name", Some(8)));
        assert!(me.owns("New-Name", None));
        let static_token = Uploader {
            login: "octocat".into(),
            github_id: None,
            session: false,
        };
        assert!(static_token.owns("OctoCat", Some(1)));
        assert!(!static_token.owns("other", Some(1)));
    }
}
