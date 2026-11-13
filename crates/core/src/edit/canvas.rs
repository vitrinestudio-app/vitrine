//! Copying a whole canvas: KONZEPT §9 "Leinwand duplizieren".
//!
//! Element ids are unique across the project, so the copy gets new ids
//! (`title-1` → `title-1-2` …, see [`super::unique_id`]); variant overrides
//! follow the new ids. Translation keys are shared with the original
//! unless the caller asks for copies ([`CanvasCopy::copy_strings`]).

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{Canvas, CanvasId, ElementId, ElementKind, ElementPatch, Project, StringKey};
use crate::ops::Op;

use super::{rename_tree, unique_id_except};

/// Result of [`copy_canvas`].
#[derive(Debug, Clone)]
pub struct CanvasCopy {
    /// The new canvas (not yet in the project; add it with `Op::AddCanvas`).
    pub canvas: Canvas,
    /// `SetString` ops creating the copied translations (empty unless
    /// `copy_strings`).
    pub string_ops: Vec<Op>,
    /// Old → new element ids.
    pub renamed: Vec<(ElementId, ElementId)>,
}

/// An id based on `base` that no canvas of the project uses (`base`,
/// `base-2`, …).
pub fn unique_canvas_id(project: &Project, base: &str) -> CanvasId {
    let base = if crate::model::is_valid_id(base) {
        base
    } else {
        "canvas"
    };
    if project.canvas(base).is_none() {
        return CanvasId::new(base);
    }
    let stem = match base.rsplit_once('-') {
        Some((s, n)) if !s.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
            s
        }
        _ => base,
    };
    // At most one candidate per existing canvas is taken.
    (2..=project.canvases.len() + 2)
        .map(|n| format!("{stem}-{n}"))
        .find(|id| project.canvas(id).is_none())
        .map_or_else(|| CanvasId::new(base), |id| CanvasId::new(&id))
}

/// A translation key based on `base` that no language uses yet.
pub fn unique_string_key(project: &Project, base: &str, taken: &BTreeSet<String>) -> StringKey {
    let used = |k: &str| {
        taken.contains(k)
            || project
                .strings
                .values()
                .any(|m| m.keys().any(|x| x.as_str() == k))
    };
    if !used(base) {
        return StringKey::new(base);
    }
    // At most one candidate per existing or taken key is in use.
    let keys: usize = project.strings.values().map(BTreeMap::len).sum();
    let key = (2..=keys + taken.len() + 2)
        .map(|n| format!("{base}-{n}"))
        .find(|k| !used(k))
        .unwrap_or_else(|| base.to_owned());
    StringKey::new(&key)
}

/// A copy of `src` as `new_id`: page spec, backgrounds, elements with new
/// unique ids and variants (overrides re-keyed). With `copy_strings`, every
/// translation key used by the copied texts (base, variant and language
/// overrides) is copied to a new key `<new_id>.<key>` in all languages.
pub fn copy_canvas(
    project: &Project,
    src: &Canvas,
    new_id: CanvasId,
    copy_strings: bool,
) -> CanvasCopy {
    let mut canvas = src.clone();
    canvas.id = new_id;
    let mut taken = BTreeSet::new();
    let mut renamed = Vec::new();
    for e in &mut canvas.elements {
        rename_tree(project, e, &mut taken, &mut renamed);
    }
    let map: BTreeMap<ElementId, ElementId> = renamed.iter().cloned().collect();
    for e in &mut canvas.elements {
        e.remap_element_refs(&map);
    }
    for v in canvas.variants.values_mut() {
        v.overrides = std::mem::take(&mut v.overrides)
            .into_iter()
            .map(|(id, mut p)| {
                p.remap_element_refs(&map);
                (map.get(&id).cloned().unwrap_or(id), p)
            })
            .collect();
    }
    let mut string_ops = Vec::new();
    if copy_strings {
        let keys = copy_keys(project, &mut canvas);
        for (old, new) in &keys {
            for (lang, strings) in &project.strings {
                if let Some(value) = strings.get(old.as_str()) {
                    string_ops.push(Op::SetString {
                        lang: lang.clone(),
                        key: new.clone(),
                        value: Some(value.clone()),
                    });
                }
            }
        }
    }
    CanvasCopy {
        canvas,
        string_ops,
        renamed,
    }
}

