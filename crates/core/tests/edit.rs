//! Edit intents (`core::edit`) on the sample project and on generated
//! projects: every intent's ops are applied, resolved again and undone.

#![allow(
    clippy::many_single_char_names,
    clippy::float_cmp,
    clippy::assert_is_empty,
    clippy::manual_is_multiple_of
)]

use std::collections::BTreeSet;
use std::path::Path;

use vitrine_core::Project;
use vitrine_core::edit::{
    EditContext, EditError, Editor, Handle, MoveOptions, ResizeSpec, ZOrder,
    default_language_impact, local_to_canvas, unique_id,
};
use vitrine_core::geometry::Vec2;
use vitrine_core::io;
use vitrine_core::layout::{FontLibrary, TextLayouter};
use vitrine_core::model::{ElementId, ElementKind, Lang, StripId, VariantId};
use vitrine_core::ops::Op;
use vitrine_core::resolve::{ResolveOptions, ResolvedElement, resolve};

fn sample() -> Project {
    Project::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sample")).unwrap()
}

fn files() -> BTreeSet<String> {
    ["screenshots/en/ios-phone/home.png"]
        .into_iter()
        .map(String::from)
        .collect()
}

fn ctx(target: &str) -> EditContext {
    EditContext::new(target, "en").unwrap()
}

/// Apply `ops` as one transaction, check that the inverse restores the
/// project exactly, keep the result.
fn apply(p: &mut Project, ops: Vec<Op>) {
    let before = p.clone();
    let inverse = p.apply_ops(ops).unwrap();
    let mut undone = p.clone();
    undone.apply_ops(inverse).unwrap();
    assert_eq!(undone, before, "undo must restore the project");
}

fn point(p: &Project, c: &EditContext, id: &str) -> Vec2 {
    let files = files();
    Editor::new(p, c, ResolveOptions::new(&files))
        .unwrap()
        .reference_point(id)
        .unwrap()
}

fn resolved(p: &Project, target: &str, id: &str) -> ResolvedElement {
    let files = files();
    resolve(p, target, "en", &ResolveOptions::new(&files))
        .unwrap()
        .element(id)
        .unwrap()
        .clone()
}

