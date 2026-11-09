//! What changing the default language would break (KONZEPT §5.0).

use std::collections::BTreeSet;

use serde::Serialize;

use super::EditError;
use crate::catalog::Catalog;
use crate::model::{ElementKind, Lang, Project, StringKey};
use crate::ops::OpError;
use crate::resolve::{AssetLookup, lang_chain, lookup_screenshot, lookup_text};

/// Texts and screenshots that resolve today but would have no source
/// (not even a fallback) with another default language.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct DefaultLanguageImpact {
    pub from: Lang,
    pub to: Lang,
    pub texts: Vec<LostText>,
    pub screenshots: Vec<LostScreenshot>,
}

impl DefaultLanguageImpact {
    /// Nothing would be lost.
    pub fn is_empty(&self) -> bool {
        self.texts.is_empty() && self.screenshots.is_empty()
    }
}

/// A text key that would be missing in `lang`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct LostText {
    pub lang: Lang,
    pub key: StringKey,
    /// Language the text comes from today.
    pub from: Lang,
}

/// A screenshot that would be missing in `lang` (for screenshot variant
/// `variant`).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct LostScreenshot {
    pub lang: Lang,
    pub key: String,
    pub variant: Option<String>,
    /// File used today.
    pub path: String,
}

/// Report what would lose its source if `new_default` became the default
/// language: every text key used by a text element (base or override) in
/// every available language, and every screenshot of every target in the
/// target's languages (screenshot variant of the target, else the
/// (mapped) device model's default variant).
pub fn default_language_impact(
    project: &Project,
    new_default: &Lang,
    catalog: &Catalog,
    assets: &dyn AssetLookup,
) -> Result<DefaultLanguageImpact, EditError> {
    if !project.languages.is_available(new_default.as_str()) {
        return Err(EditError::Op(OpError::UnknownLanguage(
            new_default.to_string(),
        )));
    }
    let mut after = project.clone();
    after.languages.default = new_default.clone();

    let mut keys = BTreeSet::new();
    for s in &project.strips {
        s.walk_elements(&mut |e| {
            if let ElementKind::Text(t) = &e.kind {
                keys.insert(t.text.clone());
            }
        });
        for v in s.variants.values() {
            for p in v.overrides.values() {
                if let Some(Some(k)) = &p.text {
                    keys.insert(k.clone());
                }
            }
        }
    }
    let mut texts = BTreeSet::new();
    for lang in &project.languages.available {
        let before_chain = lang_chain(project, lang.as_str());
        let after_chain = lang_chain(&after, lang.as_str());
        for key in &keys {
            if let Some((_, from)) = lookup_text(project, &before_chain, key.as_str())
                && lookup_text(&after, &after_chain, key.as_str()).is_none()
            {
                texts.insert(LostText {
                    lang: lang.clone(),
                    key: key.clone(),
                    from,
                });
            }
        }
    }

    let mut screenshots = BTreeSet::new();
    for t in &project.targets {
        let Some(strip) = project.strip(t.strip.as_str()) else {
            continue;
        };
        let overrides = t
            .variant
            .as_ref()
            .and_then(|v| strip.variants.get(v))
            .map(|v| &v.overrides);
        let mut wanted = BTreeSet::new();
        strip.walk_elements(&mut |e| {
            let e = overrides
                .and_then(|o| o.get(&e.id))
                .and_then(|p| e.patched(p).ok())
                .map_or_else(|| e.clone(), |(p, _)| p);
            if let ElementKind::Device(d) = &e.kind
                && let Some(key) = &d.screenshot
            {
                let model = t.device_map.get(&d.model).unwrap_or(&d.model);
                let variant = t.screenshots.clone().or_else(|| {
                    catalog
                        .device(model)
                        .and_then(|m| m.screenshot.variant.clone())
                });
                wanted.insert((key.clone(), variant));
            }
        });
        let langs = t
            .langs
            .clone()
            .unwrap_or_else(|| project.languages.available.clone());
        for lang in langs {
            let before_chain = lang_chain(project, lang.as_str());
            let after_chain = lang_chain(&after, lang.as_str());
            for (key, variant) in &wanted {
                let before = lookup_screenshot(assets, &before_chain, variant.as_deref(), key);
                let Some(path) = before.path else { continue };
                let now = lookup_screenshot(assets, &after_chain, variant.as_deref(), key);
                if now.path.is_none() {
                    screenshots.insert(LostScreenshot {
                        lang: lang.clone(),
                        key: key.clone(),
                        variant: variant.clone(),
                        path,
                    });
                }
            }
        }
    }
    Ok(DefaultLanguageImpact {
        from: project.languages.default.clone(),
        to: new_default.clone(),
        texts: texts.into_iter().collect(),
        screenshots: screenshots.into_iter().collect(),
    })
}
