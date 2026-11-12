//! Client of the template gallery API (`api.vitrinestudio.app`,
//! `docs/api/API.md`, ADR 0025).
//!
//! Browsing, previews and downloads need no login; uploads take a bearer
//! token (a session token from the GitHub device flow or an owner-issued
//! static token). Base URL: [`DEFAULT_API_URL`], overridable per call
//! (`--api`, https or loopback only) or with [`API_URL_ENV`] (loopback
//! URLs only: tests, dev bridge).
//!
//! Unreachable servers (offline, DNS, refused, timeout) are
//! [`GalleryError::Unreachable`] so front ends can show a quiet "gallery
//! not reachable". Tokens never appear in errors or `Debug` output.

use std::time::Duration;

use reqwest::{StatusCode, Url};
use serde::{Deserialize, Serialize};

/// The production API.
pub const DEFAULT_API_URL: &str = "https://api.vitrinestudio.app";
/// Base URL override (loopback URLs only).
pub const API_URL_ENV: &str = "VITRINE_API_URL";
/// Largest download accepted: the client package limit
/// (`Limits::CLIENT_PACKAGE_MB`), which no server upload limit should exceed.
pub const MAX_DOWNLOAD: usize =
    (vitrine_core::template::Limits::CLIENT_PACKAGE_MB * 1024 * 1024) as usize;
/// Largest preview image accepted.
pub const MAX_PREVIEW: usize = 16 * 1024 * 1024;

/// Time allowed for one template upload or download (packages up to a few
/// hundred MB on slow lines; other requests keep the default 180 s).
const UPLOAD_TIMEOUT: Duration = Duration::from_mins(15);

/// Errors of the gallery client.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GalleryError {
    /// The server could not be reached (offline, DNS, refused, timeout).
    #[error("the gallery server is not reachable ({0})")]
    Unreachable(String),
    /// The server answered with an error (`code` from its JSON body).
    #[error("{message}")]
    Api {
        status: u16,
        code: String,
        message: String,
        /// `slow_down`: the new polling interval.
        interval: Option<u64>,
    },
    /// A bad base URL or an unexpected answer.
    #[error("{0}")]
    Invalid(String),
}

impl GalleryError {
    /// The server's error code (`not_found`, `authorization_pending`, …).
    pub fn code(&self) -> Option<&str> {
        match self {
            Self::Api { code, .. } => Some(code),
            _ => None,
        }
    }
}

pub type Result<T, E = GalleryError> = std::result::Result<T, E>;

/// Is `url` an `http://` loopback URL (tests, dev bridge)?
pub fn is_loopback_url(url: &str) -> bool {
    ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"]
        .iter()
        .any(|p| url.starts_with(p))
}

/// The API base: `explicit` (https or loopback http), else
/// [`API_URL_ENV`] when it is a loopback URL, else [`DEFAULT_API_URL`].
pub fn api_url(explicit: Option<&str>) -> Result<Url> {
    let raw = match explicit {
        Some(u) => {
            let u = u.trim();
            if !(u.starts_with("https://") || is_loopback_url(u)) {
                return Err(GalleryError::Invalid(format!(
                    "the API URL must use https (or http on 127.0.0.1/localhost): `{u}`"
                )));
            }
            u.to_owned()
        }
        None => std::env::var(API_URL_ENV)
            .ok()
            .filter(|u| is_loopback_url(u))
            .unwrap_or_else(|| DEFAULT_API_URL.to_owned()),
    };
    let mut url = Url::parse(raw.trim_end_matches('/'))
        .map_err(|e| GalleryError::Invalid(format!("invalid API URL `{raw}`: {e}")))?;
    if url.cannot_be_a_base() || url.host_str().is_none() {
        return Err(GalleryError::Invalid(format!("invalid API URL `{raw}`")));
    }
    if !url.path().ends_with('/') {
        let p = format!("{}/", url.path());
        url.set_path(&p);
    }
    Ok(url)
}

/// Keychain user name of the session token for an API: `gallery/<host[:port]>`.
pub fn keychain_user(api: &Url) -> String {
    let host = api.host_str().unwrap_or("unknown");
    match api.port() {
        Some(p) => format!("gallery/{host}:{p}"),
        None => format!("gallery/{host}"),
    }
}

/// The gallery sprite of a canvas: all pages side by side without gaps
/// (`width` × `height` px; the `url_2x` image is exactly twice as large).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteSprite {
    pub url: String,
    pub url_2x: String,
    pub width: u32,
    pub height: u32,
    pub page_width: u32,
    pub page_height: u32,
}

