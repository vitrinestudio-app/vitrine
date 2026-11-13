//! Operations API (KONZEPT §10.2).
//!
//! Every change to a project is an [`Op`]. [`Project::apply`] validates the
//! op, applies it and returns the inverse op (undo). [`Project::apply_ops`]
//! applies a list atomically (all or nothing) and returns the inverse list.
//! [`History`] keeps undo/redo stacks of such transactions.
//!
//! Validation covers what an op touches: ids exist / are unique, page
//! indices are within the canvas, referenced styles, colours, shadows,
//! languages, targets and variants exist, device models, colour variants,
//! gallery profiles and store slot values exist in [`crate::catalog`], and
//! removals of things that are
//! still referenced are refused. Pre-existing problems elsewhere in the
//! project (e.g. after a hand edit) do not block unrelated ops; they are
//! reported by `core::validate`.

mod history;
mod lang;
mod patch;
mod refs;
mod structure;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub use history::History;
pub use patch::{CanvasPatch, TargetPatch};
pub use refs::ClipRef;

use crate::catalog;
use crate::model::{
    Canvas, CanvasId, Checks, ColorRef, DeviceProps, Element, ElementId, ElementKind, ElementPatch,
    FrameStyle, ImportSettings, Lang, Languages, Project, Region, StoreKind, StoreSettings,
    StoreSlot, StringKey, StyleKind, Styles, Target, TargetId, Variant, VariantId, is_valid_id,
};

/// Overrides of removed elements, by variant, so that removal can be undone.
pub type VariantOverrides = BTreeMap<VariantId, BTreeMap<ElementId, ElementPatch>>;

