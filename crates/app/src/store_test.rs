//! "Test connection" for store accounts (Phase 5.1 seam, wired with the
//! store clients of Phase 5.3/5.4; ADR 0016/0017).
//!
//! Read-only on purpose:
//!
//! - Apple: sign an ES256 token and `GET /v1/apps?limit=1` → number of
//!   visible apps + the first app's name.
//! - Google: fetch an OAuth access token (proves Google accepts the service
//!   account key). Play Console permissions are only checked at upload time;
//!   no edit is opened.
//!
//! Installed by the GUI and the dev bridge (`AppState::set_connection_tester`);
//! the CLI and the headless MCP server do not test connections.

use std::time::Duration;

use vitrine_core::model::StoreKind;
use vitrine_publish::credentials::Secret;
use vitrine_publish::{ClientOptions, Error, RetryPolicy, apple, google};

use crate::stores::{
    ConnectionError, ConnectionFailure, ConnectionInfo, ConnectionTester, StoreAccount,
};

/// Env overrides of the store endpoints (dev bridge only; loopback URLs only).
pub const ENV_ASC_BASE_URL: &str = "VITRINE_ASC_BASE_URL";
pub const ENV_PLAY_BASE_URL: &str = "VITRINE_PLAY_BASE_URL";
pub const ENV_PLAY_TOKEN_URL: &str = "VITRINE_PLAY_TOKEN_URL";

/// [`ConnectionTester`] backed by `vitrine_publish::{apple, google}`.
#[derive(Debug, Clone)]
pub struct StoreClientTester {
    /// App Store Connect API base URL (`None` = production).
    pub apple_base_url: Option<String>,
    /// Android Publisher base URL (`None` = production; unused by the token-only test).
    pub google_base_url: Option<String>,
    /// Google OAuth token endpoint (`None` = production).
    pub google_token_url: Option<String>,
    /// Client options (short timeout, one retry).
    pub options: ClientOptions,
}

impl Default for StoreClientTester {
    fn default() -> Self {
        Self {
            apple_base_url: None,
            google_base_url: None,
            google_token_url: None,
            options: ClientOptions {
                retry: RetryPolicy {
                    max_attempts: 2,
                    max_retry_after: Duration::from_secs(5),
                    ..RetryPolicy::default()
                },
                request_timeout: Duration::from_secs(20),
                ..ClientOptions::default()
            },
        }
    }
}

impl StoreClientTester {
    /// Production endpoints, overridable through [`ENV_ASC_BASE_URL`],
    /// [`ENV_PLAY_BASE_URL`], [`ENV_PLAY_TOKEN_URL`] – only loopback URLs are
    /// accepted (a mock server), so key material never goes elsewhere.
    pub fn from_env() -> Self {
        let var = |name: &str| std::env::var(name).ok().filter(|u| is_loopback_url(u));
        Self {
            apple_base_url: var(ENV_ASC_BASE_URL),
            google_base_url: var(ENV_PLAY_BASE_URL),
            google_token_url: var(ENV_PLAY_TOKEN_URL),
            ..Self::default()
        }
    }

    async fn test_apple(
        &self,
        account: &StoreAccount,
        secret: &Secret,
    ) -> Result<ConnectionInfo, ConnectionError> {
        let Some(facts) = &account.apple else {
            return Err(ConnectionError::new(
                ConnectionFailure::InvalidKey,
                "the account has no issuer id / key id",
            ));
        };
        let mut config = apple::Config::team(
            facts.issuer_id.clone(),
            facts.key_id.clone(),
            secret.expose(),
        );
        if let Some(url) = &self.apple_base_url {
            config = config.with_base_url(url.clone());
        }
        let client = apple::Client::new(config, self.options.clone())
            .map_err(|e| map_error(StoreKind::Apple, &e))?;
        let overview = client
            .apps_overview()
            .await
            .map_err(|e| map_error(StoreKind::Apple, &e))?;
        let first = overview.first.as_ref().map(|a| a.name.clone());
        let summary = match (overview.total, &first) {
            (Some(0), _) | (None, None) => {
                "key accepted, but no apps are visible to it (check the key's role and app access)"
                    .to_owned()
            }
            (Some(1), Some(name)) => format!("key accepted, 1 app visible ({name})"),
            (Some(n), Some(name)) => format!("key accepted, {n} apps visible (first: {name})"),
            (Some(n), None) => format!("key accepted, {n} apps visible"),
            (None, Some(name)) => format!("key accepted, apps visible (first: {name})"),
        };
        Ok(ConnectionInfo {
            summary,
            apps_visible: overview.total,
            first_app: first,
            client_email: None,
        })
    }

    async fn test_google(&self, secret: &Secret) -> Result<ConnectionInfo, ConnectionError> {
        let mut config = google::Config::new(secret.expose(), "");
        if let Some(token) = &self.google_token_url {
            let base = self
                .google_base_url
                .clone()
                .unwrap_or_else(|| config.base_url.clone());
            config = config.with_endpoints(base, token.clone());
        }
        let client = google::Client::new(config, self.options.clone())
            .map_err(|e| map_error(StoreKind::Google, &e))?;
        client
            .authenticate()
            .await
            .map_err(|e| map_error(StoreKind::Google, &e))?;
        let email = client.client_email().to_owned();
        Ok(ConnectionInfo {
            summary: format!(
                "Google accepted the service account {email} (Play Console access is checked \
                 at upload)"
            ),
            apps_visible: None,
            first_app: None,
            client_email: Some(email),
        })
    }

