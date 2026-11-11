//! Configuration from `VITRINE_API_*` environment variables (Docker).
//!
//! Errors name the variable and, for lists, the position of the bad entry –
//! never its value (tokens are secrets).

use std::net::SocketAddr;
use std::path::PathBuf;

/// The public client id of the GitHub OAuth App (org `vitrinestudio-app`,
/// device flow enabled). Not a secret.
pub const DEFAULT_GITHUB_CLIENT_ID: &str = "Ov23linU2WEHOoBNEFpq";
/// The org account that publishes the official templates.
pub const DEFAULT_OFFICIAL_LOGIN: &str = "vitrinestudio-app";

/// A static upload token (`VITRINE_API_UPLOAD_TOKENS`).
#[derive(Clone, PartialEq, Eq)]
pub struct StaticToken {
    pub token: String,
    /// GitHub login the uploads are attributed to.
    pub login: String,
    /// Numeric GitHub user id, when given (`token:login:id`).
    pub github_id: Option<i64>,
}

impl std::fmt::Debug for StaticToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StaticToken")
            .field("token", &"<redacted>")
            .field("login", &self.login)
            .field("github_id", &self.github_id)
            .finish()
    }
}

/// Server configuration.
#[derive(Clone)]
pub struct Config {
    /// `VITRINE_API_BIND` (default `0.0.0.0:8080`).
    pub bind: SocketAddr,
    /// `VITRINE_API_DATA_DIR` (default `/data`): SQLite database and files.
    pub data_dir: PathBuf,
    /// `VITRINE_API_MAX_UPLOAD_MB` (default 50): request body limit of uploads.
    pub max_upload: usize,
    /// `VITRINE_API_ADMIN_TOKEN`: bearer token of the moderation endpoints
    /// (unset = moderation disabled).
    pub admin_token: Option<String>,
    /// `VITRINE_API_UPLOAD_TOKENS` = `token:login[:github_id],…` – static
    /// uploader tokens issued by the owner (accounts that cannot use the
    /// GitHub login, e.g. the org `vitrinestudio-app`; testers).
    pub upload_tokens: Vec<StaticToken>,
    /// `VITRINE_API_AUTO_APPROVE=1`: uploads are public at once (testing only).
    pub auto_approve: bool,
    /// `VITRINE_API_TRUST_PROXY=1`: take the client address from
    /// `X-Forwarded-For` (behind the reverse proxy).
    pub trust_proxy: bool,
    /// `VITRINE_API_RATE_LIMIT_PER_HOUR` (default 20): uploads, reports and
    /// login starts per client and hour.
    pub rate_limit_per_hour: u32,
    /// `VITRINE_API_READ_RATE_LIMIT_PER_HOUR` (default 3000): public reads
    /// (list, detail, previews, downloads) per client and hour.
    pub read_rate_limit_per_hour: u32,
    /// `VITRINE_API_PUBLIC_URL` (default `https://api.vitrinestudio.app`):
    /// base of download/preview links and of the OAuth callback.
    pub public_url: String,
    /// `VITRINE_API_GITHUB_CLIENT_ID` (default [`DEFAULT_GITHUB_CLIENT_ID`]).
    pub github_client_id: String,
    /// `VITRINE_API_GITHUB_CLIENT_SECRET`: only needed by the web flow
    /// (unset = web login answers 503; the device flow needs no secret).
    pub github_client_secret: Option<String>,
    /// `VITRINE_API_SESSION_DAYS` (default 90): lifetime of session tokens.
    pub session_days: u32,
    /// `VITRINE_API_OFFICIAL_LOGINS` (default `vitrinestudio-app`): uploader
    /// logins whose templates are marked `official`.
    pub official_logins: Vec<String>,
    /// `VITRINE_API_WEB_LOGIN_RETURN_URL`: where the web flow sends the
    /// browser after login (token in the URL fragment); unset = the
    /// callback answers JSON.
    pub web_login_return_url: Option<String>,
    /// GitHub web base (`https://github.com`); tests point it to a fake.
    pub github_url: String,
    /// GitHub REST API base (`https://api.github.com`); tests: a fake.
    pub github_api_url: String,
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let set = |o: &Option<String>| if o.is_some() { "<set>" } else { "<unset>" };
        f.debug_struct("Config")
            .field("bind", &self.bind)
            .field("data_dir", &self.data_dir)
            .field("max_upload", &self.max_upload)
            .field("admin_token", &set(&self.admin_token))
            .field("upload_tokens", &self.upload_tokens)
            .field("auto_approve", &self.auto_approve)
            .field("trust_proxy", &self.trust_proxy)
            .field("rate_limit_per_hour", &self.rate_limit_per_hour)
            .field("read_rate_limit_per_hour", &self.read_rate_limit_per_hour)
            .field("public_url", &self.public_url)
            .field("github_client_id", &self.github_client_id)
            .field("github_client_secret", &set(&self.github_client_secret))
            .field("session_days", &self.session_days)
            .field("official_logins", &self.official_logins)
            .field("web_login_return_url", &self.web_login_return_url)
            .field("github_url", &self.github_url)
            .field("github_api_url", &self.github_api_url)
            .finish()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind: SocketAddr::from(([0, 0, 0, 0], 8080)),
            data_dir: PathBuf::from("/data"),
            max_upload: 50 * 1024 * 1024,
            admin_token: None,
            upload_tokens: Vec::new(),
            auto_approve: false,
            trust_proxy: false,
            rate_limit_per_hour: 20,
            read_rate_limit_per_hour: 3000,
            public_url: "https://api.vitrinestudio.app".to_owned(),
            github_client_id: DEFAULT_GITHUB_CLIENT_ID.to_owned(),
            github_client_secret: None,
            session_days: 90,
            official_logins: vec![DEFAULT_OFFICIAL_LOGIN.to_owned()],
            web_login_return_url: None,
            github_url: "https://github.com".to_owned(),
            github_api_url: "https://api.github.com".to_owned(),
        }
    }
}

