//! `clip_to` on image elements (ADR 0031): YAML form, resolve, validation,
//! and keeping references consistent through ops and edit intents.

#![allow(clippy::float_cmp, clippy::many_single_char_names)]

use std::collections::BTreeSet;

use vitrine_core::Project;
use vitrine_core::edit::{EditContext, Editor, copy_canvas};
use vitrine_core::geometry::Vec2;
use vitrine_core::io::{self, project_from_yaml};
use vitrine_core::model::{CanvasId, Element, ElementId, ElementPatch, ShapeKind, VariantId};
use vitrine_core::ops::{Op, OpError};
use vitrine_core::resolve::{
    DiagnosticCode, NoAssets, ResolveOptions, ResolvedKind, ResolvedScene, resolve,
};
use vitrine_core::validate::{FindingCode, ValidateOptions, validate};

const PROJECT: &str = r##"
format: vitrine/1
name: clip
languages: {default: en, available: [en]}
canvases:
- id: c
  page: {aspect: 1:1, count: 2}
  elements:
  - {id: mask, kind: shape, page: 0, at: {x: 300, y: 300}, width: 400, height: 200, shape: rect, radius: 40, fill: "#FF0000"}
  - {id: hidden-mask, kind: shape, page: 0, at: {x: 700, y: 700}, width: 200, height: 200, shape: ellipse, visible: false}
  - {id: line, kind: shape, page: 0, at: {x: 500, y: 900}, width: 400, shape: line, stroke: {color: "#000000", width: 4}}
  - id: g
    kind: group
    page: 0
    rotation: 30
    children:
    - {id: gmask, kind: shape, page: 0, at: {x: 500, y: 500}, width: 100, height: 100, shape: rect, visible: false}
    - {id: gphoto, kind: image, page: 0, at: {x: 500, y: 500}, width: 300, height: 300, asset: assets/a.png, clip_to: gmask}
  - {id: photo, kind: image, page: 0, at: {x: 300, y: 300}, width: 600, height: 600, asset: assets/a.png, clip_to: mask}
  - {id: photo2, kind: image, page: 0, at: {x: 700, y: 700}, width: 300, height: 300, asset: assets/a.png, clip_to: hidden-mask}
  - {id: bad, kind: image, page: 0, at: {x: 500, y: 500}, width: 100, height: 100, asset: assets/a.png, clip_to: line}
  - {id: bad2, kind: image, page: 0, at: {x: 500, y: 500}, width: 100, height: 100, asset: assets/a.png, clip_to: nope}
  - {id: other, kind: image, page: 1, at: {x: 300, y: 300}, width: 100, height: 100, asset: assets/a.png, clip_to: mask, allow: [clip_to_other_page]}
  - {id: other2, kind: image, page: 1, at: {x: 300, y: 300}, width: 100, height: 100, asset: assets/a.png, clip_to: mask}
  variants:
    v:
      overrides:
        mask: {at: {x: 400, y: 350}}
        photo2: {clip_to: mask}
targets:
- {id: t, canvas: c, size: [1000, 1000]}
- {id: tv, canvas: c, size: [1000, 1000], variant: v}
"##;

fn project() -> Project {
    project_from_yaml(PROJECT).unwrap()
}

fn scene(p: &Project, target: &str) -> ResolvedScene {
    resolve(p, target, "en", &ResolveOptions::new(&NoAssets)).unwrap()
}

fn image<'s>(s: &'s ResolvedScene, id: &str) -> &'s vitrine_core::resolve::ResolvedImage {
    let e = s.elements.iter().find(|e| e.id.as_str() == id).unwrap();
    match &e.props {
        ResolvedKind::Image(i) => i,
        _ => panic!("{id} is not an image"),
    }
}

fn element<'p>(p: &'p Project, id: &str) -> &'p Element {
    p.element(id).unwrap().1
}

fn clip_of(p: &Project, id: &str) -> Option<String> {
    element(p, id).clip_to().map(ToString::to_string)
}

fn override_clip(p: &Project, element: &str) -> Option<String> {
    p.canvas("c").unwrap().variants[&VariantId::new("v")]
        .overrides
        .get(element)
        .and_then(|o| o.clip_to.clone().flatten())
        .map(|c| c.to_string())
}

/// Apply as one transaction; undo must restore the project exactly.
fn apply(p: &mut Project, ops: Vec<Op>) {
    let before = p.clone();
    let inverse = p.apply_ops(ops).unwrap();
    let mut undone = p.clone();
    undone.apply_ops(inverse).unwrap();
    assert_eq!(undone, before, "undo must restore the project");
}

#[test]
fn yaml_form_keeps_clip_to_after_asset() {
    let e: Element = io::from_yaml(
        "{ id: p, kind: image, page: 0, clip_to: m, asset: a.png, width: 10, fit: cover }",
    )
    .unwrap();
    assert_eq!(e.clip_to().map(ElementId::as_str), Some("m"));
    let yaml = io::to_yaml(&e).unwrap();
    assert!(
        yaml.find("asset:").unwrap() < yaml.find("clip_to: m").unwrap()
            && yaml.find("clip_to").unwrap() < yaml.find("fit:").unwrap(),
        "{yaml}"
    );
    let err = io::from_yaml::<Element>(
        "{ id: s, kind: shape, page: 0, shape: rect, width: 1, height: 1, clip_to: m }",
    )
    .unwrap_err();
    assert!(err.contains("clip_to"), "{err}");
}

