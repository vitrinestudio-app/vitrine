//! Login with GitHub (device flow for the app/CLI, web flow for a website),
//! session info and logout (ADR 0025).

use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{Client, St, login_limit, poll_limit};
use crate::AppState;
use crate::auth::github::{GithubError, Poll, pkce};
use crate::auth::{self, session};
use crate::error::{ApiError, ApiResult};

/// Cookie binding the web-flow state to the browser (login CSRF).
const STATE_COOKIE: &str = "vitrine_oauth_state";
const COOKIE_PATH: &str = "/v1/auth/github";

pub(super) fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/auth/github/device", post(device))
        .route("/v1/auth/github/token", post(token))
        .route("/v1/auth/github/login", get(web_login))
        .route("/v1/auth/github/callback", get(callback))
        .route("/v1/auth/me", get(me))
        .route("/v1/auth/logout", post(logout))
}

fn github_error(e: &GithubError) -> ApiError {
    match e {
        GithubError::OAuth(code, _) => match code.as_str() {
            "expired_token" | "token_expired" => ApiError::new(
                StatusCode::BAD_REQUEST,
                "expired_token",
                "the device code expired; start the login again",
            ),
            "access_denied" => ApiError::new(
                StatusCode::FORBIDDEN,
                "access_denied",
                "the login was cancelled on GitHub",
            ),
            "incorrect_device_code" => ApiError::new(
                StatusCode::BAD_REQUEST,
                "incorrect_device_code",
                "unknown device code; start the login again",
            ),
            "bad_verification_code" => ApiError::new(
                StatusCode::BAD_REQUEST,
                "bad_verification_code",
                "the GitHub code is invalid or expired; log in again",
            ),
            "device_flow_disabled" => ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "device_flow_disabled",
                "the device flow is disabled for the GitHub OAuth App",
            ),
            _ => ApiError::new(StatusCode::BAD_GATEWAY, "github_error", e.to_string()),
        },
        GithubError::Upstream(_) => {
            ApiError::new(StatusCode::BAD_GATEWAY, "github_error", e.to_string())
        }
    }
}

/// Reads the GitHub user of `access_token` (then drops the token) and
/// issues a session token.
async fn sign_in(state: &AppState, access_token: String) -> ApiResult<Value> {
    let user = state
        .github
        .user(&access_token)
        .await
        .map_err(|e| github_error(&e))?;
    drop(access_token);
    let (token, expires_at) =
        session::issue(&state.db(), &user.login, user.id, state.config.session_days)
            .map_err(ApiError::internal)?;
    Ok(json!({
        "token": token,
        "login": user.login,
        "github_id": user.id,
        "expires_at": expires_at,
        "official": state.config.is_official(&user.login),
    }))
}

async fn device(State(state): St, client: Client) -> ApiResult<Json<Value>> {
    login_limit(&state, &client)?;
    let code = state
        .github
        .device_code()
        .await
        .map_err(|e| github_error(&e))?;
    state
        .poll_gate
        .started(&code.device_code, code.interval, code.expires_in);
    Ok(Json(
        serde_json::to_value(code).map_err(ApiError::internal)?,
    ))
}

#[derive(Debug, Deserialize)]
struct TokenBody {
    device_code: String,
}

fn slow_down(interval: u64) -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "slow_down",
        "polling too fast; wait `interval` seconds between polls",
    )
    .with("interval", interval)
}

async fn token(
    State(state): St,
    client: Client,
    Json(body): Json<TokenBody>,
) -> ApiResult<Json<Value>> {
    poll_limit(&state, &client)?;
    let code = body.device_code.trim();
    if code.is_empty() || code.len() > 200 {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "incorrect_device_code",
            "`device_code` is missing or malformed",
        ));
    }
    // GitHub's interval is enforced here, so a fast client never makes the
    // server poll GitHub faster than allowed.
    if state.config.enforce_poll_interval
        && let Err(interval) = state.poll_gate.poll(code)
    {
        return Err(slow_down(interval));
    }
    let answer = state
        .github
        .poll_device(code)
        .await
        .map_err(|e| github_error(&e));
    match answer {
        Ok(Poll::SlowDown(i)) => state.poll_gate.set_interval(code, i),
        Ok(Poll::Token(_)) | Err(_) => state.poll_gate.forget(code),
        Ok(Poll::Pending) => {}
    }
    match answer? {
        Poll::Token(t) => Ok(Json(sign_in(&state, t).await?)),
        Poll::Pending => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "authorization_pending",
            "waiting for the user to enter the code on GitHub",
        )),
        Poll::SlowDown(interval) => Err(slow_down(interval)),
    }
}

fn redirect_uri(state: &AppState) -> String {
    format!("{}/v1/auth/github/callback", state.config.public_url)
}