/// GitHub logins: 1–39 ASCII letters, digits and hyphens.
pub fn is_github_login(s: &str) -> bool {
    !s.is_empty() && s.len() <= 39 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Non-empty, trimmed entries of a comma-separated list with their
/// 1-based position among the non-empty entries.
fn entries(v: &str) -> impl Iterator<Item = (usize, &str)> {
    v.split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .enumerate()
        .map(|(i, e)| (i + 1, e))
}

fn parse_upload_tokens(v: &str) -> anyhow::Result<Vec<StaticToken>> {
    const VAR: &str = "VITRINE_API_UPLOAD_TOKENS";
    let mut out: Vec<StaticToken> = Vec::new();
    for (pos, entry) in entries(v) {
        let bad = |why: &str| {
            anyhow::anyhow!(
                "{VAR}: entry {pos} {why} (expected token:login or token:login:github_id)"
            )
        };
        let mut parts = entry.split(':').map(str::trim);
        let token = parts.next().unwrap_or_default();
        let login = parts.next().ok_or_else(|| bad("has no login"))?;
        let id = parts.next();
        if parts.next().is_some() {
            return Err(bad("has too many `:`"));
        }
        if token.is_empty() {
            return Err(bad("has an empty token"));
        }
        if !is_github_login(login) {
            return Err(bad("has an invalid GitHub login"));
        }
        let github_id = match id {
            None => None,
            Some(id) => Some(
                id.parse::<i64>()
                    .ok()
                    .filter(|n| *n > 0)
                    .ok_or_else(|| bad("has an invalid GitHub user id"))?,
            ),
        };
        if out.iter().any(|t| t.token == token) {
            return Err(bad("repeats a token of an earlier entry"));
        }
        out.push(StaticToken {
            token: token.to_owned(),
            login: login.to_owned(),
            github_id,
        });
    }
    Ok(out)
}

fn parse_logins(var: &str, v: &str) -> anyhow::Result<Vec<String>> {
    entries(v)
        .map(|(pos, login)| {
            if is_github_login(login) {
                Ok(login.to_owned())
            } else {
                Err(anyhow::anyhow!(
                    "{var}: entry {pos} is not a valid GitHub login"
                ))
            }
        })
        .collect()
}

fn parse<T: std::str::FromStr>(var: &str, v: &str) -> anyhow::Result<T> {
    v.trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("{var}: cannot parse `{}`", v.trim()))
}

