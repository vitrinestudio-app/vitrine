//! Structural ops added for the GUI editor (Phase 2): renaming ids,
//! regrouping elements, resetting single override fields, languages and
//! translation keys. Each returns the inverse op.

use std::collections::BTreeMap;

use super::{Op, OpError, check_id, find_mut, invalid, list_mut};
use crate::model::{
    CanvasId, ColorRef, Element, ElementId, ElementKind, ElementPatch, Lang, Project, StringKey,
    StyleKind, TargetId, VariantId,
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
        let s = &mut self.canvases[loc.canvas];
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

    pub(super) fn rename_canvas(&mut self, from: CanvasId, to: CanvasId) -> Result<Op, OpError> {
        check_id("canvas id", to.as_str())?;
        let si = self.canvas_index(&from)?;
        if from == to {
            return Ok(Op::RenameCanvas { from, to });
        }
        if self.canvas(to.as_str()).is_some() {
            return Err(OpError::DuplicateId {
                what: "canvas",
                id: to.to_string(),
            });
        }
        self.canvases[si].id = to.clone();
        for t in self.targets.iter_mut().filter(|t| t.canvas == from) {
            t.canvas = to.clone();
        }
        Ok(Op::RenameCanvas { from: to, to: from })
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
        for s in &mut self.canvases {
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
        canvas: CanvasId,
        from: VariantId,
        to: VariantId,
    ) -> Result<Op, OpError> {
        check_id("variant id", to.as_str())?;
        let si = self.canvas_index(&canvas)?;
        let variants = &mut self.canvases[si].variants;
        if !variants.contains_key(&from) {
            return Err(OpError::UnknownVariant {
                canvas: canvas.to_string(),
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
                if t.canvas == canvas && t.variant.as_ref() == Some(&from) {
                    t.variant = Some(to.clone());
                }
            }
        }
        Ok(Op::RenameVariant {
            canvas,
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
        let s = &self.canvases[loc.canvas];
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
        let s = &mut self.canvases[loc.canvas];
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
        canvas: CanvasId,
        variant: VariantId,
        element: ElementId,
        field: &str,
    ) -> Result<Op, OpError> {
        if !ElementPatch::is_field_name(field) {
            return Err(invalid(format!("unknown element field `{field}`")));
        }
        let v = self.variant_mut(&canvas, &variant)?;
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
            canvas,
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
        lang_overrides: BTreeMap<ElementId, ElementPatch>,
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
        let backup = self.clone();
        self.languages.available.insert(index, lang.clone());
        if !strings.is_empty() {
            self.strings.insert(lang.clone(), strings);
        }
        for (element, patch) in lang_overrides {
            if let Err(e) = self.set_lang_override(element, lang.clone(), Some(patch)) {
                *self = backup;
                return Err(e);
            }
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
        let mut lang_overrides = BTreeMap::new();
        for s in &mut self.canvases {
            walk_mut(&mut s.elements, &mut |e| {
                if let ElementKind::Text(t) = &mut e.kind
                    && let Some(p) = t.langs.remove(&lang)
                {
                    lang_overrides.insert(e.id.clone(), p);
                }
            });
        }
        Ok(Op::AddLanguage {
            lang,
            index: Some(index),
            strings,
            lang_overrides,
        })
    }

    /// Is `key` used by a text element or a variant override?
    pub fn string_key_in_use(&self, key: &str) -> bool {
        let mut used = false;
        for s in &self.canvases {
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
        for s in &mut self.canvases {
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

    pub(super) fn rename_style(
        &mut self,
        kind: StyleKind,
        from: String,
        to: String,
    ) -> Result<Op, OpError> {
        check_id("style name", &to)?;
        let st = &self.styles;
        let (exists, taken) = match kind {
            StyleKind::Color => (st.colors.contains_key(&from), st.colors.contains_key(&to)),
            StyleKind::Text => (st.text.contains_key(&from), st.text.contains_key(&to)),
            StyleKind::Span => (st.spans.contains_key(&from), st.spans.contains_key(&to)),
            // A built-in name may be taken over (redefined) like any other.
            StyleKind::Shadow => (st.shadows.contains_key(&from), st.shadows.contains_key(&to)),
        };
        if !exists {
            return Err(invalid(format!("unknown style `{from}`")));
        }
        if from == to {
            return Ok(Op::RenameStyle { kind, from, to });
        }
        if taken {
            return Err(OpError::DuplicateId {
                what: "style",
                id: to,
            });
        }
        if kind == StyleKind::Span
            && self
                .strings
                .values()
                .flat_map(|m| m.values())
                .any(|v| v != &rename_span_markup(v, &to, "\u{0}"))
        {
            // `[to]` already appears (as an unknown span): undo could not
            // tell the two apart.
            return Err(invalid(format!(
                "translations already use `[{to}]`; rename the span there first"
            )));
        }
        let (f, t) = (from.as_str(), to.as_str());
        let styles = &mut self.styles;
        match kind {
            StyleKind::Color => {
                if let Some(v) = styles.colors.remove(f) {
                    styles.colors.insert(to.clone(), v);
                }
                for c in styles.color_refs_mut() {
                    rename_color(c, f, t);
                }
            }
            StyleKind::Text => {
                let v = styles.text.remove(f).unwrap_or_default();
                styles.text.insert(to.clone(), v);
            }
            StyleKind::Span => {
                let v = styles.spans.remove(f).unwrap_or_default();
                styles.spans.insert(to.clone(), v);
                for m in self.strings.values_mut() {
                    for v in m.values_mut() {
                        let renamed = rename_span_markup(v, f, t);
                        if renamed != *v {
                            *v = renamed;
                        }
                    }
                }
            }
            StyleKind::Shadow => {
                let v = styles.shadows.remove(f).unwrap_or_default();
                styles.shadows.insert(to.clone(), v);
                let refs = styles
                    .text
                    .values_mut()
                    .map(|x| &mut x.shadow)
                    .chain(styles.spans.values_mut().map(|x| &mut x.shadow));
                for r in refs {
                    if r.as_deref() == Some(f) {
                        *r = Some(to.clone());
                    }
                }
            }
        }
        for s in &mut self.canvases {
            if kind == StyleKind::Color {
                for b in &mut s.backgrounds {
                    for c in b.colors_mut() {
                        rename_color(c, f, t);
                    }
                }
            }
            walk_mut(&mut s.elements, &mut |e| rename_element_refs(e, kind, f, t));
            for v in s.variants.values_mut() {
                for p in v.overrides.values_mut() {
                    p.rename_style_refs(kind, f, t);
                }
            }
        }
        Ok(Op::RenameStyle {
            kind,
            from: to,
            to: from,
        })
    }
}

fn rename_color(c: &mut ColorRef, from: &str, to: &str) {
    if matches!(c, ColorRef::Named(n) if n == from) {
        *c = ColorRef::Named(to.to_owned());
    }
}

/// Style references of one element (not its children) and of its
/// language overrides.
fn rename_element_refs(e: &mut Element, kind: StyleKind, from: &str, to: &str) {
    match kind {
        StyleKind::Color => {
            for c in e.color_refs_mut() {
                rename_color(c, from, to);
            }
        }
        StyleKind::Shadow => {
            if let Some(s) = e.shadow_mut()
                && s.as_deref() == Some(from)
            {
                *s = Some(to.to_owned());
            }
        }
        StyleKind::Text => {
            if let ElementKind::Text(t) = &mut e.kind
                && t.style.as_deref() == Some(from)
            {
                t.style = Some(to.to_owned());
            }
        }
        StyleKind::Span => {}
    }
    if let ElementKind::Text(t) = &mut e.kind {
        for p in t.langs.values_mut() {
            p.rename_style_refs(kind, from, to);
        }
    }
}

/// `text` with every span tag `[from]` replaced by `[to]` (escapes like
/// `\[from]` stay literal, ADR 0003).
pub(crate) fn rename_span_markup(text: &str, from: &str, to: &str) -> String {
    let tag = format!("[{from}]");
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(c) = rest.chars().next() {
        if c == '\\' {
            let n = 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
            out.push_str(&rest[..n]);
            rest = &rest[n..];
        } else if rest.starts_with(&tag) {
            out.push('[');
            out.push_str(to);
            out.push(']');
            rest = &rest[tag.len()..];
        } else {
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}
