//! Project settings: name, languages (add/remove/order/fallback, default
//! language with the KONZEPT §5.0 impact report) and style tokens. The UI
//! sends values; the ops are built here.

use std::collections::BTreeMap;

use arrangia_core::catalog;
use arrangia_core::edit::{DefaultLanguageImpact, default_language_impact as impact};
use arrangia_core::model::{
    ColorRef, FrameStyle, Lang, Languages, Rgba, ShadowPreset, SpanStyle, Styles, TextStyle,
};
use arrangia_core::ops::Op;
use arrangia_core::resolve::FsAssets;
use serde::{Deserialize, Serialize};

use super::edit::double_option;
use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError};
use crate::session::ApplyResult;

fn lang(s: &str) -> CmdResult<Lang> {
    Lang::parse(s).map_err(CommandError::invalid)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectRenameArgs {
    pub name: String,
}

pub fn project_rename(app: &AppState, args: ProjectRenameArgs) -> CmdResult<ApplyResult> {
    let name = args.name.trim().to_owned();
    if name.is_empty() {
        return Err(CommandError::invalid("the project name must not be empty"));
    }
    let session = app.session()?;
    if session.with_project(|p| p.name == name) {
        return Ok(session.unchanged());
    }
    session.apply_ops(vec![Op::SetName { name }], Some("Rename project".into()))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageArgs {
    pub lang: String,
}

pub fn language_add(app: &AppState, args: LanguageArgs) -> CmdResult<ApplyResult> {
    let lang = lang(&args.lang)?;
    app.session()?.apply_ops(
        vec![Op::AddLanguage {
            lang: lang.clone(),
            index: None,
            strings: BTreeMap::new(),
        }],
        Some(format!("Add language {lang}")),
    )
}

pub fn language_remove(app: &AppState, args: LanguageArgs) -> CmdResult<ApplyResult> {
    let lang = lang(&args.lang)?;
    app.session()?.apply_ops(
        vec![Op::RemoveLanguage { lang: lang.clone() }],
        Some(format!("Remove language {lang}")),
    )
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguagesSetArgs {
    /// The available languages in the new order (same set as today; add
    /// and remove languages with `language_add`/`language_remove`).
    #[serde(default)]
    pub order: Option<Vec<String>>,
    /// Extra fallbacks `lang → fallback lang` (replaces the map).
    #[serde(default)]
    pub fallback: Option<BTreeMap<String, String>>,
}

/// Reorders the languages and/or sets the fallback map.
pub fn languages_set(app: &AppState, args: LanguagesSetArgs) -> CmdResult<ApplyResult> {
    let session = app.session()?;
    let mut languages: Languages = session.with_project(|p| p.languages.clone());
    if let Some(order) = args.order {
        let order = order
            .iter()
            .map(|l| lang(l))
            .collect::<CmdResult<Vec<_>>>()?;
        let mut a: Vec<&str> = order.iter().map(Lang::as_str).collect();
        let mut b: Vec<&str> = languages.available.iter().map(Lang::as_str).collect();
        a.sort_unstable();
        b.sort_unstable();
        if a != b {
            return Err(CommandError::invalid(
                "`order` must list exactly the available languages",
            ));
        }
        languages.available = order;
    }
    if let Some(map) = args.fallback {
        languages.fallback = map
            .iter()
            .filter(|(_, v)| !v.is_empty())
            .map(|(k, v)| Ok((lang(k)?, lang(v)?)))
            .collect::<CmdResult<_>>()?;
    }
    if session.with_project(|p| p.languages == languages) {
        return Ok(session.unchanged());
    }
    session.apply_ops(
        vec![Op::SetLanguages { languages }],
        Some("Edit languages".into()),
    )
}

/// What would lose its fallback if `lang` became the default (§5.0).
pub fn default_language_impact(
    app: &AppState,
    args: LanguageArgs,
) -> CmdResult<DefaultLanguageImpact> {
    let session = app.session()?;
    let project = session.snapshot();
    let fs = FsAssets::new(session.dir());
    Ok(impact(
        &project,
        &lang(&args.lang)?,
        catalog::catalog(),
        &fs,
    )?)
}

pub fn default_language_set(app: &AppState, args: LanguageArgs) -> CmdResult<ApplyResult> {
    let lang = lang(&args.lang)?;
    app.session()?.apply_ops(
        vec![Op::SetDefaultLanguage { lang: lang.clone() }],
        Some(format!("Default language {lang}")),
    )
}

pub fn styles_get(app: &AppState, _: NoArgs) -> CmdResult<Styles> {
    Ok(app.session()?.with_project(|p| p.styles.clone()))
}

/// Which style map a rename applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StyleKind {
    Color,
    Text,
    Span,
    Shadow,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleRename {
    pub kind: StyleKind,
    pub from: String,
    pub to: String,
}

/// Partial update of the style tokens: per map `name → value` (`null`
/// removes the entry), the frame style (`null` = default), and an
/// optional rename (references inside the styles follow; references from
/// elements make the op fail, as for a removal).
#[allow(clippy::option_option)]
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StylesPatchArgs {
    #[serde(default)]
    pub colors: BTreeMap<String, Option<Rgba>>,
    #[serde(default)]
    pub text: BTreeMap<String, Option<TextStyle>>,
    #[serde(default)]
    pub spans: BTreeMap<String, Option<SpanStyle>>,
    #[serde(default)]
    pub shadows: BTreeMap<String, Option<ShadowPreset>>,
    #[serde(default, deserialize_with = "double_option")]
    pub frame: Option<Option<String>>,
    #[serde(default)]
    pub rename: Option<StyleRename>,
}

fn merge<T>(map: &mut BTreeMap<String, T>, patch: BTreeMap<String, Option<T>>) {
    for (k, v) in patch {
        match v {
            Some(v) => {
                map.insert(k, v);
            }
            None => {
                map.remove(&k);
            }
        }
    }
}

fn rename_key<T>(map: &mut BTreeMap<String, T>, from: &str, to: &str) -> CmdResult<()> {
    if map.contains_key(to) {
        return Err(CommandError::invalid(format!("`{to}` exists already")));
    }
    let v = map
        .remove(from)
        .ok_or_else(|| CommandError::not_found(format!("unknown style `{from}`")))?;
    map.insert(to.to_owned(), v);
    Ok(())
}

fn rename_color_ref(c: &mut ColorRef, from: &str, to: &str) {
    if matches!(c, ColorRef::Named(n) if n == from) {
        *c = ColorRef::Named(to.to_owned());
    }
}

fn apply_rename(styles: &mut Styles, r: &StyleRename) -> CmdResult<()> {
    let (from, to) = (r.from.as_str(), r.to.as_str());
    match r.kind {
        StyleKind::Color => {
            rename_key(&mut styles.colors, from, to)?;
            for c in styles.color_refs_mut() {
                rename_color_ref(c, from, to);
            }
        }
        StyleKind::Text => rename_key(&mut styles.text, from, to)?,
        StyleKind::Span => rename_key(&mut styles.spans, from, to)?,
        StyleKind::Shadow => {
            rename_key(&mut styles.shadows, from, to)?;
            let refs = styles
                .text
                .values_mut()
                .map(|t| &mut t.shadow)
                .chain(styles.spans.values_mut().map(|s| &mut s.shadow));
            for r in refs {
                if r.as_deref() == Some(from) {
                    *r = Some(to.to_owned());
                }
            }
        }
    }
    Ok(())
}

pub fn styles_patch(app: &AppState, args: StylesPatchArgs) -> CmdResult<ApplyResult> {
    let session = app.session()?;
    let old = session.with_project(|p| p.styles.clone());
    let mut styles = old.clone();
    if let Some(r) = &args.rename {
        apply_rename(&mut styles, r)?;
    }
    merge(&mut styles.colors, args.colors);
    merge(&mut styles.text, args.text);
    merge(&mut styles.spans, args.spans);
    merge(&mut styles.shadows, args.shadows);
    if let Some(frame) = args.frame {
        styles.frame = frame
            .map(|f| FrameStyle::parse(&f))
            .transpose()
            .map_err(CommandError::invalid)?;
    }
    if styles == old {
        return Ok(session.unchanged());
    }
    session.apply_ops(vec![Op::SetStyles { styles }], Some("Edit styles".into()))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use crate::commands::dispatch;
    use crate::commands::targets::tests::{call, opened};

    #[test]
    fn name_and_languages() {
        let (_tmp, app) = opened();
        call(&app, "project_rename", json!({"name": "  Renamed "}));
        assert_eq!(
            call(&app, "project_summary", Value::Null)["name"],
            "Renamed"
        );
        assert!(dispatch(&app, "project_rename", json!({"name": " "})).is_err());

        call(&app, "language_add", json!({"lang": "fr"}));
        let s = call(&app, "project_summary", Value::Null);
        assert_eq!(s["languages"]["available"], json!(["en", "de", "fr"]));
        call(
            &app,
            "languages_set",
            json!({"order": ["fr", "en", "de"], "fallback": {"fr": "de"}}),
        );
        let s = call(&app, "project_summary", Value::Null);
        assert_eq!(s["languages"]["available"], json!(["fr", "en", "de"]));
        assert_eq!(s["languages"]["fallback"], json!({"fr": "de"}));
        assert!(dispatch(&app, "languages_set", json!({"order": ["fr"]})).is_err());
        call(&app, "languages_set", json!({"fallback": {}}));
        call(&app, "language_remove", json!({"lang": "fr"}));
        let s = call(&app, "project_summary", Value::Null);
        assert_eq!(s["languages"]["available"], json!(["en", "de"]));
    }

    #[test]
    fn default_language_impact_then_switch() {
        let (_tmp, app) = opened();
        let impact = call(&app, "default_language_impact", json!({"lang": "de"}));
        assert_eq!(impact["from"], "en");
        assert_eq!(impact["to"], "de");
        // The example has English-only screenshots German falls back to.
        assert_ne!(impact["screenshots"], json!([]));
        assert!(dispatch(&app, "default_language_impact", json!({"lang": "xx"})).is_err());
        call(&app, "default_language_set", json!({"lang": "de"}));
        let s = call(&app, "project_summary", Value::Null);
        assert_eq!(s["languages"]["default"], "de");
    }

    #[test]
    fn styles_patch_add_rename_remove() {
        let (_tmp, app) = opened();
        let styles = call(&app, "styles_get", Value::Null);
        assert!(styles["colors"]["leaf"].is_string());

        call(
            &app,
            "styles_patch",
            json!({"colors": {"brand": "#112233"}, "text": {"tiny": {"size": 20, "color": "@brand"}}, "frame": "stylized"}),
        );
        let s = call(&app, "styles_get", Value::Null);
        assert_eq!(s["colors"]["brand"], "#112233");
        assert_eq!(s["frame"], "stylized");

        // a rename follows references inside the styles
        call(
            &app,
            "styles_patch",
            json!({"rename": {"kind": "color", "from": "brand", "to": "brand2"}}),
        );
        let s = call(&app, "styles_get", Value::Null);
        assert_eq!(s["text"]["tiny"]["color"], "@brand2");
        assert!(s["colors"].get("brand").is_none());

        // a colour still referenced cannot be removed
        assert!(dispatch(&app, "styles_patch", json!({"colors": {"brand2": null}})).is_err());
        call(
            &app,
            "styles_patch",
            json!({"text": {"tiny": null}, "colors": {"brand2": null}, "frame": null}),
        );
        let s = call(&app, "styles_get", Value::Null);
        assert!(s["text"].get("tiny").is_none());
        assert!(s.get("frame").is_none());

        let r = call(&app, "styles_patch", json!({}));
        assert_eq!(r["applied"], false);
    }
}