/// A serializable change to a project. JSON form: `{"op": "update_element", "id": "…", "patch": {…}}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Op {
    SetName {
        name: String,
    },
    SetLanguages {
        languages: Languages,
    },
    SetStyles {
        styles: Styles,
    },
    /// Replace the screenshot import mappings (`import:` in `project.yaml`).
    SetImport {
        import: ImportSettings,
    },
    /// Replace the design-check settings (`checks:` in `project.yaml`).
    SetChecks {
        checks: Checks,
    },
    /// Replace the store mapping (`store:` in `project.yaml`).
    SetStore {
        store: StoreSettings,
    },
    /// Set (`Some`) or remove (`None`) a translation.
    SetString {
        lang: Lang,
        key: StringKey,
        value: Option<String>,
    },
    AddCanvas {
        canvas: Canvas,
        /// Position in the canvas list (default: end).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    UpdateCanvas {
        id: CanvasId,
        patch: CanvasPatch,
    },
    RemoveCanvas {
        id: CanvasId,
    },
    AddElement {
        canvas: CanvasId,
        /// Group to add the element to (default: top level of the canvas).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<ElementId>,
        /// Position in the element list (default: end = topmost).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
        element: Element,
        /// Variant overrides for the element (and its children), restored
        /// when undoing a removal.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        overrides: VariantOverrides,
    },
    UpdateElement {
        id: ElementId,
        patch: ElementPatch,
    },
    /// Remove an element (with its children and variant overrides).
    /// Refused while an image outside it is clipped to it (`clip_to`;
    /// release with [`Project::clip_to_release_ops`] first).
    RemoveElement {
        id: ElementId,
    },
    /// Move an element to another position in its list (painting order).
    MoveElement {
        id: ElementId,
        index: usize,
    },
    AddVariant {
        canvas: CanvasId,
        id: VariantId,
        #[serde(default)]
        variant: Variant,
    },
    RemoveVariant {
        canvas: CanvasId,
        id: VariantId,
    },
    SetVariantContentScale {
        canvas: CanvasId,
        variant: VariantId,
        content_scale: Option<f64>,
    },
    /// Set (`Some`) or remove (`None`) the override of one element in a variant.
    SetVariantOverride {
        canvas: CanvasId,
        variant: VariantId,
        element: ElementId,
        patch: Option<ElementPatch>,
    },
    AddTarget {
        target: Target,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    UpdateTarget {
        id: TargetId,
        patch: TargetPatch,
    },
    RemoveTarget {
        id: TargetId,
    },
    /// Rename an element; its variant overrides and `clip_to`
    /// references to it follow.
    RenameElement {
        from: ElementId,
        to: ElementId,
    },
    /// Rename a canvas; targets showing it follow.
    RenameCanvas {
        from: CanvasId,
        to: CanvasId,
    },
    /// Rename a target; `visible_in.targets` of elements and variant
    /// overrides follow.
    RenameTarget {
        from: TargetId,
        to: TargetId,
    },
    /// Rename a variant of a canvas; targets using it follow.
    RenameVariant {
        canvas: CanvasId,
        from: VariantId,
        to: VariantId,
    },
    /// Move an element into a group, out of it (`parent: None` = top level
    /// of its canvas) or to another position. `index` is the position in the
    /// new list after the element was taken out of its old one (default:
    /// end = topmost). Positions are not compensated (see
    /// `core::edit::Editor::group`/`ungroup` for that).
    MoveElementTo {
        id: ElementId,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<ElementId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    /// Remove one field (YAML name, e.g. `at`) from an element's variant
    /// override; an override left empty is removed.
    ResetOverrideField {
        canvas: CanvasId,
        variant: VariantId,
        element: ElementId,
        field: String,
    },
    /// Make an available language the default (fallback) language.
    SetDefaultLanguage {
        lang: Lang,
    },
    /// Add a language to `languages.available` (default: at the end).
    /// `strings` restores its translations when undoing a removal.
    AddLanguage {
        lang: Lang,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        strings: BTreeMap<StringKey, String>,
        /// Language overrides (`langs.<lang>`) of text elements, restored
        /// when undoing a removal.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        lang_overrides: BTreeMap<ElementId, ElementPatch>,
    },
    /// Remove a language, its translations and the language overrides
    /// for exactly this language. Refused for the default language and
    /// for languages still named by `visible_in` or a target's `langs`.
    /// Explicit `languages.fallback` entries are kept (they may name
    /// languages outside `available`).
    RemoveLanguage {
        lang: Lang,
    },
    /// Rename a translation key in every language and in every text element
    /// (and variant override) using it.
    RenameStringKey {
        from: StringKey,
        to: StringKey,
    },
    /// Set (`Some`) or remove (`None`) the language override of a text
    /// element (`langs.<lang>`, KONZEPT §5.1). Only
    /// [`crate::model::LANG_OVERRIDE_FIELDS`] may be set; setting needs an
    /// available language, removing works for any.
    SetLangOverride {
        element: ElementId,
        lang: Lang,
        patch: Option<ElementPatch>,
    },
    /// Remove one field from a language override; an override left empty
    /// is removed.
    ResetLangOverrideField {
        element: ElementId,
        lang: Lang,
        field: String,
    },
    /// Rename a colour, text style, span style or shadow preset; every
    /// reference follows (styles, elements, variant and language
    /// overrides, canvas backgrounds; span names in translations).
    RenameStyle {
        kind: StyleKind,
        from: String,
        to: String,
    },
}

impl Op {
    /// Overrides never store `null` (ADR 0009 amendment): a `null` field in
    /// an override patch carried by this op is dropped, i.e. the field is
    /// not overridden. Empty override patches become "no override".
    fn drop_override_nulls(&mut self) {
        let clean = |p: &mut Option<ElementPatch>| {
            if let Some(patch) = p {
                patch.drop_nulls();
                if patch.is_empty() {
                    *p = None;
                }
            }
        };
        match self {
            Op::SetVariantOverride { patch, .. } | Op::SetLangOverride { patch, .. } => {
                clean(patch);
            }
            Op::AddElement {
                element, overrides, ..
            } => {
                element.drop_lang_override_nulls();
                for map in overrides.values_mut() {
                    map.retain(|_, p| {
                        p.drop_nulls();
                        !p.is_empty()
                    });
                }
            }
            Op::AddCanvas { canvas, .. } => {
                canvas.drop_override_nulls();
            }
            Op::AddLanguage { lang_overrides, .. } => {
                lang_overrides.retain(|_, p| {
                    p.drop_nulls();
                    !p.is_empty()
                });
            }
            _ => {}
        }
    }
}

/// Why an op was rejected. The project is unchanged in that case.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OpError {
    #[error("unknown canvas `{0}`")]
    UnknownCanvas(String),
    #[error("unknown element `{0}`")]
    UnknownElement(String),
    #[error("unknown target `{0}`")]
    UnknownTarget(String),
    #[error("unknown variant `{variant}` in canvas `{canvas}`")]
    UnknownVariant { canvas: String, variant: String },
    #[error("unknown language `{0}` (not in `languages.available`)")]
    UnknownLanguage(String),
    #[error("unknown text style `{0}`")]
    UnknownTextStyle(String),
    #[error("unknown colour `@{0}`")]
    UnknownColor(String),
    #[error("unknown shadow preset `{0}`")]
    UnknownShadow(String),
    #[error("unknown device model `{0}` (not in the device catalog)")]
    UnknownDevice(String),
    #[error("{what} `{id}` already exists")]
    DuplicateId { what: &'static str, id: String },
    #[error(
        "element `{element}`: page {page} is out of range (canvas `{canvas}` has {count} pages)"
    )]
    PageOutOfRange {
        element: String,
        canvas: String,
        page: u32,
        count: u32,
    },
    #[error("{what} is still referenced by {by}")]
    StillReferenced { what: String, by: String },
    #[error("index {index} is out of range (max {max})")]
    IndexOutOfRange { index: usize, max: usize },
    #[error("{0}")]
    Invalid(String),
    #[error("op {index} of the transaction failed: {source}")]
    Transaction {
        index: usize,
        #[source]
        source: Box<OpError>,
    },
}

/// Where an element lives.
#[derive(Clone, Debug, PartialEq)]
struct Loc {
    canvas: usize,
    parent: Option<ElementId>,
    index: usize,
}

