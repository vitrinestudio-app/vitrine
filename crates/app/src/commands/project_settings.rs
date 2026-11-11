//! Project settings: name, languages (add/remove/order/fallback, default
//! language with the KONZEPT §5.0 impact report) and style tokens. The UI
//! sends values; the ops are built here.

use std::collections::BTreeMap;

use serde::Deserialize;
use vitrine_core::catalog;
use vitrine_core::edit::{DefaultLanguageImpact, default_language_impact as impact};
use vitrine_core::model::{
    FontSettings, FrameStyle, Lang, Languages, Rgba, ShadowPreset, SpanStyle, StyleKind, Styles,
    TextStyle,
};
use vitrine_core::ops::Op;
use vitrine_core::resolve::FsAssets;

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
            lang_overrides: BTreeMap::new(),
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StyleRename {
    pub kind: StyleKind,
    pub from: String,
    pub to: String,
}

/// Partial update of the style tokens: per map `name → value` (`null`
/// removes the entry), the frame style (`null` = default), and an
/// optional rename (`Op::RenameStyle`: every reference follows – styles,
/// elements, variant and language overrides, span tags in translations –
/// in the same undo step).
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
    /// Per-family font settings (fallback lists).
    #[serde(default)]
    pub fonts: BTreeMap<String, Option<FontSettings>>,
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

