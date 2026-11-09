//! Inspector and layers commands: read one element as base / override /
//! effective values, patch fields (base or variant override decided in
//! core, [`Editor::patch`]), rename, re-arrange in the layer tree, and the
//! strip settings (page geometry, backgrounds, variants).
//!
//! Like the `edit_*` commands every change is one undo step and answers
//! with an [`EditResult`]; a change that changes nothing answers
//! `applied: false`.

use std::collections::BTreeMap;

use arrangia_core::Project;
use arrangia_core::model::{
    Aspect, ElementId, ElementKindTag, ElementPatch, Fill, Lang, StringKey, Strip, Variant,
    VariantId,
};
use arrangia_core::ops::{Op, StripPatch};
use arrangia_core::resolve::{lang_chain, lookup_text};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::AppState;
use super::edit::{EditResult, Plan, ViewContext, double_option, edit, with_editor};
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
    pub strip: String,
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
    /// Base with the override applied (what the view shows).
    pub effective: Value,
    /// Fields set by the override (YAML names).
    pub overridden: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<ElementText>,
}

fn element_json(e: &arrangia_core::model::Element) -> CmdResult<Value> {
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
    let (variant, overridden) = with_editor(&session, &project, &ctx, |ed| {
        // Checks that the element exists in the view's strip.
        ed.kind(&args.id)
            .ok_or_else(|| arrangia_core::edit::EditError::UnknownElement(args.id.clone()))?;
        if project
            .element(&args.id)
            .is_none_or(|(s, _)| s.id != ed.strip().id)
        {
            return Err(arrangia_core::edit::EditError::WrongStrip {
                element: args.id.clone(),
                strip: ed.strip().id.to_string(),
            });
        }
        Ok((ed.variant().cloned(), ed.overridden_fields(&args.id)))
    })?;
    let (strip, e) = project
        .element(&args.id)
        .ok_or_else(|| CommandError::not_found(format!("unknown element `{}`", args.id)))?;
    let over = variant
        .as_ref()
        .and_then(|v| strip.variants.get(v))
        .and_then(|v| v.overrides.get(&e.id))
        .cloned();
    let effective = match &over {
        Some(p) => e.patched(p).map_or_else(|_| e.clone(), |(x, _)| x),
        None => e.clone(),
    };
    let text = match &effective.kind {
        arrangia_core::model::ElementKind::Text(t) => {
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
        strip: strip.id.to_string(),
        parent: parent_of(strip, &e.id),
        children: e.children().iter().map(|c| c.id.to_string()).collect(),
        variant: variant.map(|v| v.to_string()),
        base: element_json(e)?,
        r#override: over,
        effective: element_json(&effective)?,
        overridden,
        text,
    })
}

