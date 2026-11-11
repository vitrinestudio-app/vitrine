//! Canvases, page specs and variants.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::common::Aspect;
use super::element::{Element, ElementPatch};
use super::fill::Fill;
use super::fmt::{flow_seq, flow_style, num};
use super::ids::{CanvasId, ElementId, VariantId, is_valid_id};

/// A continuous canvas of `page.count` equally sized pages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Canvas {
    pub id: CanvasId,
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

impl Canvas {
    pub fn new(id: CanvasId, aspect: Aspect, count: u32) -> Self {
        Self {
            id,
            page: PageSpec {
                aspect,
                count,
                ids: Vec::new(),
            },
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

    /// Removes `null` fields from the variant overrides and the language
    /// overrides of the canvas's elements (see
    /// [`super::Project::drop_override_nulls`]); returns where they were.
    pub fn drop_override_nulls(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        for e in &mut self.elements {
            out.extend(e.drop_lang_override_nulls());
        }
        for (vid, v) in &mut self.variants {
            v.overrides.retain(|id, p| {
                for f in p.drop_nulls() {
                    out.push(format!(
                        "canvas `{}`, variant `{vid}`, element `{id}`: {f}",
                        self.id
                    ));
                }
                !p.is_empty()
            });
        }
        out
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

/// Page geometry of a canvas: `page: { aspect: "1320:2868", count: 5 }`,
/// optionally with page ids used in export file names
/// (`ids: [hero, planner]` → `01_hero.png`, `02_planner.png`, `03_page-03.png`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub struct PageSpec {
    /// Design aspect ratio of one page.
    pub aspect: Aspect,
    /// Number of pages (>= 1).
    pub count: u32,
    /// Optional page ids by index (may be shorter than `count`; missing ids
    /// default to `page-01`, `page-02` …). Unique within the canvas.
    #[serde(
        default,
        skip_serializing_if = "Vec::is_empty",
        serialize_with = "flow_seq"
    )]
    pub ids: Vec<String>,
}
flow_style!(PageSpec);

impl PageSpec {
    /// Id of page `index` (0-based): the explicit id or `page-NN` (1-based).
    pub fn page_id(&self, index: u32) -> String {
        self.ids
            .get(index as usize)
            .cloned()
            .unwrap_or_else(|| format!("page-{:02}", index + 1))
    }

    /// Check count and page ids on their own.
    pub fn check(&self) -> Result<(), String> {
        if self.count == 0 {
            return Err("page count must be >= 1".into());
        }
        if self.ids.len() > self.count as usize {
            return Err(format!(
                "{} page ids given but only {} pages",
                self.ids.len(),
                self.count
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for i in 0..self.count {
            let id = self.page_id(i);
            if !is_valid_id(&id) {
                return Err(format!("invalid page id `{id}`"));
            }
            if !seen.insert(id.clone()) {
                return Err(format!("duplicate page id `{id}`"));
            }
        }
        Ok(())
    }
}

/// Derived geometry of a canvas for another format (§3.4). Only the
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

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(yaml: &str) -> PageSpec {
        crate::io::from_yaml(yaml).unwrap()
    }

    #[test]
    fn page_ids_default_and_explicit() {
        let p = spec("{ aspect: \"1320:2868\", count: 3, ids: [hero, planner] }");
        assert_eq!(p.page_id(0), "hero");
        assert_eq!(p.page_id(1), "planner");
        assert_eq!(p.page_id(2), "page-03");
        assert!(p.check().is_ok());
        let yaml = crate::io::to_yaml(&p).unwrap();
        assert_eq!(
            yaml,
            "{aspect: 1320:2868, count: 3, ids: [hero, planner]}\n"
        );
        let p = spec("{ aspect: \"1:2\", count: 2 }");
        assert_eq!(p.page_id(1), "page-02");
        assert_eq!(crate::io::to_yaml(&p).unwrap(), "{aspect: 1:2, count: 2}\n");
    }

    #[test]
    fn page_id_errors() {
        let err = |y: &str| spec(y).check().unwrap_err();
        assert!(err("{ aspect: \"1:2\", count: 1, ids: [a, b] }").contains("only 1 pages"));
        assert!(err("{ aspect: \"1:2\", count: 2, ids: [a, a] }").contains("duplicate"));
        assert!(err("{ aspect: \"1:2\", count: 2, ids: [page-02] }").contains("duplicate"));
        assert!(err("{ aspect: \"1:2\", count: 1, ids: [\"a b\"] }").contains("invalid"));
        assert!(err("{ aspect: \"1:2\", count: 0 }").contains(">= 1"));
    }
}
