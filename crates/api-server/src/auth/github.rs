//! GitHub OAuth client (OAuth App of the org `vitrinestudio-app`).
//!
//! Endpoints (GitHub docs "Authorizing OAuth apps",
//! <https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps>,
//! checked 2026-11-11):
//!
//! - device flow: `POST {github}/login/device/code` (`client_id`, no
//!   scope) → `device_code`, `user_code`, `verification_uri`, `expires_in`,
//!   `interval`; poll `POST {github}/login/oauth/access_token`
//!   (`client_id`, `device_code`, `grant_type =
//!   urn:ietf:params:oauth:grant-type:device_code`) → `access_token` or
//!   `error` (`authorization_pending`, `slow_down` + new `interval`,
//!   `expired_token`, `access_denied`, `incorrect_device_code`,
//!   `device_flow_disabled`, …), answered with HTTP 200;
//! - web flow: `GET {github}/login/oauth/authorize` (`client_id`,
//!   `redirect_uri`, `state`, `code_challenge` + `code_challenge_method =
//!   S256`), exchange `POST {github}/login/oauth/access_token`
//!   (`client_id`, `client_secret`, `code`, `redirect_uri`,
//!   `code_verifier`);
//! - identity: `GET {api}/user` with `Authorization: Bearer`, `Accept:
//!   application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28` and a
//!   `User-Agent` (required by the REST API).
//!
//! No scope is requested: the login and id of `GET /user` are public. The
//! GitHub access token is dropped right after reading the user.

use std::time::Duration;

use serde::Deserialize;

const DEVICE_GRANT: &str = "urn:ietf:params:oauth:grant-type:device_code";
const USER_AGENT: &str = concat!("vitrine-api-server/", env!("CARGO_PKG_VERSION"));

/// Errors talking to GitHub.
#[derive(Debug)]
pub enum GithubError {
    /// GitHub answered with an OAuth error code (`error`, description).
    OAuth(String, String),
    /// Transport, HTTP status or unexpected response.
    Upstream(String),
}

impl std::fmt::Display for GithubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OAuth(code, desc) if desc.is_empty() => write!(f, "GitHub: {code}"),
            Self::OAuth(code, desc) => write!(f, "GitHub: {code} ({desc})"),
            Self::Upstream(m) => write!(f, "GitHub: {m}"),
        }
    }
}

#[allow(clippy::needless_pass_by_value)]
fn upstream(e: reqwest::Error) -> GithubError {
    // Without the URL (it may carry a code).
    GithubError::Upstream(e.without_url().to_string())
}

/// Answer of the device code endpoint (passed on to the client).
#[allow(clippy::struct_field_names)] // GitHub's field names
#[derive(Clone, Debug, Deserialize, serde::Serialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// Raw token endpoint answer: a token or an OAuth error.
#[derive(Debug, Deserialize)]
struct TokenAnswer {
    access_token: Option<String>,
    error: Option<String>,
    #[serde(default)]
    error_description: String,
    interval: Option<u64>,
}

/// Result of one device-flow poll.
#[derive(Debug)]
pub enum Poll {
    /// The user confirmed; GitHub's access token (used once for `/user`).
    Token(String),
    /// Not confirmed yet.
    Pending,
    /// Polled too fast; wait this many seconds from now on.
    SlowDown(u64),
}

/// The GitHub identity.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct GithubUser {
    pub login: String,
    pub id: i64,
}

/// HTTP client for github.com and api.github.com.
#[derive(Clone, Debug)]
pub struct Github {
    http: reqwest::Client,
    web: String,
    api: String,
    client_id: String,
}

