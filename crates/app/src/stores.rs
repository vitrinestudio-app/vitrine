//! App-wide store accounts (Phase 5.1, ADR 0016).
//!
//! An account is listed in `settings.yaml` (`store_accounts:`) with its
//! public facts only; the secret (Apple `.p8` key, Google service account
//! JSON) lives in the system keychain (`vitrine_publish::credentials`),
//! service `app.vitrinestudio`, user `<id>/<kind>`. Projects reference accounts
//! by id (`store.apple.account` in `project.yaml`).
//!
//! ```yaml
//! store_accounts:
//! - id: apple-team
//!   kind: apple
//!   label: My team
//!   apple: {issuer_id: 57246542-96fe-1a63-e053-0824d011072a, key_id: 2X9R4HXF34}
//! - id: google-play
//!   kind: google
//!   label: Play uploads
//!   google: {client_email: uploader@demo.iam.gserviceaccount.com, project_id: demo}
//! ```
//!
//! **CI credentials** (`vitrine_publish::env_credentials`): the CLI may
//! install credentials read from `VITRINE_ASC_*` / `VITRINE_PLAY_*`
//! variables ([`crate::AppState::with_env_credentials`]). They act as the
//! account of their store with the id [`ENV_ACCOUNT_ID`] and win over the
//! account the project references; the GUI and MCP never install them.

use serde::{Deserialize, Serialize};
use vitrine_core::model::{StoreKind, is_valid_id};
pub use vitrine_publish::credentials;
use vitrine_publish::credentials::Secret;

/// Account id of credentials from the environment (never in `settings.yaml`).
pub const ENV_ACCOUNT_ID: &str = "env";

/// The environment credentials of `kind` as an account plus its secret.
pub fn env_account(
    env: &vitrine_publish::env_credentials::EnvCredentials,
    kind: StoreKind,
) -> Option<(StoreAccount, Secret)> {
    match kind {
        StoreKind::Apple => env.apple.as_ref().map(|a| {
            (
                StoreAccount {
                    id: ENV_ACCOUNT_ID.to_owned(),
                    kind,
                    label: format!("environment ({})", a.source),
                    apple: Some(AppleAccount {
                        issuer_id: a.issuer_id.clone(),
                        key_id: a.key_id.clone(),
                    }),
                    google: None,
                },
                a.key.clone(),
            )
        }),
        StoreKind::Google => env.google.as_ref().map(|g| {
            (
                StoreAccount {
                    id: ENV_ACCOUNT_ID.to_owned(),
                    kind,
                    label: format!("environment ({})", g.source),
                    apple: None,
                    google: Some(GoogleAccount {
                        client_email: g.info.client_email.clone(),
                        project_id: g.info.project_id.clone(),
                    }),
                },
                g.key.clone(),
            )
        }),
    }
}

/// One store account (no secrets).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreAccount {
    pub id: String,
    pub kind: StoreKind,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple: Option<AppleAccount>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleAccount>,
}

/// App Store Connect API key facts (Users and Access → Integrations).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppleAccount {
    pub issuer_id: String,
    pub key_id: String,
}

/// Google service account facts (from the JSON key).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GoogleAccount {
    pub client_email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

impl StoreAccount {
    pub fn check(&self) -> Result<(), String> {
        let id = &self.id;
        if !is_valid_id(id) {
            return Err(format!("store account: invalid id `{id}`"));
        }
        if self.label.trim().is_empty() {
            return Err(format!("store account `{id}`: label must not be empty"));
        }
        match (self.kind, &self.apple, &self.google) {
            (StoreKind::Apple, Some(a), None) => {
                if !credentials::is_apple_issuer_id(&a.issuer_id) {
                    return Err(format!(
                        "store account `{id}`: issuer id `{}` is not a UUID",
                        a.issuer_id
                    ));
                }
                if !credentials::is_apple_key_id(&a.key_id) {
                    return Err(format!(
                        "store account `{id}`: key id `{}` must be 10 letters/digits",
                        a.key_id
                    ));
                }
            }
            (StoreKind::Google, None, Some(g)) => {
                if !g.client_email.contains('@') {
                    return Err(format!("store account `{id}`: invalid client email"));
                }
            }
            (kind, ..) => {
                return Err(format!(
                    "store account `{id}`: kind {kind} needs exactly the `{kind}` details"
                ));
            }
        }
        Ok(())
    }
}

