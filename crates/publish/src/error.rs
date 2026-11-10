//! Error type shared by both store clients.
//!
//! Every failure maps to one [`ErrorKind`] so the executor (Phase 5.5) can
//! decide generically: re-authenticate, give up, wait and retry later, or
//! show the store's validation message to the user.

use std::fmt;
use std::time::Duration;

/// One error entry as reported by a store API (JSON:API `errors[]` for App
/// Store Connect, `error.errors[]`/`error` for Google APIs).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize)]
pub struct ApiErrorDetail {
    /// Machine-readable code (`ENTITY_ERROR.ATTRIBUTE.INVALID`,
    /// `STATE_ERROR`, Google `status`/`reason`, …).
    pub code: String,
    /// Short summary.
    pub title: String,
    /// Human-readable explanation (shown to the user, never parsed).
    pub detail: String,
}

impl fmt::Display for ApiErrorDetail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.title.is_empty(), self.detail.is_empty()) {
            (false, false) => write!(f, "{}: {} ({})", self.code, self.title, self.detail),
            (false, true) => write!(f, "{}: {}", self.code, self.title),
            (true, false) => write!(f, "{}: {}", self.code, self.detail),
            (true, true) => write!(f, "{}", self.code),
        }
    }
}

/// Broad error category (stable, for UI and executor decisions).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Credentials missing/invalid/expired, or insufficient permissions.
    Auth,
    /// The app, version, localization, set, edit … does not exist.
    NotFound,
    /// The resource is in a state that forbids the change (e.g. no editable
    /// App Store version, edit already committed).
    Conflict,
    /// Too many requests; retry later.
    RateLimited,
    /// The store rejected the request content (size, format, attributes).
    Validation,
    /// The store processed an upload and reported it as failed.
    Processing,
    /// Network/TLS/protocol problem or a store server error (5xx).
    Transport,
    /// Waiting for the store (asset processing) took too long.
    Timeout,
    /// The operation was cancelled through the cancellation token.
    Cancelled,
    /// Invalid local configuration (unreadable key, bad service account JSON).
    Config,
}

/// Errors of the store clients.
///
/// Messages never contain secrets (tokens, keys, signed upload URLs).
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Authentication or authorization failed.
    #[error("authentication failed (HTTP {status:?}): {message}")]
    Auth {
        /// HTTP status (401 = bad/expired credentials, 403 = missing permission).
        status: Option<u16>,
        /// Store message.
        message: String,
    },
    /// Resource not found.
    #[error("not found: {message}")]
    NotFound {
        /// What was not found.
        message: String,
    },
    /// State conflict (e.g. no editable version, set locked during review).
    #[error("conflict: {message}")]
    Conflict {
        /// Explanation.
        message: String,
        /// Store error details, if any.
        details: Vec<ApiErrorDetail>,
    },
    /// Rate limit hit and retries exhausted (or the requested wait is too long).
    #[error("rate limited{}", retry_after.map(|d| format!(" (retry after {}s)", d.as_secs())).unwrap_or_default())]
    RateLimited {
        /// Wait time requested by the server (`Retry-After`), if any.
        retry_after: Option<Duration>,
    },
    /// The store rejected the request content.
    #[error("rejected by the store: {message}")]
    Validation {
        /// Summary.
        message: String,
        /// Store error details.
        details: Vec<ApiErrorDetail>,
    },
    /// Asset processing failed on the store side (`assetDeliveryState` FAILED).
    #[error("processing of {asset_id} failed: {}", join_details(details))]
    Processing {
        /// Store id of the asset.
        asset_id: String,
        /// Errors reported by the store.
        details: Vec<ApiErrorDetail>,
    },
    /// Transport failure or server error after all retries.
    #[error("transport error{}: {message}", status.map(|s| format!(" (HTTP {s})")).unwrap_or_default())]
    Transport {
        /// HTTP status for server errors, `None` for network errors.
        status: Option<u16>,
        /// Explanation (no URLs with signatures, no tokens).
        message: String,
    },
    /// Timed out waiting for the store.
    #[error("timed out: {message}")]
    Timeout {
        /// What was awaited.
        message: String,
    },
    /// Cancelled by the caller.
    #[error("cancelled")]
    Cancelled,
    /// Invalid configuration (keys, JSON, arguments).
    #[error("invalid configuration: {message}")]
    Config {
        /// Explanation (never contains key material).
        message: String,
    },
}

fn join_details(details: &[ApiErrorDetail]) -> String {
    if details.is_empty() {
        return "no details".to_owned();
    }
    details
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

impl Error {
    /// The broad category of this error.
    pub fn kind(&self) -> ErrorKind {
        match self {
            Self::Auth { .. } => ErrorKind::Auth,
            Self::NotFound { .. } => ErrorKind::NotFound,
            Self::Conflict { .. } => ErrorKind::Conflict,
            Self::RateLimited { .. } => ErrorKind::RateLimited,
            Self::Validation { .. } => ErrorKind::Validation,
            Self::Processing { .. } => ErrorKind::Processing,
            Self::Transport { .. } => ErrorKind::Transport,
            Self::Timeout { .. } => ErrorKind::Timeout,
            Self::Cancelled => ErrorKind::Cancelled,
            Self::Config { .. } => ErrorKind::Config,
        }
    }

    /// Store error details attached to this error (empty if none).
    pub fn details(&self) -> &[ApiErrorDetail] {
        match self {
            Self::Conflict { details, .. }
            | Self::Validation { details, .. }
            | Self::Processing { details, .. } => details,
            _ => &[],
        }
    }

    /// Whether a later retry of the whole step may succeed (rate limits,
    /// transport problems, timeouts).
    pub fn is_transient(&self) -> bool {
        matches!(
            self.kind(),
            ErrorKind::RateLimited | ErrorKind::Transport | ErrorKind::Timeout
        )
    }

    pub(crate) fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }

    pub(crate) fn is_unauthorized(&self) -> bool {
        matches!(
            self,
            Self::Auth {
                status: Some(401),
                ..
            }
        )
    }
}

/// Result alias of this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;
