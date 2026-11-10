//! Store credentials (Phase 5.1, ADR 0016): validation of imported key
//! files and the system keychain.
//!
//! - **Apple**: the App Store Connect API key (`AuthKey_<KEYID>.p8`, a PEM
//!   PKCS#8 EC P-256 private key). Issuer id and key id are not secret and
//!   live in the app settings.
//! - **Google**: the service account JSON (`type: service_account`,
//!   `client_email`, `private_key`). The whole file is the secret; the
//!   client email and project id are copied to the app settings for display.
//!
//! Secrets are stored in the system keychain (macOS Keychain, Windows
//! Credential Manager, Secret Service on Linux) through `keyring`, service
//! `app.arrangia` (override: `ARRANGIA_KEYRING_SERVICE`), user
//! `<account id>/<apple|google>`. `ARRANGIA_KEYRING=mock` switches to an
//! in-memory store shared by the process (tests, dev bridge) so nothing
//! touches a real keychain.
//!
//! Nothing here ever puts key material into an error message, `Debug`
//! output or log line.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use arrangia_core::model::StoreKind;
use base64::Engine as _;
use keyring_core::{CredentialStore, Entry};

/// Default keychain service name.
pub const DEFAULT_SERVICE: &str = "app.arrangia";
/// Environment variable overriding the keychain service name (tests).
pub const SERVICE_ENV: &str = "ARRANGIA_KEYRING_SERVICE";
/// Environment variable choosing the backend: `mock` = in-memory store of
/// this process (tests, dev bridge); anything else = the system keychain.
pub const BACKEND_ENV: &str = "ARRANGIA_KEYRING";
/// Largest key file accepted (both formats are a few KB).
pub const MAX_KEY_FILE: u64 = 64 * 1024;

/// Errors of this module. Messages never contain secret material.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CredentialError {
    /// The key file is not in the expected format.
    #[error("{0}")]
    InvalidKey(String),
    /// No secret stored for the account.
    #[error("no secret stored in the keychain for `{0}`")]
    Missing(String),
    /// The keychain could not be used.
    #[error("keychain: {0}")]
    Keychain(String),
}

/// Secret text with a redacted `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: String) -> Self {
        Self(s)
    }

    /// The secret itself (only for signing / the keychain).
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

// ----- validation ---------------------------------------------------------------------

/// DER of the OID 1.2.840.10045.2.1 (`id-ecPublicKey`).
const OID_EC_PUBLIC_KEY: &[u8] = &[0x06, 0x07, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x02, 0x01];
/// DER of the OID 1.2.840.10045.3.1.7 (`prime256v1` / P-256).
const OID_P256: &[u8] = &[0x06, 0x08, 0x2A, 0x86, 0x48, 0xCE, 0x3D, 0x03, 0x01, 0x07];