/// Checks a list of accounts (unique ids, each valid).
pub fn check_accounts(accounts: &[StoreAccount]) -> Result<(), String> {
    let mut seen = std::collections::BTreeSet::new();
    for a in accounts {
        a.check()?;
        if !seen.insert(a.id.as_str()) {
            return Err(format!("duplicate store account id `{}`", a.id));
        }
    }
    Ok(())
}

/// A free account id derived from `label` (`apple-my-team`, `-2` …).
pub fn new_account_id(kind: StoreKind, label: &str, taken: &[StoreAccount]) -> String {
    let slug = vitrine_core::frame_import::slug(label);
    let base = if slug.is_empty() {
        kind.as_str().to_owned()
    } else if slug.starts_with(kind.as_str()) {
        slug
    } else {
        format!("{}-{slug}", kind.as_str())
    };
    let free = |id: &str| !taken.iter().any(|a| a.id == id);
    if free(&base) {
        return base;
    }
    (2..=taken.len() + 2)
        .map(|n| format!("{base}-{n}"))
        .find(|id| free(id))
        .unwrap_or(base)
}

/// What a connection test found out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConnectionInfo {
    /// Short human summary, e.g. "key accepted, 3 apps visible (first: …)".
    pub summary: String,
    /// Apple: number of apps the key can see.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apps_visible: Option<u64>,
    /// Apple: name of the first visible app.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_app: Option<String>,
    /// Google: the service account e-mail Google accepted.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_email: Option<String>,
}

/// Why a connection test failed (stable, for the UI: `details.reason`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionFailure {
    /// The store rejected the key (wrong ids, revoked/deleted key, clock).
    KeyRejected,
    /// The key is accepted but lacks permissions.
    NoPermission,
    /// No network / store not reachable.
    Offline,
    /// The store had a server problem or answered unexpectedly.
    StoreError,
    /// Rate limit reached.
    RateLimited,
    /// No answer in time.
    Timeout,
    /// The stored key cannot be used by the client.
    InvalidKey,
    /// The app / package is unknown to the store or not visible to the key.
    NotFound,
    /// Anything else.
    Other,
}

/// A failed connection test: reason + user-facing message (no secrets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionError {
    pub reason: ConnectionFailure,
    pub message: String,
}

impl ConnectionError {
    pub fn new(reason: ConnectionFailure, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: message.into(),
        }
    }
}

/// Seam for the store clients' "who am I" call. The GUI and the dev bridge
/// install [`crate::store_test::StoreClientTester`] (`vitrine_publish::apple`
/// / `google`); without a tester (CLI, headless MCP) `store_account_test`
/// answers `not_implemented` after checking that the secret is in the
/// keychain and valid.
pub trait ConnectionTester: Send + Sync {
    fn test(
        &self,
        account: &StoreAccount,
        secret: &Secret,
    ) -> Result<ConnectionInfo, ConnectionError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apple(id: &str) -> StoreAccount {
        StoreAccount {
            id: id.into(),
            kind: StoreKind::Apple,
            label: "Team".into(),
            apple: Some(AppleAccount {
                issuer_id: "57246542-96fe-1a63-e053-0824d011072a".into(),
                key_id: "2X9R4HXF34".into(),
            }),
            google: None,
        }
    }

    #[test]
    fn ids_and_checks() {
        let taken = vec![apple("apple-team")];
        assert_eq!(
            new_account_id(StoreKind::Apple, "Team", &taken),
            "apple-team-2"
        );
        assert_eq!(
            new_account_id(StoreKind::Google, "Play!", &taken),
            "google-play"
        );
        assert_eq!(new_account_id(StoreKind::Google, "", &taken), "google");
        assert!(check_accounts(&taken).is_ok());
        assert!(check_accounts(&[apple("a"), apple("a")]).is_err());
        let mut wrong = apple("b");
        wrong.kind = StoreKind::Google;
        assert!(wrong.check().is_err());
        let mut bad = apple("c");
        bad.apple.as_mut().unwrap().key_id = "short".into();
        assert!(bad.check().is_err());
    }
}