/// A canvas of a gallery template.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteCanvas {
    pub id: String,
    pub aspect: String,
    pub pages: u32,
    pub targets: Vec<String>,
    pub preview_url: Option<String>,
    /// Page aspect `w:h`.
    pub page_aspect: Option<String>,
    pub sprite: Option<RemoteSprite>,
}

/// A template as the gallery lists it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RemoteTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    /// Facet tags (`style`, `color`, `use`); an old server's flat list is
    /// sorted into facets.
    pub tags: vitrine_core::template::TemplateTags,
    /// Derived device tags (`phone`, `tablet`, …).
    pub devices: Vec<String>,
    /// Number of sample screenshots (0 = placeholders only).
    pub samples: u32,
    pub version: String,
    pub license: String,
    pub uploader: String,
    /// Published by the official account (`vitrinestudio-app`).
    pub official: bool,
    pub canvases: Vec<RemoteCanvas>,
    pub fonts: Vec<String>,
    pub size: u64,
    pub downloads: u64,
    pub created_at: i64,
    pub updated_at: i64,
    pub download_url: Option<String>,
}

/// One page of `GET /v1/templates`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TemplatePage {
    pub templates: Vec<RemoteTemplate>,
    pub limit: u32,
    pub offset: u32,
}

/// Query of [`GalleryClient::list`]. Within a facet any value matches,
/// across facets all must.
#[derive(Debug, Clone, Default)]
pub struct ListQuery {
    pub query: Option<String>,
    pub tag: Option<String>,
    pub style: Vec<String>,
    pub color: Vec<String>,
    pub use_: Vec<String>,
    pub devices: Vec<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Answer of `POST /v1/templates`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadResult {
    pub id: String,
    pub version: String,
    pub status: String,
    /// Sample screenshots kept by the server.
    #[serde(default)]
    pub samples: u32,
    /// Sample screenshots replaced by placeholders (not confirmed).
    #[serde(default)]
    pub samples_removed: u32,
}

/// `GET /v1/admin/queue`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AdminQueue {
    pub pending: Vec<RemoteTemplate>,
    pub reports: Vec<serde_json::Value>,
}

/// A moderation action of [`GalleryClient::admin_action`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminAction {
    Approve,
    Reject,
    /// Render previews and sprites again.
    Rerender,
}

impl AdminAction {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Approve => "approve",
            Self::Reject => "reject",
            Self::Rerender => "rerender",
        }
    }
}

/// Answer of `POST /v1/auth/github/device`.
#[allow(clippy::struct_field_names)] // GitHub's field names
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

/// A server session (`POST /v1/auth/github/token`).
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionToken {
    pub token: String,
    pub login: String,
    #[serde(default)]
    pub github_id: Option<i64>,
    pub expires_at: i64,
    #[serde(default)]
    pub official: bool,
}

impl std::fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionToken")
            .field("token", &"<redacted>")
            .field("login", &self.login)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

/// Result of one device-flow poll.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevicePoll {
    Done(SessionToken),
    Pending,
    /// Poll at most every `n` seconds from now on.
    SlowDown(u64),
}

/// Who a token belongs to (`GET /v1/auth/me`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Me {
    pub login: String,
    #[serde(default)]
    pub github_id: Option<i64>,
    /// `session` or `static`.
    pub kind: String,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub official: bool,
}

/// HTTP client of the gallery API.
#[derive(Debug, Clone)]
pub struct GalleryClient {
    http: reqwest::Client,
    base: Url,
}

#[allow(clippy::needless_pass_by_value)]
fn transport(e: reqwest::Error) -> GalleryError {
    let e = e.without_url();
    if e.is_connect() || e.is_timeout() || e.is_request() {
        GalleryError::Unreachable(e.to_string())
    } else {
        GalleryError::Invalid(format!("unexpected answer: {e}"))
    }
}

async fn api_error(res: reqwest::Response) -> GalleryError {
    #[derive(Deserialize)]
    struct Body {
        error: Option<String>,
        code: Option<String>,
        interval: Option<u64>,
    }
    let status = res.status();
    let body: Option<Body> = res
        .bytes()
        .await
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let (message, code, interval) = match body {
        Some(b) => (b.error, b.code, b.interval),
        None => (None, None, None),
    };
    // A proxy answering 502/503/504 means the API itself is down.
    if code.is_none()
        && matches!(
            status,
            StatusCode::BAD_GATEWAY | StatusCode::SERVICE_UNAVAILABLE | StatusCode::GATEWAY_TIMEOUT
        )
    {
        return GalleryError::Unreachable(format!("HTTP {status}"));
    }
    GalleryError::Api {
        status: status.as_u16(),
        code: code.unwrap_or_else(|| "http_error".to_owned()),
        message: message.unwrap_or_else(|| format!("the gallery answered HTTP {status}")),
        interval,
    }
}