impl Config {
    /// Reads the `VITRINE_API_*` variables (unset or empty = default).
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_lookup(|k| std::env::var(k).ok())
    }

    /// Same as [`Self::from_env`] with an explicit lookup (tests).
    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> anyhow::Result<Self> {
        let mut c = Self::default();
        let get = |k: &str| get(k).filter(|v| !v.trim().is_empty());
        if let Some(v) = get("VITRINE_API_BIND") {
            c.bind = parse("VITRINE_API_BIND", &v)?;
        }
        if let Some(v) = get("VITRINE_API_DATA_DIR") {
            c.data_dir = PathBuf::from(v.trim());
        }
        if let Some(v) = get("VITRINE_API_MAX_UPLOAD_MB") {
            c.max_upload = parse::<usize>("VITRINE_API_MAX_UPLOAD_MB", &v)? * 1024 * 1024;
        }
        c.admin_token = get("VITRINE_API_ADMIN_TOKEN").map(|v| v.trim().to_owned());
        if let Some(v) = get("VITRINE_API_UPLOAD_TOKENS") {
            c.upload_tokens = parse_upload_tokens(&v)?;
        }
        let flag = |v: Option<String>| v.is_some_and(|v| matches!(v.trim(), "1" | "true" | "yes"));
        c.auto_approve = flag(get("VITRINE_API_AUTO_APPROVE"));
        c.trust_proxy = flag(get("VITRINE_API_TRUST_PROXY"));
        if let Some(v) = get("VITRINE_API_RATE_LIMIT_PER_HOUR") {
            c.rate_limit_per_hour = parse("VITRINE_API_RATE_LIMIT_PER_HOUR", &v)?;
        }
        if let Some(v) = get("VITRINE_API_READ_RATE_LIMIT_PER_HOUR") {
            c.read_rate_limit_per_hour = parse("VITRINE_API_READ_RATE_LIMIT_PER_HOUR", &v)?;
        }
        if let Some(v) = get("VITRINE_API_PUBLIC_URL") {
            v.trim().trim_end_matches('/').clone_into(&mut c.public_url);
        }
        if let Some(v) = get("VITRINE_API_GITHUB_CLIENT_ID") {
            v.trim().clone_into(&mut c.github_client_id);
        }
        c.github_client_secret =
            get("VITRINE_API_GITHUB_CLIENT_SECRET").map(|v| v.trim().to_owned());
        if let Some(v) = get("VITRINE_API_SESSION_DAYS") {
            c.session_days = parse("VITRINE_API_SESSION_DAYS", &v)?;
            if c.session_days == 0 {
                anyhow::bail!("VITRINE_API_SESSION_DAYS: must be at least 1");
            }
        }
        if let Some(v) = get("VITRINE_API_OFFICIAL_LOGINS") {
            c.official_logins = parse_logins("VITRINE_API_OFFICIAL_LOGINS", &v)?;
        }
        c.web_login_return_url =
            get("VITRINE_API_WEB_LOGIN_RETURN_URL").map(|v| v.trim().to_owned());
        Ok(c)
    }

    /// Is `login` one of the official accounts (case-insensitive, like
    /// GitHub logins)?
    pub fn is_official(&self, login: &str) -> bool {
        self.official_logins
            .iter()
            .any(|o| o.eq_ignore_ascii_case(login))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(pairs: &[(&str, &str)]) -> anyhow::Result<Config> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        Config::from_lookup(|k| pairs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()))
    }

    #[test]
    fn reads_variables() {
        let c = with(&[
            ("VITRINE_API_BIND", "127.0.0.1:9000"),
            ("VITRINE_API_UPLOAD_TOKENS", "abc:octocat, def:other:42"),
            ("VITRINE_API_AUTO_APPROVE", "1"),
            ("VITRINE_API_MAX_UPLOAD_MB", "5"),
            ("VITRINE_API_PUBLIC_URL", "https://example.test/"),
            ("VITRINE_API_SESSION_DAYS", "7"),
        ])
        .unwrap();
        assert_eq!(c.bind.port(), 9000);
        assert_eq!(c.upload_tokens.len(), 2);
        assert_eq!(c.upload_tokens[1].login, "other");
        assert_eq!(c.upload_tokens[1].github_id, Some(42));
        assert_eq!(c.upload_tokens[0].github_id, None);
        assert!(c.auto_approve && !c.trust_proxy);
        assert_eq!(c.max_upload, 5 * 1024 * 1024);
        assert_eq!(c.public_url, "https://example.test");
        assert_eq!(c.session_days, 7);
        assert_eq!(c.github_client_id, DEFAULT_GITHUB_CLIENT_ID);
        assert!(c.is_official("VitrineStudio-App"));
        assert!(!c.is_official("octocat"));
    }

    #[test]
    fn list_entries_ignore_empty_parts() {
        let c = with(&[
            ("VITRINE_API_UPLOAD_TOKENS", " abc:octocat , ,def:other,"),
            (
                "VITRINE_API_OFFICIAL_LOGINS",
                "vitrinestudio-app, ,looking4cache,",
            ),
        ])
        .unwrap();
        assert_eq!(c.upload_tokens.len(), 2);
        assert_eq!(c.upload_tokens[0].token, "abc");
        assert_eq!(c.official_logins, ["vitrinestudio-app", "looking4cache"]);
        let c = with(&[("VITRINE_API_UPLOAD_TOKENS", " , ")]).unwrap();
        assert_eq!(c.upload_tokens.len(), 0);
    }

    #[test]
    fn bad_entries_name_the_position_not_the_token() {
        for (value, pos) in [
            ("good:octocat,SECRETVALUE", "entry 2"),
            ("SECRETVALUE:", "entry 1"),
            (":octocat", "entry 1"),
            ("good:octocat, SECRETVALUE:bad login", "entry 2"),
            ("SECRETVALUE:octocat:x", "entry 1"),
            ("SECRETVALUE:octocat:1:2", "entry 1"),
            ("SECRETVALUE:a,SECRETVALUE:b", "entry 2"),
        ] {
            let e = with(&[("VITRINE_API_UPLOAD_TOKENS", value)])
                .unwrap_err()
                .to_string();
            assert!(e.contains(pos), "{value}: {e}");
            assert!(e.contains("VITRINE_API_UPLOAD_TOKENS"), "{e}");
            assert!(!e.contains("SECRETVALUE"), "{e}");
        }
        let e = with(&[("VITRINE_API_OFFICIAL_LOGINS", "ok,,not ok")])
            .unwrap_err()
            .to_string();
        assert!(e.contains("entry 2"), "{e}");
        let e = with(&[("VITRINE_API_BIND", "nope")])
            .unwrap_err()
            .to_string();
        assert!(e.contains("VITRINE_API_BIND"), "{e}");
        assert!(with(&[("VITRINE_API_SESSION_DAYS", "0")]).is_err());
    }

    #[test]
    fn debug_never_prints_secrets() {
        let c = Config {
            admin_token: Some("ADMINSECRET".into()),
            github_client_secret: Some("CLIENTSECRET".into()),
            upload_tokens: vec![StaticToken {
                token: "UPLOADSECRET".into(),
                login: "octocat".into(),
                github_id: None,
            }],
            ..Config::default()
        };
        let s = format!("{c:?}");
        for secret in ["ADMINSECRET", "CLIENTSECRET", "UPLOADSECRET"] {
            assert!(!s.contains(secret), "{s}");
        }
    }
}
