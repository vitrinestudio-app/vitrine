//! API errors: `{"error": "...", "code": "..."}` with an HTTP status.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    /// Extra fields of the error body (e.g. `interval` of `slow_down`).
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            extra: serde_json::Map::new(),
        }
    }

    /// Adds a field to the error body.
    #[must_use]
    pub fn with(mut self, key: &str, value: impl Into<serde_json::Value>) -> Self {
        self.extra.insert(key.to_owned(), value.into());
        self
    }

    pub fn not_found(what: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{what} not found"),
        )
    }

    pub fn unauthorized() -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "unauthorized",
            "a valid bearer token is required",
        )
    }

    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", e.to_string())
    }
}

impl From<rusqlite::Error> for ApiError {
    fn from(e: rusqlite::Error) -> Self {
        Self::internal(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut body = self.extra;
        body.insert("error".into(), self.message.into());
        body.insert("code".into(), self.code.into());
        let body = Json(serde_json::Value::Object(body));
        (self.status, body).into_response()
    }
}

pub type ApiResult<T> = Result<T, ApiError>;
