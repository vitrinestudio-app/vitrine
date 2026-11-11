//! Inspector and layers commands: read one element as base / override /
//! effective values, patch fields (base or variant override decided in
//! core, [`Editor::patch`]), rename, re-arrange in the layer tree, and the
//! canvas settings (page geometry, backgrounds, variants).
//!
//! Like the `edit_*` commands every change is one undo step and answers
//! with an [`EditResult`]; a change that changes nothing answers
//! `applied: false`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use vitrine_core::Project;
use vitrine_core::model::{
    Aspect, Canvas, ElementId, ElementKindTag, ElementPatch, Fill, Lang, StringKey, Variant,
    VariantId,
};
use vitrine_core::ops::{CanvasPatch, Op, TargetPatch};
use vitrine_core::resolve::{lang_chain, lookup_text};

use super::AppState;
use super::edit::{EditResult, Plan, ViewContext, ctx_for, double_option, edit, with_editor};
use crate::error::{CmdResult, CommandError};

view_args! {
    pub struct ElementGetArgs {
        pub id: String,
    }
}

/// Text of a text element in the view language.
#[derive(Debug, Clone, Serialize)]
pub struct ElementText {
    pub key: String,
    pub lang: String,
    /// The string in `lang` itself (`None` = missing there).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// What is shown instead when `value` is missing: text and language
    /// from the fallback chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_lang: Option<String>,
}

/// One element as the inspector sees it.
#[derive(Debug, Clone, Serialize)]
pub struct ElementInfo {
    pub id: String,
    pub kind: ElementKindTag,
    pub canvas: String,
    /// Enclosing group.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// Direct children (groups).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    /// Variant edits are written to (`None` = base).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// The base element (YAML field names; without `children`).
    pub base: Value,
    /// The override of `variant` for this element.
    #[serde(rename = "override", skip_serializing_if = "Option::is_none")]
    pub r#override: Option<ElementPatch>,
    /// Base with the variant override, then the language override applied
    /// (what the view shows).
    pub effective: Value,
    /// Fields set by the override (YAML names).
    pub overridden: Vec<&'static str>,
    /// Language override(s) for the view language (texts), merged: `de`
    /// then `de-AT` for `de-AT`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lang_override: Option<ElementPatch>,
    /// Languages whose overrides apply, most general first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lang_override_langs: Vec<String>,
    /// Fields set by the language override(s) (YAML names).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub lang_overridden: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ElementText>,
}

fn element_json(e: &vitrine_core::model::Element) -> CmdResult<Value> {
    let mut v = serde_json::to_value(e).map_err(|e| CommandError::internal(e.to_string()))?;
    if let Value::Object(m) = &mut v {
        m.remove("children");
    }
    Ok(v)
}

pub fn element_get(app: &AppState, args: ElementGetArgs) -> CmdResult<ElementInfo> {
    let session = app.session()?;
    let project = session.snapshot();
    let ctx = args.view().edit_context(&session, &project)?;
    let lang = ctx.lang.to_string();
    let (variant, overridden, (lang_langs, lang_overridden)) =
        with_editor(&session, &project, &ctx, |ed| {
            // Checks that the element exists in the view's canvas.
            ed.kind(&args.id)
                .ok_or_else(|| vitrine_core::edit::EditError::UnknownElement(args.id.clone()))?;
            if project
                .element(&args.id)
                .is_none_or(|(s, _)| s.id != ed.canvas().id)
            {
                return Err(vitrine_core::edit::EditError::WrongCanvas {
                    element: args.id.clone(),
                    canvas: ed.canvas().id.to_string(),
                });
            }
            Ok((
                ed.variant().cloned(),
                ed.overridden_fields(&args.id),
                ed.lang_overridden_fields(&args.id),
            ))
        })?;
    let (canvas, e) = project
        .element(&args.id)
        .ok_or_else(|| CommandError::not_found(format!("unknown element `{}`", args.id)))?;
    let over = variant
        .as_ref()
        .and_then(|v| canvas.variants.get(v))
        .and_then(|v| v.overrides.get(&e.id))
        .cloned();
    let effective = match &over {
        Some(p) => e.patched(p).map_or_else(|_| e.clone(), |(x, _)| x),
        None => e.clone(),
    };
    let lang_override = e
        .text_props()
        .and_then(|t| t.lang_override(&lang))
        .map(|(_, p)| p);
    let effective = match effective.for_lang(&lang) {
        Ok(Some((_, x))) => x,
        _ => effective,
    };
    let text = match &effective.kind {
        vitrine_core::model::ElementKind::Text(t) => {
            let key = t.text.as_str();
            let value = project.string(&lang, key).map(str::to_owned);
            let (fallback, fallback_lang) = if value.is_none() {
                lookup_text(&project, &lang_chain(&project, &lang), key)
                    .map_or((None, None), |(s, l)| {
                        (Some(s.to_owned()), Some(l.to_string()))
                    })
            } else {
                (None, None)
            };
            Some(ElementText {
                key: key.to_owned(),
                lang: lang.clone(),
                value,
                fallback,
                fallback_lang,
            })
        }
        _ => None,
    };
    Ok(ElementInfo {
        id: e.id.to_string(),
        kind: e.kind_tag(),
        canvas: canvas.id.to_string(),
        parent: parent_of(canvas, &e.id),
        children: e.children().iter().map(|c| c.id.to_string()).collect(),
        variant: variant.map(|v| v.to_string()),
        base: element_json(e)?,
        r#override: over,
        effective: element_json(&effective)?,
        overridden,
        lang_override,
        lang_override_langs: lang_langs.iter().map(ToString::to_string).collect(),
        lang_overridden,
        text,
    })
}