fn locate_in(
    list: &[Element],
    id: &str,
    parent: Option<&ElementId>,
) -> Option<(Option<ElementId>, usize)> {
    for (i, e) in list.iter().enumerate() {
        if e.id.as_str() == id {
            return Some((parent.cloned(), i));
        }
        if let Some(found) = locate_in(e.children(), id, Some(&e.id)) {
            return Some(found);
        }
    }
    None
}

fn find_mut<'a>(list: &'a mut [Element], id: &str) -> Option<&'a mut Element> {
    for e in list {
        if e.id.as_str() == id {
            return Some(e);
        }
        if let Some(children) = e.children_mut()
            && let Some(found) = find_mut(children, id)
        {
            return Some(found);
        }
    }
    None
}

fn check_id(what: &str, id: &str) -> Result<(), OpError> {
    if is_valid_id(id) {
        Ok(())
    } else {
        Err(OpError::Invalid(format!("invalid {what} `{id}`")))
    }
}

fn check_index(index: Option<usize>, len: usize) -> Result<usize, OpError> {
    match index {
        None => Ok(len),
        Some(i) if i <= len => Ok(i),
        Some(i) => Err(OpError::IndexOutOfRange { index: i, max: len }),
    }
}

fn invalid(e: String) -> OpError {
    OpError::Invalid(e)
}