#[test]
fn images_get_the_outline_of_their_shape() {
    let p = project();
    let s = scene(&p, "t");
    let c = image(&s, "photo").clip_shape.as_ref().unwrap();
    assert_eq!(c.element.as_str(), "mask");
    assert_eq!((c.shape, c.radius), (ShapeKind::Rect, 40.0));
    assert_eq!(c.center, Vec2::new(300.0, 300.0));
    assert_eq!(c.size, Vec2::new(400.0, 200.0));
    // A hidden shape still masks, but is not drawn.
    let h = image(&s, "photo2").clip_shape.as_ref().unwrap();
    assert_eq!(h.shape, ShapeKind::Ellipse);
    assert!(s.elements.iter().all(|e| e.id.as_str() != "hidden-mask"));
    assert!(s.elements.iter().all(|e| e.id.as_str() != "gmask"));
    // Group transforms apply to the mask like to the image.
    let g = image(&s, "gphoto").clip_shape.as_ref().unwrap();
    assert_eq!(g.rotation, 30.0);
    let gphoto = s
        .elements
        .iter()
        .find(|e| e.id.as_str() == "gphoto")
        .unwrap();
    let d = g.center - gphoto.center;
    assert!(d.x.abs() < 1e-9 && d.y.abs() < 1e-9);
    // Lines and unknown ids: drawn unclipped, reported.
    assert!(image(&s, "bad").clip_shape.is_none());
    assert!(image(&s, "bad2").clip_shape.is_none());
    let invalid: Vec<_> = s
        .diagnostics
        .iter()
        .filter(|d| d.code == DiagnosticCode::InvalidClipTo)
        .filter_map(|d| d.element.as_ref().map(ElementId::as_str))
        .collect();
    assert_eq!(invalid, ["bad", "bad2"]);
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::ClipToOtherPage
                && d.element.as_ref().map(ElementId::as_str) == Some("other2"))
    );
}

#[test]
fn variant_overrides_move_the_mask_and_switch_it() {
    let p = project();
    let s = scene(&p, "tv");
    let c = image(&s, "photo").clip_shape.as_ref().unwrap();
    assert_eq!(c.center, Vec2::new(400.0, 350.0));
    let o = image(&s, "photo2").clip_shape.as_ref().unwrap();
    assert_eq!(o.element.as_str(), "mask");
}

