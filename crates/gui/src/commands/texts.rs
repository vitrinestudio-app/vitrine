//! Text editing helpers: the resolved state of one element in a view
//! (`element_info`: text key, effective and own string, markup
//! diagnostics; device screenshot lookup) and markup checks of uncommitted
//! strings (`markup_check`). Strings are changed with the `set_string` op
//! via `apply_ops`; live previews of uncommitted strings come from
//! `render_preview_string`.

use arrangia_core::markup::{self, MarkupDiagnostic};
use arrangia_core::resolve::{ResolvedElement, ResolvedKind};
use serde::{Deserialize, Serialize};

use super::edit::double_option;
use super::render::scene;
use super::{AppState, lang_or_default};
use crate::error::{CmdResult, CommandError};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct ElementInfoArgs {
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub variant: Option<Option<String>>,
    pub id: String,
}

/// An element as resolved for the view.
#[derive(Debug, Clone, Serialize)]
pub struct ElementInfo {
    pub target: String,
    pub lang: String,
    /// The resolved element (`props.kind` = `text`, `device`, …).
    pub element: ResolvedElement,
    /// Texts: the string stored for the view's language itself (`None` =
    /// missing; the element then shows the fallback in `props.text`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub own_text: Option<String>,
    /// Texts: markup problems of the effective string.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<MarkupDiagnostic>,
}

pub fn element_info(app: &AppState, args: ElementInfoArgs) -> CmdResult<ElementInfo> {
    let session = app.session()?;
    let lang = lang_or_default(&session, args.lang);
    let (target, lang, scene) = scene(
        &session,
        None,
        args.target,
        Some(lang),
        args.variant.as_ref(),
    )?;
    let element = scene
        .element(&args.id)
        .cloned()
        .ok_or_else(|| CommandError::not_found(format!("no element `{}` in this view", args.id)))?;
    let (own_text, diagnostics) = match &element.props {
        ResolvedKind::Text(t) => (
            session.with_project(|p| p.string(&lang, t.key.as_str()).map(ToOwned::to_owned)),
            t.text
                .as_deref()
                .map(|s| markup::parse(s).diagnostics)
                .unwrap_or_default(),
        ),
        _ => (None, Vec::new()),
    };
    Ok(ElementInfo {
        target,
        lang,
        element,
        own_text,
        diagnostics,
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarkupCheckArgs {
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MarkupCheck {
    /// Text without markup (soft hyphens as U+00AD).
    pub visible: String,
    /// Named span styles used.
    pub spans: Vec<String>,
    pub diagnostics: Vec<MarkupDiagnostic>,
}

pub fn markup_check(_: &AppState, args: MarkupCheckArgs) -> CmdResult<MarkupCheck> {
    let m = markup::parse(&args.text);
    let mut spans: Vec<String> = m.spans.iter().filter_map(|s| s.style.clone()).collect();
    spans.dedup();
    Ok(MarkupCheck {
        visible: m.visible_text(),
        spans,
        diagnostics: m.diagnostics,
    })
}
