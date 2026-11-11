//! Translations: read all strings, list missing ones. Strings are changed
//! with the `set_string` op via `apply_ops`.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use vitrine_core::Project;
use vitrine_core::model::ElementKind;
use vitrine_core::resolve::{lang_chain, lookup_text};

use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringsGetArgs {
    /// Only this language (default: all).
    #[serde(default)]
    pub lang: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StringsTable {
    pub default_lang: String,
    pub languages: Vec<String>,
    /// All keys: used by text elements or present in any language, sorted.
    pub keys: Vec<String>,
    /// lang → key → text.
    pub strings: BTreeMap<String, BTreeMap<String, String>>,
    /// Key → text elements using it (base elements, all canvases).
    pub used_by: BTreeMap<String, Vec<String>>,
}

/// A key without text in a language.
#[derive(Debug, Clone, Serialize)]
pub struct MissingString {
    pub lang: String,
    pub key: String,
    /// Text shown instead (fallback chain), if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    /// Language the fallback comes from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_lang: Option<String>,
    /// Text elements using the key.
    pub used_by: Vec<String>,
}

/// Keys used by text elements → element ids.
fn used_keys(p: &Project) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in &p.canvases {
        s.walk_elements(&mut |e| {
            if let ElementKind::Text(t) = &e.kind {
                out.entry(t.text.to_string())
                    .or_default()
                    .push(e.id.to_string());
            }
        });
    }
    out
}

fn all_keys(p: &Project) -> BTreeSet<String> {
    let mut keys: BTreeSet<String> = used_keys(p).into_keys().collect();
    for map in p.strings.values() {
        keys.extend(map.keys().map(ToString::to_string));
    }
    keys
}

pub fn strings_get(app: &AppState, args: StringsGetArgs) -> CmdResult<StringsTable> {
    let session = app.session()?;
    session.with_project(|p| {
        if let Some(l) = &args.lang
            && !p.languages.is_available(l)
        {
            return Err(CommandError::not_found(format!(
                "language `{l}` is not a project language"
            )));
        }
        let languages: Vec<String> = p
            .languages
            .available
            .iter()
            .map(ToString::to_string)
            .collect();
        let strings = p
            .strings
            .iter()
            .filter(|(l, _)| args.lang.as_deref().is_none_or(|x| x == l.as_str()))
            .map(|(l, m)| {
                (
                    l.to_string(),
                    m.iter().map(|(k, v)| (k.to_string(), v.clone())).collect(),
                )
            })
            .collect();
        Ok(StringsTable {
            default_lang: p.languages.default.to_string(),
            languages,
            keys: all_keys(p).into_iter().collect(),
            strings,
            used_by: used_keys(p),
        })
    })
}

/// Keys used by text elements or present in the default language that a
/// project language has no text for.
pub fn strings_missing(app: &AppState, _: NoArgs) -> CmdResult<Vec<MissingString>> {
    let session = app.session()?;
    Ok(session.with_project(|p| {
        let used = used_keys(p);
        let mut keys: BTreeSet<String> = used.keys().cloned().collect();
        if let Some(m) = p.strings.get(p.languages.default.as_str()) {
            keys.extend(m.keys().map(ToString::to_string));
        }
        let mut out = Vec::new();
        for lang in &p.languages.available {
            let chain = lang_chain(p, lang.as_str());
            for key in &keys {
                if p.string(lang.as_str(), key).is_some() {
                    continue;
                }
                let fallback = lookup_text(p, &chain, key);
                out.push(MissingString {
                    lang: lang.to_string(),
                    key: key.clone(),
                    fallback: fallback.as_ref().map(|(t, _)| (*t).to_owned()),
                    fallback_lang: fallback.map(|(_, l)| l.to_string()),
                    used_by: used.get(key).cloned().unwrap_or_default(),
                });
            }
        }
        out
    }))
}