    async fn run(
        &self,
        account: &StoreAccount,
        secret: &Secret,
    ) -> Result<ConnectionInfo, ConnectionError> {
        match account.kind {
            StoreKind::Apple => self.test_apple(account, secret).await,
            StoreKind::Google => self.test_google(secret).await,
        }
    }
}

impl ConnectionTester for StoreClientTester {
    fn test(
        &self,
        account: &StoreAccount,
        secret: &Secret,
    ) -> Result<ConnectionInfo, ConnectionError> {
        // Own thread + runtime: the caller may already be inside a runtime
        // (Tauri, dev bridge), where `block_on` would panic.
        std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| {
                            ConnectionError::new(
                                ConnectionFailure::Other,
                                format!("cannot start the network runtime: {e}"),
                            )
                        })?;
                    rt.block_on(self.run(account, secret))
                })
                .join()
                .unwrap_or_else(|_| {
                    Err(ConnectionError::new(
                        ConnectionFailure::Other,
                        "the connection test crashed",
                    ))
                })
        })
    }
}

pub(crate) fn is_loopback_url(url: &str) -> bool {
    ["http://127.0.0.1:", "http://localhost:", "http://[::1]:"]
        .iter()
        .any(|p| url.starts_with(p))
}

/// Client error → user-facing message (also used by the app picker and the
/// Play package check, `commands::stores`).
pub(crate) fn map_error(store: StoreKind, e: &Error) -> ConnectionError {
    use ConnectionFailure as F;
    let name = match store {
        StoreKind::Apple => "App Store Connect",
        StoreKind::Google => "Google",
    };
    let (reason, message) = match e {
        Error::Auth {
            status: Some(403), ..
        } if store == StoreKind::Apple => (
            F::NoPermission,
            "key accepted, but it has no access to the App Store Connect API (check the key's \
             role, e.g. App Manager)"
                .to_owned(),
        ),
        Error::Auth {
            status: Some(403), ..
        } => (
            F::NoPermission,
            "Google accepted the service account, but it has no permission for this app – \
             invite it in Play Console → Users and permissions"
                .to_owned(),
        ),
        Error::NotFound { message } if store == StoreKind::Google => (
            F::NotFound,
            format!(
                "Google Play does not know this package, or the service account has no access \
                 to it in Play Console ({message})"
            ),
        ),
        Error::Auth { message, .. } => (
            F::KeyRejected,
            match store {
                StoreKind::Apple => format!(
                    "key rejected by App Store Connect – check issuer id and key id, whether the \
                     key was revoked, and the system clock ({message})"
                ),
                StoreKind::Google => format!(
                    "key rejected by Google – the service account key may be deleted or \
                     disabled, or the system clock is wrong ({message})"
                ),
            },
        ),
        Error::Transport { status: None, .. } => (
            F::Offline,
            format!("cannot reach {name} – offline or blocked by a proxy/firewall?"),
        ),
        Error::Transport {
            status: Some(s), ..
        } => (
            F::StoreError,
            format!("{name} has a server problem (HTTP {s}) – try again later"),
        ),
        Error::RateLimited { .. } => (
            F::RateLimited,
            format!("{name} rate limit reached – try again later"),
        ),
        Error::Timeout { .. } => (
            F::Timeout,
            format!("{name} did not answer in time – try again later"),
        ),
        Error::Config { message } => (
            F::InvalidKey,
            format!("the stored key cannot be used: {message}"),
        ),
        other => (F::StoreError, format!("{name} answered: {other}")),
    };
    ConnectionError::new(reason, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_loopback_overrides() {
        assert!(is_loopback_url("http://127.0.0.1:4010"));
        assert!(is_loopback_url("http://localhost:1/x"));
        assert!(!is_loopback_url("https://api.appstoreconnect.apple.com"));
        assert!(!is_loopback_url("http://127.0.0.1.evil.example:80"));
    }

    #[test]
    fn errors_map_to_reasons() {
        let auth = |status| Error::Auth {
            status: Some(status),
            message: "NOT_AUTHORIZED".into(),
        };
        assert_eq!(
            map_error(StoreKind::Apple, &auth(401)).reason,
            ConnectionFailure::KeyRejected
        );
        assert_eq!(
            map_error(StoreKind::Apple, &auth(403)).reason,
            ConnectionFailure::NoPermission
        );
        assert_eq!(
            map_error(StoreKind::Google, &auth(400)).reason,
            ConnectionFailure::KeyRejected
        );
        let offline = Error::Transport {
            status: None,
            message: "dns".into(),
        };
        let e = map_error(StoreKind::Google, &offline);
        assert_eq!(e.reason, ConnectionFailure::Offline);
        assert!(e.message.contains("offline"));
        let server = Error::Transport {
            status: Some(503),
            message: String::new(),
        };
        assert_eq!(
            map_error(StoreKind::Apple, &server).reason,
            ConnectionFailure::StoreError
        );
    }
}