#[test]
fn validation_reports_bad_references() {
    let p = project();
    let r = validate(&p, &ValidateOptions::new(&NoAssets));
    let of = |code| {
        r.with_code(code)
            .filter(|f| f.target.as_ref().map(vitrine_core::model::TargetId::as_str) == Some("t"))
            .filter_map(|f| f.element.as_ref().map(ToString::to_string))
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(
        of(FindingCode::InvalidClipTo),
        BTreeSet::from(["bad".to_owned(), "bad2".to_owned()])
    );
    // `allow: [clip_to_other_page]` silences it for `other`.
    assert_eq!(
        of(FindingCode::ClipToOtherPage),
        BTreeSet::from(["other2".to_owned()])
    );
}

#[test]
fn rename_follows_and_undo_restores() {
    let mut p = project();
    apply(
        &mut p,
        vec![Op::RenameElement {
            from: ElementId::new("mask"),
            to: ElementId::new("frame"),
        }],
    );
    assert_eq!(clip_of(&p, "photo").as_deref(), Some("frame"));
    assert_eq!(clip_of(&p, "other").as_deref(), Some("frame"));
    assert_eq!(override_clip(&p, "photo2").as_deref(), Some("frame"));
    assert!(
        scene(&p, "t")
            .diagnostics
            .iter()
            .all(|d| d.code != DiagnosticCode::InvalidClipTo
                || d.element.as_ref().map(ElementId::as_str) != Some("photo"))
    );
    // `bad2` points to the missing `nope`: renaming onto it is refused
    // (undo could not tell the references apart).
    let err = p
        .apply(Op::RenameElement {
            from: ElementId::new("frame"),
            to: ElementId::new("nope"),
        })
        .unwrap_err();
    assert!(matches!(err, OpError::Invalid(_)), "{err}");
}

#[test]
fn removing_a_referenced_shape_is_refused_but_the_editor_releases_it() {
    let mut p = project();
    let err = p
        .apply(Op::RemoveElement {
            id: ElementId::new("mask"),
        })
        .unwrap_err();
    assert!(matches!(err, OpError::StillReferenced { .. }), "{err}");
    // A group with both the image and its mask: nothing outside refers to it.
    apply(
        &mut p.clone(),
        vec![Op::RemoveElement {
            id: ElementId::new("g"),
        }],
    );
    // The editor's delete drops the references in the same step.
    let ctx = EditContext::new("t", "en").unwrap();
    let files = BTreeSet::<String>::new();
    let ops = Editor::new(&p, &ctx, ResolveOptions::new(&files))
        .unwrap()
        .remove("mask")
        .unwrap();
    assert_eq!(ops.len(), 5, "{ops:?}");
    apply(&mut p, ops);
    assert!(p.element("mask").is_none());
    assert_eq!(clip_of(&p, "photo"), None);
    assert_eq!(clip_of(&p, "other"), None);
    assert_eq!(override_clip(&p, "photo2"), None);
}

#[test]
fn copies_point_to_their_copied_shape() {
    let p = project();
    let ctx = EditContext::new("t", "en").unwrap();
    let files = BTreeSet::<String>::new();
    let (ops, new_id) = Editor::new(&p, &ctx, ResolveOptions::new(&files))
        .unwrap()
        .duplicate("g", Vec2::ZERO)
        .unwrap();
    let mut q = p.clone();
    apply(&mut q, ops);
    let copy = element(&q, new_id.as_str());
    let (shape, img) = (&copy.children()[0], &copy.children()[1]);
    assert_eq!(img.clip_to(), Some(&shape.id));
    assert_ne!(shape.id.as_str(), "gmask");
    // A copied canvas: references follow the renamed elements.
    let copy = copy_canvas(&p, p.canvas("c").unwrap(), CanvasId::new("c2"), false);
    let photo = copy
        .renamed
        .iter()
        .find(|(old, _)| old.as_str() == "photo")
        .map(|(_, new)| new.clone())
        .unwrap();
    let mask = copy
        .renamed
        .iter()
        .find(|(old, _)| old.as_str() == "mask")
        .map(|(_, new)| new.clone())
        .unwrap();
    let e = copy.canvas.element(photo.as_str()).unwrap();
    assert_eq!(e.clip_to(), Some(&mask));
    let v = &copy.canvas.variants[&VariantId::new("v")];
    let photo2 = copy
        .renamed
        .iter()
        .find(|(old, _)| old.as_str() == "photo2")
        .map(|(_, new)| new.clone())
        .unwrap();
    assert_eq!(v.overrides[&photo2].clip_to, Some(Some(mask)));
}

#[test]
fn clearing_clip_to_through_a_patch() {
    let mut p = project();
    apply(
        &mut p,
        vec![Op::UpdateElement {
            id: ElementId::new("photo"),
            patch: ElementPatch {
                clip_to: Some(None),
                ..ElementPatch::default()
            },
        }],
    );
    assert_eq!(clip_of(&p, "photo"), None);
    assert!(image(&scene(&p, "t"), "photo").clip_shape.is_none());
}

const SCALED: &str = r##"
format: vitrine/1
name: scaled
languages: {default: en, available: [en]}
styles:
  spans:
    pill: {background: "#FF0000", padding: {x: 20, y: 10}, radius: 30}
canvases:
- id: c
  page: {aspect: 1:1, count: 1}
  elements:
  - id: g
    kind: group
    page: 0
    scale: 0.5
    children:
    - {id: mask, kind: shape, page: 0, at: {x: 300, y: 500}, width: 400, height: 200, shape: rect, radius: 40, visible: false}
    - {id: label, kind: text, page: 0, at: {x: 500, y: 300}, width: 900, text: t}
  - {id: photo, kind: image, page: 0, at: {x: 500, y: 500}, width: 600, height: 600, asset: assets/a.png, clip_to: mask}
targets:
- {id: t, canvas: c, size: [1000, 1000]}
"##;

/// Inside a scaled group (ADR 0030) the mask uses the scaled geometry and
/// span highlight padding/radius scale with the text.
#[test]
fn scaled_groups_scale_masks_and_span_boxes() {
    let mut p = project_from_yaml(SCALED).unwrap();
    p.strings
        .entry(vitrine_core::model::Lang::new("en"))
        .or_default()
        .insert(
            vitrine_core::model::StringKey::new("t"),
            "a [pill]NEW[/] b".into(),
        );
    let s = scene(&p, "t");
    let c = image(&s, "photo").clip_shape.as_ref().unwrap();
    assert_eq!(c.size, Vec2::new(200.0, 100.0));
    assert_eq!(c.radius, 20.0);

    let text = s
        .elements
        .iter()
        .find(|e| e.id.as_str() == "label")
        .unwrap();
    let ResolvedKind::Text(t) = &text.props else {
        panic!("label is not a text")
    };
    let spans = vitrine_core::layout::span_styles(&s.spans);
    let mut l =
        vitrine_core::layout::TextLayouter::new(vitrine_core::layout::FontLibrary::bundled());
    let placed = vitrine_core::layout::layout_text(&mut l, t, &spans).unwrap();
    let h = placed.layout.highlights[0];
    assert!((h.padding.0 - 10.0).abs() < 1e-4 && (h.padding.1 - 5.0).abs() < 1e-4);
    assert!((h.radius - 15.0).abs() < 1e-4);
}
