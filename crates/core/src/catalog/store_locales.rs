//! Store locale codes and default language mappings
//! (`data/store-locales.yaml`, KONZEPT §7.2).
//!
//! [`StoreLocales::resolve`] turns a project language into the locale code
//! the store API expects: explicit project map → the language itself when
//! it is a store code → `defaults` for the full tag → `defaults` for the
//! primary subtag.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use super::Source;
use crate::model::StoreKind;

const STORE_LOCALES_YAML: &str = include_str!("../../data/store-locales.yaml");

/// `store-locales.yaml`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreLocalesFile {
    /// Date the lists were last checked (`YYYY-MM-DD`).
    pub checked: String,
    pub sources: BTreeMap<String, Source>,
    pub apple: StoreLocales,
    pub google: StoreLocales,
}

/// The locale codes of one store.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreLocales {
    /// Source key of the code list.
    pub source: String,
    pub locales: Vec<StoreLocale>,
    /// Project language tag → store code.
    pub defaults: BTreeMap<String, String>,
}

/// One store locale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoreLocale {
    pub code: String,
    pub name: String,
}

/// How a project language got its store locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocaleSource {
    /// `store.locales.<store>` in `project.yaml`.
    Project,
    /// The language is a store code itself.
    Exact,
    /// Built-in default map.
    Default,
}

impl StoreLocales {
    /// The store's spelling of `code` (case-insensitive match), if known.
    pub fn code(&self, code: &str) -> Option<&str> {
        self.locales
            .iter()
            .find(|l| l.code.eq_ignore_ascii_case(code))
            .map(|l| l.code.as_str())
    }

    pub fn is_known(&self, code: &str) -> bool {
        self.locales.iter().any(|l| l.code == code)
    }

    /// Built-in mapping of `lang` (exact code, full tag, primary subtag);
    /// `None` = no default.
    pub fn default_for(&self, lang: &str) -> Option<(&str, LocaleSource)> {
        if let Some(c) = self.code(lang) {
            return Some((c, LocaleSource::Exact));
        }
        let lookup = |tag: &str| {
            self.defaults
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(tag))
                .map(|(_, v)| v.as_str())
        };
        if let Some(c) = lookup(lang) {
            return Some((c, LocaleSource::Default));
        }
        let primary = lang.split(['-', '_']).next().unwrap_or(lang);
        if primary != lang {
            if let Some(c) = self.code(primary) {
                return Some((c, LocaleSource::Default));
            }
            if let Some(c) = lookup(primary) {
                return Some((c, LocaleSource::Default));
            }
        }
        None
    }

    /// Store locale of `lang` given the project's explicit map.
    pub fn resolve<'a>(
        &'a self,
        explicit: &'a BTreeMap<String, String>,
        lang: &str,
    ) -> Option<(&'a str, LocaleSource)> {
        if let Some(c) = explicit.get(lang) {
            return Some((c.as_str(), LocaleSource::Project));
        }
        self.default_for(lang)
    }
}

impl StoreLocalesFile {
    pub fn parse(text: &str) -> Result<Self, String> {
        crate::io::from_yaml(text).map_err(|e| format!("store-locales.yaml: {e}"))
    }

    pub fn store(&self, store: StoreKind) -> &StoreLocales {
        match store {
            StoreKind::Apple => &self.apple,
            StoreKind::Google => &self.google,
        }
    }

    /// Consistency problems (empty = fine).
    pub fn check(&self) -> Vec<String> {
        let mut errors = Vec::new();
        for store in [StoreKind::Apple, StoreKind::Google] {
            let s = self.store(store);
            let what = store.as_str();
            if !self.sources.contains_key(&s.source) {
                errors.push(format!("{what}: unknown source `{}`", s.source));
            }
            let mut seen = std::collections::BTreeSet::new();
            for l in &s.locales {
                if !seen.insert(l.code.to_ascii_lowercase()) {
                    errors.push(format!("{what}: duplicate locale `{}`", l.code));
                }
                if l.code.trim().is_empty() || l.name.trim().is_empty() {
                    errors.push(format!("{what}: empty locale code or name"));
                }
            }
            for (k, v) in &s.defaults {
                if !s.is_known(v) {
                    errors.push(format!("{what}: default `{k}: {v}` is not a store locale"));
                }
                if s.code(k).is_some() {
                    errors.push(format!(
                        "{what}: default key `{k}` is a store code itself (redundant)"
                    ));
                }
            }
        }
        errors
    }
}

static STORE_LOCALES: LazyLock<StoreLocalesFile> = LazyLock::new(|| {
    StoreLocalesFile::parse(STORE_LOCALES_YAML)
        .unwrap_or_else(|e| panic!("built-in store locales are invalid: {e}"))
});

/// The built-in store locale lists (parsed on first use).
pub fn store_locales() -> &'static StoreLocalesFile {
    &STORE_LOCALES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_lists_are_consistent() {
        let f = store_locales();
        assert_eq!(f.check(), Vec::<String>::new());
        assert_eq!(f.apple.locales.len(), 50);
        assert_eq!(f.google.locales.len(), 86);
        assert!(f.apple.is_known("no") && f.google.is_known("no-NO"));
    }

    #[test]
    fn resolve_order() {
        let f = store_locales();
        let none = BTreeMap::new();
        let a = &f.apple;
        let g = &f.google;
        assert_eq!(
            a.resolve(&none, "de"),
            Some(("de-DE", LocaleSource::Default))
        );
        assert_eq!(
            a.resolve(&none, "de-AT"),
            Some(("de-DE", LocaleSource::Default))
        );
        assert_eq!(
            a.resolve(&none, "fr-CA"),
            Some(("fr-CA", LocaleSource::Exact))
        );
        assert_eq!(
            a.resolve(&none, "pt-br"),
            Some(("pt-BR", LocaleSource::Exact))
        );
        assert_eq!(a.resolve(&none, "ja"), Some(("ja", LocaleSource::Exact)));
        assert_eq!(
            g.resolve(&none, "ja"),
            Some(("ja-JP", LocaleSource::Default))
        );
        assert_eq!(
            g.resolve(&none, "he"),
            Some(("iw-IL", LocaleSource::Default))
        );
        assert_eq!(
            a.resolve(&none, "zh-Hant"),
            Some(("zh-Hant", LocaleSource::Exact))
        );
        assert_eq!(
            g.resolve(&none, "zh-Hant"),
            Some(("zh-TW", LocaleSource::Default))
        );
        assert_eq!(a.resolve(&none, "xx"), None);
        let explicit = BTreeMap::from([("en".to_owned(), "en-GB".to_owned())]);
        assert_eq!(
            a.resolve(&explicit, "en"),
            Some(("en-GB", LocaleSource::Project))
        );
    }
}