impl Project {
    /// Validate and apply one op. Returns the inverse op.
    #[allow(clippy::too_many_lines)]
    pub fn apply(&mut self, mut op: Op) -> Result<Op, OpError> {
        op.drop_override_nulls();
        match op {
            Op::SetName { name } => {
                let old = std::mem::replace(&mut self.name, name);
                Ok(Op::SetName { name: old })
            }
            Op::SetLanguages { languages } => {
                languages.check().map_err(invalid)?;
                self.check_languages_in_use(&languages)?;
                let old = std::mem::replace(&mut self.languages, languages);
                Ok(Op::SetLanguages { languages: old })
            }
            Op::SetStyles { styles } => {
                self.check_styles(&styles)?;
                let old = std::mem::replace(&mut self.styles, styles);
                Ok(Op::SetStyles { styles: old })
            }
            Op::SetImport { import } => {
                import.check().map_err(invalid)?;
                let old = std::mem::replace(&mut self.import, import);
                Ok(Op::SetImport { import: old })
            }
            Op::SetChecks { checks } => {
                checks.check().map_err(invalid)?;
                let old = std::mem::replace(&mut self.checks, checks);
                Ok(Op::SetChecks { checks: old })
            }
            Op::SetStore { store } => {
                check_store(&store)?;
                let old = std::mem::replace(&mut self.store, store);
                Ok(Op::SetStore { store: old })
            }
            Op::SetString { lang, key, value } => {
                if !self.languages.is_available(lang.as_str()) {
                    return Err(OpError::UnknownLanguage(lang.to_string()));
                }
                let old = if let Some(v) = value {
                    self.strings
                        .entry(lang.clone())
                        .or_default()
                        .insert(key.clone(), v)
                } else {
                    let old = self.strings.get_mut(&lang).and_then(|m| m.remove(&key));
                    if self.strings.get(&lang).is_some_and(BTreeMap::is_empty) {
                        self.strings.remove(&lang);
                    }
                    old
                };
                Ok(Op::SetString {
                    lang,
                    key,
                    value: old,
                })
            }
            Op::AddCanvas { canvas, index } => {
                check_id("canvas id", canvas.id.as_str())?;
                if self.canvas(canvas.id.as_str()).is_some() {
                    return Err(OpError::DuplicateId {
                        what: "canvas",
                        id: canvas.id.to_string(),
                    });
                }
                let index = check_index(index, self.canvases.len())?;
                let mut ids = BTreeSet::new();
                for e in &canvas.elements {
                    self.check_new_element_ids(e, &mut ids)?;
                }
                self.check_canvas(&canvas)?;
                let id = canvas.id.clone();
                self.canvases.insert(index, canvas);
                Ok(Op::RemoveCanvas { id })
            }
            Op::UpdateCanvas { id, patch } => {
                let si = self.canvas_index(&id)?;
                let mut canvas = self.canvases[si].clone();
                let inverse = patch.apply(&mut canvas);
                self.check_canvas(&canvas)?;
                for t in self.targets.iter().filter(|t| t.canvas == id) {
                    check_region(t, &canvas)?;
                }
                self.canvases[si] = canvas;
                Ok(Op::UpdateCanvas { id, patch: inverse })
            }
            Op::RemoveCanvas { id } => {
                let si = self.canvas_index(&id)?;
                if let Some(t) = self.targets.iter().find(|t| t.canvas == id) {
                    return Err(OpError::StillReferenced {
                        what: format!("canvas `{id}`"),
                        by: format!("target `{}`", t.id),
                    });
                }
                let canvas = self.canvases.remove(si);
                Ok(Op::AddCanvas {
                    canvas,
                    index: Some(si),
                })
            }
            Op::AddElement {
                canvas,
                parent,
                index,
                element,
                overrides,
            } => {
                let si = self.canvas_index(&canvas)?;
                let mut ids = BTreeSet::new();
                self.check_new_element_ids(&element, &mut ids)?;
                let s = &self.canvases[si];
                let list_len = match &parent {
                    None => s.elements.len(),
                    Some(p) => {
                        let pe = s
                            .element(p.as_str())
                            .ok_or_else(|| OpError::UnknownElement(p.to_string()))?;
                        if !matches!(pe.kind, ElementKind::Group(_)) {
                            return Err(invalid(format!("parent `{p}` is not a group")));
                        }
                        pe.children().len()
                    }
                };
                let index = check_index(index, list_len)?;
                self.check_element(s, &element)?;
                for (vid, map) in &overrides {
                    if !s.variants.contains_key(vid) {
                        return Err(OpError::UnknownVariant {
                            canvas: canvas.to_string(),
                            variant: vid.to_string(),
                        });
                    }
                    for (eid, patch) in map {
                        if !ids.contains(eid) {
                            return Err(invalid(format!(
                                "override for `{eid}` does not belong to the added element"
                            )));
                        }
                        let mut found = None;
                        element.walk(&mut |e| {
                            if &e.id == eid {
                                found = Some(e);
                            }
                        });
                        let base = found.ok_or_else(|| OpError::UnknownElement(eid.to_string()))?;
                        self.check_override(s, base, patch)?;
                    }
                }
                let id = element.id.clone();
                let s = &mut self.canvases[si];
                list_mut(s, parent.as_ref()).insert(index, element);
                for (vid, map) in overrides {
                    if let Some(v) = s.variants.get_mut(&vid) {
                        v.overrides.extend(map);
                    }
                }
                Ok(Op::RemoveElement { id })
            }
            Op::UpdateElement { id, patch } => {
                let loc = self.locate(&id)?;
                let s = &self.canvases[loc.canvas];
                let old = s
                    .element(id.as_str())
                    .ok_or_else(|| OpError::UnknownElement(id.to_string()))?;
                let (new, inverse) = old.patched(&patch).map_err(invalid)?;
                self.check_element(s, &new)?;
                let slot = find_mut(&mut self.canvases[loc.canvas].elements, id.as_str())
                    .ok_or_else(|| OpError::UnknownElement(id.to_string()))?;
                *slot = new;
                Ok(Op::UpdateElement { id, patch: inverse })
            }
            Op::RemoveElement { id } => {
                let loc = self.locate(&id)?;
                let mut subtree = BTreeSet::new();
                if let Some(e) = self.canvases[loc.canvas].element(id.as_str()) {
                    e.walk(&mut |e| {
                        subtree.insert(e.id.clone());
                    });
                }
                self.check_clip_to_unreferenced(&subtree)?;
                let s = &mut self.canvases[loc.canvas];
                let element = list_mut(s, loc.parent.as_ref()).remove(loc.index);
                let mut overrides = VariantOverrides::new();
                for (vid, v) in &mut s.variants {
                    let removed: BTreeMap<_, _> = subtree
                        .iter()
                        .filter_map(|eid| v.overrides.remove_entry(eid))
                        .collect();
                    if !removed.is_empty() {
                        overrides.insert(vid.clone(), removed);
                    }
                }
                Ok(Op::AddElement {
                    canvas: s.id.clone(),
                    parent: loc.parent,
                    index: Some(loc.index),
                    element,
                    overrides,
                })
            }
            Op::MoveElement { id, index } => {
                let loc = self.locate(&id)?;
                let list = list_mut(&mut self.canvases[loc.canvas], loc.parent.as_ref());
                if index >= list.len() {
                    return Err(OpError::IndexOutOfRange {
                        index,
                        max: list.len() - 1,
                    });
                }
                let e = list.remove(loc.index);
                list.insert(index, e);
                Ok(Op::MoveElement {
                    id,
                    index: loc.index,
                })
            }
            Op::AddVariant {
                canvas,
                id,
                variant,
            } => {
                check_id("variant id", id.as_str())?;
                let si = self.canvas_index(&canvas)?;
                let s = &self.canvases[si];
                if s.variants.contains_key(&id) {
                    return Err(OpError::DuplicateId {
                        what: "variant",
                        id: id.to_string(),
                    });
                }
                check_scale(variant.content_scale)?;
                for (eid, patch) in &variant.overrides {
                    let base = s
                        .element(eid.as_str())
                        .ok_or_else(|| OpError::UnknownElement(eid.to_string()))?;
                    self.check_override(s, base, patch)?;
                }
                self.canvases[si].variants.insert(id.clone(), variant);
                Ok(Op::RemoveVariant { canvas, id })
            }
            Op::RemoveVariant { canvas, id } => {
                let si = self.canvas_index(&canvas)?;
                if !self.canvases[si].variants.contains_key(&id) {
                    return Err(OpError::UnknownVariant {
                        canvas: canvas.to_string(),
                        variant: id.to_string(),
                    });
                }
                if let Some(t) = self
                    .targets
                    .iter()
                    .find(|t| t.canvas == canvas && t.variant.as_ref() == Some(&id))
                {
                    return Err(OpError::StillReferenced {
                        what: format!("variant `{id}`"),
                        by: format!("target `{}`", t.id),
                    });
                }
                let variant = self.canvases[si].variants.remove(&id).unwrap_or_default();
                Ok(Op::AddVariant {
                    canvas,
                    id,
                    variant,
                })
            }
            Op::SetVariantContentScale {
                canvas,
                variant,
                content_scale,
            } => {
                check_scale(content_scale)?;
                let v = self.variant_mut(&canvas, &variant)?;
                let old = std::mem::replace(&mut v.content_scale, content_scale);
                Ok(Op::SetVariantContentScale {
                    canvas,
                    variant,
                    content_scale: old,
                })
            }
            Op::SetVariantOverride {
                canvas,
                variant,
                element,
                patch,
            } => {
                let si = self.canvas_index(&canvas)?;
                let s = &self.canvases[si];
                let base = s
                    .element(element.as_str())
                    .ok_or_else(|| OpError::UnknownElement(element.to_string()))?;
                let patch = patch.filter(|p| !p.is_empty());
                if let Some(p) = &patch {
                    self.check_override(s, base, p)?;
                }
                let v = self.variant_mut(&canvas, &variant)?;
                let old = match patch {
                    Some(p) => v.overrides.insert(element.clone(), p),
                    None => v.overrides.remove(&element),
                };
                Ok(Op::SetVariantOverride {
                    canvas,
                    variant,
                    element,
                    patch: old,
                })
            }
            Op::AddTarget { target, index } => {
                check_id("target id", target.id.as_str())?;
                if self.target(target.id.as_str()).is_some() {
                    return Err(OpError::DuplicateId {
                        what: "target",
                        id: target.id.to_string(),
                    });
                }
                let index = check_index(index, self.targets.len())?;
                self.check_target(&target)?;
                let id = target.id.clone();
                self.targets.insert(index, target);
                Ok(Op::RemoveTarget { id })
            }
            Op::UpdateTarget { id, patch } => {
                let ti = self.target_index(&id)?;
                let mut target = self.targets[ti].clone();
                let inverse = patch.apply(&mut target);
                self.check_target(&target)?;
                self.targets[ti] = target;
                Ok(Op::UpdateTarget { id, patch: inverse })
            }
            Op::RemoveTarget { id } => {
                let ti = self.target_index(&id)?;
                let mut user = None;
                for s in &self.canvases {
                    s.walk_elements(&mut |e| {
                        if user.is_none()
                            && e.visible_in
                                .as_ref()
                                .is_some_and(|v| v.targets.contains(&id))
                        {
                            user = Some(e.id.clone());
                        }
                    });
                }
                if let Some(e) = user {
                    return Err(OpError::StillReferenced {
                        what: format!("target `{id}`"),
                        by: format!("`visible_in` of element `{e}`"),
                    });
                }
                let target = self.targets.remove(ti);
                Ok(Op::AddTarget {
                    target,
                    index: Some(ti),
                })
            }
            Op::RenameElement { from, to } => self.rename_element(from, to),
            Op::RenameCanvas { from, to } => self.rename_canvas(from, to),
            Op::RenameTarget { from, to } => self.rename_target(from, to),
            Op::RenameVariant { canvas, from, to } => self.rename_variant(canvas, from, to),
            Op::MoveElementTo { id, parent, index } => {
                self.move_element_to(id, parent.as_ref(), index)
            }
            Op::ResetOverrideField {
                canvas,
                variant,
                element,
                field,
            } => self.reset_override_field(canvas, variant, element, &field),
            Op::SetDefaultLanguage { lang } => self.set_default_language(lang),
            Op::AddLanguage {
                lang,
                index,
                strings,
                lang_overrides,
            } => self.add_language(lang, index, strings, lang_overrides),
            Op::RemoveLanguage { lang } => self.remove_language(lang),
            Op::RenameStringKey { from, to } => self.rename_string_key(from, to),
            Op::SetLangOverride {
                element,
                lang,
                patch,
            } => self.set_lang_override(element, lang, patch),
            Op::ResetLangOverrideField {
                element,
                lang,
                field,
            } => self.reset_lang_override_field(element, lang, &field),
            Op::RenameStyle { kind, from, to } => self.rename_style(kind, from, to),
        }
    }

