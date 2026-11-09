//! Language fallback chain (§5.1) and screenshot lookup chain (§5.2).

use std::collections::BTreeSet;
use std::path::PathBuf;

use super::scene::{ResolvedScreenshot, ScreenshotSource};
use crate::model::{Lang, Project};

/// Language chain for `lang`: the language itself, its shorter prefixes
/// (`de-AT` → `de`), then explicit `languages.fallback` entries (each again
/// with its prefixes), finally the default language. No duplicates.
///
/// `pt-BR` with `fallback: { pt-BR: es }` and default `en` → `pt-BR, pt, es, en`.
pub fn lang_chain(project: &Project, lang: &str) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    let mut seen = BTreeSet::new();
    let mut next = Some(lang.to_owned());
    while let Some(cur) = next.take() {
        let start = chain.len();
        let mut l = cur.as_str();
        loop {
            if seen.insert(l.to_owned()) {
                chain.push(l.to_owned());
            }
            match l.rfind('-') {
                Some(i) => l = &l[..i],
                None => break,
            }
        }
        // Explicit fallback for the most specific language that has one.
        next = chain[start..]
            .iter()
            .find_map(|l| project.languages.fallback.get(l.as_str()))
            .filter(|to| !seen.contains(to.as_str()))
            .map(|to| to.as_str().to_owned());
    }
    let default = project.languages.default.as_str();
    if seen.insert(default.to_owned()) {
        chain.push(default.to_owned());
    }
    chain
}

/// Text of `key` along the chain: `(text, language it came from)`.
pub fn lookup_text<'p>(
    project: &'p Project,
    chain: &[String],
    key: &str,
) -> Option<(&'p str, Lang)> {
    chain.iter().find_map(|l| {
        project.string(l, key).map(|t| {
            (
                t,
                Lang::parse(l).unwrap_or_else(|_| project.languages.default.clone()),
            )
        })
    })
}

/// File existence, abstracted so resolve needs no filesystem in tests and
/// the GUI can cache directory listings.
pub trait AssetLookup {
    /// Does the file `path` (relative to the project folder, `/`-separated)
    /// exist?
    fn exists(&self, path: &str) -> bool;
}

/// Looks files up below a project folder on disk.
#[derive(Clone, Debug)]
pub struct FsAssets {
    pub root: PathBuf,
}

impl FsAssets {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl AssetLookup for FsAssets {
    fn exists(&self, path: &str) -> bool {
        self.root.join(path).is_file()
    }
}

/// No files at all (every screenshot is a placeholder).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoAssets;

impl AssetLookup for NoAssets {
    fn exists(&self, _: &str) -> bool {
        false
    }
}

/// An in-memory set of relative paths.
impl AssetLookup for BTreeSet<String> {
    fn exists(&self, path: &str) -> bool {
        self.contains(path)
    }
}

/// Accepted screenshot file extensions, in lookup order.
pub const SCREENSHOT_EXTENSIONS: [&str; 3] = ["png", "jpg", "jpeg"];

/// Screenshot lookup: for every language of the chain first
/// `screenshots/<lang>/<variant>/<key>.<ext>`, then
/// `screenshots/<lang>/<key>.<ext>` (a file directly in the language folder
/// serves all variants). The first language of the chain is the requested
/// one; everything after it is a fallback.
pub fn lookup_screenshot(
    assets: &dyn AssetLookup,
    chain: &[String],
    variant: Option<&str>,
    key: &str,
) -> ResolvedScreenshot {
    for (i, lang) in chain.iter().enumerate() {
        let dirs = variant
            .map(|v| (format!("screenshots/{lang}/{v}"), true))
            .into_iter()
            .chain(std::iter::once((format!("screenshots/{lang}"), false)));
        for (dir, with_variant) in dirs {
            for ext in SCREENSHOT_EXTENSIONS {
                let path = format!("{dir}/{key}.{ext}");
                if assets.exists(&path) {
                    let source = match (i == 0, with_variant) {
                        (true, true) => ScreenshotSource::LangVariant,
                        (true, false) => ScreenshotSource::Lang,
                        (false, true) => ScreenshotSource::FallbackVariant,
                        (false, false) => ScreenshotSource::Fallback,
                    };
                    return ResolvedScreenshot {
                        key: key.to_owned(),
                        path: Some(path),
                        lang: Lang::parse(lang).ok(),
                        variant: variant.map(str::to_owned),
                        source,
                    };
                }
            }
        }
    }
    ResolvedScreenshot {
        key: key.to_owned(),
        path: None,
        lang: None,
        variant: variant.map(str::to_owned),
        source: ScreenshotSource::Missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Lang;

    fn project(fallback: &[(&str, &str)]) -> Project {
        let mut p = Project::new("t", Lang::new("en"));
        for (a, b) in fallback {
            p.languages.fallback.insert(Lang::new(a), Lang::new(b));
        }
        p
    }

    #[test]
    fn chains() {
        let p = project(&[("pt-BR", "es"), ("es", "fr")]);
        assert_eq!(lang_chain(&p, "de-AT"), ["de-AT", "de", "en"]);
        assert_eq!(lang_chain(&p, "en"), ["en"]);
        assert_eq!(lang_chain(&p, "pt-BR"), ["pt-BR", "pt", "es", "fr", "en"]);
        assert_eq!(
            lang_chain(&p, "zh-Hant-TW"),
            ["zh-Hant-TW", "zh-Hant", "zh", "en"]
        );
        // A fallback on the prefix applies to the longer code too.
        let p = project(&[("pt", "es")]);
        assert_eq!(lang_chain(&p, "pt-BR"), ["pt-BR", "pt", "es", "en"]);
        // Cycles terminate.
        let p = project(&[("a", "b"), ("b", "a")]);
        assert_eq!(lang_chain(&p, "a"), ["a", "b", "en"]);
    }

    #[test]
    fn text_lookup_reports_language() {
        let mut p = project(&[]);
        p.strings
            .entry(Lang::new("de"))
            .or_default()
            .insert(crate::model::StringKey::new("k"), "de text".into());
        let chain = lang_chain(&p, "de-AT");
        let (t, l) = lookup_text(&p, &chain, "k").unwrap();
        assert_eq!((t, l.as_str()), ("de text", "de"));
        assert!(lookup_text(&p, &chain, "nope").is_none());
    }

    #[test]
    fn screenshot_chain() {
        let files: BTreeSet<String> = [
            "screenshots/de/ios-phone/home.png",
            "screenshots/de/planner.jpg",
            "screenshots/en/ios-phone/harvest.jpeg",
            "screenshots/en/web.png",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let chain = ["de".to_owned(), "en".to_owned()];
        let s = |key| lookup_screenshot(&files, &chain, Some("ios-phone"), key);
        let home = s("home");
        assert_eq!(home.source, ScreenshotSource::LangVariant);
        assert_eq!(
            home.path.as_deref(),
            Some("screenshots/de/ios-phone/home.png")
        );
        assert_eq!(s("planner").source, ScreenshotSource::Lang);
        let harvest = s("harvest");
        assert_eq!(harvest.source, ScreenshotSource::FallbackVariant);
        assert_eq!(harvest.lang.unwrap().as_str(), "en");
        assert_eq!(s("web").source, ScreenshotSource::Fallback);
        let missing = s("nope");
        assert_eq!(missing.source, ScreenshotSource::Missing);
        assert!(missing.path.is_none());
        // Without variant only the language folders are searched.
        let none = lookup_screenshot(&files, &chain, None, "home");
        assert_eq!(none.source, ScreenshotSource::Missing);
        assert!(!NoAssets.exists("screenshots/de/planner.jpg"));
    }
}