impl Github {
    pub fn new(web: &str, api: &str, client_id: &str) -> anyhow::Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        Ok(Self {
            http,
            web: web.trim_end_matches('/').to_owned(),
            api: api.trim_end_matches('/').to_owned(),
            client_id: client_id.to_owned(),
        })
    }

    /// The authorize URL of the web flow.
    pub fn authorize_url(&self, redirect_uri: &str, state: &str, challenge: &str) -> String {
        let mut url = reqwest::Url::parse(&format!("{}/login/oauth/authorize", self.web))
            .expect("the GitHub base URL is valid");
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("allow_signup", "true");
        url.into()
    }

    async fn post_form<T: for<'de> Deserialize<'de>>(
        &self,
        path: &str,
        form: &[(&str, &str)],
    ) -> Result<T, GithubError> {
        let res = self
            .http
            .post(format!("{}{path}", self.web))
            .header(reqwest::header::ACCEPT, "application/json")
            .form(form)
            .send()
            .await
            .map_err(upstream)?;
        let status = res.status();
        let body = res.bytes().await.map_err(upstream)?;
        if !status.is_success() {
            // GitHub answers OAuth errors with 200; anything else is upstream.
            if let Ok(TokenAnswer {
                error: Some(code),
                error_description,
                ..
            }) = serde_json::from_slice::<TokenAnswer>(&body)
            {
                return Err(GithubError::OAuth(code, error_description));
            }
            return Err(GithubError::Upstream(format!("HTTP {status} from {path}")));
        }
        serde_json::from_slice(&body)
            .map_err(|e| GithubError::Upstream(format!("unexpected answer from {path}: {e}")))
    }

    /// Starts the device flow (no scope).
    pub async fn device_code(&self) -> Result<DeviceCode, GithubError> {
        #[derive(Deserialize)]
        struct Either {
            #[serde(flatten)]
            code: Option<DeviceCode>,
            error: Option<String>,
            #[serde(default)]
            error_description: String,
        }
        let a: Either = self
            .post_form("/login/device/code", &[("client_id", &self.client_id)])
            .await?;
        match (a.code, a.error) {
            (_, Some(code)) => Err(GithubError::OAuth(code, a.error_description)),
            (Some(c), None) => Ok(c),
            (None, None) => Err(GithubError::Upstream(
                "unexpected answer from /login/device/code".into(),
            )),
        }
    }

    /// One poll of the device flow's token endpoint.
    pub async fn poll_device(&self, device_code: &str) -> Result<Poll, GithubError> {
        let a: TokenAnswer = self
            .post_form(
                "/login/oauth/access_token",
                &[
                    ("client_id", &self.client_id),
                    ("device_code", device_code),
                    ("grant_type", DEVICE_GRANT),
                ],
            )
            .await?;
        match (a.access_token, a.error.as_deref()) {
            (Some(t), None) => Ok(Poll::Token(t)),
            (_, Some("authorization_pending")) => Ok(Poll::Pending),
            (_, Some("slow_down")) => Ok(Poll::SlowDown(a.interval.unwrap_or(10))),
            (_, Some(code)) => Err(GithubError::OAuth(code.to_owned(), a.error_description)),
            (None, None) => Err(GithubError::Upstream(
                "token endpoint answered neither a token nor an error".into(),
            )),
        }
    }

    /// Web flow: exchanges the code (with the client secret and the PKCE
    /// verifier) for an access token.
    pub async fn exchange_code(
        &self,
        secret: &str,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<String, GithubError> {
        let a: TokenAnswer = self
            .post_form(
                "/login/oauth/access_token",
                &[
                    ("client_id", &self.client_id),
                    ("client_secret", secret),
                    ("code", code),
                    ("redirect_uri", redirect_uri),
                    ("code_verifier", verifier),
                ],
            )
            .await?;
        match (a.access_token, a.error) {
            (Some(t), None) => Ok(t),
            (_, Some(code)) => Err(GithubError::OAuth(code, a.error_description)),
            (None, None) => Err(GithubError::Upstream(
                "token endpoint answered neither a token nor an error".into(),
            )),
        }
    }

    /// Reads the user of `access_token` (the token is not kept).
    pub async fn user(&self, access_token: &str) -> Result<GithubUser, GithubError> {
        let res = self
            .http
            .get(format!("{}/user", self.api))
            .bearer_auth(access_token)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await
            .map_err(upstream)?;
        let status = res.status();
        if !status.is_success() {
            return Err(GithubError::Upstream(format!("HTTP {status} from /user")));
        }
        res.json()
            .await
            .map_err(|e| GithubError::Upstream(format!("unexpected answer from /user: {e}")))
    }
}

/// PKCE: a random verifier and its S256 challenge.
pub fn pkce() -> Result<(String, String), getrandom::Error> {
    use base64::Engine as _;
    use sha2::Digest as _;
    let verifier = super::session::random_hex(32)?;
    let challenge = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(sha2::Sha256::digest(verifier.as_bytes()));
    Ok((verifier, challenge))
}