fn parent_of(canvas: &Canvas, id: &ElementId) -> Option<String> {
    let mut out = None;
    canvas.walk_elements(&mut |e| {
        if e.children().iter().any(|c| &c.id == id) {
            out = Some(e.id.to_string());
        }
    });
    out
}

/// A translation to set along with an element patch.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StringEdit {
    pub lang: String,
    pub key: String,
    /// `None` removes the translation.
    #[serde(default)]
    pub value: Option<String>,
}

view_args! {
    pub struct ElementPatchArgs {
        /// The element (or use `ids` to patch several in one step).
        #[serde(default)]
        pub id: Option<String>,
        #[serde(default)]
        pub ids: Vec<String>,
        /// Fields to set (YAML names; `null` = reset: to the default in the
        /// base, out of the override in a variant or language override).
        /// In a variant view they go into the variant's override.
        #[serde(default)]
        pub patch: ElementPatch,
        /// Translations set in the same undo step (live text editing, a new
        /// key for a text element).
        #[serde(default)]
        pub strings: Vec<StringEdit>,
        /// More patches in the same undo step, each with its own view
        /// context (missing `target`/`lang`: those of the command), applied
        /// in order after `id`/`ids`.
        #[serde(default)]
        pub updates: Vec<PatchUpdate>,
    }
}

/// One entry of [`ElementPatchArgs::updates`].
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct PatchUpdate {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub ids: Vec<String>,
    #[serde(default)]
    pub patch: ElementPatch,
    #[serde(default)]
    pub strings: Vec<StringEdit>,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub lang: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub variant: Option<Option<String>>,
    #[serde(default)]
    pub scope: Option<vitrine_core::edit::EditScope>,
}

fn string_ops(project: &Project, edits: &[StringEdit]) -> CmdResult<Vec<Op>> {
    let mut ops = Vec::new();
    for s in edits {
        let lang = Lang::parse(&s.lang).map_err(CommandError::invalid)?;
        let key = StringKey::parse(&s.key).map_err(CommandError::invalid)?;
        let value = s.value.clone();
        if project.string(lang.as_str(), key.as_str()) == value.as_deref() {
            continue;
        }
        ops.push(Op::SetString { lang, key, value });
    }
    Ok(ops)
}

/// `id` and `ids` without duplicates (order kept).
fn id_list(id: Option<&String>, ids: &[String]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    id.into_iter()
        .chain(ids)
        .filter(|i| seen.insert((*i).clone()))
        .cloned()
        .collect()
}

/// One part of an [`element_patch`]: view, ids, patch, strings.
type PatchPart<'a> = (ViewContext, Vec<String>, &'a ElementPatch, &'a [StringEdit]);

