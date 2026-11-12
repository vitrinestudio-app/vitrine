//! Template tags (owner decision 2026-11-12, ADR 0023 amendment): a fixed
//! taxonomy in three facets the author sets – `style` (1–2), `color` (1–2),
//! `use` (1–3) – plus `devices`, derived from the targets and canvases. The
//! data is `crates/core/data/template-tags.yaml` (labels in English and
//! German), shared by the app, the CLI, MCP, the API server and the website.
//!
//! In `template.yaml` the tags are a map:
//!
//! ```yaml
//! tags:
//!   style: [minimal]
//!   color: [dark, gradient]
//!   use: [travel]
//! ```
//!
//! Packages written before the taxonomy had a free list (`tags: [a, b]`);
//! it is still read: known values are sorted into their facet, other
//! words are dropped ([`TemplateTags::dropped`]).

use std::collections::BTreeSet;
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::catalog::{DeviceClass, Platform};
use crate::model::{ElementKind, Project, TargetKind};

const TAGS_YAML: &str = include_str!("../../data/template-tags.yaml");

/// A label in the two UI languages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub en: String,
    pub de: String,
}

/// One value of a facet.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TagValue {
    pub id: String,
    pub label: Label,
    /// Derived facets: target kinds that give this value.
    #[serde(default, skip_serializing)]
    pub kinds: Vec<TargetKind>,
    /// Derived facets: target preset ids that give this value.
    #[serde(default, skip_serializing)]
    pub presets: Vec<String>,
    /// Derived facets: preset platforms that give this value.
    #[serde(default, skip_serializing)]
    pub platforms: Vec<Platform>,
    /// Derived facets: device classes on the canvas that give this value.
    #[serde(default, skip_serializing)]
    pub classes: Vec<DeviceClass>,
}

/// One facet of the taxonomy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Facet {
    pub id: String,
    pub label: Label,
    #[serde(default)]
    pub min: u32,
    #[serde(default)]
    pub max: u32,
    /// Computed from the template, not set by the author.
    #[serde(default)]
    pub derived: bool,
    pub values: Vec<TagValue>,
}

impl Facet {
    pub fn has(&self, value: &str) -> bool {
        self.values.iter().any(|v| v.id == value)
    }

