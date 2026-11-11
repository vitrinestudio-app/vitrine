//! Session tokens and pending OAuth states (SQLite).
//!
//! Tokens are 32 random bytes, hex-encoded with the prefix `vss_`; only
//! their SHA-256 hash is stored. OAuth states (web flow) are stored the
//! same way, single use, 10 minutes.

use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};

use crate::db::now;

/// Prefix of session tokens (recognisable in logs and secret scanners).
pub const TOKEN_PREFIX: &str = "vss_";
/// Lifetime of a pending web-flow state.
pub const STATE_TTL_SECS: i64 = 600;

/// `n` random bytes as lowercase hex.
pub fn random_hex(n: usize) -> Result<String, getrandom::Error> {
    let mut buf = vec![0u8; n];
    getrandom::fill(&mut buf)?;
    Ok(hex(&buf))
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

pub fn hash(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes()))
}

/// A signed-in GitHub user.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    pub login: String,
    pub github_id: i64,
    pub expires_at: i64,
}

/// Issues a new session token for the GitHub user (and drops expired
/// sessions and states). Returns the token (shown once) and its expiry.
pub fn issue(
    db: &Connection,
    login: &str,
    github_id: i64,
    days: u32,
) -> anyhow::Result<(String, i64)> {
    let token = format!(
        "{TOKEN_PREFIX}{}",
        random_hex(32).map_err(|e| anyhow::anyhow!("random: {e}"))?
    );
    let now = now();
    let expires_at = now + i64::from(days) * 86_400;
    db.execute("DELETE FROM sessions WHERE expires_at <= ?1", params![now])?;
    db.execute(
        "DELETE FROM oauth_states WHERE expires_at <= ?1",
        params![now],
    )?;
    db.execute(
        "INSERT INTO sessions (token_hash, login, github_id, created_at, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![hash(&token), login, github_id, now, expires_at],
    )?;
    Ok((token, expires_at))
}

/// The live session of `token` (expired = none).
pub fn lookup(db: &Connection, token: &str) -> rusqlite::Result<Option<Session>> {
    if !token.starts_with(TOKEN_PREFIX) {
        return Ok(None);
    }
    db.query_row(
        "SELECT login, github_id, expires_at FROM sessions
         WHERE token_hash = ?1 AND expires_at > ?2",
        params![hash(token), now()],
        |r| {
            Ok(Session {
                login: r.get(0)?,
                github_id: r.get(1)?,
                expires_at: r.get(2)?,
            })
        },
    )
    .optional()
}

/// Revokes `token`; `false` when it was not a session token.
pub fn revoke(db: &Connection, token: &str) -> rusqlite::Result<bool> {
    let n = db.execute(
        "DELETE FROM sessions WHERE token_hash = ?1",
        params![hash(token)],
    )?;
    Ok(n > 0)
}

/// Stores a pending web-flow state with its PKCE verifier.
pub fn put_state(db: &Connection, state: &str, verifier: &str) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO oauth_states (state_hash, verifier, expires_at) VALUES (?1, ?2, ?3)",
        params![hash(state), verifier, now() + STATE_TTL_SECS],
    )?;
    Ok(())
}

/// Consumes a pending state (single use); returns its PKCE verifier when
/// it existed and had not expired.
pub fn take_state(db: &Connection, state: &str) -> rusqlite::Result<Option<String>> {
    let h = hash(state);
    let found: Option<(String, i64)> = db
        .query_row(
            "SELECT verifier, expires_at FROM oauth_states WHERE state_hash = ?1",
            [&h],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    db.execute("DELETE FROM oauth_states WHERE state_hash = ?1", [&h])?;
    Ok(found.and_then(|(v, exp)| (exp > now()).then_some(v)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        crate::db::init(&db).unwrap();
        db
    }

    #[test]
    fn issue_lookup_revoke() {
        let db = db();
        let (token, exp) = issue(&db, "octocat", 1, 90).unwrap();
        assert!(token.starts_with(TOKEN_PREFIX) && token.len() == 4 + 64);
        assert!(exp > now() + 89 * 86_400);
        // Only the hash is stored.
        let stored: String = db
            .query_row("SELECT token_hash FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_ne!(stored, token);
        let s = lookup(&db, &token).unwrap().unwrap();
        assert_eq!((s.login.as_str(), s.github_id), ("octocat", 1));
        assert!(lookup(&db, "vss_nope").unwrap().is_none());
        assert!(lookup(&db, "other").unwrap().is_none());
        assert!(revoke(&db, &token).unwrap());
        assert!(lookup(&db, &token).unwrap().is_none());
        assert!(!revoke(&db, &token).unwrap());
    }

    #[test]
    fn expired_sessions_are_refused_and_dropped() {
        let db = db();
        let (token, _) = issue(&db, "octocat", 1, 1).unwrap();
        db.execute("UPDATE sessions SET expires_at = ?1", [now() - 1])
            .unwrap();
        assert!(lookup(&db, &token).unwrap().is_none());
        // The next issue cleans up.
        issue(&db, "other", 2, 1).unwrap();
        let n: i64 = db
            .query_row("SELECT COUNT(*) FROM sessions", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn states_are_single_use_and_expire() {
        let db = db();
        put_state(&db, "s1", "v1").unwrap();
        assert_eq!(take_state(&db, "s1").unwrap().as_deref(), Some("v1"));
        assert_eq!(take_state(&db, "s1").unwrap(), None);
        put_state(&db, "s2", "v2").unwrap();
        db.execute("UPDATE oauth_states SET expires_at = 0", [])
            .unwrap();
        assert_eq!(take_state(&db, "s2").unwrap(), None);
    }
}
