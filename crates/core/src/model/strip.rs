//! Strips (canvases), page specs and variants.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::Aspect;
use super::element::{Element, ElementPatch};
use super::fill::Fill;
use super::fmt::{flow_style, num};
use super::ids::{ElementId, StripId, VariantId};

/// A continuous canvas of `page.count` equally sized pages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Strip {
    pub id: StripId,
    pub page: PageSpec,
    /// Canvas-wide backgrounds (bottom to top), running over pages and gaps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backgrounds: Vec<Fill>,
    /// Elements in painting order (bottom to top, before `z`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub elements: Vec<Element>,
    /// Variants by name (written sorted).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub variants: BTreeMap<VariantId, Variant>,
}

impl Strip {
    pub fn new(id: StripId, aspect: Aspect, count: u32) -> Self {
        Self {
            id,
            page: PageSpec { aspect, count },
            backgrounds: Vec::new(),
            elements: Vec::new(),
            variants: BTreeMap::new(),
        }
    }

    /// Visit all elements including group children (pre-order).
    pub fn walk_elements<'a>(&'a self, f: &mut impl FnMut(&'a Element)) {
        for e in &self.elements {
            e.walk(f);
        }
    }

    /// Find an element (also inside groups).
    pub fn element(&self, id: &str) -> Option<&Element> {
        let mut found = None;
        self.walk_elements(&mut |e| {
            if found.is_none() && e.id.as_str() == id {
                found = Some(e);
            }
        });
        found
    }
}

/// Page geometry of a strip: `page: { aspect: "1320:2868", count: 5 }`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct PageSpec {
    /// Design aspect ratio of one page.
    pub aspect: Aspect,
    /// Number of pages (>= 1).
    pub count: u32,
}
flow_style!(PageSpec);

/// Derived geometry of a strip for another format (§3.4). Only the
/// overridden properties differ; everything else is shared.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    /// Scale applied to device content (default 1).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "num"
    )]
    pub content_scale: Option<f64>,
    /// Per-element overrides, by element id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub overrides: BTreeMap<ElementId, ElementPatch>,
}