/// Body of the PEM block `label` (base64 decoded), if present and valid.
fn pem_block(text: &str, label: &str) -> Result<Option<Vec<u8>>, CredentialError> {
    let begin = format!("-----BEGIN {label}-----");
    let end = format!("-----END {label}-----");
    let Some(start) = text.find(&begin) else {
        return Ok(None);
    };
    let rest = &text[start + begin.len()..];
    let Some(stop) = rest.find(&end) else {
        return Err(CredentialError::InvalidKey(format!(
            "the PEM block `{label}` has no END line"
        )));
    };
    let b64: String = rest[..stop]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    base64::engine::general_purpose::STANDARD
        .decode(b64.as_bytes())
        .map(Some)
        .map_err(|_| {
            CredentialError::InvalidKey(format!("the `{label}` block is not valid base64"))
        })
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// Checks an App Store Connect API key (`.p8`): a PEM `PRIVATE KEY`
/// (PKCS#8) or `EC PRIVATE KEY` (SEC1) on the P-256 curve.
pub fn validate_apple_key(text: &str) -> Result<(), CredentialError> {
    if let Some(der) = pem_block(text, "PRIVATE KEY")? {
        if !contains(&der, OID_EC_PUBLIC_KEY) {
            return Err(CredentialError::InvalidKey(
                "the .p8 file holds a private key but not an EC key (App Store Connect keys \
                 are ES256)"
                    .into(),
            ));
        }
        if !contains(&der, OID_P256) {
            return Err(CredentialError::InvalidKey(
                "the EC key is not on the P-256 curve (App Store Connect keys are ES256)".into(),
            ));
        }
        return Ok(());
    }
    if let Some(der) = pem_block(text, "EC PRIVATE KEY")? {
        if !contains(&der, OID_P256) {
            return Err(CredentialError::InvalidKey(
                "the EC key is not on the P-256 curve (App Store Connect keys are ES256)".into(),
            ));
        }
        return Ok(());
    }
    Err(CredentialError::InvalidKey(
        "not an App Store Connect API key: expected a PEM `-----BEGIN PRIVATE KEY-----` block \
         (the AuthKey_<KEYID>.p8 file downloaded from App Store Connect)"
            .into(),
    ))
}

/// Key id from Apple's file name `AuthKey_<KEYID>.p8`.
pub fn apple_key_id_from_file_name(name: &str) -> Option<String> {
    let id = name.strip_prefix("AuthKey_")?.strip_suffix(".p8")?;
    is_apple_key_id(id).then(|| id.to_owned())
}

/// App Store Connect key id: 10 upper-case letters / digits.
pub fn is_apple_key_id(s: &str) -> bool {
    s.len() == 10
        && s.bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
}

/// Issuer id: a UUID (`57246542-96fe-1a63-e053-0824d011072a`).
pub fn is_apple_issuer_id(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    groups.len() == 5
        && groups
            .iter()
            .zip([8, 4, 4, 4, 12])
            .all(|(g, n)| g.len() == n && g.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// Public facts of a Google service account file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoogleAccountInfo {
    pub client_email: String,
    pub project_id: Option<String>,
}

/// Checks a Google service account JSON key file and returns its public
/// facts. JSON errors report only line and column (serde messages may
/// quote values).
pub fn validate_google_key(text: &str) -> Result<GoogleAccountInfo, CredentialError> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| {
        CredentialError::InvalidKey(format!(
            "not a JSON file (line {}, column {})",
            e.line(),
            e.column()
        ))
    })?;
    let field = |name: &str| value.get(name).and_then(serde_json::Value::as_str);
    if field("type") != Some("service_account") {
        return Err(CredentialError::InvalidKey(
            "not a service account key: `type` must be `service_account` (create a JSON key \
             for a service account in Google Cloud → IAM → Service accounts)"
                .into(),
        ));
    }
    let email = field("client_email").unwrap_or_default().trim();
    if email.is_empty() || !email.contains('@') {
        return Err(CredentialError::InvalidKey(
            "the service account key has no valid `client_email`".into(),
        ));
    }
    let key = field("private_key").unwrap_or_default();
    if pem_block(key, "PRIVATE KEY")?.is_none_or(|der| der.is_empty()) {
        return Err(CredentialError::InvalidKey(
            "the service account key has no PEM `private_key`".into(),
        ));
    }
    Ok(GoogleAccountInfo {
        client_email: email.to_owned(),
        project_id: field("project_id")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned),
    })
}

/// A structurally valid PKCS#8 P-256 key for tests (random bytes, not a
/// key used anywhere). Assembled at runtime so the source holds no PEM
/// block that secret scanners would flag.
#[doc(hidden)]
pub fn sample_p8_for_tests() -> String {
    let body = [
        "MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgevZzL1gdAFr88hb2",
        "OF/2NxApJCzGCEDdfSp6VQO30hyhRANCAAQRWz+jn65BtOMvdyHKcvjBeBSDZH2r",
        "1RTwjmYSi9R/zpBnuQ4EiMnCqfMPWiZqB4QdbAd0E7oH50VpuZ1P087G",
    ];
    let kind = ["PRIVATE", "KEY"].join(" ");
    format!(
        "-----BEGIN {kind}-----\n{}\n-----END {kind}-----\n",
        body.join("\n")
    )
}