impl GalleryClient {
    /// A client for the API at `base` (see [`api_url`]).
    pub fn new(base: Url) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(concat!("VitrineStudio/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|e| GalleryError::Invalid(format!("HTTP client setup failed: {e}")))?;
        Ok(Self { http, base })
    }

    pub fn base(&self) -> &Url {
        &self.base
    }

    fn url(&self, path: &str) -> Result<Url> {
        self.base
            .join(path.trim_start_matches('/'))
            .map_err(|e| GalleryError::Invalid(e.to_string()))
    }

    async fn send(&self, req: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let res = req.send().await.map_err(transport)?;
        if res.status().is_success() {
            Ok(res)
        } else {
            Err(api_error(res).await)
        }
    }

    async fn json<T: for<'de> Deserialize<'de>>(&self, req: reqwest::RequestBuilder) -> Result<T> {
        let res = self.send(req).await?;
        let bytes = res.bytes().await.map_err(transport)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| GalleryError::Invalid(format!("unexpected answer from the gallery: {e}")))
    }

    /// Reads a body of at most `max` bytes.
    async fn bytes(&self, req: reqwest::RequestBuilder, max: usize) -> Result<Vec<u8>> {
        let mut res = self.send(req).await?;
        let mut out = Vec::new();
        while let Some(chunk) = res.chunk().await.map_err(transport)? {
            if out.len() + chunk.len() > max {
                return Err(GalleryError::Invalid(format!(
                    "the answer is larger than {} MB",
                    max / (1024 * 1024)
                )));
            }
            out.extend_from_slice(&chunk);
        }
        Ok(out)
    }

    /// `GET /v1/templates` (approved templates, newest first).
    pub async fn list(&self, q: &ListQuery) -> Result<TemplatePage> {
        let mut url = self.url("v1/templates")?;
        {
            let mut p = url.query_pairs_mut();
            if let Some(v) = q.query.as_deref().filter(|s| !s.is_empty()) {
                p.append_pair("q", v);
            }
            if let Some(v) = q.tag.as_deref().filter(|s| !s.is_empty()) {
                p.append_pair("tag", v);
            }
            for (facet, values) in [
                ("style", &q.style),
                ("color", &q.color),
                ("use", &q.use_),
                ("devices", &q.devices),
            ] {
                for v in values.iter().filter(|v| !v.is_empty()) {
                    p.append_pair(facet, v);
                }
            }
            if let Some(v) = q.limit {
                p.append_pair("limit", &v.to_string());
            }
            if let Some(v) = q.offset {
                p.append_pair("offset", &v.to_string());
            }
        }
        if url.query() == Some("") {
            url.set_query(None);
        }
        self.json(self.http.get(url)).await
    }

    /// `GET /v1/templates/{id}`.
    pub async fn detail(&self, id: &str) -> Result<RemoteTemplate> {
        self.json(self.http.get(self.template_url(id, "")?)).await
    }

    fn template_url(&self, id: &str, rest: &str) -> Result<Url> {
        if !vitrine_core::template::is_template_id(id) {
            return Err(GalleryError::Invalid(format!("invalid template id `{id}`")));
        }
        self.url(&format!("v1/templates/{id}{rest}"))
    }

    /// `GET /v1/templates/{id}/download`: the `.vitrine-template` bytes
    /// (unchecked – callers verify them with `Package::from_zip_bytes`).
    pub async fn download(&self, id: &str) -> Result<Vec<u8>> {
        let url = self.template_url(id, "/download")?;
        self.bytes(self.http.get(url).timeout(UPLOAD_TIMEOUT), MAX_DOWNLOAD)
            .await
    }

    /// `GET /v1/templates/{id}/previews/{canvas}`: PNG bytes.
    pub async fn preview(&self, id: &str, canvas: &str) -> Result<Vec<u8>> {
        if !vitrine_core::model::is_valid_id(canvas) {
            return Err(GalleryError::Invalid(format!(
                "invalid canvas id `{canvas}`"
            )));
        }
        let url = self.template_url(id, &format!("/previews/{canvas}"))?;
        self.bytes(self.http.get(url), MAX_PREVIEW).await
    }

    /// `GET /v1/tags`: the tag taxonomy with labels.
    pub async fn tags(&self) -> Result<serde_json::Value> {
        self.json(self.http.get(self.url("v1/tags")?)).await
    }