    /// Apply ops atomically: either all succeed or the project is left
    /// unchanged. Returns the inverse ops in the order they must be applied
    /// to undo the transaction.
    pub fn apply_ops(&mut self, ops: impl IntoIterator<Item = Op>) -> Result<Vec<Op>, OpError> {
        let backup = self.clone();
        let mut inverse = Vec::new();
        for (index, op) in ops.into_iter().enumerate() {
            match self.apply(op) {
                Ok(inv) => inverse.push(inv),
                Err(e) => {
                    *self = backup;
                    return Err(OpError::Transaction {
                        index,
                        source: Box::new(e),
                    });
                }
            }
        }
        inverse.reverse();
        Ok(inverse)
    }

    // ----- lookups ---------------------------------------------------------

    fn canvas_index(&self, id: &CanvasId) -> Result<usize, OpError> {
        self.canvases
            .iter()
            .position(|s| &s.id == id)
            .ok_or_else(|| OpError::UnknownCanvas(id.to_string()))
    }

    fn target_index(&self, id: &TargetId) -> Result<usize, OpError> {
        self.targets
            .iter()
            .position(|t| &t.id == id)
            .ok_or_else(|| OpError::UnknownTarget(id.to_string()))
    }

    fn locate(&self, id: &ElementId) -> Result<Loc, OpError> {
        self.canvases
            .iter()
            .enumerate()
            .find_map(|(si, s)| {
                locate_in(&s.elements, id.as_str(), None).map(|(parent, index)| Loc {
                    canvas: si,
                    parent,
                    index,
                })
            })
            .ok_or_else(|| OpError::UnknownElement(id.to_string()))
    }

