//! Operations API (KONZEPT §10.2).
//!
//! Every change to a project is an [`Op`]. [`Project::apply`] validates the
//! op, applies it and returns the inverse op (undo). [`Project::apply_ops`]
//! applies a list atomically (all or nothing) and returns the inverse list.
//! [`History`] keeps undo/redo stacks of such transactions.
//!
//! Validation covers what an op touches: ids exist / are unique, page
//! indices are within the strip, referenced styles, colours, shadows,
//! languages, targets and variants exist, device models, colour variants,
//! gallery profiles and store slot values exist in [`crate::catalog`], and
//! removals of things that are
//! still referenced are refused. Pre-existing problems elsewhere in the
//! project (e.g. after a hand edit) do not block unrelated ops; they are
//! reported by `core::validate`.

mod history;
mod patch;
mod structure;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub use history::History;
pub use patch::{StripPatch, TargetPatch};

use crate::catalog;
use crate::model::{
    ColorRef, DeviceProps, Element, ElementId, ElementKind, ElementPatch, ImportSettings, Lang,
    Languages, Project, Region, StoreSlot, StringKey, Strip, StripId, Styles, Target, TargetId,
    Variant, VariantId, is_valid_id,
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
    /// Set (`Some`) or remove (`None`) a translation.
    SetString {
        lang: Lang,
        key: StringKey,
        value: Option<String>,
    },
    AddStrip {
        strip: Strip,
        /// Position in the strip list (default: end).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        index: Option<usize>,
    },
    UpdateStrip {
        id: StripId,
        patch: StripPatch,
    },
    RemoveStrip {
        id: StripId,
    },
    AddElement {
        strip: StripId,
        /// Group to add the element to (default: top level of the strip).
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
    RemoveElement {
        id: ElementId,
    },
    /// Move an element to another position in its list (painting order).
    MoveElement {
        id: ElementId,
        index: usize,
    },
    AddVariant {
        strip: StripId,
        id: VariantId,
        #[serde(default)]
        variant: Variant,
    },
    RemoveVariant {
        strip: StripId,
        id: VariantId,
    },
    SetVariantContentScale {
        strip: StripId,
        variant: VariantId,
        content_scale: Option<f64>,
    },
    /// Set (`Some`) or remove (`None`) the override of one element in a variant.
    SetVariantOverride {
        strip: StripId,
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
    /// Rename an element; its variant overrides follow.
    RenameElement {
        from: ElementId,
        to: ElementId,
    },
    /// Rename a strip; targets showing it follow.
    RenameStrip {
        from: StripId,
        to: StripId,
    },
    /// Rename a target; `visible_in.targets` of elements and variant
    /// overrides follow.
    RenameTarget {
        from: TargetId,
        to: TargetId,
    },
    /// Rename a variant of a strip; targets using it follow.
    RenameVariant {
        strip: StripId,
        from: VariantId,
        to: VariantId,
    },
    /// Move an element into a group, out of it (`parent: None` = top level
    /// of its strip) or to another position. `index` is the position in the
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
        strip: StripId,
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
    },
    /// Remove a language and its translations. Refused for the default
    /// language and for languages still named by `visible_in` or a target's
    /// `langs`. Explicit `languages.fallback` entries are kept (they may
    /// name languages outside `available`).
    RemoveLanguage {
        lang: Lang,
    },
    /// Rename a translation key in every language and in every text element
    /// (and variant override) using it.
    RenameStringKey {
        from: StringKey,
        to: StringKey,
    },
}

/// Why an op was rejected. The project is unchanged in that case.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum OpError {
    #[error("unknown strip `{0}`")]
    UnknownStrip(String),
    #[error("unknown element `{0}`")]
    UnknownElement(String),
    #[error("unknown target `{0}`")]
    UnknownTarget(String),
    #[error("unknown variant `{variant}` in strip `{strip}`")]
    UnknownVariant { strip: String, variant: String },
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
    #[error("element `{element}`: page {page} is out of range (strip `{strip}` has {count} pages)")]
    PageOutOfRange {
        element: String,
        strip: String,
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
    strip: usize,
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
    pub fn apply(&mut self, op: Op) -> Result<Op, OpError> {
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
            Op::AddStrip { strip, index } => {
                check_id("strip id", strip.id.as_str())?;
                if self.strip(strip.id.as_str()).is_some() {
                    return Err(OpError::DuplicateId {
                        what: "strip",
                        id: strip.id.to_string(),
                    });
                }
                let index = check_index(index, self.strips.len())?;
                let mut ids = BTreeSet::new();
                for e in &strip.elements {
                    self.check_new_element_ids(e, &mut ids)?;
                }
                self.check_strip(&strip)?;
                let id = strip.id.clone();
                self.strips.insert(index, strip);
                Ok(Op::RemoveStrip { id })
            }
            Op::UpdateStrip { id, patch } => {
                let si = self.strip_index(&id)?;
                let mut strip = self.strips[si].clone();
                let inverse = patch.apply(&mut strip);
                self.check_strip(&strip)?;
                for t in self.targets.iter().filter(|t| t.strip == id) {
                    check_region(t, &strip)?;
                }
                self.strips[si] = strip;
                Ok(Op::UpdateStrip { id, patch: inverse })
            }
            Op::RemoveStrip { id } => {
                let si = self.strip_index(&id)?;
                if let Some(t) = self.targets.iter().find(|t| t.strip == id) {
                    return Err(OpError::StillReferenced {
                        what: format!("strip `{id}`"),
                        by: format!("target `{}`", t.id),
                    });
                }
                let strip = self.strips.remove(si);
                Ok(Op::AddStrip {
                    strip,
                    index: Some(si),
                })
            }
            Op::AddElement {
                strip,
                parent,
                index,
                element,
                overrides,
            } => {
                let si = self.strip_index(&strip)?;
                let mut ids = BTreeSet::new();
                self.check_new_element_ids(&element, &mut ids)?;
                let s = &self.strips[si];
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
                            strip: strip.to_string(),
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
                let s = &mut self.strips[si];
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
                let s = &self.strips[loc.strip];
                let old = s
                    .element(id.as_str())
                    .ok_or_else(|| OpError::UnknownElement(id.to_string()))?;
                let (new, inverse) = old.patched(&patch).map_err(invalid)?;
                self.check_element(s, &new)?;
                let slot = find_mut(&mut self.strips[loc.strip].elements, id.as_str())
                    .ok_or_else(|| OpError::UnknownElement(id.to_string()))?;
                *slot = new;
                Ok(Op::UpdateElement { id, patch: inverse })
            }
            Op::RemoveElement { id } => {
                let loc = self.locate(&id)?;
                let s = &mut self.strips[loc.strip];
                let element = list_mut(s, loc.parent.as_ref()).remove(loc.index);
                let mut subtree = BTreeSet::new();
                element.walk(&mut |e| {
                    subtree.insert(e.id.clone());
                });
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
                    strip: s.id.clone(),
                    parent: loc.parent,
                    index: Some(loc.index),
                    element,
                    overrides,
                })
            }
            Op::MoveElement { id, index } => {
                let loc = self.locate(&id)?;
                let list = list_mut(&mut self.strips[loc.strip], loc.parent.as_ref());
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
            Op::AddVariant { strip, id, variant } => {
                check_id("variant id", id.as_str())?;
                let si = self.strip_index(&strip)?;
                let s = &self.strips[si];
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
                self.strips[si].variants.insert(id.clone(), variant);
                Ok(Op::RemoveVariant { strip, id })
            }
            Op::RemoveVariant { strip, id } => {
                let si = self.strip_index(&strip)?;
                if !self.strips[si].variants.contains_key(&id) {
                    return Err(OpError::UnknownVariant {
                        strip: strip.to_string(),
                        variant: id.to_string(),
                    });
                }
                if let Some(t) = self
                    .targets
                    .iter()
                    .find(|t| t.strip == strip && t.variant.as_ref() == Some(&id))
                {
                    return Err(OpError::StillReferenced {
                        what: format!("variant `{id}`"),
                        by: format!("target `{}`", t.id),
                    });
                }
                let variant = self.strips[si].variants.remove(&id).unwrap_or_default();
                Ok(Op::AddVariant { strip, id, variant })
            }
            Op::SetVariantContentScale {
                strip,
                variant,
                content_scale,
            } => {
                check_scale(content_scale)?;
                let v = self.variant_mut(&strip, &variant)?;
                let old = std::mem::replace(&mut v.content_scale, content_scale);
                Ok(Op::SetVariantContentScale {
                    strip,
                    variant,
                    content_scale: old,
                })
            }
            Op::SetVariantOverride {
                strip,
                variant,
                element,
                patch,
            } => {
                let si = self.strip_index(&strip)?;
                let s = &self.strips[si];
                let base = s
                    .element(element.as_str())
                    .ok_or_else(|| OpError::UnknownElement(element.to_string()))?;
                let patch = patch.filter(|p| !p.is_empty());
                if let Some(p) = &patch {
                    self.check_override(s, base, p)?;
                }
                let v = self.variant_mut(&strip, &variant)?;
                let old = match patch {
                    Some(p) => v.overrides.insert(element.clone(), p),
                    None => v.overrides.remove(&element),
                };
                Ok(Op::SetVariantOverride {
                    strip,
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
                for s in &self.strips {
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
            Op::RenameStrip { from, to } => self.rename_strip(from, to),
            Op::RenameTarget { from, to } => self.rename_target(from, to),
            Op::RenameVariant { strip, from, to } => self.rename_variant(strip, from, to),
            Op::MoveElementTo { id, parent, index } => {
                self.move_element_to(id, parent.as_ref(), index)
            }
            Op::ResetOverrideField {
                strip,
                variant,
                element,
                field,
            } => self.reset_override_field(strip, variant, element, &field),
            Op::SetDefaultLanguage { lang } => self.set_default_language(lang),
            Op::AddLanguage {
                lang,
                index,
                strings,
            } => self.add_language(lang, index, strings),
            Op::RemoveLanguage { lang } => self.remove_language(lang),
            Op::RenameStringKey { from, to } => self.rename_string_key(from, to),
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

    fn strip_index(&self, id: &StripId) -> Result<usize, OpError> {
        self.strips
            .iter()
            .position(|s| &s.id == id)
            .ok_or_else(|| OpError::UnknownStrip(id.to_string()))
    }

    fn target_index(&self, id: &TargetId) -> Result<usize, OpError> {
        self.targets
            .iter()
            .position(|t| &t.id == id)
            .ok_or_else(|| OpError::UnknownTarget(id.to_string()))
    }

    fn locate(&self, id: &ElementId) -> Result<Loc, OpError> {
        self.strips
            .iter()
            .enumerate()
            .find_map(|(si, s)| {
                locate_in(&s.elements, id.as_str(), None).map(|(parent, index)| Loc {
                    strip: si,
                    parent,
                    index,
                })
            })
            .ok_or_else(|| OpError::UnknownElement(id.to_string()))
    }

    fn variant_mut(
        &mut self,
        strip: &StripId,
        variant: &VariantId,
    ) -> Result<&mut Variant, OpError> {
        let si = self.strip_index(strip)?;
        self.strips[si]
            .variants
            .get_mut(variant)
            .ok_or_else(|| OpError::UnknownVariant {
                strip: strip.to_string(),
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

    /// References of one element and its children, against `strip`.
    fn check_element(&self, strip: &Strip, e: &Element) -> Result<(), OpError> {
        let mut result = Ok(());
        e.walk(&mut |e| {
            if result.is_ok() {
                result = self.check_element_own(strip, e, &self.styles);
            }
        });
        result
    }

    fn check_element_own(
        &self,
        strip: &Strip,
        e: &Element,
        styles: &Styles,
    ) -> Result<(), OpError> {
        if e.page >= strip.page.count {
            return Err(OpError::PageOutOfRange {
                element: e.id.to_string(),
                strip: strip.id.to_string(),
                page: e.page,
                count: strip.page.count,
            });
        }
        for c in e.color_refs() {
            check_color(c, styles)?;
        }
        if let ElementKind::Device(d) = &e.kind {
            check_device(d)?;
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
        strip: &Strip,
        base: &Element,
        patch: &ElementPatch,
    ) -> Result<(), OpError> {
        let (patched, _) = base
            .patched(patch)
            .map_err(|e| invalid(format!("override: {e}")))?;
        self.check_element_own(strip, &patched, &self.styles)
    }

    /// Strip contents (elements, overrides, backgrounds) against the project.
    fn check_strip(&self, strip: &Strip) -> Result<(), OpError> {
        strip
            .page
            .check()
            .map_err(|e| invalid(format!("strip `{}`: {e}", strip.id)))?;
        for b in &strip.backgrounds {
            for c in b.colors() {
                check_color(c, &self.styles)?;
            }
        }
        for e in &strip.elements {
            self.check_element(strip, e)?;
        }
        for (vid, v) in &strip.variants {
            check_id("variant id", vid.as_str())?;
            check_scale(v.content_scale)?;
            for (eid, patch) in &v.overrides {
                let base = strip
                    .element(eid.as_str())
                    .ok_or_else(|| OpError::UnknownElement(eid.to_string()))?;
                self.check_override(strip, base, patch)?;
            }
        }
        Ok(())
    }

    fn check_target(&self, t: &Target) -> Result<(), OpError> {
        t.check().map_err(invalid)?;
        let strip = self
            .strip(t.strip.as_str())
            .ok_or_else(|| OpError::UnknownStrip(t.strip.to_string()))?;
        if let Some(v) = &t.variant
            && !strip.variants.contains_key(v)
        {
            return Err(OpError::UnknownVariant {
                strip: t.strip.to_string(),
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
        check_region(t, strip)?;
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
        for strip in &self.strips {
            for b in &strip.backgrounds {
                for c in b.colors() {
                    check_color(c, styles)?;
                }
            }
            let mut result = Ok(());
            strip.walk_elements(&mut |e| {
                if result.is_ok() {
                    result = Self::check_element_style_refs(e, styles);
                }
            });
            result?;
        }
        Ok(())
    }

    fn check_element_style_refs(e: &Element, styles: &Styles) -> Result<(), OpError> {
        for c in e.color_refs() {
            check_color(c, styles)?;
        }
        if let ElementKind::Device(d) = &e.kind {
            check_device(d)?;
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
        for s in &self.strips {
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

fn list_mut<'a>(strip: &'a mut Strip, parent: Option<&ElementId>) -> &'a mut Vec<Element> {
    match parent {
        None => &mut strip.elements,
        Some(p) => find_mut(&mut strip.elements, p.as_str())
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

fn check_region(t: &Target, strip: &Strip) -> Result<(), OpError> {
    let max = strip.page.count;
    let bad = match t.region {
        Some(Region::Pages([a, b])) => a >= max || b >= max,
        Some(Region::Rect(r)) => r.page >= max,
        None => false,
    };
    if bad {
        return Err(invalid(format!(
            "target `{}`: region is outside strip `{}` ({max} pages)",
            t.id, strip.id
        )));
    }
    Ok(())
}

/// Device model and colour variant must exist in the catalog.
fn check_device(d: &DeviceProps) -> Result<(), OpError> {
    let model = catalog::device(&d.model).ok_or_else(|| OpError::UnknownDevice(d.model.clone()))?;
    if let Some(c) = &d.color
        && !c.starts_with('#')
        && !model.has_color(c)
    {
        return Err(invalid(format!(
            "device `{}` has no colour variant `{c}` (available: {})",
            d.model,
            model.colors.join(", ")
        )));
    }
    Ok(())
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
