//! Store credentials from environment variables, for CI (no keychain).
//!
//! The CLI reads them for `vitrine upload …`; the GUI and the MCP server
//! never do. When a store's variables are set they win over the app-wide
//! account the project references (ADR 0016, amendment 2026-11-12).
//!
//! | store | variables |
//! |---|---|
//! | App Store Connect | `VITRINE_ASC_KEY_ID`, `VITRINE_ASC_ISSUER_ID`, and `VITRINE_ASC_KEY` (the `.p8` text, or it base64-encoded) **or** `VITRINE_ASC_KEY_PATH` (path of the `.p8` file) |
//! | Google Play | `VITRINE_PLAY_SERVICE_ACCOUNT` (the JSON text, or it base64-encoded) **or** `VITRINE_PLAY_SERVICE_ACCOUNT_PATH` (path of the JSON file) |
//!
//! Rules: a store whose variables are all unset (or empty) has no
//! environment credentials. As soon as one of them is set, the set must be
//! complete and valid, otherwise the CLI stops before anything else
//! happens – a half-configured CI job should fail, not fall back to some
//! other account. `VITRINE_ASC_KEY_ID` may be left out with
//! `VITRINE_ASC_KEY_PATH` pointing at `AuthKey_<KEYID>.p8`.
//!
//! The key text is only held in memory ([`Secret`]); nothing is written to
//! disk, and errors name variables, never their values.

use base64::Engine as _;

use crate::credentials::{
    CredentialError, GoogleAccountInfo, MAX_KEY_FILE, Secret, apple_key_id_from_file_name,
    is_apple_issuer_id, is_apple_key_id, validate_apple_key, validate_google_key,
};

/// App Store Connect API key id (10 characters).
pub const ENV_ASC_KEY_ID: &str = "VITRINE_ASC_KEY_ID";
/// App Store Connect issuer id (UUID).
pub const ENV_ASC_ISSUER_ID: &str = "VITRINE_ASC_ISSUER_ID";
/// The `.p8` key text (PEM, or the PEM base64-encoded).
pub const ENV_ASC_KEY: &str = "VITRINE_ASC_KEY";
/// Path of the `.p8` key file.
pub const ENV_ASC_KEY_PATH: &str = "VITRINE_ASC_KEY_PATH";
/// The Google service account JSON (text, or base64-encoded).
pub const ENV_PLAY_SERVICE_ACCOUNT: &str = "VITRINE_PLAY_SERVICE_ACCOUNT";
/// Path of the Google service account JSON file.
pub const ENV_PLAY_SERVICE_ACCOUNT_PATH: &str = "VITRINE_PLAY_SERVICE_ACCOUNT_PATH";

/// All variables, for docs and `--help`.
pub const ALL_VARS: &[&str] = &[
    ENV_ASC_KEY_ID,
    ENV_ASC_ISSUER_ID,
    ENV_ASC_KEY,
    ENV_ASC_KEY_PATH,
    ENV_PLAY_SERVICE_ACCOUNT,
    ENV_PLAY_SERVICE_ACCOUNT_PATH,
];

/// An App Store Connect team key from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvApple {
    pub issuer_id: String,
    pub key_id: String,
    pub key: Secret,
    /// Where the key came from (`VITRINE_ASC_KEY` or `VITRINE_ASC_KEY_PATH`).
    pub source: &'static str,
}

/// A Google Play service account from the environment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvGoogle {
    pub info: GoogleAccountInfo,
    pub key: Secret,
    /// `VITRINE_PLAY_SERVICE_ACCOUNT` or `VITRINE_PLAY_SERVICE_ACCOUNT_PATH`.
    pub source: &'static str,
}

/// Credentials found in the environment (each store optional).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnvCredentials {
    pub apple: Option<EnvApple>,
    pub google: Option<EnvGoogle>,
}