fn close(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

fn with_editor<T>(p: &Project, c: &EditContext, f: impl FnOnce(&Editor<'_>) -> T) -> T {
    let files = files();
    let ed = Editor::new(p, c, ResolveOptions::new(&files)).unwrap();
    f(&ed)
}

#[test]
fn move_device_changes_home_page() {
    let mut p = sample();
    let c = ctx("ios-phone-69");
    let goal = Vec2::new(2500.0, 900.0);
    // `lock_page: true` in the sample keeps the home page.
    let ops = with_editor(&p, &c, |ed| {
        ed.move_to("hero-phone", goal, MoveOptions::default())
            .unwrap()
    });
    apply(&mut p.clone(), ops.clone());
    let mut locked = p.clone();
    locked.apply_ops(ops).unwrap();
    assert_eq!(locked.element("hero-phone").unwrap().1.page, 0);
    assert!(close(point(&locked, &c, "hero-phone"), goal));
    apply(
        &mut p,
        vec![Op::UpdateElement {
            id: ElementId::new("hero-phone"),
            patch: io::from_yaml("{lock_page: ~}").unwrap(),
        }],
    );
    let ops = with_editor(&p, &c, |ed| {
        ed.move_to("hero-phone", goal, MoveOptions::default())
            .unwrap()
    });
    assert!(matches!(&ops[..], [Op::UpdateElement { .. }]));
    apply(&mut p, ops);
    assert!(close(point(&p, &c, "hero-phone"), goal));
    // 2500 lu with 45 lu gaps lies on page 2 (index 2 starts at 2090).
    assert_eq!(p.element("hero-phone").unwrap().1.page, 2);

    // Locked home page.
    let ops = with_editor(&p, &c, |ed| {
        ed.move_to(
            "hero-phone",
            Vec2::new(100.0, 100.0),
            MoveOptions { keep_page: true },
        )
        .unwrap()
    });
    apply(&mut p, ops);
    assert_eq!(p.element("hero-phone").unwrap().1.page, 2);
    assert!(close(point(&p, &c, "hero-phone"), Vec2::new(100.0, 100.0)));
}

#[test]
fn move_in_variant_writes_override() {
    let mut p = sample();
    p.apply(Op::UpdateElement {
        id: ElementId::new("hero-phone"),
        patch: io::from_yaml("{lock_page: ~}").unwrap(),
    })
    .unwrap();
    let c = ctx("ios-ipad-13-landscape");
    let base_before = p.element("hero-phone").unwrap().1.clone();
    let iphone_before = point(&p, &ctx("ios-phone-69"), "hero-phone");
    let goal = Vec2::new(1500.0, 700.0);
    let ops = with_editor(&p, &c, |ed| {
        assert_eq!(ed.variant().unwrap().as_str(), "tablet-landscape");
        ed.move_to("hero-phone", goal, MoveOptions::default())
            .unwrap()
    });
    let [Op::SetVariantOverride { patch: Some(o), .. }] = &ops[..] else {
        panic!("{ops:?}")
    };
    // The existing override (rotation) is kept, `at` and `page` replaced.
    assert_eq!(o.fields(), vec!["page", "at", "rotation"]);
    apply(&mut p, ops);
    assert!(close(point(&p, &c, "hero-phone"), goal));
    assert_eq!(p.element("hero-phone").unwrap().1, &base_before);
    assert!(close(
        point(&p, &ctx("ios-phone-69"), "hero-phone"),
        iphone_before
    ));

    // The base context edits the element itself.
    let base = ctx("ios-ipad-13-landscape").base();
    let ops = with_editor(&p, &base, |ed| {
        assert!(ed.variant().is_none());
        ed.move_by("band", Vec2::new(10.0, 0.0), MoveOptions::default())
            .unwrap()
    });
    assert!(matches!(&ops[..], [Op::UpdateElement { .. }]));
}

#[test]
fn move_margin_text_shifts_the_box() {
    let mut p = sample();
    let c = ctx("android-phone");
    let before = point(&p, &c, "title-1");
    let delta = Vec2::new(1040.0, 30.0); // one page + gap to the right
    let ops = with_editor(&p, &c, |ed| {
        ed.move_by("title-1", delta, MoveOptions::default())
            .unwrap()
    });
    apply(&mut p, ops);
    let e = p.element("title-1").unwrap().1;
    assert_eq!(e.page, 1);
    let ElementKind::Text(t) = &e.kind else {
        panic!()
    };
    let b = t.r#box.unwrap();
    // The margins follow the part of the move beyond one page + gap.
    let files = files();
    let scene = resolve(&p, "android-phone", "en", &ResolveOptions::new(&files)).unwrap();
    let step = scene.frame.pages[1].rect.x - scene.frame.pages[0].rect.x;
    assert!(
        (b.left.unwrap() - (80.0 + 1040.0 - step)).abs() < 1e-6,
        "{b:?}"
    );
    assert!(
        (b.right.unwrap() - (80.0 - 1040.0 + step)).abs() < 1e-6,
        "{b:?}"
    );
    assert!((b.top.unwrap() - 170.0).abs() < 1e-6, "{b:?}");
    assert!(close(point(&p, &c, "title-1"), before + delta));
}

#[test]
fn move_group_and_group_child() {
    let mut p = sample();
    let c = ctx("ios-phone-69");
    let goal = Vec2::new(2000.0, 1000.0);
    let ops = with_editor(&p, &c, |ed| {
        ed.move_to("badge-group", goal, MoveOptions::default())
            .unwrap()
    });
    apply(&mut p, ops);
    assert!(close(point(&p, &c, "badge-group"), goal));
    assert_eq!(
        p.element("badge-group").unwrap().1.page,
        2,
        "groups keep their page"
    );

    // Rotate the group, then move a child: the group pivot follows the
    // children, the solver still hits the point.
    let ops = with_editor(&p, &c, |ed| ed.rotate_to("badge-group", 30.0).unwrap());
    apply(&mut p, ops);
    let goal = Vec2::new(2300.0, 800.0);
    let ops = with_editor(&p, &c, |ed| {
        ed.move_to("badge", goal, MoveOptions::default()).unwrap()
    });
    apply(&mut p, ops);
    assert!(close(point(&p, &c, "badge"), goal));
    assert!((resolved(&p, "ios-phone-69", "badge").rotation - 30.0).abs() < 1e-9);
}

#[test]
fn page_background_moves_to_another_page() {
    let mut p = sample();
    let c = ctx("ios-phone-69");
    let ops = with_editor(&p, &c, |ed| {
        ed.move_to("page4-bg", Vec2::new(1100.0, 10.0), MoveOptions::default())
            .unwrap()
    });
    apply(&mut p, ops);
    assert_eq!(p.element("page4-bg").unwrap().1.page, 1);
    with_editor(&p, &c, |ed| {
        assert!(matches!(
            ed.resize("page4-bg", ResizeSpec::default(), Handle::Nw),
            Err(EditError::Unsupported(_))
        ));
    });
}

fn handle_point(r: &ResolvedElement, h: Handle) -> Vec2 {
    let u = h.unit();
    local_to_canvas(r, Vec2::new(u.x * r.size.x / 2.0, u.y * r.size.y / 2.0))
}

#[test]
fn resize_keeps_the_fixed_handle() {
    let mut p = sample();
    // Device in a content-scaled variant: width only, aspect locked.
    let target = "ios-ipad-13-landscape";
    let c = ctx(target);
    let before = resolved(&p, target, "hero-phone");
    let fixed = handle_point(&before, Handle::Nw);
    let ops = with_editor(&p, &c, |ed| {
        ed.resize(
            "hero-phone",
            ResizeSpec {
                width: Some(500.0),
                height: None,
            },
            Handle::Nw,
        )
        .unwrap()
    });
    apply(&mut p, ops);
    let after = resolved(&p, target, "hero-phone");
    assert!((after.size.x - 500.0).abs() < 1e-6);
    assert!((after.size.y / after.size.x - before.size.y / before.size.x).abs() < 1e-9);
    assert!(close(handle_point(&after, Handle::Nw), fixed));
    // The base width is untouched; the override holds width / content_scale.
    assert_eq!(p.element("hero-phone").unwrap().1.width, Some(820.0));
    let o = &p.strips[0].variants[&VariantId::new("tablet-landscape")].overrides["hero-phone"];
    assert!((o.width.unwrap().unwrap() - 500.0 / 0.75).abs() < 1e-6);

    // Rotated shape, both dimensions, fixed east edge.
    let target = "android-phone";
    let c = ctx(target);
    let before = resolved(&p, target, "band");
    let fixed = handle_point(&before, Handle::E);
    let ops = with_editor(&p, &c, |ed| {
        ed.resize(
            "band",
            ResizeSpec {
                width: Some(3000.0),
                height: Some(200.0),
            },
            Handle::E,
        )
        .unwrap()
    });
    apply(&mut p, ops);
    let after = resolved(&p, target, "band");
    assert!(close(after.size, Vec2::new(3000.0, 200.0)));
    assert!(close(handle_point(&after, Handle::E), fixed));

    // Margin text box: the margins move, the west edge stays.
    let before = resolved(&p, target, "sub-1");
    let fixed = handle_point(&before, Handle::Nw);
    let ops = with_editor(&p, &c, |ed| {
        ed.resize(
            "sub-1",
            ResizeSpec {
                width: Some(600.0),
                height: Some(300.0),
            },
            Handle::Nw,
        )
        .unwrap()
    });
    apply(&mut p, ops);
    let after = resolved(&p, target, "sub-1");
    assert!(close(after.size, Vec2::new(600.0, 300.0)));
    assert!(close(handle_point(&after, Handle::Nw), fixed));
    let ElementKind::Text(t) = &p.element("sub-1").unwrap().1.kind else {
        panic!()
    };
    let b = t.r#box.unwrap();
    assert_eq!(
        (b.left, b.right, b.top),
        (Some(80.0), Some(320.0), Some(420.0))
    );
    assert!(b.bottom.is_some());
}

#[test]
fn rotate_and_pivot() {
    let mut p = sample();
    let c = ctx("ios-phone-69");
    let ops = with_editor(&p, &c, |ed| ed.rotate_to("hero-phone", 370.0).unwrap());
    apply(&mut p, ops);
    assert_eq!(p.element("hero-phone").unwrap().1.rotation, 10.0);
    // Rotating to the current angle is a no-op.
    assert!(with_editor(&p, &c, |ed| ed.rotate_to("hero-phone", 10.0).unwrap()).is_empty());

    // Pivot at the top-left corner, keeping the drawn position.
    let r = resolved(&p, "ios-phone-69", "hero-phone");
    let corner = handle_point(&r, Handle::Nw);
    let centre = point(&p, &c, "hero-phone");
    let ops = with_editor(&p, &c, |ed| {
        ed.set_pivot("hero-phone", Some(corner), true).unwrap()
    });
    apply(&mut p, ops);
    let r = resolved(&p, "ios-phone-69", "hero-phone");
    assert!(close(r.center + r.pivot, corner));
    assert!(close(point(&p, &c, "hero-phone"), centre));
    // Rotating now turns around the corner: it stays put.
    let ops = with_editor(&p, &c, |ed| ed.rotate_to("hero-phone", -20.0).unwrap());
    apply(&mut p, ops);
    let r = resolved(&p, "ios-phone-69", "hero-phone");
    assert!(close(handle_point(&r, Handle::Nw), corner));
    assert!((r.rotation + 20.0).abs() < 1e-9);
}

/// Auto-height texts rotate around the centre of their laid-out content
/// (ADR 0004), in resolve, edit intents and with an explicit pivot.
#[test]
fn auto_height_text_rotates_around_its_content() {
    use vitrine_core::layout::{layout_text, span_styles};
    use vitrine_core::resolve::ResolvedKind;
    let mut p = sample();
    let c = ctx("ios-phone-69");
    let content_centre = |p: &Project| {
        let files = files();
        let scene = resolve(p, "ios-phone-69", "en", &ResolveOptions::new(&files)).unwrap();
        let r = scene.element("title-1").unwrap().clone();
        let ResolvedKind::Text(t) = &r.props else {
            panic!()
        };
        let mut l = TextLayouter::new(FontLibrary::bundled());
        let placed = layout_text(&mut l, t, &span_styles(&scene.spans)).unwrap();
        (r, placed.content_rect().center())
    };
    let (r, centre) = content_centre(&p);
    assert!(close(r.center + r.pivot, centre), "{r:?}");
    // The content stays in place when rotated (the box top moves).
    let before = point(&p, &c, "title-1");
    let ops = with_editor(&p, &c, |ed| ed.rotate_to("title-1", 30.0).unwrap());
    apply(&mut p, ops);
    let (r, rotated) = content_centre(&p);
    assert!(close(local_to_canvas(&r, centre - r.center), centre));
    assert!(close(rotated, centre));
    assert!(!close(point(&p, &c, "title-1"), before));
    // Moving the rotated text keeps working (the solver sees the pivot).
    let goal = point(&p, &c, "title-1") + Vec2::new(25.0, 40.0);
    let ops = with_editor(&p, &c, |ed| {
        ed.move_to("title-1", goal, MoveOptions::default()).unwrap()
    });
    apply(&mut p, ops);
    assert!(close(point(&p, &c, "title-1"), goal));
    // An explicit pivot is an offset from the content centre.
    let (r, centre) = content_centre(&p);
    let target = centre + Vec2::new(100.0, 0.0);
    let ops = with_editor(&p, &c, |ed| {
        ed.set_pivot(
            "title-1",
            Some(local_to_canvas(&r, target - r.center)),
            false,
        )
        .unwrap()
    });
    apply(&mut p, ops);
    let pivot = p.element("title-1").unwrap().1.pivot.unwrap();
    assert!(
        (pivot.x - 100.0).abs() < 1e-6 && pivot.y.abs() < 1e-6,
        "{pivot:?}"
    );
}

#[test]
fn set_home_page_keeps_or_moves() {
    let mut p = sample();
    let c = ctx("ios-phone-69");
    let before = point(&p, &c, "hero-phone");
    let ops = with_editor(&p, &c, |ed| {
        ed.set_home_page("hero-phone", 1, true).unwrap()
    });
    apply(&mut p, ops);
    assert_eq!(p.element("hero-phone").unwrap().1.page, 1);
    assert!(close(point(&p, &c, "hero-phone"), before));
    let ops = with_editor(&p, &c, |ed| {
        ed.set_home_page("hero-phone", 2, false).unwrap()
    });
    apply(&mut p, ops);
    let after = point(&p, &c, "hero-phone");
    assert!(
        (after.x - before.x - 1045.0).abs() < 1e-6,
        "moves with the page"
    );
    with_editor(&p, &c, |ed| {
        assert!(ed.set_home_page("hero-phone", 9, true).is_err());
    });
}

#[test]
fn order_duplicate_remove() {
    let mut p = sample();
    let c = ctx("ios-phone-69");
    let ops = with_editor(&p, &c, |ed| {
        ed.reorder_z("hero-phone", ZOrder::Front).unwrap()
    });
    apply(&mut p, ops);
    assert_eq!(
        p.strips[0].elements.last().unwrap().id.as_str(),
        "hero-phone"
    );
    let ops = with_editor(&p, &c, |ed| {
        ed.reorder_z("hero-phone", ZOrder::Backward).unwrap()
    });
    apply(&mut p, ops);
    let ids: Vec<&str> = p.strips[0].elements.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(&ids[ids.len() - 2..], ["hero-phone", "page4-bg"]);
    assert!(
        with_editor(&p, &c, |ed| ed.reorder_z("band", ZOrder::Back).unwrap())
            .iter()
            .all(|o| matches!(o, Op::MoveElement { .. }))
    );

    // Duplicate with overrides and an offset.
    let before = point(&p, &c, "hero-phone");
    let (ops, id) = with_editor(&p, &c, |ed| {
        ed.duplicate("hero-phone", Vec2::new(40.0, 40.0)).unwrap()
    });
    assert_eq!(id.as_str(), "hero-phone-2");
    apply(&mut p, ops);
    assert!(close(
        point(&p, &c, "hero-phone-2"),
        before + Vec2::new(40.0, 40.0)
    ));
    let v = &p.strips[0].variants[&VariantId::new("tablet-landscape")];
    let o = &v.overrides["hero-phone-2"];
    assert_eq!(
        o.at,
        Some(Some(vitrine_core::model::Point::new(1040.0, 1540.0)))
    );
    assert_eq!(unique_id(&p, "hero-phone-2").as_str(), "hero-phone-3");
    assert_eq!(unique_id(&p, "fresh").as_str(), "fresh");

    // A group duplicates with fresh child ids.
    let (ops, gid) = with_editor(&p, &c, |ed| {
        ed.duplicate("badge-group", Vec2::ZERO).unwrap()
    });
    apply(&mut p, ops);
    let (_, g) = p.element(gid.as_str()).unwrap();
    let kids: Vec<&str> = g.children().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(kids, ["badge-2", "badge-ring-2"]);

    let ops = with_editor(&p, &c, |ed| ed.remove("hero-phone").unwrap());
    apply(&mut p, ops);
    assert!(p.element("hero-phone").is_none());
    assert!(
        !p.strips[0].variants[&VariantId::new("tablet-landscape")]
            .overrides
            .contains_key("hero-phone")
    );
}

#[test]
fn patch_writes_base_or_override() {
    use vitrine_core::model::{ElementPatch, Point};
    let mut p = sample();
    let c = ctx("ios-phone-69");
    let patch = ElementPatch {
        opacity: Some(Some(0.5)),
        at: Some(Some(Point::new(400.0, 1200.0))),
        ..ElementPatch::default()
    };
    let ops = with_editor(&p, &c, |ed| ed.patch("hero-phone", &patch).unwrap());
    assert!(matches!(ops.as_slice(), [Op::UpdateElement { .. }]));
    apply(&mut p, ops);
    let e = p.element("hero-phone").unwrap().1;
    assert_eq!((e.opacity, e.at), (0.5, Point::new(400.0, 1200.0)));
    // Unchanged values give no ops; invalid fields are refused.
    assert!(with_editor(&p, &c, |ed| ed.patch("hero-phone", &patch).unwrap()).is_empty());
    let bad = ElementPatch {
        font_size: Some(Some(40.0)),
        ..ElementPatch::default()
    };
    assert!(with_editor(&p, &c, |ed| ed.patch("hero-phone", &bad)).is_err());

    // In a variant view the patch merges into the override.
    let v = ctx("ios-phone-69").with_variant(VariantId::new("tablet-landscape"));
    let ops = with_editor(&p, &v, |ed| ed.patch("hero-phone", &bad.only(&[])).unwrap());
    assert!(ops.is_empty());
    let z = ElementPatch {
        z: Some(Some(3)),
        ..ElementPatch::default()
    };
    let ops = with_editor(&p, &v, |ed| ed.patch("hero-phone", &z).unwrap());
    apply(&mut p, ops);
    let o = &p.strips[0].variants[&VariantId::new("tablet-landscape")].overrides["hero-phone"];
    assert_eq!(o.z, Some(Some(3)));
    assert_eq!(o.rotation, Some(Some(-4.0)));
    assert_eq!(p.element("hero-phone").unwrap().1.z, 0);
}

#[test]
fn arrange_moves_between_groups_in_place() {
    let mut p = sample();
    let c = ctx("android-phone");
    // Rotate the group first so leaving it must compensate the angle.
    let ops = with_editor(&p, &c, |ed| ed.rotate_to("badge-group", 20.0).unwrap());
    apply(&mut p, ops);
    let before = point(&p, &c, "badge");
    let rot = resolved(&p, "android-phone", "badge").rotation;
    let ops = with_editor(&p, &c, |ed| ed.arrange("badge", None, None).unwrap());
    apply(&mut p, ops);
    let s = &p.strips[0];
    assert_eq!(s.elements.last().unwrap().id.as_str(), "badge");
    assert!(close(point(&p, &c, "badge"), before));
    assert!((resolved(&p, "android-phone", "badge").rotation - rot).abs() < 1e-9);
    // ... and back into the group, at the bottom.
    let ops = with_editor(&p, &c, |ed| {
        ed.arrange("badge", Some("badge-group"), Some(0)).unwrap()
    });
    apply(&mut p, ops);
    let (_, g) = p.element("badge-group").unwrap();
    assert_eq!(g.children()[0].id.as_str(), "badge");
    assert!(close(point(&p, &c, "badge"), before));
    // Reordering within a list is a plain move; the same place is a no-op.
    assert!(
        with_editor(&p, &c, |ed| ed
            .arrange("badge", Some("badge-group"), Some(0))
            .unwrap())
        .is_empty()
    );
    let ops = with_editor(&p, &c, |ed| ed.arrange("band", None, Some(0)).unwrap());
    assert!(matches!(ops.as_slice(), [Op::MoveElementTo { .. }]));
    // Not into a non-group.
    assert!(with_editor(&p, &c, |ed| ed.arrange("band", Some("hero-phone"), None)).is_err());
}

#[test]
fn group_and_ungroup_keep_positions() {
    let mut p = sample();
    let c = ctx("android-phone");
    let ids = [ElementId::new("band"), ElementId::new("hero-phone")];
    let before: Vec<Vec2> = ids.iter().map(|i| point(&p, &c, i.as_str())).collect();
    let (ops, gid) = with_editor(&p, &c, |ed| ed.group(&ids, None).unwrap());
    assert_eq!(gid.as_str(), "group");
    apply(&mut p, ops);
    let (_, g) = p.element("group").unwrap();
    // Painting order of the members is kept.
    let kids: Vec<&str> = g.children().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(kids, ["hero-phone", "band"]);
    assert_eq!(p.strips[0].elements[0].id.as_str(), "group");
    for (i, b) in ids.iter().zip(&before) {
        assert!(close(point(&p, &c, i.as_str()), *b));
    }

    // Move and rotate the group, then dissolve it: nothing moves.
    let ops = with_editor(&p, &c, |ed| {
        let mut ops = ed
            .move_by("group", Vec2::new(120.0, -60.0), MoveOptions::default())
            .unwrap();
        ops.extend(ed.rotate_to("group", 25.0).unwrap());
        ops
    });
    apply(&mut p, ops);
    let moved: Vec<(Vec2, f64)> = ids
        .iter()
        .map(|i| {
            let r = resolved(&p, "android-phone", i.as_str());
            (point(&p, &c, i.as_str()), r.rotation)
        })
        .collect();
    let ops = with_editor(&p, &c, |ed| ed.ungroup("group").unwrap());
    apply(&mut p, ops);
    assert!(p.element("group").is_none());
    for (i, (pt, rot)) in ids.iter().zip(&moved) {
        assert!(close(point(&p, &c, i.as_str()), *pt), "{i}");
        let r = resolved(&p, "android-phone", i.as_str());
        assert!((r.rotation - rot).abs() < 1e-9);
    }

    with_editor(&p, &c, |ed| {
        let err = ed
            .group(&[ElementId::new("band"), ElementId::new("badge")], None)
            .unwrap_err();
        assert!(err.to_string().contains("not in the same group"), "{err}");
    });
}

#[test]
fn reset_overrides() {
    let mut p = sample();
    let c = ctx("ios-ipad-13-landscape");
    let ops = with_editor(&p, &c, |ed| {
        assert_eq!(ed.overridden_fields("hero-phone"), ["at", "rotation"]);
        ed.reset_override("hero-phone", Some("rotation")).unwrap()
    });
    apply(&mut p, ops);
    let ops = with_editor(&p, &c, |ed| {
        assert_eq!(ed.overridden_fields("hero-phone"), ["at"]);
        assert!(
            ed.reset_override("hero-phone", Some("width"))
                .unwrap()
                .is_empty()
        );
        ed.reset_override("title-1", None).unwrap()
    });
    apply(&mut p, ops);
    let v = &p.strips[0].variants[&VariantId::new("tablet-landscape")];
    assert!(!v.overrides.contains_key("title-1"));
    // Free function, base context: nothing to reset.
    assert!(
        vitrine_core::edit::reset_override(
            &p,
            &StripId::new("phone"),
            &VariantId::new("tablet-landscape"),
            &ElementId::new("band"),
            None
        )
        .unwrap()
        .is_empty()
    );
    with_editor(&p, &ctx("ios-phone-69"), |ed| {
        assert!(ed.reset_override("hero-phone", None).unwrap().is_empty());
    });
}

#[test]
fn hidden_and_foreign_elements() {
    let p = sample();
    // sub-1 is hidden in the tablet variant but can still be moved there.
    let c = ctx("ios-ipad-13-landscape");
    let ops = with_editor(&p, &c, |ed| {
        ed.move_by("sub-1", Vec2::new(0.0, 50.0), MoveOptions::default())
            .unwrap()
    });
    assert_eq!(ops.len(), 1);
    with_editor(&p, &c, |ed| {
        assert!(matches!(
            ed.move_by("web-browser", Vec2::ZERO, MoveOptions::default()),
            Err(EditError::WrongStrip { .. })
        ));
        assert!(matches!(
            ed.rotate_to("nope", 1.0),
            Err(EditError::UnknownElement(_))
        ));
    });
    let bad = ctx("ios-phone-69").with_variant(VariantId::new("nope"));
    let files = files();
    assert!(matches!(
        Editor::new(&p, &bad, ResolveOptions::new(&files)),
        Err(EditError::UnknownVariant { .. })
    ));
}

#[test]
fn language_scope_writes_language_overrides() {
    let mut p = sample();
    let c = EditContext::new("ios-phone-69", "es").unwrap().lang_only();
    // Inspector edit: merged into the `es` override (created here).
    let ops = with_editor(&p, &c, |ed| {
        ed.patch("title-1", &io::from_yaml("{font_size: 50}").unwrap())
            .unwrap()
    });
    let [
        Op::SetLangOverride {
            lang,
            patch: Some(o),
            ..
        },
    ] = &ops[..]
    else {
        panic!("{ops:?}")
    };
    assert_eq!(lang.as_str(), "es");
    assert_eq!(o.fields(), ["font_size"]);
    apply(&mut p, ops);
    // Fields that are not per language are refused.
    let err = with_editor(&p, &c, |ed| {
        ed.patch("title-1", &io::from_yaml("{rotation: 5}").unwrap())
    });
    assert!(err.is_err(), "{err:?}");
    // A drag moves the box in this language only; the home page stays.
    let before_en = point(&p, &ctx("ios-phone-69"), "title-1");
    let goal = point(&p, &c, "title-1") + Vec2::new(1200.0, 40.0);
    let ops = with_editor(&p, &c, |ed| {
        ed.move_to("title-1", goal, MoveOptions::default()).unwrap()
    });
    let [Op::SetLangOverride { patch: Some(o), .. }] = &ops[..] else {
        panic!("{ops:?}")
    };
    assert_eq!(o.fields(), ["box", "font_size"], "merged, no `page`");
    apply(&mut p, ops);
    assert!(close(point(&p, &c, "title-1"), goal));
    assert!(close(point(&p, &ctx("ios-phone-69"), "title-1"), before_en));
    // Devices are not texts: edited as usual.
    let ops = with_editor(&p, &c, |ed| {
        ed.patch("hero-phone", &io::from_yaml("{width: 700}").unwrap())
            .unwrap()
    });
    assert!(matches!(&ops[..], [Op::UpdateElement { .. }]));
    // Overridden fields and reset per field / whole override.
    with_editor(&p, &c, |ed| {
        let (langs, fields) = ed.lang_overridden_fields("title-1");
        assert_eq!(langs, [Lang::new("es")]);
        assert_eq!(fields, ["box", "font_size"]);
    });
    let ops = with_editor(&p, &c, |ed| {
        ed.reset_lang_override("title-1", Some("box")).unwrap()
    });
    assert!(matches!(&ops[..], [Op::ResetLangOverrideField { .. }]));
    apply(&mut p, ops);
    let ops = with_editor(&p, &c, |ed| {
        ed.reset_lang_override("title-1", None).unwrap()
    });
    apply(&mut p, ops);
    let (_, e) = p.element("title-1").unwrap();
    assert!(!e.text_props().unwrap().langs.contains_key("es"));
    // `de-AT` sees the `de` override; a reset there targets `de`.
    let at = EditContext::new("ios-phone-69", "de-AT")
        .unwrap()
        .lang_only();
    let ops = with_editor(&p, &at, |ed| {
        assert_eq!(
            ed.lang_overridden_fields("title-1").1,
            ["style", "box", "font_size"]
        );
        ed.reset_lang_override("title-1", Some("style")).unwrap()
    });
    let [Op::ResetLangOverrideField { lang, .. }] = &ops[..] else {
        panic!("{ops:?}")
    };
    assert_eq!(lang.as_str(), "de");
}

#[test]
fn null_in_an_override_scope_resets_the_field() {
    let mut p = sample();
    let es = EditContext::new("ios-phone-69", "es").unwrap().lang_only();
    let ops = with_editor(&p, &es, |ed| {
        ed.patch(
            "title-1",
            &io::from_yaml("{font_size: 50, box: {top: 100, left: 40, right: 40}}").unwrap(),
        )
        .unwrap()
    });
    apply(&mut p, ops);
    // `null` removes the field from the override, never stores `null`.
    let ops = with_editor(&p, &es, |ed| {
        ed.patch("title-1", &io::from_yaml("{font_size: null}").unwrap())
            .unwrap()
    });
    let [Op::SetLangOverride { patch: Some(o), .. }] = &ops[..] else {
        panic!("{ops:?}")
    };
    assert_eq!(o.fields(), ["box"]);
    assert!(o.null_fields().is_empty());
    apply(&mut p, ops);
    // Resetting a field the override does not set changes nothing.
    let ops = with_editor(&p, &es, |ed| {
        ed.patch("title-1", &io::from_yaml("{font_size: null}").unwrap())
            .unwrap()
    });
    assert!(ops.is_empty(), "{ops:?}");
    // The last field: the override goes away.
    let ops = with_editor(&p, &es, |ed| {
        ed.patch("title-1", &io::from_yaml("{box: null}").unwrap())
            .unwrap()
    });
    assert!(
        matches!(&ops[..], [Op::SetLangOverride { patch: None, .. }]),
        "{ops:?}"
    );
    apply(&mut p, ops);
    let (_, e) = p.element("title-1").unwrap();
    assert!(!e.text_props().unwrap().langs.contains_key("es"));
    // Variant scope: the same for variant overrides; values and resets mix.
    let v = ctx("ios-ipad-13-landscape");
    let ops = with_editor(&p, &v, |ed| {
        ed.patch(
            "hero-phone",
            &io::from_yaml("{rotation: null, width: 600}").unwrap(),
        )
        .unwrap()
    });
    let [Op::SetVariantOverride { patch: Some(o), .. }] = &ops[..] else {
        panic!("{ops:?}")
    };
    assert_eq!(o.fields(), ["at", "width"]);
    apply(&mut p, ops);
    // Base scope: `null` still resets the base field to its default.
    let ops = with_editor(&p, &ctx("ios-phone-69"), |ed| {
        ed.patch("hero-phone", &io::from_yaml("{shadow: null}").unwrap())
            .unwrap()
    });
    let [Op::UpdateElement { patch, .. }] = &ops[..] else {
        panic!("{ops:?}")
    };
    assert_eq!(patch.null_fields(), ["shadow"]);
}

#[test]
fn edit_context_json() {
    let c: EditContext = serde_json::from_str(r#"{"target": "t", "lang": "en"}"#).unwrap();
    assert_eq!(c.variant, None);
    let c: EditContext =
        serde_json::from_str(r#"{"target": "t", "lang": "en", "variant": null}"#).unwrap();
    assert_eq!(c.variant, Some(None));
    let c: EditContext =
        serde_json::from_str(r#"{"target": "t", "lang": "en", "variant": "v"}"#).unwrap();
    assert_eq!(c.variant, Some(Some(VariantId::new("v"))));
    let c: EditContext =
        serde_json::from_str(r#"{"target": "t", "lang": "de", "scope": "lang"}"#).unwrap();
    assert_eq!(c.scope, vitrine_core::edit::EditScope::Lang);
}

#[test]
fn default_language_impact_lists_lost_sources() {
    let mut p = sample();
    // A key that exists only in English.
    p.apply_ops([
        Op::SetString {
            lang: Lang::new("en"),
            key: vitrine_core::model::StringKey::new("hero.extra"),
            value: Some("Extra".into()),
        },
        Op::UpdateElement {
            id: ElementId::new("sub-1"),
            patch: io::from_yaml("{ text: hero.extra }").unwrap(),
        },
    ])
    .unwrap();
    let catalog = vitrine_core::catalog::catalog();
    let files: BTreeSet<String> = [
        "screenshots/en/ios-phone/home.png",
        "screenshots/de/ios-phone/home.png",
        "screenshots/en/android-phone/home.png",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    let r = default_language_impact(&p, &Lang::new("de"), catalog, &files).unwrap();
    assert_eq!(r.from.as_str(), "en");
    let texts: Vec<(&str, &str, &str)> = r
        .texts
        .iter()
        .map(|t| (t.lang.as_str(), t.key.as_str(), t.from.as_str()))
        .collect();
    assert_eq!(
        texts,
        [
            ("de", "hero.extra", "en"),
            ("es", "hero.extra", "en"),
            ("fr", "hero.extra", "en")
        ]
    );
    // android-phone has only an English screenshot; iOS has a German one.
    let shots: Vec<(&str, Option<&str>)> = r
        .screenshots
        .iter()
        .map(|s| (s.lang.as_str(), s.variant.as_deref()))
        .collect();
    assert_eq!(
        shots,
        [
            ("de", Some("android-phone")),
            ("es", Some("android-phone")),
            ("fr", Some("android-phone"))
        ]
    );
    assert!(r.screenshots.iter().all(|s| s.key == "home"));
    let same = default_language_impact(&p, &Lang::new("en"), catalog, &files).unwrap();
    assert!(same.is_empty());
    assert!(default_language_impact(&p, &Lang::new("it"), catalog, &files).is_err());
}

/// Snapshot form: numbers rounded to 3 decimals.
fn rounded(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64()
                && !n.is_i64()
                && !n.is_u64()
            {
                let r = (f * 1000.0).round() / 1000.0;
                *v = if r == r.trunc() && r.abs() < 1e15 {
                    serde_json::json!(r as i64)
                } else {
                    serde_json::json!(r)
                };
            }
        }
        serde_json::Value::Array(a) => a.iter_mut().for_each(rounded),
        serde_json::Value::Object(o) => o.values_mut().for_each(rounded),
        _ => {}
    }
}

#[test]
fn overlay_snapshot_grouped_variant() {
    let p = sample();
    let files = files();
    let ed = Editor::new(
        &p,
        &ctx("ios-ipad-13-landscape"),
        ResolveOptions::new(&files),
    )
    .unwrap();
    let mut layouter = TextLayouter::new(FontLibrary::bundled());
    let overlay = ed.overlay(&mut layouter).unwrap();
    // Geometry checks independent of the snapshot.
    assert_eq!(overlay.pages.len(), 5);
    assert_eq!(overlay.images.len(), 3);
    assert_eq!(overlay.gaps.len(), 2);
    let phone = overlay
        .elements
        .iter()
        .find(|e| e.id.as_str() == "hero-phone")
        .unwrap();
    assert_eq!(phone.overridden, ["at", "rotation"]);
    assert!(phone.aspect_locked);
    assert_eq!(phone.handles.len(), 8);
    let title = overlay
        .elements
        .iter()
        .find(|e| e.id.as_str() == "title-1")
        .unwrap();
    let t = title.text.as_ref().unwrap();
    assert!(t.margins && t.auto_height && t.lines >= 1);
    assert!(overlay.elements.iter().all(|e| e.id.as_str() != "sub-1"));
    let g = &overlay.groups[0];
    assert_eq!(g.id.as_str(), "badge-group");
    assert_eq!(g.members.len(), 2);
    assert!(close(
        g.move_point,
        ed.reference_point("badge-group").unwrap()
    ));
    let mut v = serde_json::to_value(&overlay).unwrap();
    rounded(&mut v);
    insta::assert_yaml_snapshot!("overlay_ipad_landscape_en", v);
}

#[test]
fn overlay_of_the_example_project() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example");
    let p = Project::load(&dir).unwrap();
    let assets = vitrine_core::resolve::FsAssets::new(&dir);
    let mut layouter = TextLayouter::new(FontLibrary::bundled());
    for t in &p.targets {
        let ed = Editor::new(
            &p,
            &EditContext::new(t.id.as_str(), "de").unwrap(),
            ResolveOptions::new(&assets),
        )
        .unwrap();
        let o = ed.overlay(&mut layouter).unwrap();
        for e in &o.elements {
            // move_point is what move_to places: moving there is a no-op.
            if e.kind == vitrine_core::model::ElementKindTag::Background {
                continue;
            }
            let ops = ed
                .move_to(e.id.as_str(), e.move_point, MoveOptions { keep_page: true })
                .unwrap();
            let mut q = p.clone();
            q.apply_ops(ops).unwrap();
            assert!(close(
                point(
                    &q,
                    &EditContext::new(t.id.as_str(), "de").unwrap(),
                    e.id.as_str()
                ),
                e.move_point
            ));
        }
    }
}

mod prop {
    //! `move_to(p)` followed by resolve puts the element's reference point on
    //! p, for random elements, anchors, targets (gaps, grouping), groups and
    //! variants; the inverse ops restore the project.

    use super::*;
    use proptest::prelude::*;

    #[derive(Clone, Debug)]
    struct Case {
        aspect: (u32, u32),
        pages: u32,
        size: (u32, u32),
        group: u32,
        gap: f64,
        kind: u8,
        anchor: u8,
        page: u32,
        at: (f64, f64),
        width: f64,
        height: f64,
        rotation: f64,
        pivot: Option<(f64, f64)>,
        in_group: Option<(f64, f64, f64)>,
        variant: Option<(f64, bool)>,
        goal: (f64, f64),
        keep_page: bool,
    }

    fn case() -> impl Strategy<Value = Case> {
        (
            (
                prop_oneof![
                    Just((1320, 2868)),
                    Just((16, 10)),
                    Just((1, 1)),
                    Just((9, 16))
                ],
                1u32..6,
                (300u32..3000, 300u32..3000),
                1u32..4,
                0.0..120.0f64,
            ),
            (
                0u8..5,
                0u8..4,
                0u32..6,
                (-500.0..2500.0f64, -300.0..2500.0f64),
            ),
            (
                50.0..900.0f64,
                10.0..600.0f64,
                -170.0..170.0f64,
                proptest::option::of((-200.0..200.0f64, -200.0..200.0f64)),
            ),
            proptest::option::of((-300.0..300.0f64, -300.0..300.0f64, -45.0..45.0f64)),
            proptest::option::of((0.5..1.3f64, any::<bool>())),
            ((-600.0..6000.0f64, -400.0..3000.0f64), any::<bool>()),
        )
            .prop_map(
                |(
                    (aspect, pages, size, group, gap),
                    (kind, anchor, page, at),
                    (width, height, rotation, pivot),
                    in_group,
                    variant,
                    (goal, keep_page),
                )| Case {
                    aspect,
                    pages,
                    size,
                    group,
                    gap,
                    kind,
                    anchor,
                    page: page % pages,
                    at,
                    width,
                    height,
                    rotation,
                    pivot,
                    in_group,
                    variant,
                    goal,
                    keep_page,
                },
            )
    }

    fn project(c: &Case) -> Project {
        let anchor = ["top", "bottom", "center", "stretch"][c.anchor as usize];
        let pivot = c
            .pivot
            .map(|(x, y)| format!("pivot: {{x: {x}, y: {y}}}, "))
            .unwrap_or_default();
        let common = format!(
            "page: {}, at: {{x: {}, y: {}}}, rotation: {}, {pivot}anchor_y: {anchor}",
            c.page, c.at.0, c.at.1, c.rotation
        );
        let element = match c.kind {
            0 => format!(
                "{{id: el, kind: device, model: iphone-island, width: {}, {common}}}",
                c.width
            ),
            1 => format!(
                "{{id: el, kind: image, asset: a.png, width: {}, height: {}, {common}}}",
                c.width, c.height
            ),
            2 => format!(
                "{{id: el, kind: shape, shape: rect, fill: \"#FF0000\", width: {}, height: {}, {common}}}",
                c.width, c.height
            ),
            3 => format!(
                "{{id: el, kind: text, text: k, width: {}, {common}}}",
                c.width
            ),
            _ => format!(
                "{{id: el, kind: text, text: k, box: {{top: {}, left: {}, right: {}{}}}, {common}}}",
                c.at.1.abs(),
                c.width / 10.0,
                c.height / 10.0,
                if c.anchor % 2 == 0 {
                    format!(", bottom: {}", c.height)
                } else {
                    String::new()
                }
            ),
        };
        let sibling = "{id: sib, kind: shape, shape: ellipse, page: 0, at: {x: 300, y: 400}, width: 50, height: 50}";
        let elements = match c.in_group {
            Some((x, y, r)) => format!(
                "  - {{id: g, kind: group, page: 0, at: {{x: {x}, y: {y}}}, rotation: {r}, children: [{element}, {sibling}]}}\n"
            ),
            None => format!("  - {element}\n  - {sibling}\n"),
        };
        let variants = match c.variant {
            Some((k, with_override)) => format!(
                "  variants:\n    v:\n      content_scale: {k}\n{}",
                if with_override {
                    "      overrides:\n        el: {rotation: 12}\n"
                } else {
                    ""
                }
            ),
            None => String::new(),
        };
        let yaml = format!(
            "format: vitrine/1\nname: t\nlanguages: {{default: en, available: [en]}}\nstrips:\n- id: s\n  page: {{aspect: \"{}:{}\", count: {}}}\n  elements:\n{elements}{variants}targets:\n- id: t\n  strip: s\n  size: [{}, {}]\n  group: {}\n  gallery: {{preset: play-phone, gap: {}}}\n{}",
            c.aspect.0,
            c.aspect.1,
            c.pages,
            c.size.0,
            c.size.1,
            c.group,
            c.gap,
            if c.variant.is_some() {
                "  variant: v\n"
            } else {
                ""
            },
        );
        io::project_from_yaml(&yaml).unwrap_or_else(|e| panic!("{e}\n{yaml}"))
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

        #[test]
        fn move_to_lands_on_the_point(c in case()) {
            let mut p = project(&c);
            let ctx = EditContext::new("t", "en").unwrap();
            let goal = Vec2::new(c.goal.0, c.goal.1);
            let files = BTreeSet::<String>::new();
            let ops = {
                let ed = Editor::new(&p, &ctx, ResolveOptions::new(&files)).unwrap();
                ed.move_to("el", goal, MoveOptions { keep_page: c.keep_page })
                    .map_err(|e| TestCaseError::fail(format!("{e}")))?
            };
            let before = p.clone();
            let inverse = p.apply_ops(ops).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let ed = Editor::new(&p, &ctx, ResolveOptions::new(&files)).unwrap();
            let got = ed.reference_point("el").unwrap();
            prop_assert!(close(got, goal), "got {:?}, want {:?}", got, goal);
            if c.keep_page {
                prop_assert_eq!(p.element("el").unwrap().1.page, c.page);
            }
            if c.variant.is_some() {
                prop_assert_eq!(p.element("el").unwrap().1, before.element("el").unwrap().1);
            }
            p.apply_ops(inverse).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(p, before);
        }

        #[test]
        fn rotate_and_resize_round_trip(c in case(), angle in -720.0..720.0f64,
                                        w in 20.0..1500.0f64, h in 20.0..900.0f64,
                                        fixed in 0usize..9) {
            let mut p = project(&c);
            let start = p.clone();
            let ctx = EditContext::new("t", "en").unwrap();
            let files = BTreeSet::<String>::new();
            let handles = [Handle::Nw, Handle::N, Handle::Ne, Handle::E, Handle::Se,
                           Handle::S, Handle::Sw, Handle::W, Handle::Center];
            let fixed = handles[fixed];
            let (ops, goal) = {
                let ed = Editor::new(&p, &ctx, ResolveOptions::new(&files)).unwrap();
                let r = resolve(&p, "t", "en", &ResolveOptions::new(&files)).unwrap();
                let el = r.element("el").unwrap();
                (ed.resize("el", ResizeSpec { width: Some(w), height: Some(h) }, fixed)
                    .map_err(|e| TestCaseError::fail(format!("{e}")))?,
                 handle_point(el, fixed))
            };
            let inverse = p.apply_ops(ops).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let r = resolve(&p, "t", "en", &ResolveOptions::new(&files)).unwrap();
            let el = r.element("el").unwrap();
            prop_assert!(close(handle_point(el, fixed), goal));
            prop_assert!((el.size.x - w).abs() < 1e-6, "width {}", el.size.x);
            if c.kind != 0 {
                prop_assert!((el.size.y - h).abs() < 1e-6, "height {}", el.size.y);
            }
            p.apply_ops(inverse).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(&p, &start);

            let ops = {
                let ed = Editor::new(&p, &ctx, ResolveOptions::new(&files)).unwrap();
                ed.rotate_to("el", angle).map_err(|e| TestCaseError::fail(format!("{e}")))?
            };
            let inverse = p.apply_ops(ops).map_err(|e| TestCaseError::fail(e.to_string()))?;
            let r = resolve(&p, "t", "en", &ResolveOptions::new(&files)).unwrap();
            let got = r.element("el").unwrap().rotation;
            let diff = (got - angle).rem_euclid(360.0);
            prop_assert!(diff < 1e-6 || diff > 360.0 - 1e-6, "rotation {} vs {}", got, angle);
            p.apply_ops(inverse).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(&p, &start);
        }
    }
}