    fn variant_mut(
        &mut self,
        canvas: &CanvasId,
        variant: &VariantId,
    ) -> Result<&mut Variant, OpError> {
        let si = self.canvas_index(canvas)?;
        self.canvases[si]
            .variants
            .get_mut(variant)
            .ok_or_else(|| OpError::UnknownVariant {
                canvas: canvas.to_string(),
                variant: variant.to_string(),
            })
    }

    // ----- validation --------------------------------------------------------

    /// New element (and children): valid ids, unique in the project and
    /// within the added subtree (`seen`).
    fn check_new_element_ids(
        &self,
        e: &Element,
        seen: &mut BTreeSet<ElementId>,
    ) -> Result<(), OpError> {
        let mut result = Ok(());
        e.walk(&mut |e| {
            if result.is_err() {
                return;
            }
            if self.element(e.id.as_str()).is_some() || !seen.insert(e.id.clone()) {
                result = Err(OpError::DuplicateId {
                    what: "element",
                    id: e.id.to_string(),
                });
            }
        });
        result
    }

    /// References of one element and its children, against `canvas`.
    fn check_element(&self, canvas: &Canvas, e: &Element) -> Result<(), OpError> {
        let mut result = Ok(());
        e.walk(&mut |e| {
            if result.is_ok() {
                result = self.check_element_own(canvas, e, &self.styles);
            }
        });
        result
    }

    fn check_element_own(
        &self,
        canvas: &Canvas,
        e: &Element,
        styles: &Styles,
    ) -> Result<(), OpError> {
        for (lang, patched) in e.lang_variants() {
            let patched =
                patched.map_err(|err| invalid(format!("language override `{lang}`: {err}")))?;
            self.check_element_refs(canvas, &patched, styles)?;
        }
        self.check_element_refs(canvas, e, styles)
    }

    fn check_element_refs(
        &self,
        canvas: &Canvas,
        e: &Element,
        styles: &Styles,
    ) -> Result<(), OpError> {
        if e.page >= canvas.page.count {
            return Err(OpError::PageOutOfRange {
                element: e.id.to_string(),
                canvas: canvas.id.to_string(),
                page: e.page,
                count: canvas.page.count,
            });
        }
        for c in e.color_refs() {
            check_color(c, styles)?;
        }
        if let ElementKind::Device(d) = &e.kind {
            check_device(d, imported_frame_context(canvas, e, d, styles))?;
        }
        if let Some(s) = e.text_style()
            && !styles.text.contains_key(s)
        {
            return Err(OpError::UnknownTextStyle(s.to_owned()));
        }
        if let Some(s) = e.shadow()
            && !styles.has_shadow(s)
        {
            return Err(OpError::UnknownShadow(s.to_owned()));
        }
        if let Some(v) = &e.visible_in {
            for t in &v.targets {
                if self.target(t.as_str()).is_none() {
                    return Err(OpError::UnknownTarget(t.to_string()));
                }
            }
            for l in &v.langs {
                if !self.languages.is_available(l.as_str()) {
                    return Err(OpError::UnknownLanguage(l.to_string()));
                }
            }
        }
        if !(0.0..=1.0).contains(&e.opacity) {
            return Err(invalid(format!(
                "element `{}`: opacity must be within 0..1",
                e.id
            )));
        }
        Ok(())
    }

    /// A variant override applied to `base` must give a valid element.
    fn check_override(
        &self,
        canvas: &Canvas,
        base: &Element,
        patch: &ElementPatch,
    ) -> Result<(), OpError> {
        let (patched, _) = base
            .patched(patch)
            .map_err(|e| invalid(format!("override: {e}")))?;
        self.check_element_own(canvas, &patched, &self.styles)
    }

    /// Canvas contents (elements, overrides, backgrounds) against the project.
    fn check_canvas(&self, canvas: &Canvas) -> Result<(), OpError> {
        canvas
            .page
            .check()
            .map_err(|e| invalid(format!("canvas `{}`: {e}", canvas.id)))?;
        for b in &canvas.backgrounds {
            for c in b.colors() {
                check_color(c, &self.styles)?;
            }
        }
        for e in &canvas.elements {
            self.check_element(canvas, e)?;
        }
        for (vid, v) in &canvas.variants {
            check_id("variant id", vid.as_str())?;
            check_scale(v.content_scale)?;
            for (eid, patch) in &v.overrides {
                let base = canvas
                    .element(eid.as_str())
                    .ok_or_else(|| OpError::UnknownElement(eid.to_string()))?;
                self.check_override(canvas, base, patch)?;
            }
        }
        Ok(())
    }