/// Visits the elements of `list` and their children (pre-order).
fn walk(list: &mut [crate::model::Element], f: &mut dyn FnMut(&mut crate::model::Element)) {
    for e in list {
        f(e);
        if let Some(c) = e.children_mut() {
            walk(c, f);
        }
    }
}

/// Replaces the translation keys of the canvas's texts (and of `text` in
/// their variant / language overrides) by new keys; returns old → new.
fn copy_keys(project: &Project, canvas: &mut Canvas) -> BTreeMap<StringKey, StringKey> {
    let mut keys: BTreeMap<StringKey, StringKey> = BTreeMap::new();
    let mut taken = BTreeSet::new();
    let prefix = canvas.id.to_string();
    let mut map = |k: &StringKey| -> StringKey {
        keys.entry(k.clone())
            .or_insert_with(|| {
                let new = unique_string_key(project, &format!("{prefix}.{k}"), &taken);
                taken.insert(new.to_string());
                new
            })
            .clone()
    };
    let patch = |p: &mut ElementPatch, map: &mut dyn FnMut(&StringKey) -> StringKey| {
        if let Some(Some(k)) = &mut p.text {
            *k = map(k);
        }
    };
    walk(&mut canvas.elements, &mut |e| {
        if let ElementKind::Text(t) = &mut e.kind {
            t.text = map(&t.text);
            for p in t.langs.values_mut() {
                patch(p, &mut map);
            }
        }
    });
    for v in canvas.variants.values_mut() {
        for p in v.overrides.values_mut() {
            patch(p, &mut map);
        }
    }
    keys
}

/// A fresh element id based on `base`, not used by the project nor in
/// `taken` (which it is added to).
pub fn fresh_element_id(
    project: &Project,
    base: &str,
    taken: &mut BTreeSet<ElementId>,
) -> ElementId {
    let id = unique_id_except(project, base, taken);
    taken.insert(id.clone());
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example() -> Project {
        let dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example");
        Project::load(&dir).unwrap()
    }

    #[test]
    fn copy_gets_new_ids_and_rekeyed_overrides() {
        let mut p = example();
        let src = p.canvases[0].clone();
        let id = unique_canvas_id(&p, src.id.as_str());
        assert_ne!(id, src.id);
        let copy = copy_canvas(&p, &src, id.clone(), false);
        assert_eq!(copy.string_ops, [] as [Op; 0]);
        let mut ids = Vec::new();
        copy.canvas.walk_elements(&mut |e| ids.push(e.id.clone()));
        assert!(ids.iter().all(|i| p.element(i.as_str()).is_none()));
        for v in copy.canvas.variants.values() {
            for eid in v.overrides.keys() {
                assert!(ids.contains(eid), "override for unknown {eid}");
            }
        }
        p.apply(Op::AddCanvas {
            canvas: copy.canvas,
            index: None,
        })
        .unwrap();
        assert!(p.canvas(id.as_str()).is_some());
    }

    #[test]
    fn copy_strings_creates_prefixed_keys() {
        let mut p = example();
        let src = p.canvases[0].clone();
        let copy = copy_canvas(&p, &src, CanvasId::new("copy"), true);
        assert_ne!(copy.string_ops, [] as [Op; 0]);
        let mut keys = Vec::new();
        copy.canvas.walk_elements(&mut |e| {
            if let ElementKind::Text(t) = &e.kind {
                keys.push(t.text.to_string());
            }
        });
        assert!(keys.iter().all(|k| k.starts_with("copy.")), "{keys:?}");
        for op in copy.string_ops {
            p.apply(op).unwrap();
        }
        let lang = p.languages.default.to_string();
        assert!(keys.iter().any(|k| p.string(&lang, k).is_some()));
    }
}