// ----- keychain -----------------------------------------------------------------------

/// Which keychain backend a [`SecretStore`] uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    /// The system keychain.
    System,
    /// In-memory store (tests, dev bridge; lost when the process ends).
    Mock,
}

/// One in-memory store per process for `ARRANGIA_KEYRING=mock`.
static PROCESS_MOCK: LazyLock<Arc<CredentialStore>> = LazyLock::new(new_mock);

fn new_mock() -> Arc<CredentialStore> {
    let store: Arc<CredentialStore> =
        keyring_core::mock::Store::new().expect("the mock credential store cannot fail");
    store
}

/// Keychain access for store secrets.
pub struct SecretStore {
    store: Arc<CredentialStore>,
    service: String,
    backend: Backend,
}

impl std::fmt::Debug for SecretStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SecretStore")
            .field("service", &self.service)
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

fn keychain_error(e: &keyring_core::Error) -> CredentialError {
    use keyring_core::Error as E;
    // Variants carrying stored bytes are not printed.
    let msg = match e {
        E::BadEncoding(_) | E::BadDataFormat(..) => "the stored secret is malformed".to_owned(),
        E::Ambiguous(_) => "several keychain entries match".to_owned(),
        other => other.to_string(),
    };
    CredentialError::Keychain(msg)
}