    fn check_target(&self, t: &Target) -> Result<(), OpError> {
        t.check().map_err(invalid)?;
        let canvas = self
            .canvas(t.canvas.as_str())
            .ok_or_else(|| OpError::UnknownCanvas(t.canvas.to_string()))?;
        if let Some(v) = &t.variant
            && !canvas.variants.contains_key(v)
        {
            return Err(OpError::UnknownVariant {
                canvas: t.canvas.to_string(),
                variant: v.to_string(),
            });
        }
        if let Some(langs) = &t.langs {
            for l in langs {
                if !self.languages.is_available(l.as_str()) {
                    return Err(OpError::UnknownLanguage(l.to_string()));
                }
            }
        }
        check_region(t, canvas)?;
        check_target_catalog_refs(t)
    }

    /// New styles must still resolve every reference in the project.
    fn check_styles(&self, styles: &Styles) -> Result<(), OpError> {
        for name in styles
            .colors
            .keys()
            .chain(styles.text.keys())
            .chain(styles.spans.keys())
            .chain(styles.shadows.keys())
        {
            check_id("style name", name)?;
        }
        for c in styles.color_refs() {
            check_color(c, styles)?;
        }
        for s in styles.shadow_refs() {
            if !styles.has_shadow(s) {
                return Err(OpError::UnknownShadow(s.to_owned()));
            }
        }
        styles.check_effects().map_err(invalid)?;
        styles.check_fonts().map_err(invalid)?;
        for canvas in &self.canvases {
            for b in &canvas.backgrounds {
                for c in b.colors() {
                    check_color(c, styles)?;
                }
            }
            let mut result = Ok(());
            canvas.walk_elements(&mut |e| {
                if result.is_ok() {
                    result = Self::check_element_style_refs(canvas, e, styles);
                }
            });
            result?;
        }
        Ok(())
    }

    fn check_element_style_refs(
        canvas: &Canvas,
        e: &Element,
        styles: &Styles,
    ) -> Result<(), OpError> {
        for (_, patched) in e.lang_variants() {
            if let Ok(p) = patched {
                Self::check_element_style_refs_own(canvas, &p, styles)?;
            }
        }
        Self::check_element_style_refs_own(canvas, e, styles)?;
        // Variant overrides against the new styles (e.g. a device colour of
        // an imported frame when the project default changes).
        for v in canvas.variants.values() {
            if let Some(patch) = v.overrides.get(&e.id)
                && let Ok((patched, _)) = e.patched(patch)
            {
                Self::check_element_style_refs_own(canvas, &patched, styles)?;
            }
        }
        Ok(())
    }

    fn check_element_style_refs_own(
        canvas: &Canvas,
        e: &Element,
        styles: &Styles,
    ) -> Result<(), OpError> {
        for c in e.color_refs() {
            check_color(c, styles)?;
        }
        if let ElementKind::Device(d) = &e.kind {
            check_device(d, imported_frame_context(canvas, e, d, styles))?;
        }
        if let Some(s) = e.text_style()
            && !styles.text.contains_key(s)
        {
            return Err(OpError::StillReferenced {
                what: format!("text style `{s}`"),
                by: format!("element `{}`", e.id),
            });
        }
        if let Some(s) = e.shadow()
            && !styles.has_shadow(s)
        {
            return Err(OpError::StillReferenced {
                what: format!("shadow preset `{s}`"),
                by: format!("element `{}`", e.id),
            });
        }
        Ok(())
    }

    /// Languages still used by elements and targets must stay available.
    fn check_languages_in_use(&self, languages: &Languages) -> Result<(), OpError> {
        let mut result = Ok(());
        for s in &self.canvases {
            s.walk_elements(&mut |e| {
                if result.is_ok()
                    && let Some(l) = e
                        .visible_in
                        .iter()
                        .flat_map(|v| &v.langs)
                        .find(|l| !languages.is_available(l.as_str()))
                {
                    result = Err(OpError::StillReferenced {
                        what: format!("language `{l}`"),
                        by: format!("`visible_in` of element `{}`", e.id),
                    });
                }
            });
        }
        result?;
        for t in &self.targets {
            if let Some(l) = t
                .langs
                .iter()
                .flatten()
                .find(|l| !languages.is_available(l.as_str()))
            {
                return Err(OpError::StillReferenced {
                    what: format!("language `{l}`"),
                    by: format!("target `{}`", t.id),
                });
            }
        }
        Ok(())
    }
}

fn list_mut<'a>(canvas: &'a mut Canvas, parent: Option<&ElementId>) -> &'a mut Vec<Element> {
    match parent {
        None => &mut canvas.elements,
        Some(p) => find_mut(&mut canvas.elements, p.as_str())
            .and_then(Element::children_mut)
            .expect("parent group exists (checked by caller)"),
    }
}