impl EnvCredentials {
    /// Reads the process environment.
    pub fn from_env() -> Result<Self, CredentialError> {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    /// Reads the variables through `get` (tests).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, CredentialError> {
        let var = |name: &str| get(name).filter(|v| !v.trim().is_empty());
        Ok(Self {
            apple: apple(&var)?,
            google: google(&var)?,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.apple.is_none() && self.google.is_none()
    }
}

fn invalid(msg: String) -> CredentialError {
    CredentialError::InvalidKey(msg)
}

/// Key text from a variable: PEM/JSON as is, else base64 (common for CI
/// secrets). `pem`: a single-line value with literal `\n` escapes is
/// unescaped (some CI secret stores flatten newlines).
fn decode_inline(
    name: &str,
    raw: &str,
    pem: bool,
    looks_right: impl Fn(&str) -> bool,
) -> Result<String, CredentialError> {
    let text = raw.trim();
    if looks_right(text) {
        if pem && !text.contains('\n') && text.contains("\\n") {
            return Ok(text.replace("\\n", "\n"));
        }
        return Ok(text.to_owned());
    }
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(compact.as_bytes())
        .map_err(|_| invalid(format!("`{name}` is neither the key text nor base64 of it")))?;
    let decoded = String::from_utf8(bytes)
        .map_err(|_| invalid(format!("`{name}` (base64) does not decode to text")))?;
    if looks_right(decoded.trim()) {
        Ok(decoded)
    } else {
        Err(invalid(format!(
            "`{name}` (base64) does not contain the expected key"
        )))
    }
}

fn read_file(name: &str, path: &str) -> Result<String, CredentialError> {
    let p = std::path::Path::new(path.trim());
    let meta = std::fs::metadata(p)
        .map_err(|e| invalid(format!("`{name}`: cannot read the file ({})", e.kind())))?;
    if !meta.is_file() {
        return Err(invalid(format!("`{name}` is not a file")));
    }
    if meta.len() > MAX_KEY_FILE {
        return Err(invalid(format!(
            "`{name}`: the file is larger than {} KB; not a key file",
            MAX_KEY_FILE / 1024
        )));
    }
    let bytes = std::fs::read(p)
        .map_err(|e| invalid(format!("`{name}`: cannot read the file ({})", e.kind())))?;
    String::from_utf8(bytes).map_err(|_| invalid(format!("`{name}`: the file is not text")))
}

fn with_var(name: &str, e: CredentialError) -> CredentialError {
    match e {
        CredentialError::InvalidKey(m) => invalid(format!("`{name}`: {m}")),
        other => other,
    }
}

fn apple(var: &impl Fn(&str) -> Option<String>) -> Result<Option<EnvApple>, CredentialError> {
    let key_id = var(ENV_ASC_KEY_ID);
    let issuer = var(ENV_ASC_ISSUER_ID);
    let inline = var(ENV_ASC_KEY);
    let path = var(ENV_ASC_KEY_PATH);
    if key_id.is_none() && issuer.is_none() && inline.is_none() && path.is_none() {
        return Ok(None);
    }
    let (text, source) = match (inline, &path) {
        (Some(_), Some(_)) => {
            return Err(invalid(format!(
                "set either `{ENV_ASC_KEY}` or `{ENV_ASC_KEY_PATH}`, not both"
            )));
        }
        (Some(raw), None) => (
            decode_inline(ENV_ASC_KEY, &raw, true, |t| t.contains("-----BEGIN"))?,
            ENV_ASC_KEY,
        ),
        (None, Some(p)) => (read_file(ENV_ASC_KEY_PATH, p)?, ENV_ASC_KEY_PATH),
        (None, None) => {
            return Err(invalid(format!(
                "App Store Connect credentials from the environment need the key: set \
                 `{ENV_ASC_KEY}` (the .p8 text) or `{ENV_ASC_KEY_PATH}`"
            )));
        }
    };
    let key_id = match key_id {
        Some(k) => k.trim().to_owned(),
        None => path
            .as_deref()
            .and_then(|p| {
                std::path::Path::new(p.trim())
                    .file_name()
                    .and_then(|n| n.to_str())
                    .and_then(apple_key_id_from_file_name)
            })
            .ok_or_else(|| {
                invalid(format!(
                    "`{ENV_ASC_KEY_ID}` is not set (and the key file is not named \
                     AuthKey_<KEYID>.p8)"
                ))
            })?,
    };
    if !is_apple_key_id(&key_id) {
        return Err(invalid(format!(
            "`{ENV_ASC_KEY_ID}` is not an App Store Connect key id (10 letters/digits)"
        )));
    }
    let Some(issuer) = issuer.map(|i| i.trim().to_owned()) else {
        return Err(invalid(format!(
            "`{ENV_ASC_ISSUER_ID}` is not set (App Store Connect → Users and Access → \
             Integrations)"
        )));
    };
    if !is_apple_issuer_id(&issuer) {
        return Err(invalid(format!(
            "`{ENV_ASC_ISSUER_ID}` is not an issuer id (a UUID)"
        )));
    }
    validate_apple_key(&text).map_err(|e| with_var(source, e))?;
    Ok(Some(EnvApple {
        issuer_id: issuer,
        key_id,
        key: Secret::new(text),
        source,
    }))
}

fn google(var: &impl Fn(&str) -> Option<String>) -> Result<Option<EnvGoogle>, CredentialError> {
    let inline = var(ENV_PLAY_SERVICE_ACCOUNT);
    let path = var(ENV_PLAY_SERVICE_ACCOUNT_PATH);
    let (text, source) = match (inline, path) {
        (None, None) => return Ok(None),
        (Some(_), Some(_)) => {
            return Err(invalid(format!(
                "set either `{ENV_PLAY_SERVICE_ACCOUNT}` or `{ENV_PLAY_SERVICE_ACCOUNT_PATH}`, \
                 not both"
            )));
        }
        (Some(raw), None) => (
            decode_inline(ENV_PLAY_SERVICE_ACCOUNT, &raw, false, |t| {
                t.starts_with('{')
            })?,
            ENV_PLAY_SERVICE_ACCOUNT,
        ),
        (None, Some(p)) => (
            read_file(ENV_PLAY_SERVICE_ACCOUNT_PATH, &p)?,
            ENV_PLAY_SERVICE_ACCOUNT_PATH,
        ),
    };
    let info = validate_google_key(&text).map_err(|e| with_var(source, e))?;
    Ok(Some(EnvGoogle {
        info,
        key: Secret::new(text),
        source,
    }))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::credentials::sample_p8_for_tests;

    const ISSUER: &str = "57246542-96fe-1a63-e053-0824d011072a";

    fn sa_json() -> String {
        let kind = ["PRIVATE", "KEY"].join(" ");
        serde_json::json!({
            "type": "service_account",
            "project_id": "demo",
            "client_email": "ci@demo.iam.gserviceaccount.com",
            "private_key": format!("-----BEGIN {kind}-----\nAAEC\n-----END {kind}-----\n"),
        })
        .to_string()
    }

    fn read(vars: &[(&str, String)]) -> Result<EnvCredentials, CredentialError> {
        let map: HashMap<String, String> = vars
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect();
        EnvCredentials::from_lookup(|n| map.get(n).cloned())
    }

    fn b64(s: &str) -> String {
        base64::engine::general_purpose::STANDARD.encode(s)
    }

    #[test]
    fn nothing_set_is_empty() {
        let c = read(&[]).unwrap();
        assert!(c.is_empty());
        // Empty values count as unset (CI often defines missing secrets as "").
        let c = read(&[
            (ENV_ASC_KEY_ID, String::new()),
            (ENV_PLAY_SERVICE_ACCOUNT, "  ".into()),
        ])
        .unwrap();
        assert!(c.is_empty());
    }

    #[test]
    fn apple_inline_pem_base64_and_escaped() {
        let pem = sample_p8_for_tests();
        for value in [pem.clone(), b64(&pem), pem.replace('\n', "\\n")] {
            let c = read(&[
                (ENV_ASC_KEY_ID, "ABCDE12345".into()),
                (ENV_ASC_ISSUER_ID, ISSUER.into()),
                (ENV_ASC_KEY, value),
            ])
            .unwrap();
            let a = c.apple.unwrap();
            assert_eq!(a.key_id, "ABCDE12345");
            assert_eq!(a.issuer_id, ISSUER);
            assert_eq!(a.source, ENV_ASC_KEY);
            assert!(validate_apple_key(a.key.expose()).is_ok());
            assert!(c.google.is_none());
        }
    }

    #[test]
    fn apple_key_file_gives_the_key_id() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("AuthKey_KEY9876543.p8");
        std::fs::write(&p, sample_p8_for_tests()).unwrap();
        let c = read(&[
            (ENV_ASC_ISSUER_ID, ISSUER.into()),
            (ENV_ASC_KEY_PATH, p.display().to_string()),
        ])
        .unwrap();
        let a = c.apple.unwrap();
        assert_eq!(a.key_id, "KEY9876543");
        assert_eq!(a.source, ENV_ASC_KEY_PATH);
    }

    #[test]
    fn incomplete_or_wrong_apple_sets_fail_without_values() {
        let secret_like = "S3CR3T-VALUE-THAT-MUST-NOT-LEAK";
        let cases: Vec<Vec<(&str, String)>> = vec![
            vec![(ENV_ASC_KEY_ID, "ABCDE12345".into())],
            vec![
                (ENV_ASC_KEY_ID, "ABCDE12345".into()),
                (ENV_ASC_KEY, sample_p8_for_tests()),
            ],
            vec![
                (ENV_ASC_ISSUER_ID, ISSUER.into()),
                (ENV_ASC_KEY, sample_p8_for_tests()),
            ],
            vec![
                (ENV_ASC_KEY_ID, "ABCDE12345".into()),
                (ENV_ASC_ISSUER_ID, ISSUER.into()),
                (ENV_ASC_KEY, secret_like.into()),
            ],
            vec![
                (ENV_ASC_KEY_ID, secret_like.into()),
                (ENV_ASC_ISSUER_ID, ISSUER.into()),
                (ENV_ASC_KEY, sample_p8_for_tests()),
            ],
            vec![
                (ENV_ASC_KEY_ID, "ABCDE12345".into()),
                (ENV_ASC_ISSUER_ID, ISSUER.into()),
                (ENV_ASC_KEY, sample_p8_for_tests()),
                (
                    ENV_ASC_KEY_PATH,
                    "/nonexistent/AuthKey_ABCDE12345.p8".into(),
                ),
            ],
            vec![
                (ENV_ASC_ISSUER_ID, ISSUER.into()),
                (ENV_ASC_KEY_PATH, format!("/nonexistent/{secret_like}.p8")),
            ],
        ];
        for vars in cases {
            let e = read(&vars).unwrap_err().to_string();
            assert!(e.contains("VITRINE_ASC_"), "{e}");
            assert!(!e.contains(secret_like), "{e}");
            assert!(!e.contains("MIGH"), "key material in: {e}");
        }
    }

    #[test]
    fn google_inline_base64_and_file() {
        let json = sa_json();
        for value in [json.clone(), b64(&json)] {
            let c = read(&[(ENV_PLAY_SERVICE_ACCOUNT, value)]).unwrap();
            let g = c.google.unwrap();
            assert_eq!(g.info.client_email, "ci@demo.iam.gserviceaccount.com");
            assert_eq!(g.source, ENV_PLAY_SERVICE_ACCOUNT);
            assert!(c.apple.is_none());
        }
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sa.json");
        std::fs::write(&p, &json).unwrap();
        let c = read(&[(ENV_PLAY_SERVICE_ACCOUNT_PATH, p.display().to_string())]).unwrap();
        assert_eq!(c.google.unwrap().source, ENV_PLAY_SERVICE_ACCOUNT_PATH);
        // Both → refused; a broken JSON names the variable, not the value.
        let e = read(&[
            (ENV_PLAY_SERVICE_ACCOUNT, json),
            (ENV_PLAY_SERVICE_ACCOUNT_PATH, p.display().to_string()),
        ])
        .unwrap_err();
        assert!(e.to_string().contains("not both"));
        let e = read(&[(
            ENV_PLAY_SERVICE_ACCOUNT,
            "{\"private_key\": \"zzz-secret\"".into(),
        )])
        .unwrap_err()
        .to_string();
        assert!(e.contains(ENV_PLAY_SERVICE_ACCOUNT), "{e}");
        assert!(!e.contains("zzz-secret"), "{e}");
    }

    #[test]
    fn debug_output_is_redacted() {
        let c = read(&[(ENV_PLAY_SERVICE_ACCOUNT, sa_json())]).unwrap();
        let dbg = format!("{c:?}");
        assert!(!dbg.contains("BEGIN"), "{dbg}");
        assert!(dbg.contains("<redacted>"));
    }
}
