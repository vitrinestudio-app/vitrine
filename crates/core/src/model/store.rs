//! Project ↔ store mapping (`store:` in `project.yaml`, KONZEPT §7,
//! Phase 5.1): which app in App Store Connect / Google Play the project
//! uploads to, with which account (a reference to an app-wide account; the
//! secrets live in the system keychain, never in the project), and the
//! project language → store locale map.
//!
//! ```yaml
//! store:
//!   apple: {account: my-team, app_id: "1234567890", bundle_id: com.example.app}
//!   google: {account: play, package_name: com.example.app}
//!   locales:
//!     apple: {en: en-GB}
//!     google: {de: de-DE}
//! ```
//!
//! Languages missing in `locales` use the built-in defaults
//! (`catalog::store_locales`, `data/store-locales.yaml`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::fmt::{flow_map, flow_style};
use super::ids::is_valid_id;

/// The two stores.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreKind {
    Apple,
    Google,
}

impl StoreKind {
    pub const ALL: [StoreKind; 2] = [StoreKind::Apple, StoreKind::Google];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apple => "apple",
            Self::Google => "google",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "apple" => Ok(Self::Apple),
            "google" => Ok(Self::Google),
            _ => Err(format!("unknown store `{s}` (apple, google)")),
        }
    }
}

impl std::fmt::Display for StoreKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Apple platform of the App Store version to upload to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApplePlatform {
    #[default]
    Ios,
    Macos,
}

impl ApplePlatform {
    #[allow(clippy::trivially_copy_pass_by_ref)] // serde passes references
    fn is_ios(&self) -> bool {
        *self == Self::Ios
    }
}

/// The `store:` section of `project.yaml`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apple: Option<AppleStore>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub google: Option<GoogleStore>,
    #[serde(default, skip_serializing_if = "StoreLocaleMaps::is_empty")]
    pub locales: StoreLocaleMaps,
}

/// App Store Connect app.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct AppleStore {
    /// Id of an app-wide store account (app settings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    /// Apple ID of the app (numeric, App Store Connect → App Information).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "string_or_number"
    )]
    pub app_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "ApplePlatform::is_ios")]
    pub platform: ApplePlatform,
}
flow_style!(AppleStore);

/// Google Play app.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct GoogleStore {
    /// Id of an app-wide store account (app settings).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_name: Option<String>,
}
flow_style!(GoogleStore);

/// Project language → store locale, per store (only explicit entries).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreLocaleMaps {
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "flow_map"
    )]
    pub apple: BTreeMap<String, String>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        serialize_with = "flow_map"
    )]
    pub google: BTreeMap<String, String>,
}

impl StoreLocaleMaps {
    pub fn is_empty(&self) -> bool {
        self.apple.is_empty() && self.google.is_empty()
    }

    pub fn get(&self, store: StoreKind) -> &BTreeMap<String, String> {
        match store {
            StoreKind::Apple => &self.apple,
            StoreKind::Google => &self.google,
        }
    }

    pub fn get_mut(&mut self, store: StoreKind) -> &mut BTreeMap<String, String> {
        match store {
            StoreKind::Apple => &mut self.apple,
            StoreKind::Google => &mut self.google,
        }
    }
}

/// Numbers are accepted for ids (`app_id: 1234567890` hand-written).
fn string_or_number<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        S(String),
        N(u64),
    }
    Ok(Option::<V>::deserialize(d)?.map(|v| match v {
        V::S(s) => s,
        V::N(n) => n.to_string(),
    }))
}

/// Reverse-DNS identifier (bundle id, package name): segments of letters,
/// digits, `_` (and `-` for Apple), separated by dots, at least two.
fn is_reverse_dns(s: &str, allow_dash: bool, segment_starts_with_letter: bool) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| {
            let mut chars = p.chars();
            let Some(first) = chars.next() else {
                return false;
            };
            (if segment_starts_with_letter {
                first.is_ascii_alphabetic()
            } else {
                first.is_ascii_alphanumeric()
            }) && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || (allow_dash && c == '-'))
        })
}

impl StoreSettings {
    pub fn is_empty(&self) -> bool {
        self.apple.is_none() && self.google.is_none() && self.locales.is_empty()
    }

    /// Is the project mapped to `store` (`store.<store>` present)?
    pub fn has(&self, store: StoreKind) -> bool {
        match store {
            StoreKind::Apple => self.apple.is_some(),
            StoreKind::Google => self.google.is_some(),
        }
    }

    /// Account id referenced for `store`.
    pub fn account(&self, store: StoreKind) -> Option<&str> {
        match store {
            StoreKind::Apple => self.apple.as_ref()?.account.as_deref(),
            StoreKind::Google => self.google.as_ref()?.account.as_deref(),
        }
    }

    /// Format checks of the values (cross checks with languages, targets
    /// and the locale lists are in `core::validate`; the ops additionally
    /// reject unknown locale codes).
    pub fn check(&self) -> Result<(), String> {
        if let Some(a) = &self.apple {
            if let Some(acc) = &a.account
                && !is_valid_id(acc)
            {
                return Err(format!("store.apple: invalid account id `{acc}`"));
            }
            if let Some(id) = &a.app_id
                && (id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()))
            {
                return Err(format!(
                    "store.apple: app_id `{id}` must be the numeric Apple ID of the app"
                ));
            }
            if let Some(b) = &a.bundle_id
                && !is_reverse_dns(b, true, false)
            {
                return Err(format!(
                    "store.apple: invalid bundle_id `{b}` (reverse DNS, e.g. com.example.app)"
                ));
            }
        }
        if let Some(g) = &self.google {
            if let Some(acc) = &g.account
                && !is_valid_id(acc)
            {
                return Err(format!("store.google: invalid account id `{acc}`"));
            }
            if let Some(p) = &g.package_name
                && !is_reverse_dns(p, false, true)
            {
                return Err(format!(
                    "store.google: invalid package_name `{p}` (e.g. com.example.app; \
                     segments start with a letter)"
                ));
            }
        }
        for store in StoreKind::ALL {
            for (lang, code) in self.locales.get(store) {
                if lang.trim().is_empty() || code.trim().is_empty() {
                    return Err(format!(
                        "store.locales.{store}: language and locale must not be empty"
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_formats() {
        assert!(is_reverse_dns("com.example.app", true, false));
        assert!(is_reverse_dns("com.example.my-app", true, false));
        assert!(!is_reverse_dns("com.example.my-app", false, true));
        assert!(!is_reverse_dns("com.1example", false, true));
        assert!(!is_reverse_dns("example", true, false));
        assert!(!is_reverse_dns("com..app", true, false));
        let mut s = StoreSettings {
            apple: Some(AppleStore {
                app_id: Some("12a".into()),
                ..AppleStore::default()
            }),
            ..StoreSettings::default()
        };
        assert!(s.check().is_err());
        s.apple.as_mut().unwrap().app_id = Some("1234567890".into());
        assert!(s.check().is_ok());
    }
}