fn check_color(c: &ColorRef, styles: &Styles) -> Result<(), OpError> {
    match c.name() {
        Some(n) if !styles.colors.contains_key(n) => Err(OpError::UnknownColor(n.to_owned())),
        _ => Ok(()),
    }
}

fn check_scale(scale: Option<f64>) -> Result<(), OpError> {
    match scale {
        Some(s) if !(s.is_finite() && s > 0.0) => {
            Err(invalid(format!("content_scale must be positive, got {s}")))
        }
        _ => Ok(()),
    }
}

fn check_region(t: &Target, canvas: &Canvas) -> Result<(), OpError> {
    let max = canvas.page.count;
    let bad = match t.region {
        Some(Region::Pages([a, b])) => a >= max || b >= max,
        Some(Region::Rect(r)) => r.page >= max,
        None => false,
    };
    if bad {
        return Err(invalid(format!(
            "target `{}`: region is outside canvas `{}` ({max} pages)",
            t.id, canvas.id
        )));
    }
    Ok(())
}

/// Can the device show an imported frame (ADR 0012)? Its own `frame`, else
/// the project default, is `imported:<source>`, or a variant override of
/// the element sets an imported frame. Which colours are installed is
/// app-wide, not project data: resolve and validation report a missing
/// one (info), ops only check the colour is a valid name.
fn imported_frame_context(canvas: &Canvas, e: &Element, d: &DeviceProps, styles: &Styles) -> bool {
    let imported = |f: Option<&FrameStyle>| f.is_some_and(FrameStyle::is_external);
    imported(d.frame.as_ref().or(styles.frame.as_ref()))
        || canvas.variants.values().any(|v| {
            v.overrides
                .get(&e.id)
                .and_then(|p| p.frame.as_ref())
                .is_some_and(|f| imported(f.as_ref()))
        })
}

/// Device model and colour must exist: a catalog colour variant or a
/// custom `#RRGGBB`; with an imported frame (`imported`) also any colour
/// slug (`burgundy`, the vendor's colours).
fn check_device(d: &DeviceProps, imported: bool) -> Result<(), OpError> {
    let model = catalog::device(&d.model).ok_or_else(|| OpError::UnknownDevice(d.model.clone()))?;
    if let Some(c) = &d.color
        && !c.starts_with('#')
        && !model.has_color(c)
        && !(imported && is_color_slug(c))
    {
        return Err(invalid(format!(
            "device `{}` has no colour variant `{c}` (available: {}{})",
            d.model,
            model.colors.join(", "),
            if imported {
                ""
            } else {
                "; colours of imported frames need `frame: imported:<source>`"
            }
        )));
    }
    Ok(())
}

/// A colour id as the frame importer writes it (`black`, `deep-blue`).
fn is_color_slug(c: &str) -> bool {
    !c.is_empty() && crate::frame_import::slug(c) == c
}

/// Gallery preset, device map models and store slot values of a target
/// must exist in the catalog.
fn check_target_catalog_refs(t: &Target) -> Result<(), OpError> {
    if let Some(p) = t.gallery.as_ref().and_then(|g| g.preset.as_ref())
        && catalog::gallery(p).is_none()
    {
        return Err(invalid(format!(
            "target `{}`: unknown gallery profile `{p}`",
            t.id
        )));
    }
    for model in t.device_map.keys().chain(t.device_map.values()) {
        if catalog::device(model).is_none() {
            return Err(OpError::UnknownDevice(model.clone()));
        }
    }
    if let Some(slot) = &t.store {
        let c = catalog::catalog();
        let presets = &c.targets.presets;
        let known = |value: &str, list: &[String], pick: fn(&StoreSlot) -> Option<&str>| {
            list.iter().any(|v| v == value)
                || presets
                    .iter()
                    .any(|p| p.slot.as_ref().and_then(pick) == Some(value))
        };
        if let Some(a) = &slot.apple
            && !known(&a.display_type, &c.targets.store_slots.apple, |s| {
                s.apple.as_ref().map(|a| a.display_type.as_str())
            })
        {
            return Err(invalid(format!(
                "target `{}`: unknown App Store display type `{}`",
                t.id, a.display_type
            )));
        }
        if let Some(g) = &slot.google
            && !known(&g.image_type, &c.targets.store_slots.google, |s| {
                s.google.as_ref().map(|g| g.image_type.as_str())
            })
        {
            return Err(invalid(format!(
                "target `{}`: unknown Google Play image type `{}`",
                t.id, g.image_type
            )));
        }
    }
    Ok(())
}

/// Format checks plus: explicit locale codes must be store codes, and the
/// map keys valid language tags.
fn check_store(store: &StoreSettings) -> Result<(), OpError> {
    store.check().map_err(invalid)?;
    let lists = catalog::store_locales();
    for kind in StoreKind::ALL {
        for (lang, code) in store.locales.get(kind) {
            Lang::parse(lang).map_err(|e| invalid(format!("store.locales.{kind}: {e}")))?;
            if !lists.store(kind).is_known(code) {
                return Err(invalid(format!(
                    "store.locales.{kind}: `{code}` is not a {kind} store locale"
                )));
            }
        }
    }
    Ok(())
}
