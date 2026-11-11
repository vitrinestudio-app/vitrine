//! # vitrine-publish
//!
//! Store upload, kept out of `vitrine-core` so the core stays free of
//! network code (`docs/KONZEPT.md`, section 7).
//!
//! Responsibilities:
//!
//! - **App Store Connect API** ([`apple::Client`]): API key (.p8) → ES256
//!   JWT, screenshot sets per display type, reserve/upload/commit, ordering.
//! - **Google Play Developer API** ([`google::Client`], Android Publisher
//!   v3): service account → OAuth 2.0 token, edits, `deleteall`, upload,
//!   commit.
//! - Shared plumbing ([`ClientOptions`]): retries with backoff + jitter for
//!   429/5xx, cancellation, redacted logging hooks, [`Progress`] events.
//!
//! - **Secrets** ([`credentials`], Phase 5.1, ADR 0016): key file
//!   validation and the system keychain wrapper.
//!
//! Later (Phase 5.2/5.5): **plan → confirm → execute** (never upload
//! without a dry-run plan and an explicit confirmation; resumable, logged to
//! `.cache/uploads.sqlite`). The clients here take key material in memory
//! only and never log it.
//!
//! Plain REST via `reqwest` + rustls, no vendor SDKs. API facts and sources:
//! `docs/adr/0017-store-clients.md`.

pub mod apple;
mod error;
pub mod google;
mod http;
mod progress;

pub use error::{ApiErrorDetail, Error, ErrorKind, Result};
pub use http::{ClientOptions, HttpEvent, HttpEventKind, HttpObserver, PollPolicy, RetryPolicy};
pub use progress::{Progress, ProgressFn, UploadStep, ignore_progress};
pub use tokio_util::sync::CancellationToken;

pub mod credentials;

/// The stores Vitrine Studio can publish to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Store {
    /// Apple App Store (App Store Connect API).
    Apple,
    /// Google Play (Android Publisher API v3).
    Google,
}

impl Store {
    /// Stable identifier as used in `project.yaml` (`store: { apple: … }`).
    pub fn id(self) -> &'static str {
        match self {
            Self::Apple => "apple",
            Self::Google => "google",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_ids_match_project_yaml_keys() {
        assert_eq!(Store::Apple.id(), "apple");
        assert_eq!(Store::Google.id(), "google");
    }
}