pub fn styles_patch(app: &AppState, args: StylesPatchArgs) -> CmdResult<ApplyResult> {
    let session = app.session()?;
    let mut ops = Vec::new();
    let mut scratch = session.snapshot();
    if let Some(r) = &args.rename
        && r.from != r.to
    {
        let op = Op::RenameStyle {
            kind: r.kind,
            from: r.from.clone(),
            to: r.to.clone(),
        };
        scratch.apply(op.clone())?;
        ops.push(op);
    }
    let old = scratch.styles.clone();
    let mut styles = old.clone();
    merge(&mut styles.colors, args.colors);
    merge(&mut styles.text, args.text);
    merge(&mut styles.spans, args.spans);
    merge(&mut styles.shadows, args.shadows);
    merge(&mut styles.fonts, args.fonts);
    styles.fonts.retain(|_, f| !f.fallback.is_empty());
    if styles != old {
        let op = Op::SetStyles { styles };
        scratch.apply(op.clone())?;
        ops.push(op);
    }
    if let Some(frame) = args.frame {
        let frame = frame
            .map(|f| FrameStyle::parse(&f))
            .transpose()
            .map_err(CommandError::invalid)?;
        // A new default frame source remaps the colours of the devices that
        // inherit it (ADR 0012), in the same undo step.
        let index = app.frame_index();
        ops.extend(vitrine_core::frame_import::set_default_frame_ops(
            &scratch,
            frame,
            index.as_deref(),
        ));
    }
    if ops.is_empty() {
        return Ok(session.unchanged());
    }
    let label = match &args.rename {
        Some(r) if ops.len() == 1 => format!("Rename {} to {}", r.from, r.to),
        _ => "Edit styles".into(),
    };
    session.apply_ops(ops, Some(label))
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

        call(&app, "language_add", json!({"lang": "it"}));
        let s = call(&app, "project_summary", Value::Null);
        assert_eq!(
            s["languages"]["available"],
            json!(["en", "de", "fr", "fr-CA", "it"])
        );
        call(
            &app,
            "languages_set",
            json!({"order": ["it", "en", "de", "fr", "fr-CA"], "fallback": {"it": "de"}}),
        );
        let s = call(&app, "project_summary", Value::Null);
        assert_eq!(
            s["languages"]["available"],
            json!(["it", "en", "de", "fr", "fr-CA"])
        );
        assert_eq!(s["languages"]["fallback"], json!({"it": "de"}));
        assert!(dispatch(&app, "languages_set", json!({"order": ["it"]})).is_err());
        call(&app, "languages_set", json!({"fallback": {}}));
        call(&app, "language_remove", json!({"lang": "it"}));
        let s = call(&app, "project_summary", Value::Null);
        assert_eq!(
            s["languages"]["available"],
            json!(["en", "de", "fr", "fr-CA"])
        );
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

    #[test]
    fn renames_follow_element_references_in_one_step() {
        let (_tmp, app) = opened();
        let dir = std::path::PathBuf::from(
            call(&app, "project_summary", Value::Null)["dir"]
                .as_str()
                .unwrap(),
        );
        let undo_depth = |app| {
            call(app, "history_state", Value::Null)["undo"]
                .as_array()
                .map_or(0, Vec::len)
        };
        let before = undo_depth(&app);
        // Text style, shadow preset, span style and colour used by
        // elements and translations: renamed everywhere.
        for (kind, from, to) in [
            ("text", "headline", "title"),
            ("shadow", "card", "lift"),
            ("span", "green", "leafy"),
            ("color", "deep", "forest"),
        ] {
            call(
                &app,
                "styles_patch",
                json!({"rename": {"kind": kind, "from": from, "to": to}}),
            );
        }
        let yaml = std::fs::read_to_string(dir.join("project.yaml")).unwrap();
        assert!(!yaml.contains("style: headline"), "{yaml}");
        assert!(yaml.contains("style: title"));
        assert!(!yaml.contains("shadow: card") && yaml.contains("shadow: lift"));
        assert!(!yaml.contains("@deep"));
        let de = std::fs::read_to_string(dir.join("i18n").join("de.yaml")).unwrap();
        assert!(
            de.contains("[leafy]Zahlen[/]") && !de.contains("[green]"),
            "{de}"
        );
        // Each rename is one undo step.
        call(&app, "undo", Value::Null);
        let s = call(&app, "styles_get", Value::Null);
        assert!(s["colors"]["deep"].is_string());
        assert_eq!(undo_depth(&app), before + 3);
        assert!(
            s["text"]["title"].is_object(),
            "only the last rename undone"
        );
        // Rename + value change in one patch is still one step.
        call(
            &app,
            "styles_patch",
            json!({"rename": {"kind": "text", "from": "title", "to": "big"}, "text": {"big": {"size": 99}}}),
        );
        let s = call(&app, "styles_get", Value::Null);
        assert_eq!(s["text"]["big"]["size"], 99);
        call(&app, "undo", Value::Null);
        let s = call(&app, "styles_get", Value::Null);
        assert!(s["text"]["title"].is_object() && s["text"].get("big").is_none());
        // Font fallbacks.
        call(
            &app,
            "styles_patch",
            json!({"fonts": {"Inter": {"fallback": ["Noto Sans JP"]}}}),
        );
        let s = call(&app, "styles_get", Value::Null);
        assert_eq!(s["fonts"]["Inter"]["fallback"], json!(["Noto Sans JP"]));
        call(
            &app,
            "styles_patch",
            json!({"fonts": {"Inter": {"fallback": []}}}),
        );
        let s = call(&app, "styles_get", Value::Null);
        assert!(s.get("fonts").is_none(), "{s}");
    }

    #[test]
    fn styles_patch_text_effects() {
        let (_tmp, app) = opened();
        call(
            &app,
            "styles_patch",
            json!({
                "colors": {"rim": "#FFFFFF"},
                "text": {"fx": {"size": 80, "stroke": {"color": "@rim", "width": 4},
                                "glow": {"color": "@rim", "radius": 10, "spread": 2}}},
                "spans": {"pop": {"stroke": {"color": "@rim", "width": 2, "position": "center"},
                                  "shadow": "hard"}}
            }),
        );
        let s = call(&app, "styles_get", Value::Null);
        assert_eq!(s["text"]["fx"]["glow"]["spread"], 2);
        assert_eq!(s["spans"]["pop"]["stroke"]["position"], "center");
        // Renames follow outline/glow colours and span shadows.
        call(
            &app,
            "styles_patch",
            json!({"rename": {"kind": "color", "from": "rim", "to": "rim2"}}),
        );
        call(
            &app,
            "styles_patch",
            json!({"shadows": {"lift": {"layers": [{"color": "#00000040", "blur": 4}]}},
                   "spans": {"pop": {"shadow": "lift"}}}),
        );
        call(
            &app,
            "styles_patch",
            json!({"rename": {"kind": "shadow", "from": "lift", "to": "lift2"}}),
        );
        let s = call(&app, "styles_get", Value::Null);
        assert_eq!(s["text"]["fx"]["stroke"]["color"], "@rim2");
        assert_eq!(s["text"]["fx"]["glow"]["color"], "@rim2");
        assert_eq!(s["spans"]["pop"]["shadow"], "lift2");
        assert!(
            s["spans"]["pop"]["stroke"].is_null(),
            "replaced by the patch"
        );
        // Unknown references and bad values are refused.
        for bad in [
            json!({"spans": {"x": {"shadow": "nope"}}}),
            json!({"text": {"x": {"glow": {"color": "#000000", "radius": -1}}}}),
            json!({"text": {"x": {"stroke": {"color": "@nope", "width": 1}}}}),
        ] {
            assert!(
                dispatch(&app, "styles_patch", bad.clone()).is_err(),
                "{bad}"
            );
        }
    }
}
