//! # arrangia-publish
//!
//! Store upload, kept out of `arrangia-core` so the core stays free of
//! network code (`docs/KONZEPT.md`, section 7).
//!
//! Responsibilities:
//!
//! - **App Store Connect API**: API key (.p8) → ES256 JWT, screenshot sets per
//!   display type, reserve/upload/commit, ordering.
//! - **Google Play Developer API** (Android Publisher v3): service account →
//!   OAuth 2.0 token, edits, `deleteall`, upload, commit.
//! - **Plan → confirm → execute**: never upload without a dry-run plan and an
//!   explicit confirmation; resumable, logged to `.cache/uploads.sqlite`.
//! - **Secrets** only in the system keychain; configs hold references.
//!
//! Plain REST via `reqwest` + rustls, no vendor SDKs.

/// The stores Arrangia can publish to.
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