fn parent_of(strip: &Strip, id: &ElementId) -> Option<String> {
    let mut out = None;
    strip.walk_elements(&mut |e| {
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
        pub id: String,
        /// Fields to set (YAML names; `null` = reset to the default). In a
        /// variant view they go into the variant's override.
        #[serde(default)]
        pub patch: ElementPatch,
        /// Translations set in the same undo step (live text editing, a new
        /// key for a text element).
        #[serde(default)]
        pub strings: Vec<StringEdit>,
    }
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

pub fn element_patch(app: &AppState, args: ElementPatchArgs) -> CmdResult<EditResult> {
    let mut what: Vec<&str> = args.patch.fields();
    if !args.strings.is_empty() && !what.contains(&"text") {
        what.push("text");
    }
    let label = if what.is_empty() {
        format!("Edit {}", args.id)
    } else {
        format!("Edit {} of {}", what.join(", "), args.id)
    };
    edit(app, &args.view(), label, |s, p, c| {
        // Strings first so a new key exists when the element points to it.
        let mut ops = string_ops(p, &args.strings)?;
        if !args.patch.is_empty() {
            ops.extend(with_editor(s, p, c, |ed| ed.patch(&args.id, &args.patch))?);
        } else if p.element(&args.id).is_none() {
            return Err(CommandError::not_found(format!(
                "unknown element `{}`",
                args.id
            )));
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
        /// Group to move into (`null`/missing = top level of the strip).
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

// ----- strips ------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StripArgs {
    pub strip: String,
}

/// A variant of a strip.
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

/// Strip settings.
#[derive(Debug, Clone, Serialize)]
pub struct StripInfo {
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
    /// Targets showing the strip.
    pub targets: Vec<String>,
}

fn strip_of<'p>(p: &'p Project, id: &str) -> CmdResult<&'p Strip> {
    p.strip(id)
        .ok_or_else(|| CommandError::not_found(format!("unknown strip `{id}`")))
}

pub fn strip_get(app: &AppState, args: StripArgs) -> CmdResult<StripInfo> {
    let session = app.session()?;
    session.with_project(|p| {
        let s = strip_of(p, &args.strip)?;
        Ok(StripInfo {
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
                        .filter(|t| t.strip == s.id && t.variant.as_ref() == Some(id))
                        .map(|t| t.id.to_string())
                        .collect(),
                })
                .collect(),
            targets: p
                .targets
                .iter()
                .filter(|t| t.strip == s.id)
                .map(|t| t.id.to_string())
                .collect(),
        })
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct StripPatchArgs {
    pub strip: String,
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
fn on_pages_from(strip: &Strip, count: u32) -> Vec<String> {
    let mut out = Vec::new();
    strip.walk_elements(&mut |e| {
        if e.page >= count {
            out.push(e.id.to_string());
        }
    });
    out
}

pub fn strip_patch(app: &AppState, args: StripPatchArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let project = session.snapshot();
    let s = strip_of(&project, &args.strip)?;
    let mut patch = StripPatch::default();
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
        return Err(CommandError::invalid("a strip needs at least one page"));
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
            vec![Op::UpdateStrip {
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

/// A change to the variants of a strip.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
#[allow(clippy::option_option)]
pub enum VariantAction {
    /// Add an empty variant (optionally with a content scale).
    Add {
        id: String,
        #[serde(default)]
        content_scale: Option<f64>,
    },
    /// Remove a variant (refused while a target shows it).
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
pub struct StripVariantArgs {
    pub strip: String,
    pub change: VariantAction,
}

pub fn strip_variant(app: &AppState, args: StripVariantArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let project = session.snapshot();
    let strip = strip_of(&project, &args.strip)?.id.clone();
    let vid = |s: &str| VariantId::parse(s).map_err(CommandError::invalid);
    let (ops, label, created) = match &args.change {
        VariantAction::Add { id, content_scale } => (
            vec![Op::AddVariant {
                strip: strip.clone(),
                id: vid(id)?,
                variant: Variant {
                    content_scale: *content_scale,
                    overrides: BTreeMap::new(),
                },
            }],
            format!("Add variant {id}"),
            vec![id.clone()],
        ),
        VariantAction::Remove { id } => (
            vec![Op::RemoveVariant {
                strip: strip.clone(),
                id: vid(id)?,
            }],
            format!("Remove variant {id}"),
            Vec::new(),
        ),
        VariantAction::Rename { id, to } => (
            vec![Op::RenameVariant {
                strip: strip.clone(),
                from: vid(id)?,
                to: vid(to)?,
            }],
            format!("Rename variant {id} to {to}"),
            vec![to.clone()],
        ),
        VariantAction::Scale { id, content_scale } => {
            let scale = content_scale.unwrap_or(None);
            let current = project
                .strip(strip.as_str())
                .and_then(|s| s.variants.get(id.as_str()))
                .and_then(|v| v.content_scale);
            let ops = if current == scale {
                Vec::new()
            } else {
                vec![Op::SetVariantContentScale {
                    strip: strip.clone(),
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
    fn element_get_shows_base_override_and_text() {
        let (_tmp, app) = app();
        let e = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "lang": "de", "id": "title-plan"}),
        );
        assert_eq!(e["kind"], "text");
        assert_eq!(e["strip"], "phone");
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
        // Invalid fields for the kind are refused.
        let err = dispatch(
            &app,
            "element_patch",
            json!({"target": "ios-iphone-69", "id": "phone-plan", "patch": {"font_size": 3}}),
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgs);
        call(&app, "undo", Value::Null);
        let e = call(
            &app,
            "element_get",
            json!({"target": "ios-iphone-69", "lang": "de", "id": "title-plan"}),
        );
        assert_ne!(e["text"]["key"], "plan.title-new");
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
        assert_eq!(s["strips"][0]["elements"][0]["id"], "hero");
    }

    #[test]
    fn strip_settings_and_variants() {
        let (_tmp, app) = app();
        let s = call(&app, "strip_get", json!({"strip": "phone"}));
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
        let err = dispatch(&app, "strip_patch", json!({"strip": "phone", "count": 5})).unwrap_err();
        assert_eq!(err.code, ErrorCode::OpRejected);
        let r = call(
            &app,
            "strip_patch",
            json!({"strip": "phone", "count": 7, "ids": ["a", "", "c"],
                   "backgrounds": ["#112233"]}),
        );
        assert_eq!(
            last_undo(&r),
            "Edit page count, page ids, backgrounds of phone"
        );
        let s = call(&app, "strip_get", json!({"strip": "phone"}));
        assert_eq!(s["ids"], json!(["a", "page-02", "c"]));
        assert_eq!(s["page_ids"][6], "page-07");
        assert_eq!(s["backgrounds"], json!(["#112233"]));
        call(&app, "strip_patch", json!({"strip": "phone", "count": 6}));
        call(
            &app,
            "strip_variant",
            json!({"strip": "phone", "change": {"action": "add", "id": "tall", "content_scale": 0.9}}),
        );
        call(
            &app,
            "strip_variant",
            json!({"strip": "phone", "change": {"action": "rename", "id": "tall", "to": "taller"}}),
        );
        let r = call(
            &app,
            "strip_variant",
            json!({"strip": "phone", "change": {"action": "scale", "id": "taller", "content_scale": null}}),
        );
        assert_eq!(last_undo(&r), "Content scale of taller");
        let s = call(&app, "strip_get", json!({"strip": "phone"}));
        let v = s["variants"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["id"] == "taller")
            .unwrap();
        assert!(v.get("content_scale").is_none());
        call(
            &app,
            "strip_variant",
            json!({"strip": "phone", "change": {"action": "remove", "id": "taller"}}),
        );
        let err = dispatch(
            &app,
            "strip_variant",
            json!({"strip": "phone", "change": {"action": "remove", "id": "android"}}),
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::OpRejected);
    }
}