pub fn element_patch(app: &AppState, args: ElementPatchArgs) -> CmdResult<EditResult> {
    let base_view = args.view();
    let mut parts: Vec<PatchPart<'_>> = Vec::new();
    let ids = id_list(args.id.as_ref(), &args.ids);
    if !ids.is_empty() || !args.strings.is_empty() {
        parts.push((base_view.clone(), ids, &args.patch, &args.strings));
    }
    for u in &args.updates {
        let ids = id_list(u.id.as_ref(), &u.ids);
        if ids.is_empty() && u.strings.is_empty() {
            return Err(CommandError::invalid("an update needs `id` or `ids`"));
        }
        let view = ViewContext {
            target: u.target.clone().or_else(|| base_view.target.clone()),
            lang: u.lang.clone().or_else(|| base_view.lang.clone()),
            // Like a view context of its own: missing = the target's own
            // variant / all languages.
            variant: u.variant.clone(),
            scope: u.scope,
        };
        parts.push((view, ids, &u.patch, &u.strings));
    }
    let all: Vec<String> = parts.iter().flat_map(|p| p.1.iter().cloned()).collect();
    let all = id_list(None, &all);
    if all.is_empty() && parts.iter().all(|p| p.3.is_empty()) {
        return Err(CommandError::invalid("no element id given"));
    }
    let mut what: Vec<&str> = Vec::new();
    for (_, _, patch, strings) in &parts {
        for f in patch.fields() {
            if !what.contains(&f) {
                what.push(f);
            }
        }
        if !strings.is_empty() && !what.contains(&"text") {
            what.push("text");
        }
    }
    let subject = match all.as_slice() {
        [one] => one.clone(),
        [] => "texts".to_owned(),
        _ => format!("{} elements", all.len()),
    };
    let label = if what.is_empty() {
        format!("Edit {subject}")
    } else {
        format!("Edit {} of {subject}", what.join(", "))
    };
    edit(app, &base_view, label, |s, p, _| {
        // Parts are planned in order on a scratch copy, so a later part
        // sees (and merges into) the overrides written by an earlier one.
        let mut scratch = p.clone();
        let mut ops = Vec::new();
        for (view, ids, patch, strings) in &parts {
            let c = view.edit_context(s, &scratch)?;
            // Strings first so a new key exists when the element points to it.
            let mut part = string_ops(&scratch, strings)?;
            for id in ids {
                if scratch.element(id).is_none() {
                    return Err(CommandError::not_found(format!("unknown element `{id}`")));
                }
                if !patch.is_empty() {
                    // Each id has its own element and override: independent
                    // ops (elements of other canvases on a target showing
                    // theirs).
                    let c = ctx_for(&scratch, &c, id)?;
                    part.extend(with_editor(s, &scratch, &c, |ed| ed.patch(id, patch))?);
                }
            }
            if !part.is_empty() {
                scratch.apply_ops(part.clone())?;
                ops.extend(part);
            }
        }
        Ok(Plan {
            ops,
            created: Vec::new(),
        })
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditRenameArgs {
    pub id: String,
    pub to: String,
}

pub fn edit_rename(app: &AppState, args: EditRenameArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let from = ElementId::parse(&args.id).map_err(CommandError::invalid)?;
    let to = ElementId::parse(&args.to).map_err(CommandError::invalid)?;
    let result = if from == to {
        session.unchanged()
    } else {
        session.apply_ops(
            vec![Op::RenameElement {
                from,
                to: to.clone(),
            }],
            Some(format!("Rename {} to {}", args.id, args.to)),
        )?
    };
    Ok(EditResult {
        result,
        created: vec![to.to_string()],
    })
}

view_args! {
    pub struct EditArrangeArgs {
        pub id: String,
        /// Group to move into (`null`/missing = top level of the canvas).
        #[serde(default)]
        pub parent: Option<String>,
        /// Position in the new list after taking the element out (0 =
        /// bottom; missing = top).
        #[serde(default)]
        pub index: Option<usize>,
    }
}

pub fn edit_arrange(app: &AppState, args: EditArrangeArgs) -> CmdResult<EditResult> {
    let label = match &args.parent {
        Some(p) => format!("Move {} into {p}", args.id),
        None => format!("Reorder {}", args.id),
    };
    edit(app, &args.view(), label, |s, p, c| {
        let ops = with_editor(s, p, c, |ed| {
            ed.arrange(&args.id, args.parent.as_deref(), args.index)
        })?;
        Ok(Plan {
            ops,
            created: Vec::new(),
        })
    })
}

// ----- canvases ------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasArgs {
    pub canvas: String,
}

/// A variant of a canvas.
#[derive(Debug, Clone, Serialize)]
pub struct VariantInfo {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_scale: Option<f64>,
    /// Elements with overrides.
    pub overrides: Vec<String>,
    /// Targets showing this variant.
    pub targets: Vec<String>,
}

/// Canvas settings.
#[derive(Debug, Clone, Serialize)]
pub struct CanvasInfo {
    pub id: String,
    /// `w:h`.
    pub aspect: String,
    pub count: u32,
    /// Explicit page ids (may be shorter than `count`).
    pub ids: Vec<String>,
    /// Ids of all pages (explicit or generated).
    pub page_ids: Vec<String>,
    pub height_lu: f64,
    /// Canvas backgrounds, bottom to top.
    pub backgrounds: Vec<Fill>,
    pub variants: Vec<VariantInfo>,
    /// Targets showing the canvas.
    pub targets: Vec<String>,
}

fn canvas_of<'p>(p: &'p Project, id: &str) -> CmdResult<&'p Canvas> {
    p.canvas(id)
        .ok_or_else(|| CommandError::not_found(format!("unknown canvas `{id}`")))
}