fn web_secret(state: &AppState) -> ApiResult<&str> {
    state.config.github_client_secret.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "web_login_disabled",
            "web login is disabled (VITRINE_API_GITHUB_CLIENT_SECRET is not set)",
        )
    })
}

fn cookie(state: &AppState, value: &str, max_age: i64) -> Option<HeaderValue> {
    let secure = if state.config.public_url.starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    HeaderValue::from_str(&format!(
        "{STATE_COOKIE}={value}; Path={COOKIE_PATH}; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"
    ))
    .ok()
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

async fn web_login(State(state): St, client: Client) -> ApiResult<Response> {
    web_secret(&state)?;
    login_limit(&state, &client)?;
    let oauth_state = session::random_hex(32).map_err(ApiError::internal)?;
    let (verifier, challenge) = pkce().map_err(ApiError::internal)?;
    session::put_state(&state.db(), &oauth_state, &verifier)?;
    let url = state
        .github
        .authorize_url(&redirect_uri(&state), &oauth_state, &challenge);
    let mut res = StatusCode::FOUND.into_response();
    let h = res.headers_mut();
    h.insert(
        header::LOCATION,
        HeaderValue::from_str(&url).map_err(ApiError::internal)?,
    );
    if let Some(c) = cookie(&state, &oauth_state, session::STATE_TTL_SECS) {
        h.insert(header::SET_COOKIE, c);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

fn bad_state() -> ApiError {
    ApiError::new(
        StatusCode::BAD_REQUEST,
        "invalid_state",
        "the login state is missing, expired or does not belong to this browser; log in again",
    )
}

async fn callback(
    State(state): St,
    headers: HeaderMap,
    Query(q): Query<CallbackQuery>,
) -> ApiResult<Response> {
    let secret = web_secret(&state)?.to_owned();
    let oauth_state = q.state.as_deref().ok_or_else(bad_state)?;
    // The state must come back in this browser's cookie (login CSRF) and
    // be a stored, unexpired, unused one.
    if cookie_value(&headers, STATE_COOKIE) != Some(oauth_state) {
        return Err(bad_state());
    }
    let verifier = session::take_state(&state.db(), oauth_state)?.ok_or_else(bad_state)?;
    if let Some(e) = q.error.as_deref() {
        return Err(github_error(&GithubError::OAuth(
            e.to_owned(),
            String::new(),
        )));
    }
    let code = q.code.as_deref().ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "missing_code",
            "no `code` from GitHub",
        )
    })?;
    let access = state
        .github
        .exchange_code(&secret, code, &redirect_uri(&state), &verifier)
        .await
        .map_err(|e| github_error(&e))?;
    let signed_in = sign_in(&state, access).await?;
    let mut res = if let Some(ret) = &state.config.web_login_return_url {
        // Token in the fragment: never sent to a server, read by the page.
        let mut frag = reqwest::Url::parse("http://x/").map_err(ApiError::internal)?;
        frag.query_pairs_mut()
            .append_pair("token", signed_in["token"].as_str().unwrap_or_default())
            .append_pair("login", signed_in["login"].as_str().unwrap_or_default())
            .append_pair("expires_at", &signed_in["expires_at"].to_string());
        let target = format!("{ret}#{}", frag.query().unwrap_or_default());
        let mut r = StatusCode::FOUND.into_response();
        r.headers_mut().insert(
            header::LOCATION,
            HeaderValue::from_str(&target).map_err(ApiError::internal)?,
        );
        r
    } else {
        Json(signed_in).into_response()
    };
    let h = res.headers_mut();
    if let Some(c) = cookie(&state, "", 0) {
        h.insert(header::SET_COOKIE, c);
    }
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(res)
}

async fn me(State(state): St, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let who = auth::uploader(&state, &headers)?;
    let expires_at = if who.session {
        auth::bearer(&headers)
            .and_then(|t| session::lookup(&state.db(), t).ok().flatten())
            .map(|s| s.expires_at)
    } else {
        None
    };
    Ok(Json(json!({
        "login": who.login,
        "github_id": who.github_id,
        "kind": if who.session { "session" } else { "static" },
        "expires_at": expires_at,
        "official": state.config.is_official(&who.login),
    })))
}

async fn logout(State(state): St, headers: HeaderMap) -> ApiResult<Json<Value>> {
    let who = auth::uploader(&state, &headers)?;
    if !who.session {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "static_token",
            "static upload tokens cannot be revoked here (remove them from VITRINE_API_UPLOAD_TOKENS)",
        ));
    }
    let token = auth::bearer(&headers).ok_or_else(ApiError::unauthorized)?;
    session::revoke(&state.db(), token)?;
    Ok(Json(json!({ "revoked": true })))
}
