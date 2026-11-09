//! Structural ops added for the GUI editor (Phase 2): renaming ids,
//! regrouping elements, resetting single override fields, languages and
//! translation keys. Each returns the inverse op.

use std::collections::BTreeMap;

use super::{Op, OpError, check_id, find_mut, invalid, list_mut};
use crate::model::{
    Element, ElementId, ElementKind, Lang, Project, StringKey, StripId, TargetId, VariantId,
};

/// Visit every element of a list (and its descendants) mutably.
fn walk_mut(list: &mut [Element], f: &mut impl FnMut(&mut Element)) {
    for e in list {
        f(e);
        if let Some(children) = e.children_mut() {
            walk_mut(children, f);
        }
    }
}

impl Project {
    pub(super) fn rename_element(&mut self, from: ElementId, to: ElementId) -> Result<Op, OpError> {
        check_id("element id", to.as_str())?;
        let loc = self.locate(&from)?;
        if from == to {
            return Ok(Op::RenameElement { from, to });
        }
        if self.element(to.as_str()).is_some() {
            return Err(OpError::DuplicateId {
                what: "element",
                id: to.to_string(),
            });
        }
        let s = &mut self.strips[loc.strip];
        let e = find_mut(&mut s.elements, from.as_str())
            .ok_or_else(|| OpError::UnknownElement(from.to_string()))?;
        e.id = to.clone();
        for v in s.variants.values_mut() {
            if let Some(p) = v.overrides.remove(&from) {
                v.overrides.insert(to.clone(), p);
            }
        }
        Ok(Op::RenameElement { from: to, to: from })
    }

    pub(super) fn rename_strip(&mut self, from: StripId, to: StripId) -> Result<Op, OpError> {
        check_id("strip id", to.as_str())?;
        let si = self.strip_index(&from)?;
        if from == to {
            return Ok(Op::RenameStrip { from, to });
        }
        if self.strip(to.as_str()).is_some() {
            return Err(OpError::DuplicateId {
                what: "strip",
                id: to.to_string(),
            });
        }
        self.strips[si].id = to.clone();
        for t in self.targets.iter_mut().filter(|t| t.strip == from) {
            t.strip = to.clone();
        }
        Ok(Op::RenameStrip { from: to, to: from })
    }

    pub(super) fn rename_target(&mut self, from: TargetId, to: TargetId) -> Result<Op, OpError> {
        check_id("target id", to.as_str())?;
        let ti = self.target_index(&from)?;
        if from == to {
            return Ok(Op::RenameTarget { from, to });
        }
        if self.target(to.as_str()).is_some() {
            return Err(OpError::DuplicateId {
                what: "target",
                id: to.to_string(),
            });
        }
        self.targets[ti].id = to.clone();
        let rename = |list: &mut Vec<TargetId>| {
            for t in list.iter_mut().filter(|t| **t == from) {
                *t = to.clone();
            }
        };
        for s in &mut self.strips {
            walk_mut(&mut s.elements, &mut |e| {
                if let Some(v) = &mut e.visible_in {
                    rename(&mut v.targets);
                }
            });
            for v in s.variants.values_mut() {
                for p in v.overrides.values_mut() {
                    if let Some(Some(vi)) = &mut p.visible_in {
                        rename(&mut vi.targets);
                    }
                }
            }
        }
        Ok(Op::RenameTarget { from: to, to: from })
    }

    pub(super) fn rename_variant(
        &mut self,
        strip: StripId,
        from: VariantId,
        to: VariantId,
    ) -> Result<Op, OpError> {
        check_id("variant id", to.as_str())?;
        let si = self.strip_index(&strip)?;
        let variants = &mut self.strips[si].variants;
        if !variants.contains_key(&from) {
            return Err(OpError::UnknownVariant {
                strip: strip.to_string(),
                variant: from.to_string(),
            });
        }
        if from != to {
            if variants.contains_key(&to) {
                return Err(OpError::DuplicateId {
                    what: "variant",
                    id: to.to_string(),
                });
            }
            let v = variants.remove(&from).unwrap_or_default();
            variants.insert(to.clone(), v);
            for t in &mut self.targets {
                if t.strip == strip && t.variant.as_ref() == Some(&from) {
                    t.variant = Some(to.clone());
                }
            }
        }
        Ok(Op::RenameVariant {
            strip,
            from: to,
            to: from,
        })
    }

    pub(super) fn move_element_to(
        &mut self,
        id: ElementId,
        parent: Option<&ElementId>,
        index: Option<usize>,
    ) -> Result<Op, OpError> {
        let loc = self.locate(&id)?;
        let s = &self.strips[loc.strip];
        if let Some(p) = parent {
            let pe = s
                .element(p.as_str())
                .ok_or_else(|| OpError::UnknownElement(p.to_string()))?;
            if !matches!(pe.kind, ElementKind::Group(_)) {
                return Err(invalid(format!("parent `{p}` is not a group")));
            }
            let e = s
                .element(id.as_str())
                .ok_or_else(|| OpError::UnknownElement(id.to_string()))?;
            let mut inside = false;
            e.walk(&mut |c| inside |= &c.id == p);
            if inside {
                return Err(invalid(format!(
                    "cannot move `{id}` into itself or its own descendant `{p}`"
                )));
            }
        }
        // Length of the destination list once the element is taken out.
        let len = {
            let list_len = match parent {
                None => s.elements.len(),
                Some(p) => s.element(p.as_str()).map_or(0, |g| g.children().len()),
            };
            if parent == loc.parent.as_ref() {
                list_len - 1
            } else {
                list_len
            }
        };
        let index = super::check_index(index, len)?;
        let s = &mut self.strips[loc.strip];
        let e = list_mut(s, loc.parent.as_ref()).remove(loc.index);
        list_mut(s, parent).insert(index, e);
        Ok(Op::MoveElementTo {
            id,
            parent: loc.parent,
            index: Some(loc.index),
        })
    }