impl SecretStore {
    /// The store selected by the environment: `ARRANGIA_KEYRING=mock` →
    /// the process-wide in-memory store, else the system keychain; service
    /// from `ARRANGIA_KEYRING_SERVICE` or [`DEFAULT_SERVICE`].
    pub fn from_env() -> Result<Self, CredentialError> {
        let service = std::env::var(SERVICE_ENV)
            .ok()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_SERVICE.to_owned());
        if std::env::var(BACKEND_ENV).is_ok_and(|v| v == "mock") {
            return Ok(Self {
                store: PROCESS_MOCK.clone(),
                service,
                backend: Backend::Mock,
            });
        }
        if let Err(e) = keyring::Entry::store_status() {
            return Err(keychain_error(e));
        }
        let store = keyring_core::get_default_store()
            .ok_or_else(|| CredentialError::Keychain("no system keychain available".into()))?;
        Ok(Self {
            store,
            service,
            backend: Backend::System,
        })
    }

    /// A fresh in-memory store (unit tests).
    pub fn mock() -> Self {
        Self {
            store: new_mock(),
            service: DEFAULT_SERVICE.to_owned(),
            backend: Backend::Mock,
        }
    }

    pub fn service(&self) -> &str {
        &self.service
    }

    pub fn backend(&self) -> Backend {
        self.backend
    }

    /// Keychain user name of an account's secret: `<account id>/<kind>`.
    pub fn user(account: &str, kind: StoreKind) -> String {
        format!("{account}/{}", kind.as_str())
    }

    fn entry(&self, account: &str, kind: StoreKind) -> Result<Entry, CredentialError> {
        self.store
            .build(
                &self.service,
                &Self::user(account, kind),
                None::<&HashMap<&str, &str>>,
            )
            .map_err(|e| keychain_error(&e))
    }

    pub fn set(
        &self,
        account: &str,
        kind: StoreKind,
        secret: &Secret,
    ) -> Result<(), CredentialError> {
        self.entry(account, kind)?
            .set_password(secret.expose())
            .map_err(|e| keychain_error(&e))
    }

    pub fn get(&self, account: &str, kind: StoreKind) -> Result<Secret, CredentialError> {
        match self.entry(account, kind)?.get_password() {
            Ok(s) => Ok(Secret(s)),
            Err(keyring_core::Error::NoEntry) => {
                Err(CredentialError::Missing(Self::user(account, kind)))
            }
            Err(e) => Err(keychain_error(&e)),
        }
    }

    /// Removes the secret; a missing entry is fine.
    pub fn delete(&self, account: &str, kind: StoreKind) -> Result<(), CredentialError> {
        match self.entry(account, kind)?.delete_credential() {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(e) => Err(keychain_error(&e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apple_key_formats() {
        assert_eq!(validate_apple_key(&sample_p8_for_tests()), Ok(()));
        // An RSA PKCS#8 header (rsaEncryption OID) is rejected.
        let rsa = "-----BEGIN PRIVATE KEY-----\nMA0GCSqGSIb3DQEBAQUA\n-----END PRIVATE KEY-----";
        assert!(validate_apple_key(rsa).is_err());
        assert!(validate_apple_key("hello").is_err());
        assert!(
            validate_apple_key("-----BEGIN PRIVATE KEY-----\n!!!\n-----END PRIVATE KEY-----")
                .is_err()
        );
        assert!(validate_apple_key("-----BEGIN PRIVATE KEY-----\nAAAA").is_err());
        assert_eq!(
            apple_key_id_from_file_name("AuthKey_2X9R4HXF34.p8").as_deref(),
            Some("2X9R4HXF34")
        );
        assert_eq!(apple_key_id_from_file_name("key.p8"), None);
        assert!(is_apple_issuer_id("57246542-96fe-1a63-e053-0824d011072a"));
        assert!(!is_apple_issuer_id("57246542-96fe-1a63-e053"));
    }

    #[test]
    fn errors_never_contain_the_key() {
        let secret_body = "SECRETSECRETSECRET";
        for text in [
            format!(
                "{{\"type\": \"service_account\", \"private_key\": 12, \"client_email\": \"{secret_body}\"}}"
            ),
            format!("{{\"type\": \"user\", \"private_key\": \"{secret_body}\"}}"),
            format!("{{\"private_key\": \"{secret_body}\", oops}}"),
        ] {
            let e = validate_google_key(&text).unwrap_err().to_string();
            assert!(!e.contains(secret_body), "{e}");
        }
        let e = validate_apple_key(&format!(
            "-----BEGIN PRIVATE KEY-----\n{secret_body}!\n-----END PRIVATE KEY-----"
        ))
        .unwrap_err()
        .to_string();
        assert!(!e.contains(secret_body), "{e}");
        assert_eq!(
            format!("{:?}", Secret::new("abc".into())),
            "Secret(<redacted>)"
        );
    }

    #[test]
    fn google_key_formats() {
        let key = sample_p8_for_tests().replace('\n', "\\n");
        let json = format!(
            "{{\"type\": \"service_account\", \"project_id\": \"demo-1\", \"private_key\": \"{key}\", \
             \"client_email\": \"uploader@demo-1.iam.gserviceaccount.com\"}}"
        );
        let info = validate_google_key(&json).unwrap();
        assert_eq!(info.client_email, "uploader@demo-1.iam.gserviceaccount.com");
        assert_eq!(info.project_id.as_deref(), Some("demo-1"));
        assert!(validate_google_key(&json.replace("service_account", "authorized_user")).is_err());
        assert!(validate_google_key(&json.replace("uploader@", "uploader")).is_err());
        assert!(validate_google_key("[]").is_err());
    }

    #[test]
    fn mock_keychain_round_trip() {
        let store = SecretStore::mock();
        assert_eq!(store.backend(), Backend::Mock);
        assert_eq!(SecretStore::user("team", StoreKind::Apple), "team/apple");
        assert!(matches!(
            store.get("team", StoreKind::Apple),
            Err(CredentialError::Missing(_))
        ));
        let s = Secret::new(sample_p8_for_tests());
        store.set("team", StoreKind::Apple, &s).unwrap();
        assert_eq!(store.get("team", StoreKind::Apple).unwrap(), s);
        assert!(
            store.get("team", StoreKind::Google).is_err(),
            "kinds are separate"
        );
        store.delete("team", StoreKind::Apple).unwrap();
        store.delete("team", StoreKind::Apple).unwrap();
        assert!(store.get("team", StoreKind::Apple).is_err());
    }
}
