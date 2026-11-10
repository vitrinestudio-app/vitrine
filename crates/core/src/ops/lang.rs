//! Per-language overrides of text elements (KONZEPT §5.1, ADR 0009).

use super::{Op, OpError, find_mut, invalid};
use crate::model::{Element, ElementId, ElementKind, ElementPatch, ElementWire, Lang, Project};

impl Project {
    pub(super) fn set_lang_override(
        &mut self,
        element: ElementId,
        lang: Lang,
        patch: Option<ElementPatch>,
    ) -> Result<Op, OpError> {
        let loc = self.locate(&element)?;
        let s = &self.strips[loc.strip];
        let old = s
            .element(element.as_str())
            .ok_or_else(|| OpError::UnknownElement(element.to_string()))?;
        let ElementKind::Text(_) = &old.kind else {
            return Err(invalid(format!(
                "element `{element}`: only text elements have language overrides"
            )));
        };
        let patch = patch.filter(|p| !p.is_empty());
        if patch.is_some() && !self.languages.is_available(lang.as_str()) {
            return Err(OpError::UnknownLanguage(lang.to_string()));
        }
        let mut new = old.clone();
        let ElementKind::Text(t) = &mut new.kind else {
            unreachable!("checked above")
        };
        let previous = match patch {
            Some(p) => t.langs.insert(lang.clone(), p),
            None => t.langs.remove(&lang),
        };
        // The wire round trip checks the allowed fields and that the
        // override gives a valid element.
        let new = Element::try_from(ElementWire::from(new)).map_err(invalid)?;
        self.check_element_own(s, &new, &self.styles)?;
        let slot = find_mut(&mut self.strips[loc.strip].elements, element.as_str())
            .ok_or_else(|| OpError::UnknownElement(element.to_string()))?;
        *slot = new;
        Ok(Op::SetLangOverride {
            element,
            lang,
            patch: previous,
        })
    }

    pub(super) fn reset_lang_override_field(
        &mut self,
        element: ElementId,
        lang: Lang,
        field: &str,
    ) -> Result<Op, OpError> {
        if !ElementPatch::is_field_name(field) {
            return Err(invalid(format!("unknown element field `{field}`")));
        }
        let (_, e) = self
            .element(element.as_str())
            .ok_or_else(|| OpError::UnknownElement(element.to_string()))?;
        let Some(old) = e.text_props().and_then(|t| t.langs.get(&lang)).cloned() else {
            return Err(invalid(format!(
                "element `{element}` has no language override for `{lang}`"
            )));
        };
        let mut patch = old.clone();
        if !patch.clear_field(field) {
            return Err(invalid(format!(
                "the `{lang}` override of `{element}` does not set `{field}`"
            )));
        }
        self.set_lang_override(element.clone(), lang.clone(), Some(patch))?;
        Ok(Op::SetLangOverride {
            element,
            lang,
            patch: Some(old),
        })
    }
}
