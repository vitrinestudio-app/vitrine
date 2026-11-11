//! Configuration from environment variables (Docker).

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;

/// Server configuration.
#[derive(Clone, Debug)]
pub struct Config {
    /// `GALLERY_BIND` (default `0.0.0.0:8080`).
    pub bind: SocketAddr,
    /// `GALLERY_DATA_DIR` (default `/data`): SQLite database and files.
    pub data_dir: PathBuf,
    /// `GALLERY_MAX_UPLOAD_MB` (default 50): request body limit of uploads.
    pub max_upload: usize,
    /// `GALLERY_ADMIN_TOKEN`: bearer token of the moderation endpoints
    /// (unset = moderation disabled).
    pub admin_token: Option<String>,
    /// `GALLERY_UPLOAD_TOKENS` = `token:login,token2:login2` – interim
    /// uploader tokens until GitHub OAuth exists (TODO, ADR 0023).
    pub upload_tokens: BTreeMap<String, String>,
    /// `GALLERY_AUTO_APPROVE=1`: uploads are public at once (testing only).
    pub auto_approve: bool,
    /// `GALLERY_TRUST_PROXY=1`: take the client address from
    /// `X-Forwarded-For` (behind the reverse proxy).
    pub trust_proxy: bool,
    /// `GALLERY_RATE_LIMIT_PER_HOUR` (default 20): uploads and reports per
    /// client and hour.
    pub rate_limit_per_hour: u32,
    /// `GALLERY_PUBLIC_URL` (default `https://api.vitrinestudio.app`):
    /// base of download/preview links in responses.
    pub public_url: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from(([0, 0, 0, 0], 8080)),
            data_dir: PathBuf::from("/data"),
            max_upload: 50 * 1024 * 1024,
            admin_token: None,
            upload_tokens: BTreeMap::new(),
            auto_approve: false,
            trust_proxy: false,
            rate_limit_per_hour: 20,
            public_url: "https://api.vitrinestudio.app".to_owned(),
        }
    }
}

impl Config {
    /// Reads the `GALLERY_*` variables (unset = default).
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// Same as [`Self::from_env`] with an explicit lookup (tests).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let mut c = Self::default();
        let get = |k: &str| get(k).filter(|v| !v.trim().is_empty());
        if let Some(v) = get("GALLERY_BIND") {
            c.bind = v.parse()?;
        }
        if let Some(v) = get("GALLERY_DATA_DIR") {
            c.data_dir = PathBuf::from(v);
        }
        if let Some(v) = get("GALLERY_MAX_UPLOAD_MB") {
            c.max_upload = v.parse::<usize>()? * 1024 * 1024;
        }
        c.admin_token = get("GALLERY_ADMIN_TOKEN");
        if let Some(v) = get("GALLERY_UPLOAD_TOKENS") {
            for pair in v.split(',') {
                let (token, login) = pair.split_once(':').ok_or_else(|| {
                    anyhow::anyhow!("GALLERY_UPLOAD_TOKENS: expected token:login")
                })?;
                c.upload_tokens
                    .insert(token.trim().to_owned(), login.trim().to_owned());
            }
        }
        let flag =
            |v: Option<String>| v.is_some_and(|v| matches!(v.as_str(), "1" | "true" | "yes"));
        c.auto_approve = flag(get("GALLERY_AUTO_APPROVE"));
        c.trust_proxy = flag(get("GALLERY_TRUST_PROXY"));
        if let Some(v) = get("GALLERY_RATE_LIMIT_PER_HOUR") {
            c.rate_limit_per_hour = v.parse()?;
        }
        if let Some(v) = get("GALLERY_PUBLIC_URL") {
            v.trim_end_matches('/').clone_into(&mut c.public_url);
        }
        Ok(c)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_variables() {
        let c = Config::from_lookup(|k| match k {
            "GALLERY_BIND" => Some("127.0.0.1:9000".into()),
            "GALLERY_UPLOAD_TOKENS" => Some("abc:octocat, def:other".into()),
            "GALLERY_AUTO_APPROVE" => Some("1".into()),
            "GALLERY_MAX_UPLOAD_MB" => Some("5".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(c.bind.port(), 9000);
        assert_eq!(c.upload_tokens["def"], "other");
        assert!(c.auto_approve && !c.trust_proxy);
        assert_eq!(c.max_upload, 5 * 1024 * 1024);
        assert!(
            Config::from_lookup(|k| (k == "GALLERY_UPLOAD_TOKENS").then(|| "x".into())).is_err()
        );
    }
}