pub fn canvas_get(app: &AppState, args: CanvasArgs) -> CmdResult<CanvasInfo> {
    let session = app.session()?;
    session.with_project(|p| {
        let s = canvas_of(p, &args.canvas)?;
        Ok(CanvasInfo {
            id: s.id.to_string(),
            aspect: s.page.aspect.to_string(),
            count: s.page.count,
            ids: s.page.ids.clone(),
            page_ids: (0..s.page.count).map(|i| s.page.page_id(i)).collect(),
            height_lu: s.page.aspect.page_height(),
            backgrounds: s.backgrounds.clone(),
            variants: s
                .variants
                .iter()
                .map(|(id, v)| VariantInfo {
                    id: id.to_string(),
                    content_scale: v.content_scale,
                    overrides: v.overrides.keys().map(ToString::to_string).collect(),
                    targets: p
                        .targets
                        .iter()
                        .filter(|t| t.canvas == s.id && t.variant.as_ref() == Some(id))
                        .map(|t| t.id.to_string())
                        .collect(),
                })
                .collect(),
            targets: p
                .targets
                .iter()
                .filter(|t| t.canvas == s.id)
                .map(|t| t.id.to_string())
                .collect(),
        })
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct CanvasPatchArgs {
    pub canvas: String,
    #[serde(default)]
    pub aspect: Option<String>,
    /// Page count; explicit page ids beyond it are dropped. Refused while
    /// a removed page is still the home page of an element.
    #[serde(default)]
    pub count: Option<u32>,
    /// Explicit page ids (`[]` = all generated). Empty strings at the end
    /// are dropped; an empty string in between becomes the generated id.
    #[serde(default)]
    pub ids: Option<Vec<String>>,
    #[serde(default)]
    pub backgrounds: Option<Vec<Fill>>,
}

/// Elements (incl. group children) whose home page is `>= count`.
fn on_pages_from(canvas: &Canvas, count: u32) -> Vec<String> {
    let mut out = Vec::new();
    canvas.walk_elements(&mut |e| {
        if e.page >= count {
            out.push(e.id.to_string());
        }
    });
    out
}

pub fn canvas_patch(app: &AppState, args: CanvasPatchArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let project = session.snapshot();
    let s = canvas_of(&project, &args.canvas)?;
    let mut patch = CanvasPatch::default();
    let mut what = Vec::new();
    if let Some(a) = &args.aspect {
        let a = Aspect::parse(a).map_err(CommandError::invalid)?;
        if a != s.page.aspect {
            patch.aspect = Some(a);
            what.push("aspect");
        }
    }
    let count = args.count.unwrap_or(s.page.count);
    if count == 0 {
        return Err(CommandError::invalid("a canvas needs at least one page"));
    }
    if count != s.page.count {
        let blocked = on_pages_from(s, count);
        if !blocked.is_empty() {
            return Err(CommandError::new(
                crate::error::ErrorCode::OpRejected,
                format!(
                    "page {} is still the home page of {}",
                    count + 1,
                    blocked.join(", ")
                ),
            ));
        }
        patch.count = Some(count);
        what.push("page count");
    }
    let mut ids = match &args.ids {
        Some(ids) => {
            let mut ids: Vec<String> = ids.iter().map(|i| i.trim().to_owned()).collect();
            while ids.last().is_some_and(String::is_empty) {
                ids.pop();
            }
            for (i, id) in ids.iter_mut().enumerate() {
                if id.is_empty() {
                    *id = format!("page-{:02}", i + 1);
                }
            }
            ids
        }
        None => s.page.ids.clone(),
    };
    ids.truncate(count as usize);
    if ids != s.page.ids {
        patch.ids = Some(ids);
        what.push("page ids");
    }
    if let Some(b) = &args.backgrounds
        && *b != s.backgrounds
    {
        patch.backgrounds = Some(b.clone());
        what.push("backgrounds");
    }
    let result = if what.is_empty() {
        session.unchanged()
    } else {
        session.apply_ops(
            vec![Op::UpdateCanvas {
                id: s.id.clone(),
                patch,
            }],
            Some(format!("Edit {} of {}", what.join(", "), s.id)),
        )?
    };
    Ok(EditResult {
        result,
        created: Vec::new(),
    })
}

/// A change to the variants of a canvas.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::option_option)]
pub enum VariantAction {
    /// Add a variant: empty (optionally with a content scale) or a copy
    /// of variant `from` (its content scale and overrides); `targets` of
    /// the canvas show it from then on (same undo step).
    Add {
        id: String,
        #[serde(default)]
        content_scale: Option<f64>,
        #[serde(default)]
        from: Option<String>,
        #[serde(default)]
        targets: Vec<String>,
    },
    /// Remove a variant; targets showing it fall back to the base layout
    /// (same undo step).
    Remove { id: String },
    /// Rename a variant; targets follow.
    Rename { id: String, to: String },
    /// Set (`null` = remove) the content scale.
    Scale {
        id: String,
        #[serde(default, deserialize_with = "double_option")]
        content_scale: Option<Option<f64>>,
    },
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CanvasVariantArgs {
    pub canvas: String,
    pub change: VariantAction,
}

/// Ops adding variant `id` to `canvas` (empty, or a copy of `from` with
/// `content_scale` replaced when given) and making `targets` show it.
fn variant_add_ops(
    project: &Project,
    canvas: &vitrine_core::model::CanvasId,
    id: &str,
    content_scale: Option<f64>,
    from: Option<&str>,
    targets: &[String],
) -> CmdResult<Vec<Op>> {
    let new = VariantId::parse(id).map_err(CommandError::invalid)?;
    let variant = match from {
        Some(f) => {
            let mut v = project
                .canvas(canvas.as_str())
                .and_then(|s| s.variants.get(f))
                .cloned()
                .ok_or_else(|| {
                    CommandError::not_found(format!("unknown variant `{f}` of `{canvas}`"))
                })?;
            if content_scale.is_some() {
                v.content_scale = content_scale;
            }
            v
        }
        None => Variant {
            content_scale,
            overrides: BTreeMap::new(),
        },
    };
    let mut ops = vec![Op::AddVariant {
        canvas: canvas.clone(),
        id: new.clone(),
        variant,
    }];
    for t in targets {
        let target = project
            .target(t)
            .filter(|x| &x.canvas == canvas)
            .ok_or_else(|| CommandError::invalid(format!("`{t}` is not a target of `{canvas}`")))?;
        ops.push(Op::UpdateTarget {
            id: target.id.clone(),
            patch: TargetPatch {
                variant: Some(Some(new.clone())),
                ..TargetPatch::default()
            },
        });
    }
    Ok(ops)
}

pub fn canvas_variant(app: &AppState, args: CanvasVariantArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let project = session.snapshot();
    let canvas = canvas_of(&project, &args.canvas)?.id.clone();
    let vid = |s: &str| VariantId::parse(s).map_err(CommandError::invalid);
    let (ops, label, created) = match &args.change {
        VariantAction::Add {
            id,
            content_scale,
            from,
            targets,
        } => {
            let ops = variant_add_ops(
                &project,
                &canvas,
                id,
                *content_scale,
                from.as_deref(),
                targets,
            )?;
            let label = match from {
                Some(f) => format!("Duplicate variant {f} as {id}"),
                None => format!("Add variant {id}"),
            };
            (ops, label, vec![id.clone()])
        }
        VariantAction::Remove { id } => {
            let v = vid(id)?;
            let mut ops: Vec<Op> = project
                .targets
                .iter()
                .filter(|t| t.canvas == canvas && t.variant.as_ref() == Some(&v))
                .map(|t| Op::UpdateTarget {
                    id: t.id.clone(),
                    patch: TargetPatch {
                        variant: Some(None),
                        ..TargetPatch::default()
                    },
                })
                .collect();
            ops.push(Op::RemoveVariant {
                canvas: canvas.clone(),
                id: v,
            });
            (ops, format!("Remove variant {id}"), Vec::new())
        }
        VariantAction::Rename { id, to } => (
            vec![Op::RenameVariant {
                canvas: canvas.clone(),
                from: vid(id)?,
                to: vid(to)?,
            }],
            format!("Rename variant {id} to {to}"),
            vec![to.clone()],
        ),
        VariantAction::Scale { id, content_scale } => {
            let scale = content_scale.unwrap_or(None);
            let current = project
                .canvas(canvas.as_str())
                .and_then(|s| s.variants.get(id.as_str()))
                .and_then(|v| v.content_scale);
            let ops = if current == scale {
                Vec::new()
            } else {
                vec![Op::SetVariantContentScale {
                    canvas: canvas.clone(),
                    variant: vid(id)?,
                    content_scale: scale,
                }]
            };
            (ops, format!("Content scale of {id}"), Vec::new())
        }
    };
    let result = if ops.is_empty() {
        session.unchanged()
    } else {
        session.apply_ops(ops, Some(label))?
    };
    Ok(EditResult { result, created })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::{Value, json};

    use super::super::{AppState, Reply, dispatch};
    use crate::dialogs::NoDialogs;
    use crate::error::ErrorCode;
    use crate::events::RecordingSink;
    use crate::settings::AppPaths;
    use crate::testutil::example_copy;

    fn app() -> (tempfile::TempDir, AppState) {
        let (tmp, dir) = example_copy();
        let app = AppState::new(
            AppPaths::in_dir(&tmp.path().join("app")),
            Arc::new(RecordingSink::default()),
            Arc::new(NoDialogs),
        )
        .without_watcher();
        call(&app, "project_open", json!({ "dir": dir }));
        (tmp, app)
    }

    fn call(app: &AppState, name: &str, args: Value) -> Value {
        match dispatch(app, name, args) {
            Ok(Reply::Json(v)) => v,
            Ok(Reply::Image(_)) => panic!("{name} returned an image"),
            Err(e) => panic!("{name} failed: {e:?}"),
        }
    }

    fn last_undo(r: &Value) -> String {
        let u = r["history"]["undo"].as_array().unwrap();
        u.last().unwrap().as_str().unwrap().to_owned()
    }

    #[test]
    fn language_scope_patch_reset_and_page_lock() {
        let (_tmp, app) = app();
        let de = json!({"target": "ios-iphone-69", "lang": "de", "scope": "lang"});
        let with = |extra: Value| {
            let mut v = de.clone();
            v.as_object_mut()
                .unwrap()
                .extend(extra.as_object().unwrap().clone());
            v
        };
        call(
            &app,
            "element_patch",
            with(json!({"id": "title-plan", "patch": {"font_size": 70, "hyphenate": true}})),
        );
        let e = call(&app, "element_get", with(json!({"id": "title-plan"})));
        assert_eq!(e["lang_overridden"], json!(["font_size", "hyphenate"]));
        assert_eq!(e["lang_override_langs"], json!(["de"]));
        assert_eq!(e["effective"]["font_size"], 70);
        assert!(e["base"].get("font_size").is_none());
        // English is unaffected.
        let en = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "lang": "en", "id": "title-plan"}),
        );
        assert!(en["effective"].get("font_size").is_none());
        assert!(en.get("lang_overridden").is_none());
        // Not per language: refused.
        assert!(
            dispatch(
                &app,
                "element_patch",
                with(json!({"id": "title-plan", "patch": {"rotation": 4}}))
            )
            .is_err()
        );
        // Per-field reset of the language override.
        call(
            &app,
            "edit_reset_override",
            with(json!({"id": "title-plan", "field": "font_size", "lang_override": true})),
        );
        let e = call(&app, "element_get", with(json!({"id": "title-plan"})));
        assert_eq!(e["lang_overridden"], json!(["hyphenate"]));
        // The page lock is a model field (written through the base).
        call(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "id": "phone-plan", "patch": {"lock_page": true}}),
        );
        let o = call(&app, "overlay", json!({"target": "ios-iphone-69"}));
        let el = o["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["id"] == "phone-plan")
            .unwrap();
        assert_eq!(el["lock_page"], true);
    }

    #[test]
    fn element_get_shows_base_override_and_text() {
        let (_tmp, app) = app();
        let e = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "lang": "de", "id": "title-plan"}),
        );
        assert_eq!(e["kind"], "text");
        assert_eq!(e["canvas"], "phone");
        assert_eq!(e["text"]["lang"], "de");
        assert!(e["text"]["value"].is_string());
        assert!(e.get("variant").is_none());
        let e = call(
            &app,
            "element_get",
            json!({"target": "play-phone", "id": "phone-plan"}),
        );
        assert_eq!(e["variant"], "android");
        assert_eq!(e["overridden"], json!(["at"]));
        assert_eq!(e["override"]["at"], json!({"x": 500, "y": 1430}));
        assert_ne!(e["base"]["at"], e["effective"]["at"]);
        let err = dispatch(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "id": "nope"}),
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::NotFound);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn overrides_reset_batch_updates_and_cross_canvas_edits() {
        let (_tmp, app) = app();
        let get = |id: &str, lang: &str| {
            call(
                &app,
                "element_get",
                json!({"target": "ios-iphone-69", "lang": lang, "id": id}),
            )
        };
        // The reported case: `at: null` in a language override resets it
        // (no `en: {at: null}` left behind).
        call(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "variant": null, "lang": "en", "scope": "lang",
                   "id": "title-plan", "patch": {"font_size": 70}}),
        );
        assert_eq!(get("title-plan", "en")["lang_override"]["font_size"], 70);
        let r = call(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "variant": null, "lang": "en", "scope": "lang",
                   "id": "title-plan", "patch": {"font_size": null}}),
        );
        assert!(r["applied"].as_bool().unwrap());
        assert!(get("title-plan", "en").get("lang_override").is_none());
        let session = app.session().unwrap();
        let yaml = std::fs::read_to_string(session.dir().join("project.yaml")).unwrap();
        assert!(!yaml.contains("null"), "no null in overrides");

        // Several language adjustments (and a base change) in one step.
        let r = call(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "variant": null, "updates": [
                {"id": "title-plan", "scope": "lang", "lang": "de", "patch": {"font_size": 60}},
                {"id": "title-plan", "scope": "lang", "lang": "fr", "patch": {"font_size": 62}},
                {"ids": ["title-beds", "mac-title-plan"], "patch": {"align": "left"}},
            ]}),
        );
        assert_eq!(last_undo(&r), "Edit font_size, align of 3 elements");
        assert_eq!(get("title-plan", "de")["lang_override"]["font_size"], 60);
        assert_eq!(get("title-plan", "fr")["lang_override"]["font_size"], 62);
        let mac = call(
            &app,
            "element_get",
            json!({"target": "mac", "id": "mac-title-plan"}),
        );
        assert_eq!(mac["base"]["align"], "left", "other canvas, same step");
        call(&app, "undo", Value::Null);
        assert!(
            get("title-plan", "de")["lang_override"]
                .get("font_size")
                .is_none()
        );
        assert!(get("title-plan", "fr").get("lang_override").is_none());

        // Bring titles of two canvases to front in one step.
        let r = call(
            &app,
            "edit_reorder",
            json!({"target": "ios-iphone-69", "ids": ["title-plan", "mac-title-plan"], "order": "front"}),
        );
        assert!(r["applied"].as_bool().unwrap());
        let summary = call(&app, "project_get", Value::Null);
        for (canvas, id) in [(0, "title-plan"), (1, "mac-title-plan")] {
            let els = summary["project"]["canvases"][canvas]["elements"]
                .as_array()
                .unwrap();
            assert_eq!(els.last().unwrap()["id"], id, "{canvas}");
        }
        // Rotate several across canvases.
        let r = call(
            &app,
            "edit_rotate",
            json!({"target": "ios-iphone-69", "ids": ["phone-plan", "macbook-plan"], "degrees": 3}),
        );
        assert_eq!(last_undo(&r), "Rotate 2 elements to 3.0°");

        // Explicit reset: variant override fields, language override.
        let r = call(
            &app,
            "element_reset_override",
            json!({"ids": ["phone-plan"], "scope": "variant", "target": "play-phone", "fields": ["at"]}),
        );
        assert!(r["applied"].as_bool().unwrap());
        let e = call(
            &app,
            "element_get",
            json!({"target": "play-phone", "id": "phone-plan"}),
        );
        assert!(!e["overridden"].as_array().unwrap().contains(&json!("at")));
        let r = call(
            &app,
            "element_reset_override",
            json!({"id": "title-companions", "scope": "lang", "lang": "de"}),
        );
        assert!(r["applied"].as_bool().unwrap());
        let r = call(
            &app,
            "element_reset_override",
            json!({"id": "title-companions", "scope": "lang", "lang": "de"}),
        );
        assert!(!r["applied"].as_bool().unwrap(), "nothing left to reset");
        let err = dispatch(
            &app,
            "element_reset_override",
            json!({"id": "phone-plan", "scope": "lang", "lang": "de"}),
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgs);
    }

    #[test]
    fn element_patch_writes_base_or_override_with_strings() {
        let (_tmp, app) = app();
        let r = call(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "id": "phone-plan", "patch": {"opacity": 0.5, "z": 2}}),
        );
        assert!(r["applied"].as_bool().unwrap());
        assert_eq!(last_undo(&r), "Edit opacity, z of phone-plan");
        let e = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "id": "phone-plan"}),
        );
        assert_eq!(e["base"]["opacity"], 0.5);
        // The same again changes nothing.
        let r = call(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "id": "phone-plan", "patch": {"opacity": 0.5}}),
        );
        assert!(!r["applied"].as_bool().unwrap());
        // In the android variant: an override, the base stays.
        call(
            &app,
            "element_patch",
            json!({"target": "play-phone", "id": "phone-plan", "patch": {"rotation": 5}}),
        );
        let e = call(
            &app,
            "element_get",
            json!({"target": "play-phone", "id": "phone-plan"}),
        );
        assert_eq!(e["overridden"], json!(["at", "rotation"]));
        assert!(e["base"].get("rotation").is_none());
        // A new key with its text in one step.
        let r = call(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "variant": null, "lang": "de", "id": "title-plan",
                   "patch": {"text": "plan.title-new"},
                   "strings": [{"lang": "de", "key": "plan.title-new", "value": "Neu"}]}),
        );
        assert_eq!(last_undo(&r), "Edit text of title-plan");
        let e = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "lang": "de", "id": "title-plan"}),
        );
        assert_eq!(e["text"]["key"], "plan.title-new");
        assert_eq!(e["text"]["value"], "Neu");
        // Several elements in one step.
        let r = call(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "ids": ["phone-plan", "phone-beds"], "patch": {"z": 4}}),
        );
        assert_eq!(last_undo(&r), "Edit z of 2 elements");
        // Invalid fields for the kind are refused.
        let err = dispatch(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "id": "phone-plan", "patch": {"font_size": 3}}),
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgs);
        call(&app, "undo", Value::Null);
        call(&app, "undo", Value::Null);
        let e = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "lang": "de", "id": "title-plan"}),
        );
        assert_ne!(e["text"]["key"], "plan.title-new");
    }

    #[test]
    fn element_patch_text_effects() {
        let (_tmp, app) = app();
        let view = |patch: Value| json!({"target": "ios-iphone-69", "variant": null, "id": "title-plan", "patch": patch});
        let r = call(
            &app,
            "element_patch",
            view(json!({
                "stroke": {"color": "@leaf", "width": 4, "position": "center"},
                "glow": {"color": "#FFFFFF", "radius": 12, "opacity": 0.5},
                "shadow": "hard"
            })),
        );
        assert_eq!(last_undo(&r), "Edit stroke, glow, shadow of title-plan");
        let e = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "id": "title-plan"}),
        );
        assert_eq!(
            e["base"]["stroke"],
            json!({"color": "@leaf", "width": 4, "position": "center"})
        );
        assert_eq!(e["base"]["glow"]["opacity"], 0.5);
        assert_eq!(e["base"]["shadow"], "hard");
        // The overlay reports the extent of the glow.
        let o = call(&app, "overlay", json!({"target": "ios-iphone-69"}));
        let el = o["elements"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["id"] == "title-plan")
            .unwrap();
        let (content, visual) = (&el["text"]["content"], &el["text"]["visual"]);
        assert!(visual["w"].as_f64().unwrap() > content["w"].as_f64().unwrap() + 70.0);
        // Invalid values and unknown colours are refused.
        for (bad, code) in [
            (
                json!({"glow": {"color": "#FFFFFF", "radius": 12, "opacity": 3}}),
                ErrorCode::InvalidArgs,
            ),
            (
                json!({"stroke": {"color": "@nope", "width": 2}}),
                ErrorCode::OpRejected,
            ),
            (json!({"shadow": "nope"}), ErrorCode::OpRejected),
        ] {
            let err = dispatch(&app, "element_patch", view(bad.clone())).unwrap_err();
            assert_eq!(err.code, code, "{bad}");
        }
        // Reset to the style's values.
        call(
            &app,
            "element_patch",
            view(json!({"stroke": null, "glow": null, "shadow": null})),
        );
        let e = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "id": "title-plan"}),
        );
        assert!(e["base"].get("stroke").is_none() && e["base"].get("glow").is_none());
    }

    #[test]
    fn rename_and_arrange() {
        let (_tmp, app) = app();
        let r = call(
            &app,
            "edit_rename",
            json!({"id": "phone-plan", "to": "hero"}),
        );
        assert_eq!(r["created"], json!(["hero"]));
        // Overrides follow the rename.
        let e = call(
            &app,
            "element_get",
            json!({"target": "play-phone", "id": "hero"}),
        );
        assert_eq!(e["overridden"], json!(["at"]));
        let r = call(
            &app,
            "edit_arrange",
            json!({"target": "ios-iphone-69", "id": "hero", "index": 0}),
        );
        assert_eq!(last_undo(&r), "Reorder hero");
        let s = call(&app, "project_summary", Value::Null);
        assert_eq!(s["canvases"][0]["elements"][0]["id"], "hero");
    }

    #[test]
    fn canvas_settings_and_variants() {
        let (_tmp, app) = app();
        let s = call(&app, "canvas_get", json!({"canvas": "phone"}));
        assert_eq!(s["count"], 6);
        assert_eq!(s["ids"][0], "plan");
        assert!(s["variants"].as_array().unwrap().iter().any(|v| {
            v["id"] == "android"
                && v["targets"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|t| t == "play-phone")
        }));
        // A page that is still a home page cannot be removed.
        let err =
            dispatch(&app, "canvas_patch", json!({"canvas": "phone", "count": 5})).unwrap_err();
        assert_eq!(err.code, ErrorCode::OpRejected);
        let r = call(
            &app,
            "canvas_patch",
            json!({"canvas": "phone", "count": 7, "ids": ["a", "", "c"],
                   "backgrounds": ["#112233"]}),
        );
        assert_eq!(
            last_undo(&r),
            "Edit page count, page ids, backgrounds of phone"
        );
        let s = call(&app, "canvas_get", json!({"canvas": "phone"}));
        assert_eq!(s["ids"], json!(["a", "page-02", "c"]));
        assert_eq!(s["page_ids"][6], "page-07");
        assert_eq!(s["backgrounds"], json!(["#112233"]));
        call(&app, "canvas_patch", json!({"canvas": "phone", "count": 6}));
        call(
            &app,
            "canvas_variant",
            json!({"canvas": "phone", "change": {"action": "add", "id": "tall", "content_scale": 0.9}}),
        );
        call(
            &app,
            "canvas_variant",
            json!({"canvas": "phone", "change": {"action": "rename", "id": "tall", "to": "taller"}}),
        );
        let r = call(
            &app,
            "canvas_variant",
            json!({"canvas": "phone", "change": {"action": "scale", "id": "taller", "content_scale": null}}),
        );
        assert_eq!(last_undo(&r), "Content scale of taller");
        let s = call(&app, "canvas_get", json!({"canvas": "phone"}));
        let v = s["variants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == "taller")
            .unwrap();
        assert!(v.get("content_scale").is_none());
        call(
            &app,
            "canvas_variant",
            json!({"canvas": "phone", "change": {"action": "remove", "id": "taller"}}),
        );
        // A variant in use is removed too; its targets fall back to the base.
        let r = call(
            &app,
            "canvas_variant",
            json!({"canvas": "phone", "change": {"action": "remove", "id": "android"}}),
        );
        assert_eq!(last_undo(&r), "Remove variant android");
        let err = dispatch(
            &app,
            "canvas_variant",
            json!({"canvas": "phone", "change": {"action": "remove", "id": "android"}}),
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OpRejected);
    }
}