    pub(super) fn reset_override_field(
        &mut self,
        strip: StripId,
        variant: VariantId,
        element: ElementId,
        field: &str,
    ) -> Result<Op, OpError> {
        if !crate::model::ElementPatch::is_field_name(field) {
            return Err(invalid(format!("unknown element field `{field}`")));
        }
        let v = self.variant_mut(&strip, &variant)?;
        let Some(patch) = v.overrides.get_mut(&element) else {
            return Err(invalid(format!(
                "variant `{variant}` has no override for `{element}`"
            )));
        };
        let old = patch.clone();
        if !patch.clear_field(field) {
            return Err(invalid(format!(
                "the override of `{element}` in variant `{variant}` does not set `{field}`"
            )));
        }
        if patch.is_empty() {
            v.overrides.remove(&element);
        }
        Ok(Op::SetVariantOverride {
            strip,
            variant,
            element,
            patch: Some(old),
        })
    }

    pub(super) fn set_default_language(&mut self, lang: Lang) -> Result<Op, OpError> {
        if !self.languages.is_available(lang.as_str()) {
            return Err(OpError::UnknownLanguage(lang.to_string()));
        }
        let old = std::mem::replace(&mut self.languages.default, lang);
        Ok(Op::SetDefaultLanguage { lang: old })
    }

    pub(super) fn add_language(
        &mut self,
        lang: Lang,
        index: Option<usize>,
        strings: BTreeMap<StringKey, String>,
    ) -> Result<Op, OpError> {
        if self.languages.is_available(lang.as_str()) {
            return Err(OpError::DuplicateId {
                what: "language",
                id: lang.to_string(),
            });
        }
        if !strings.is_empty() && self.strings.get(&lang).is_some_and(|m| !m.is_empty()) {
            return Err(invalid(format!(
                "language `{lang}` already has translations; cannot restore others"
            )));
        }
        let index = super::check_index(index, self.languages.available.len())?;
        self.languages.available.insert(index, lang.clone());
        if !strings.is_empty() {
            self.strings.insert(lang.clone(), strings);
        }
        Ok(Op::RemoveLanguage { lang })
    }

    pub(super) fn remove_language(&mut self, lang: Lang) -> Result<Op, OpError> {
        let Some(index) = self.languages.available.iter().position(|l| *l == lang) else {
            return Err(OpError::UnknownLanguage(lang.to_string()));
        };
        if self.languages.default == lang {
            return Err(invalid(format!(
                "`{lang}` is the default language; make another language the default first"
            )));
        }
        let mut languages = self.languages.clone();
        languages.available.remove(index);
        self.check_languages_in_use(&languages)?;
        self.languages = languages;
        let strings = self.strings.remove(&lang).unwrap_or_default();
        Ok(Op::AddLanguage {
            lang,
            index: Some(index),
            strings,
        })
    }

    /// Is `key` used by a text element or a variant override?
    pub fn string_key_in_use(&self, key: &str) -> bool {
        let mut used = false;
        for s in &self.strips {
            s.walk_elements(&mut |e| {
                if let ElementKind::Text(t) = &e.kind {
                    used |= t.text.as_str() == key;
                }
            });
            for v in s.variants.values() {
                for p in v.overrides.values() {
                    used |= matches!(&p.text, Some(Some(k)) if k.as_str() == key);
                }
            }
        }
        used
    }

    pub(super) fn rename_string_key(
        &mut self,
        from: StringKey,
        to: StringKey,
    ) -> Result<Op, OpError> {
        let has_strings = |p: &Project, k: &str| p.strings.values().any(|m| m.contains_key(k));
        if !has_strings(self, from.as_str()) && !self.string_key_in_use(from.as_str()) {
            return Err(invalid(format!("unknown translation key `{from}`")));
        }
        if from == to {
            return Ok(Op::RenameStringKey { from, to });
        }
        if has_strings(self, to.as_str()) || self.string_key_in_use(to.as_str()) {
            return Err(OpError::DuplicateId {
                what: "translation key",
                id: to.to_string(),
            });
        }
        for m in self.strings.values_mut() {
            if let Some(v) = m.remove(&from) {
                m.insert(to.clone(), v);
            }
        }
        for s in &mut self.strips {
            walk_mut(&mut s.elements, &mut |e| {
                if let ElementKind::Text(t) = &mut e.kind
                    && t.text == from
                {
                    t.text = to.clone();
                }
            });
            for v in s.variants.values_mut() {
                for p in v.overrides.values_mut() {
                    if matches!(&p.text, Some(Some(k)) if *k == from) {
                        p.text = Some(Some(to.clone()));
                    }
                }
            }
        }
        Ok(Op::RenameStringKey { from: to, to: from })
    }
}