    /// `POST /v1/templates` with a bearer token. `confirm_own_screenshots`:
    /// the uploader confirms the package's sample screenshots are their
    /// own and CC0 (else the server replaces them with placeholders).
    pub async fn upload(
        &self,
        token: &str,
        package: Vec<u8>,
        confirm_own_screenshots: bool,
    ) -> Result<UploadResult> {
        let mut url = self.url("v1/templates")?;
        if confirm_own_screenshots {
            url.set_query(Some("confirm_own_screenshots=true"));
        }
        let req = self
            .http
            .post(url)
            .bearer_auth(token)
            .header(reqwest::header::CONTENT_TYPE, "application/zip")
            // Large packages on slow lines: more than the default 180 s.
            .timeout(UPLOAD_TIMEOUT)
            .body(package);
        self.json(req).await
    }

    /// `POST /v1/auth/github/device`: starts the GitHub device flow.
    pub async fn device_start(&self) -> Result<DeviceCode> {
        self.json(self.http.post(self.url("v1/auth/github/device")?))
            .await
    }

    /// `POST /v1/auth/github/token`: one poll.
    pub async fn device_poll(&self, device_code: &str) -> Result<DevicePoll> {
        let req = self
            .http
            .post(self.url("v1/auth/github/token")?)
            .json(&serde_json::json!({ "device_code": device_code }));
        match self.json::<SessionToken>(req).await {
            Ok(s) => Ok(DevicePoll::Done(s)),
            Err(e) => match e.code() {
                Some("authorization_pending") => Ok(DevicePoll::Pending),
                Some("slow_down") => Ok(DevicePoll::SlowDown(match &e {
                    GalleryError::Api {
                        interval: Some(i), ..
                    } => *i,
                    _ => 10,
                })),
                _ => Err(e),
            },
        }
    }

    /// `GET /v1/admin/queue` (admin token).
    pub async fn admin_queue(&self, admin_token: &str) -> Result<AdminQueue> {
        self.json(
            self.http
                .get(self.url("v1/admin/queue")?)
                .bearer_auth(admin_token),
        )
        .await
    }

    /// `POST /v1/admin/templates/{id}/{approve|reject|rerender}` (admin
    /// token) with an optional moderation note.
    pub async fn admin_action(
        &self,
        admin_token: &str,
        id: &str,
        action: AdminAction,
        note: &str,
    ) -> Result<serde_json::Value> {
        if !vitrine_core::template::is_template_id(id) {
            return Err(GalleryError::Invalid(format!("invalid template id `{id}`")));
        }
        let url = self.url(&format!("v1/admin/templates/{id}/{}", action.as_str()))?;
        self.json(
            self.http
                .post(url)
                .bearer_auth(admin_token)
                .json(&serde_json::json!({ "note": note })),
        )
        .await
    }

    /// `GET /v1/auth/me`.
    pub async fn me(&self, token: &str) -> Result<Me> {
        self.json(self.http.get(self.url("v1/auth/me")?).bearer_auth(token))
            .await
    }

    /// `POST /v1/auth/logout`: revokes a session token.
    pub async fn logout(&self, token: &str) -> Result<()> {
        self.send(
            self.http
                .post(self.url("v1/auth/logout")?)
                .bearer_auth(token),
        )
        .await
        .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_urls() {
        assert_eq!(
            api_url(Some("https://api.example.test/")).unwrap().as_str(),
            "https://api.example.test/"
        );
        assert!(api_url(Some("http://api.example.test")).is_err());
        assert!(api_url(Some("http://127.0.0.1.evil.test:80")).is_err());
        let local = api_url(Some("http://127.0.0.1:4100")).unwrap();
        assert_eq!(keychain_user(&local), "gallery/127.0.0.1:4100");
        assert_eq!(
            keychain_user(&Url::parse(DEFAULT_API_URL).unwrap()),
            "gallery/api.vitrinestudio.app"
        );
        let sub = api_url(Some("https://example.test/api")).unwrap();
        let c = GalleryClient::new(sub).unwrap();
        assert_eq!(
            c.url("v1/templates").unwrap().as_str(),
            "https://example.test/api/v1/templates"
        );
        assert!(c.template_url("../x", "").is_err());
    }

    #[test]
    fn session_debug_is_redacted() {
        let s = SessionToken {
            token: "vss_SECRET".into(),
            login: "octocat".into(),
            github_id: Some(1),
            expires_at: 0,
            official: false,
        };
        assert!(!format!("{s:?}").contains("SECRET"));
    }

    #[tokio::test]
    async fn unreachable_is_its_own_error() {
        // A port nobody listens on (bound, then dropped).
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let c = GalleryClient::new(api_url(Some(&format!("http://127.0.0.1:{port}"))).unwrap())
            .unwrap();
        let e = c.list(&ListQuery::default()).await.unwrap_err();
        assert!(matches!(e, GalleryError::Unreachable(_)), "{e:?}");
    }
}
