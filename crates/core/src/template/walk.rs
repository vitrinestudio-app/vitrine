//! What a canvas refers to: asset files, screenshot keys, translation keys,
//! font families.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::model::{Canvas, Element, ElementKind, Orientation, Project, StringKey, Styles};

/// Visits every string value stored under `key` in a JSON tree.
fn visit_strings(v: &mut Value, key: &str, f: &mut dyn FnMut(&mut String)) {
    match v {
        Value::Object(map) => {
            for (k, child) in map.iter_mut() {
                if k == key
                    && let Value::String(s) = child
                {
                    f(s);
                } else {
                    visit_strings(child, key, f);
                }
            }
        }
        Value::Array(list) => {
            for child in list {
                visit_strings(child, key, f);
            }
        }
        _ => {}
    }
}

fn to_json(canvas: &Canvas) -> Value {
    serde_json::to_value(canvas).unwrap_or(Value::Null)
}

/// Asset paths used by the canvas (image elements, image fills in
/// backgrounds and shapes, also inside variant and language overrides).
pub(crate) fn assets(canvas: &Canvas) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    visit_strings(&mut to_json(canvas), "asset", &mut |s| {
        out.insert(s.clone());
    });
    out
}

/// The canvas with asset paths replaced by `map` (paths not in the map
/// stay).
pub(crate) fn rewrite_assets(canvas: &Canvas, map: &BTreeMap<String, String>) -> Canvas {
    if map.is_empty() {
        return canvas.clone();
    }
    let mut v = to_json(canvas);
    visit_strings(&mut v, "asset", &mut |s| {
        if let Some(n) = map.get(s.as_str()) {
            s.clone_from(n);
        }
    });
    serde_json::from_value(v).unwrap_or_else(|_| canvas.clone())
}

/// A screenshot key and the device showing it (for the placeholder aspect).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ShotUse {
    pub key: String,
    pub model: Option<String>,
    pub orientation: Option<Orientation>,
}

/// Screenshot keys used by the canvas, first use per key.
pub(crate) fn screenshots(canvas: &Canvas) -> Vec<ShotUse> {
    let mut out: Vec<ShotUse> = Vec::new();
    canvas.walk_elements(&mut |e| {
        if let ElementKind::Device(d) = &e.kind
            && let Some(key) = d.screenshot.as_ref().filter(|k| !k.is_empty())
            && !out.iter().any(|u| &u.key == key)
        {
            out.push(ShotUse {
                key: key.clone(),
                model: Some(d.model.clone()),
                orientation: d.orientation,
            });
        }
    });
    // Keys only named in variant overrides.
    let mut extra = Vec::new();
    for v in canvas.variants.values() {
        for p in v.overrides.values() {
            if let Some(Some(k)) = &p.screenshot {
                extra.push(k.clone());
            }
        }
    }
    for k in extra {
        if !out.iter().any(|u| u.key == k) {
            out.push(ShotUse {
                key: k,
                model: None,
                orientation: None,
            });
        }
    }
    out
}

/// Visits every element (also group children), mutably.
pub(crate) fn elements_mut(list: &mut [Element], f: &mut dyn FnMut(&mut Element)) {
    for e in list {
        f(e);
        if let Some(c) = e.children_mut() {
            elements_mut(c, f);
        }
    }
}

/// Translation keys used by the canvas (texts, their language overrides
/// and variant overrides).
pub(crate) fn string_keys(canvas: &Canvas) -> BTreeSet<StringKey> {
    let mut out = BTreeSet::new();
    canvas.walk_elements(&mut |e| {
        if let ElementKind::Text(t) = &e.kind {
            out.insert(t.text.clone());
            for p in t.langs.values() {
                if let Some(Some(k)) = &p.text {
                    out.insert(k.clone());
                }
            }
        }
    });
    for v in canvas.variants.values() {
        for p in v.overrides.values() {
            if let Some(Some(k)) = &p.text {
                out.insert(k.clone());
            }
        }
    }
    out
}

/// Font families named by the styles (text and span styles, font
/// settings and their fallbacks).
pub(crate) fn font_families(styles: &Styles) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    out.extend(styles.text.values().filter_map(|t| t.font.clone()));
    out.extend(styles.spans.values().filter_map(|s| s.font.clone()));
    for (family, f) in &styles.fonts {
        out.insert(family.clone());
        out.extend(f.fallback.iter().cloned());
    }
    out
}

/// Keeps only `canvases` (and the targets showing them) and only the
/// strings they use; drops store and import settings.
pub(crate) fn subset(project: &Project, canvases: &[String]) -> Project {
    let mut p = project.clone();
    p.canvases
        .retain(|c| canvases.iter().any(|id| id == c.id.as_str()));
    p.targets
        .retain(|t| canvases.iter().any(|id| id == t.canvas.as_str()));
    let keys: BTreeSet<StringKey> = p.canvases.iter().flat_map(string_keys).collect();
    for map in p.strings.values_mut() {
        map.retain(|k, _| keys.contains(k));
    }
    p.strings.retain(|_, m| !m.is_empty());
    p.store = crate::model::StoreSettings::default();
    p.import = crate::model::ImportSettings::default();
    p.dropped_override_nulls.clear();
    p
}