    /// The allowed values, comma separated (for error messages).
    pub fn allowed(&self) -> String {
        self.values
            .iter()
            .map(|v| v.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The whole taxonomy (`template-tags.yaml`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Taxonomy {
    pub facets: Vec<Facet>,
}

impl Taxonomy {
    pub fn facet(&self, id: &str) -> Option<&Facet> {
        self.facets.iter().find(|f| f.id == id)
    }

    /// The facet holding `value` (author-set facets only).
    pub fn facet_of(&self, value: &str) -> Option<&Facet> {
        self.facets.iter().find(|f| !f.derived && f.has(value))
    }
}

static TAXONOMY: LazyLock<Taxonomy> = LazyLock::new(|| {
    crate::io::from_yaml(TAGS_YAML).unwrap_or_else(|e| panic!("template-tags.yaml: {e}"))
});

/// The embedded taxonomy.
pub fn taxonomy() -> &'static Taxonomy {
    &TAXONOMY
}

/// Facet ids the author sets, in order.
pub const FACETS: [&str; 3] = ["style", "color", "use"];
/// The derived facet.
pub const DEVICES: &str = "devices";

/// The tags of a template (author-set facets).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct TemplateTags {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub style: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub color: Vec<String>,
    #[serde(default, rename = "use", skip_serializing_if = "Vec::is_empty")]
    pub use_: Vec<String>,
    /// Words of an old free tag list that are not in the taxonomy (not
    /// written back).
    #[serde(skip)]
    pub dropped: Vec<String>,
}

/// The map form in YAML/JSON.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TagMap {
    #[serde(default)]
    style: Vec<String>,
    #[serde(default)]
    color: Vec<String>,
    #[serde(default, rename = "use")]
    use_: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TagsRepr {
    Map(TagMap),
    List(Vec<String>),
}

impl<'de> Deserialize<'de> for TemplateTags {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(match TagsRepr::deserialize(d)? {
            TagsRepr::Map(m) => Self {
                style: m.style,
                color: m.color,
                use_: m.use_,
                dropped: Vec::new(),
            },
            TagsRepr::List(list) => Self::from_flat(&list),
        })
    }
}

/// Why tags are not acceptable.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct TagError(pub String);

impl TemplateTags {
    /// Sorts known values of a flat list into their facets; unknown words
    /// go to [`Self::dropped`].
    pub fn from_flat(list: &[String]) -> Self {
        let tax = taxonomy();
        let mut out = Self::default();
        for t in list {
            let t = t.trim().to_lowercase();
            match tax.facet_of(&t).map(|f| f.id.as_str()) {
                Some("style") => push_unique(&mut out.style, t),
                Some("color") => push_unique(&mut out.color, t),
                Some("use") => push_unique(&mut out.use_, t),
                _ => out.dropped.push(t),
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.style.is_empty() && self.color.is_empty() && self.use_.is_empty()
    }

    /// The values of a facet by id.
    pub fn values(&self, facet: &str) -> &[String] {
        match facet {
            "style" => &self.style,
            "color" => &self.color,
            "use" => &self.use_,
            _ => &[],
        }
    }

    /// All values (style, colour, use), for search and lists.
    pub fn all(&self) -> Vec<String> {
        FACETS
            .iter()
            .flat_map(|f| self.values(f).iter().cloned())
            .collect()
    }

    /// Lowercases, trims and removes duplicates.
    #[must_use]
    pub fn normalized(&self) -> Self {
        let n = |v: &[String]| {
            let mut out = Vec::new();
            for t in v {
                push_unique(&mut out, t.trim().to_lowercase());
            }
            out
        };
        Self {
            style: n(&self.style),
            color: n(&self.color),
            use_: n(&self.use_),
            dropped: Vec::new(),
        }
    }

    /// Every value is known in its facet and no facet has more than its
    /// maximum. With `complete`, every facet also needs its minimum (the
    /// gallery requires that; a local template may leave facets empty).
    pub fn check(&self, complete: bool) -> Result<(), TagError> {
        let tax = taxonomy();
        let mut problems = Vec::new();
        for id in FACETS {
            let Some(facet) = tax.facet(id) else { continue };
            let values = self.values(id);
            for v in values {
                if !facet.has(v) {
                    let hint = tax
                        .facet_of(v)
                        .map(|f| format!(" (`{v}` is a `{}` value)", f.id))
                        .unwrap_or_default();
                    problems.push(format!(
                        "unknown {id} tag `{v}`{hint}; allowed: {}",
                        facet.allowed()
                    ));
                }
            }
            let n = u32::try_from(values.len()).unwrap_or(u32::MAX);
            if facet.max > 0 && n > facet.max {
                problems.push(format!(
                    "at most {} {id} tag{} (got {n})",
                    facet.max,
                    if facet.max == 1 { "" } else { "s" }
                ));
            }
            if complete && n < facet.min {
                problems.push(format!(
                    "at least {} {id} tag{} needed; allowed: {}",
                    facet.min,
                    if facet.min == 1 { "" } else { "s" },
                    facet.allowed()
                ));
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(TagError(problems.join("; ")))
        }
    }
}

fn push_unique(v: &mut Vec<String>, t: String) {
    if !t.is_empty() && !v.contains(&t) {
        v.push(t);
    }
}

/// The derived `devices` values of a project (all canvases), in taxonomy
/// order.
pub fn devices(project: &Project) -> Vec<String> {
    let ids: Vec<&str> = project.canvases.iter().map(|c| c.id.as_str()).collect();
    devices_of(project, &ids)
}

/// The derived `devices` values of some canvases of a project: per target
/// showing the canvas, banners give `banner`; page targets named like a
/// preset give the preset's value (preset id, then platform); other page
/// targets give the classes of the device elements on the canvas.
pub fn devices_of(project: &Project, canvases: &[&str]) -> Vec<String> {
    let Some(facet) = taxonomy().facet(DEVICES) else {
        return Vec::new();
    };
    let mut found: BTreeSet<String> = BTreeSet::new();
    for canvas in project
        .canvases
        .iter()
        .filter(|c| canvases.contains(&c.id.as_str()))
    {
        let mut classes = Vec::new();
        canvas.walk_elements(&mut |e| {
            if let ElementKind::Device(d) = &e.kind
                && let Some(dev) = crate::catalog::device(&d.model)
            {
                classes.push(dev.class);
            }
        });
        let by_classes = |found: &mut BTreeSet<String>| {
            for v in &facet.values {
                if classes.iter().any(|c| v.classes.contains(c)) {
                    found.insert(v.id.clone());
                }
            }
        };
        for target in project.targets.iter().filter(|t| t.canvas == canvas.id) {
            if let Some(v) = facet.values.iter().find(|v| v.kinds.contains(&target.kind)) {
                found.insert(v.id.clone());
                continue;
            }
            let preset = crate::catalog::target_preset(target.id.as_str());
            let hit = preset.and_then(|p| {
                facet
                    .values
                    .iter()
                    .find(|v| v.presets.contains(&p.id))
                    .or_else(|| {
                        facet
                            .values
                            .iter()
                            .find(|v| v.platforms.contains(&p.platform))
                    })
            });
            match hit {
                Some(v) => {
                    found.insert(v.id.clone());
                }
                None => by_classes(&mut found),
            }
        }
    }
    facet
        .values
        .iter()
        .filter(|v| found.contains(&v.id))
        .map(|v| v.id.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taxonomy_loads_with_the_agreed_slugs() {
        let t = taxonomy();
        let ids: Vec<&str> = t.facets.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(ids, ["style", "color", "use", "devices"]);
        assert_eq!(t.facet("style").unwrap().values.len(), 10);
        assert_eq!(t.facet("color").unwrap().values.len(), 8);
        assert_eq!(t.facet("use").unwrap().values.len(), 25);
        assert_eq!(t.facet("devices").unwrap().values.len(), 7);
        // Slugs are unique across the author-set facets.
        let mut all = BTreeSet::new();
        for f in t.facets.iter().filter(|f| !f.derived) {
            for v in &f.values {
                assert!(all.insert(v.id.clone()), "duplicate {}", v.id);
                assert!(super::super::is_template_id(&v.id), "{}", v.id);
                assert!(!v.label.en.is_empty() && !v.label.de.is_empty());
            }
        }
    }

    #[test]
    fn check_reports_unknown_and_counts() {
        let tags = TemplateTags {
            style: vec!["minimal".into()],
            color: vec!["dark".into()],
            use_: vec!["travel".into()],
            dropped: vec![],
        };
        tags.check(true).unwrap();
        assert!(TemplateTags::default().check(false).is_ok());
        let err = TemplateTags::default().check(true).unwrap_err().0;
        assert!(err.contains("at least 1 style tag"), "{err}");
        let bad = TemplateTags {
            style: vec!["dark".into(), "fancy".into(), "bold".into()],
            ..tags
        };
        let err = bad.check(false).unwrap_err().0;
        assert!(err.contains("`dark` is a `color` value"), "{err}");
        assert!(err.contains("unknown style tag `fancy`"), "{err}");
        assert!(err.contains("allowed: minimal, bold"), "{err}");
        assert!(err.contains("at most 2 style tags"), "{err}");
    }

    #[test]
    fn reads_the_map_and_the_old_list() {
        let t: TemplateTags =
            crate::io::from_yaml("style: [bold]\ncolor: [neon]\nuse: [games, kids]\n").unwrap();
        assert_eq!(t.use_, ["games", "kids"]);
        let t: TemplateTags = crate::io::from_yaml("[phone, gradient, Travel]").unwrap();
        assert_eq!(t.color, ["gradient"]);
        assert_eq!(t.use_, ["travel"]);
        assert_eq!(t.dropped, ["phone"]);
        let yaml = crate::io::to_yaml(&t).unwrap();
        assert!(yaml.contains("use:") && !yaml.contains("phone"), "{yaml}");
    }
}
