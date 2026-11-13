//! Element references between elements: `clip_to` of image elements
//! (ADR 0031). Renaming an element renames the references; removing a
//! referenced element is refused unless the references are released
//! first ([`Project::clip_to_release_ops`], used by the editor's delete).

use std::collections::{BTreeMap, BTreeSet};

use super::{Op, OpError};
use crate::model::{CanvasId, Element, ElementId, Project, VariantId};

/// Where an element reference lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipRef {
    pub canvas: CanvasId,
    /// `None` = the base element, else the variant override.
    pub variant: Option<VariantId>,
    /// The image element whose `clip_to` it is.
    pub element: ElementId,
    /// The referenced element.
    pub target: ElementId,
}

impl Project {
    /// Every `clip_to` (base elements and variant overrides) naming one of
    /// `targets`, except those of elements in `targets` themselves.
    pub fn clip_to_refs(&self, targets: &BTreeSet<ElementId>) -> Vec<ClipRef> {
        self.clip_refs_where(|_, e, t| targets.contains(t) && !targets.contains(e))
    }

    /// Every `clip_to` for which `keep(canvas, element, target)` holds.
    fn clip_refs_where(
        &self,
        keep: impl Fn(&CanvasId, &ElementId, &ElementId) -> bool,
    ) -> Vec<ClipRef> {
        let mut out = Vec::new();
        for s in &self.canvases {
            s.walk_elements(&mut |e: &Element| {
                if let Some(t) = e.clip_to()
                    && keep(&s.id, &e.id, t)
                {
                    out.push(ClipRef {
                        canvas: s.id.clone(),
                        variant: None,
                        element: e.id.clone(),
                        target: t.clone(),
                    });
                }
            });
            for (vid, v) in &s.variants {
                for (eid, p) in &v.overrides {
                    if let Some(Some(t)) = &p.clip_to
                        && keep(&s.id, eid, t)
                    {
                        out.push(ClipRef {
                            canvas: s.id.clone(),
                            variant: Some(vid.clone()),
                            element: eid.clone(),
                            target: t.clone(),
                        });
                    }
                }
            }
        }
        out
    }

    /// Ops that drop every `clip_to` naming one of `targets` from
    /// elements outside `targets` (base: `clip_to: null`; variant
    /// override: the field is reset). Put them before removing `targets`
    /// in the same transaction, so one undo restores both.
    pub fn clip_to_release_ops(&self, targets: &BTreeSet<ElementId>) -> Vec<Op> {
        release_ops(self.clip_to_refs(targets))
    }

    /// Ops that drop every `clip_to` of the elements of `canvas` and every
    /// one naming an element of it (before removing all of its elements
    /// one by one).
    pub fn clip_to_release_canvas_ops(&self, canvas: &CanvasId) -> Vec<Op> {
        let mut inside = BTreeSet::new();
        if let Some(s) = self.canvas(canvas.as_str()) {
            s.walk_elements(&mut |e: &Element| {
                inside.insert(e.id.clone());
            });
        }
        release_ops(self.clip_refs_where(|c, _, t| c == canvas || inside.contains(t)))
    }

    /// Refuses removing `subtree` while an element outside it is clipped
    /// to one of its elements.
    pub(super) fn check_clip_to_unreferenced(
        &self,
        subtree: &BTreeSet<ElementId>,
    ) -> Result<(), OpError> {
        match self.clip_to_refs(subtree).into_iter().next() {
            None => Ok(()),
            Some(r) => Err(OpError::StillReferenced {
                what: format!("element `{}`", r.target),
                by: match r.variant {
                    None => format!("`clip_to` of element `{}`", r.element),
                    Some(v) => format!("`clip_to` of element `{}` in variant `{v}`", r.element),
                },
            }),
        }
    }

    /// Renames `clip_to` references `from` → `to` everywhere (elements of
    /// all canvases and their variant overrides). Refused if a reference
    /// to `to` exists already (a dangling one: undo could not tell the
    /// two apart).
    pub(super) fn rename_clip_to_refs(
        &mut self,
        from: &ElementId,
        to: &ElementId,
    ) -> Result<(), OpError> {
        let dangling = self.clip_to_refs(&BTreeSet::from([to.clone()]));
        if let Some(r) = dangling.first() {
            return Err(OpError::Invalid(format!(
                "element `{}` has `clip_to: {to}` already; change it first",
                r.element
            )));
        }
        let map = BTreeMap::from([(from.clone(), to.clone())]);
        for s in &mut self.canvases {
            for e in &mut s.elements {
                e.remap_element_refs(&map);
            }
            for v in s.variants.values_mut() {
                for p in v.overrides.values_mut() {
                    p.remap_element_refs(&map);
                }
            }
        }
        Ok(())
    }
}

fn release_ops(refs: Vec<ClipRef>) -> Vec<Op> {
    refs.into_iter()
        .map(|r| match r.variant {
            None => Op::UpdateElement {
                id: r.element,
                patch: crate::model::ElementPatch {
                    clip_to: Some(None),
                    ..Default::default()
                },
            },
            Some(variant) => Op::ResetOverrideField {
                canvas: r.canvas,
                variant,
                element: r.element,
                field: "clip_to".to_owned(),
            },
        })
        .collect()
}
