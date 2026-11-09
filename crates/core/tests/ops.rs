//! Operations API tests on the sample project.

// Op generators pick with `n % k`; `Default::default()` keeps op literals short.
#![allow(
    clippy::manual_is_multiple_of,
    clippy::default_trait_access,
    clippy::single_match_else
)]

use std::path::Path;

use arrangia_core::Project;
use arrangia_core::io;
use arrangia_core::model::{
    Aspect, ElementId, ElementPatch, Fill, Lang, PixelSize, Point, Rgba, StringKey, Strip, StripId,
    Target, TargetId, VariantId,
};
use arrangia_core::ops::{History, Op, OpError, StripPatch, TargetPatch};

fn sample() -> Project {
    Project::load(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/sample")).unwrap()
}

fn patch(yaml: &str) -> ElementPatch {
    io::from_yaml(yaml).unwrap()
}

fn text_element(id: &str, page: u32) -> arrangia_core::model::Element {
    io::from_yaml(&format!(
        "{{ id: {id}, kind: text, page: {page}, text: x.y, style: headline }}"
    ))
    .unwrap()
}

/// Apply, check that the inverse restores the original, return the inverse.
fn apply_and_undo(p: &mut Project, op: Op) -> Op {
    let before = p.clone();
    let inverse = p.apply(op).unwrap();
    let mut undone = p.clone();
    undone.apply(inverse.clone()).unwrap();
    assert_eq!(undone, before);
    inverse
}

#[test]
fn update_element_and_inverse() {
    let mut p = sample();
    let inv = apply_and_undo(
        &mut p,
        Op::UpdateElement {
            id: ElementId::new("hero-phone"),
            patch: patch("{ at: { x: 500, y: 100 }, rotation: ~, page: 1 }"),
        },
    );
    let (_, e) = p.element("hero-phone").unwrap();
    assert_eq!(e.at, Point::new(500.0, 100.0));
    assert!(e.rotation.abs() < f64::EPSILON);
    assert_eq!(e.page, 1);
    assert_eq!(
        inv,
        Op::UpdateElement {
            id: ElementId::new("hero-phone"),
            patch: patch("{ page: 0, at: { x: 1000, y: 2172 }, rotation: -8 }"),
        }
    );
}

#[test]
fn update_element_validates() {
    let mut p = sample();
    let before = p.clone();
    let cases = [
        ("title-1", "{ page: 5 }", "page 5 is out of range"),
        ("title-1", "{ style: nope }", "unknown text style `nope`"),
        ("title-1", "{ color: \"@nope\" }", "unknown colour `@nope`"),
        (
            "title-1",
            "{ model: iphone-home }",
            "not allowed on text elements",
        ),
        (
            "hero-phone",
            "{ shadow: huge }",
            "unknown shadow preset `huge`",
        ),
        (
            "hero-phone",
            "{ visible_in: { langs: [it] } }",
            "unknown language `it`",
        ),
        ("nope", "{ page: 0 }", "unknown element `nope`"),
    ];
    for (id, yaml, msg) in cases {
        let err = p
            .apply(Op::UpdateElement {
                id: ElementId::new(id),
                patch: patch(yaml),
            })
            .unwrap_err();
        assert!(err.to_string().contains(msg), "{yaml}: {err}");
        assert_eq!(p, before, "failed op must not change the project");
    }
}

#[test]
fn add_remove_element_restores_overrides() {
    let mut p = sample();
    let before = p.clone();
    let inv = p
        .apply(Op::RemoveElement {
            id: ElementId::new("title-1"),
        })
        .unwrap();
    assert!(p.element("title-1").is_none());
    let variant = &p.strips[0].variants[&VariantId::new("tablet-landscape")];
    assert!(!variant.overrides.contains_key("title-1"));
    let Op::AddElement {
        index, overrides, ..
    } = &inv
    else {
        panic!("{inv:?}")
    };
    assert_eq!(*index, Some(1));
    assert_eq!(overrides.len(), 1);
    p.apply(inv).unwrap();
    assert_eq!(p, before);

    // Removing a group removes children and their overrides as well.
    apply_and_undo(
        &mut p,
        Op::RemoveElement {
            id: ElementId::new("badge-group"),
        },
    );
}

#[test]
fn add_element_checks_ids_and_parent() {
    let mut p = sample();
    let err = p
        .apply(Op::AddElement {
            strip: StripId::new("phone"),
            parent: None,
            index: None,
            element: text_element("hero-phone", 0),
            overrides: Default::default(),
        })
        .unwrap_err();
    assert_eq!(
        err,
        OpError::DuplicateId {
            what: "element",
            id: "hero-phone".into()
        }
    );
    let err = p
        .apply(Op::AddElement {
            strip: StripId::new("phone"),
            parent: Some(ElementId::new("title-1")),
            index: None,
            element: text_element("new", 0),
            overrides: Default::default(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("not a group"), "{err}");

    // Into a group, at a position.
    apply_and_undo(
        &mut p,
        Op::AddElement {
            strip: StripId::new("phone"),
            parent: Some(ElementId::new("badge-group")),
            index: Some(0),
            element: text_element("badge-text", 2),
            overrides: Default::default(),
        },
    );
    let (_, g) = p.element("badge-group").unwrap();
    assert_eq!(g.children()[0].id.as_str(), "badge-text");
}

#[test]
fn move_element() {
    let mut p = sample();
    let inv = apply_and_undo(
        &mut p,
        Op::MoveElement {
            id: ElementId::new("hero-phone"),
            index: 3,
        },
    );
    assert_eq!(p.strips[0].elements[3].id.as_str(), "hero-phone");
    assert_eq!(
        inv,
        Op::MoveElement {
            id: ElementId::new("hero-phone"),
            index: 0
        }
    );
}

#[test]
fn strings() {
    let mut p = sample();
    apply_and_undo(
        &mut p,
        Op::SetString {
            lang: Lang::new("fr"),
            key: StringKey::new("hero.title"),
            value: Some("Planifiez **en minutes**".into()),
        },
    );
    assert_eq!(
        p.string("fr", "hero.title"),
        Some("Planifiez **en minutes**")
    );
    apply_and_undo(
        &mut p,
        Op::SetString {
            lang: Lang::new("fr"),
            key: StringKey::new("hero.title"),
            value: None,
        },
    );
    assert!(
        !p.strings.contains_key("fr"),
        "empty language maps are dropped"
    );
    let err = p
        .apply(Op::SetString {
            lang: Lang::new("it"),
            key: StringKey::new("a"),
            value: Some("b".into()),
        })
        .unwrap_err();
    assert_eq!(err, OpError::UnknownLanguage("it".into()));
}

#[test]
fn strips_and_references() {
    let mut p = sample();
    let err = p
        .apply(Op::RemoveStrip {
            id: StripId::new("phone"),
        })
        .unwrap_err();
    assert!(matches!(err, OpError::StillReferenced { .. }), "{err}");
    apply_and_undo(
        &mut p,
        Op::RemoveStrip {
            id: StripId::new("web"),
        },
    );
    let err = p
        .apply(Op::UpdateStrip {
            id: StripId::new("phone"),
            patch: StripPatch {
                count: Some(4),
                ..Default::default()
            },
        })
        .unwrap_err();
    assert!(err.to_string().contains("out of range"), "{err}");
    apply_and_undo(
        &mut p,
        Op::UpdateStrip {
            id: StripId::new("phone"),
            patch: StripPatch {
                aspect: Some(Aspect::new(1290, 2796)),
                count: Some(8),
                ids: Some(vec!["hero".into(), "planner".into()]),
                backgrounds: Some(vec![Fill::Color {
                    color: arrangia_core::model::ColorRef::Named("primary".into()),
                }]),
            },
        },
    );
    let mut s = Strip::new(StripId::new("tablet"), Aspect::new(4, 3), 3);
    s.elements.push(text_element("tablet-title", 2));
    apply_and_undo(
        &mut p,
        Op::AddStrip {
            strip: s.clone(),
            index: Some(0),
        },
    );
    s.elements[0] = text_element("hero-phone", 0);
    assert!(matches!(
        p.apply(Op::AddStrip {
            strip: s,
            index: None
        }),
        Err(OpError::DuplicateId { .. })
    ));
}

#[test]
fn variants() {
    let mut p = sample();
    let strip = StripId::new("phone");
    let tl = VariantId::new("tablet-landscape");
    let err = p
        .apply(Op::RemoveVariant {
            strip: strip.clone(),
            id: tl.clone(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("ios-ipad-13-landscape"), "{err}");
    apply_and_undo(
        &mut p,
        Op::SetVariantOverride {
            strip: strip.clone(),
            variant: tl.clone(),
            element: ElementId::new("hero-phone"),
            patch: Some(patch("{ width: 600 }")),
        },
    );
    apply_and_undo(
        &mut p,
        Op::SetVariantOverride {
            strip: strip.clone(),
            variant: tl.clone(),
            element: ElementId::new("hero-phone"),
            patch: None,
        },
    );
    let err = p
        .apply(Op::SetVariantOverride {
            strip: strip.clone(),
            variant: tl.clone(),
            element: ElementId::new("title-1"),
            patch: Some(patch("{ screenshot: home }")),
        })
        .unwrap_err();
    assert!(err.to_string().contains("not allowed"), "{err}");
    apply_and_undo(
        &mut p,
        Op::SetVariantContentScale {
            strip: strip.clone(),
            variant: tl.clone(),
            content_scale: Some(0.5),
        },
    );
    assert!(
        p.apply(Op::SetVariantContentScale {
            strip: strip.clone(),
            variant: tl.clone(),
            content_scale: Some(0.0),
        })
        .is_err()
    );
    apply_and_undo(
        &mut p,
        Op::AddVariant {
            strip: strip.clone(),
            id: VariantId::new("short"),
            variant: io::from_yaml("{ overrides: { title-1: { anchor_y: center } } }").unwrap(),
        },
    );
    apply_and_undo(
        &mut p,
        Op::RemoveVariant {
            strip,
            id: VariantId::new("short"),
        },
    );
}

#[test]
fn targets() {
    let mut p = sample();
    let mut t = Target::new(
        TargetId::new("mac"),
        StripId::new("web"),
        PixelSize::new(2880, 1800),
    );
    apply_and_undo(
        &mut p,
        Op::AddTarget {
            target: t.clone(),
            index: None,
        },
    );
    t.id = TargetId::new("mac2");
    t.variant = Some(VariantId::new("nope"));
    assert!(matches!(
        p.apply(Op::AddTarget {
            target: t,
            index: None
        }),
        Err(OpError::UnknownVariant { .. })
    ));
    let id = TargetId::new("blog-header");
    apply_and_undo(
        &mut p,
        Op::UpdateTarget {
            id: id.clone(),
            patch: io::from_yaml(
                "{ size: [1200, 630], region: ~, langs: ~, gallery: appstore-iphone }",
            )
            .unwrap(),
        },
    );
    let err = p
        .apply(Op::UpdateTarget {
            id: id.clone(),
            patch: TargetPatch {
                group: Some(Some(2)),
                ..Default::default()
            },
        })
        .unwrap_err();
    assert!(err.to_string().contains("not allowed on banner"), "{err}");
    let err = p
        .apply(Op::UpdateTarget {
            id,
            patch: io::from_yaml("{ region: { pages: [3, 9] } }").unwrap(),
        })
        .unwrap_err();
    assert!(err.to_string().contains("region"), "{err}");
    // Referenced by `visible_in` of page4-bg.
    let err = p
        .apply(Op::RemoveTarget {
            id: TargetId::new("ios-phone-69"),
        })
        .unwrap_err();
    assert!(err.to_string().contains("page4-bg"), "{err}");
    apply_and_undo(
        &mut p,
        Op::RemoveTarget {
            id: TargetId::new("android-phone"),
        },
    );
}

#[test]
fn styles_and_languages() {
    let mut p = sample();
    let mut styles = p.styles.clone();
    styles
        .colors
        .insert("bg".into(), Rgba::parse("#FAFAFA").unwrap());
    apply_and_undo(
        &mut p,
        Op::SetStyles {
            styles: styles.clone(),
        },
    );
    styles.colors.remove("accent");
    let err = p.apply(Op::SetStyles { styles }).unwrap_err();
    assert_eq!(err, OpError::UnknownColor("accent".into()));
    let mut styles = p.styles.clone();
    styles.text.remove("headline");
    let err = p.apply(Op::SetStyles { styles }).unwrap_err();
    assert!(err.to_string().contains("text style `headline`"), "{err}");

    let mut langs = p.languages.clone();
    langs.available.push(Lang::new("it"));
    apply_and_undo(&mut p, Op::SetLanguages { languages: langs });
    let mut langs = p.languages.clone();
    langs.available.retain(|l| l.as_str() != "de");
    let err = p.apply(Op::SetLanguages { languages: langs }).unwrap_err();
    assert!(err.to_string().contains("blog-header"), "{err}");
    apply_and_undo(
        &mut p,
        Op::SetName {
            name: "other".into(),
        },
    );
}

#[test]
fn transactions_are_atomic() {
    let mut p = sample();
    let before = p.clone();
    let ops = vec![
        Op::SetName { name: "x".into() },
        Op::RemoveElement {
            id: ElementId::new("title-1"),
        },
        Op::RemoveElement {
            id: ElementId::new("title-1"),
        },
    ];
    let err = p.apply_ops(ops).unwrap_err();
    assert!(
        matches!(err, OpError::Transaction { index: 2, .. }),
        "{err}"
    );
    assert_eq!(p, before);

    let ops = vec![
        Op::SetName { name: "x".into() },
        Op::RemoveElement {
            id: ElementId::new("title-1"),
        },
        Op::UpdateElement {
            id: ElementId::new("sub-1"),
            patch: patch("{ page: 3 }"),
        },
    ];
    let inverse = p.apply_ops(ops).unwrap();
    assert_eq!(inverse.len(), 3);
    p.apply_ops(inverse).unwrap();
    assert_eq!(p, before);
}

#[test]
fn history_undo_redo() {
    let mut p = sample();
    let original = p.clone();
    let mut h = History::new();
    assert!(!h.undo(&mut p).unwrap());
    h.apply(&mut p, vec![Op::SetName { name: "a".into() }])
        .unwrap();
    h.apply(
        &mut p,
        vec![
            Op::RemoveElement {
                id: ElementId::new("hero-phone"),
            },
            Op::SetName { name: "b".into() },
        ],
    )
    .unwrap();
    let after = p.clone();
    assert!(h.undo(&mut p).unwrap());
    assert_eq!(p.name, "a");
    assert!(p.element("hero-phone").is_some());
    assert!(h.undo(&mut p).unwrap());
    assert_eq!(p, original);
    assert!(!h.can_undo());
    assert!(h.redo(&mut p).unwrap());
    assert!(h.redo(&mut p).unwrap());
    assert_eq!(p, after);
    assert!(!h.redo(&mut p).unwrap());
    // A new step clears redo.
    h.undo(&mut p).unwrap();
    h.apply(&mut p, vec![Op::SetName { name: "c".into() }])
        .unwrap();
    assert!(!h.can_redo());
    // Failed apply records nothing.
    assert!(
        h.apply(
            &mut p,
            vec![Op::RemoveStrip {
                id: StripId::new("nope")
            }]
        )
        .is_err()
    );
    h.undo(&mut p).unwrap();
    assert_eq!(p.name, "a");
}

#[test]
fn ops_as_json() {
    let op: Op = serde_json::from_str(
        r#"{"op":"update_element","id":"hero-phone","patch":{"at":{"x":10,"y":20},"anchor_y":null}}"#,
    )
    .unwrap();
    assert_eq!(
        op,
        Op::UpdateElement {
            id: ElementId::new("hero-phone"),
            patch: patch("{ at: { x: 10, y: 20 }, anchor_y: ~ }"),
        }
    );
    let json = serde_json::to_string(&op).unwrap();
    assert_eq!(
        json,
        r#"{"op":"update_element","id":"hero-phone","patch":{"at":{"x":10,"y":20},"anchor_y":null}}"#
    );

    let op: Op = serde_json::from_str(
        r#"{"op":"add_element","strip":"phone","element":{"id":"t2","kind":"text","page":1,"text":"hero.sub","box":{"top":100}}}"#,
    )
    .unwrap();
    let mut p = sample();
    apply_and_undo(&mut p, op.clone());
    let back: Op = serde_json::from_str(&serde_json::to_string(&op).unwrap()).unwrap();
    assert_eq!(back, op);

    let err = serde_json::from_str::<Op>(r#"{"op":"remove_element","id":"a","x":1}"#)
        .unwrap_err()
        .to_string();
    assert!(err.contains("unknown field `x`"), "{err}");
    let err = serde_json::from_str::<Op>(
        r#"{"op":"add_element","strip":"phone","element":{"id":"t2","kind":"text","text":"a","model":"x"}}"#,
    )
    .unwrap_err()
    .to_string();
    assert!(err.contains("not allowed on text elements"), "{err}");
}

mod prop {
    //! apply(op) + apply(inverse) == original, on random ops generated
    //! from the current state of the sample project.

    use super::*;
    use proptest::prelude::*;

    /// Everything an op generator can pick from.
    struct Pick<'a> {
        p: &'a Project,
        elements: Vec<(StripId, ElementId, bool)>,
    }

    impl<'a> Pick<'a> {
        fn new(p: &'a Project) -> Self {
            let mut elements = Vec::new();
            for s in &p.strips {
                s.walk_elements(&mut |e| {
                    elements.push((
                        s.id.clone(),
                        e.id.clone(),
                        matches!(e.kind, arrangia_core::model::ElementKind::Text(_)),
                    ));
                });
            }
            Self { p, elements }
        }

        fn element(&self, n: u32) -> Option<&(StripId, ElementId, bool)> {
            (!self.elements.is_empty()).then(|| &self.elements[n as usize % self.elements.len()])
        }

        fn strip(&self, n: u32) -> Option<&Strip> {
            (!self.p.strips.is_empty()).then(|| &self.p.strips[n as usize % self.p.strips.len()])
        }

        fn target(&self, n: u32) -> Option<&Target> {
            (!self.p.targets.is_empty()).then(|| &self.p.targets[n as usize % self.p.targets.len()])
        }

        fn lang(&self, n: u32) -> Lang {
            let l = &self.p.languages.available;
            l[n as usize % l.len()].clone()
        }
    }

    #[allow(clippy::too_many_lines, clippy::many_single_char_names)]
    fn make_op(p: &Project, kind: u8, a: u32, b: u32, x: f64) -> Option<Op> {
        let pick = Pick::new(p);
        let page = b % 6; // sometimes out of range on purpose
        Some(match kind % 16 {
            0 => {
                let (_, id, is_text) = pick.element(a)?;
                let yaml = match b % 7 {
                    0 => format!("{{ at: {{ x: {x}, y: {} }} }}", x * 2.0),
                    1 => format!("{{ rotation: {x}, page: {page} }}"),
                    2 => "{ rotation: ~, at: ~, anchor_y: bottom }".into(),
                    3 => format!("{{ width: {}, opacity: 0.5, z: {} }}", x.abs() + 1.0, a % 5),
                    4 if *is_text => format!("{{ box: {{ top: {x}, left: 10 }}, align: center }}"),
                    5 if *is_text => {
                        "{ text: other.key, style: sub, fit: { shrink_to: 30 } }".into()
                    }
                    _ => "{ visible: false, clip: page }".into(),
                };
                Op::UpdateElement {
                    id: id.clone(),
                    patch: io::from_yaml(&yaml).unwrap(),
                }
            }
            1 => {
                let s = pick.strip(a)?;
                let id = format!("gen-{a}-{b}");
                let element = if b % 2 == 0 {
                    text_element(&id, page)
                } else {
                    io::from_yaml(&format!(
                        "{{ id: {id}, kind: shape, shape: ellipse, page: {page}, width: 10, height: 10, fill: \"@primary\" }}"
                    ))
                    .unwrap()
                };
                Op::AddElement {
                    strip: s.id.clone(),
                    parent: None,
                    index: Some(b as usize % (s.elements.len() + 1)),
                    element,
                    overrides: Default::default(),
                }
            }
            2 => Op::RemoveElement {
                id: pick.element(a)?.1.clone(),
            },
            3 => {
                let (_, id, _) = pick.element(a)?;
                Op::MoveElement {
                    id: id.clone(),
                    index: b as usize % 4,
                }
            }
            4 => Op::SetString {
                lang: pick.lang(a),
                key: StringKey::new(["hero.title", "hero.sub", "new.key"][b as usize % 3]),
                value: (b % 4 != 0).then(|| format!("text {x}")),
            },
            5 => {
                let (strip, id, _) = pick.element(a)?;
                let s = p.strip(strip.as_str())?;
                let variant = s
                    .variants
                    .keys()
                    .nth(b as usize % s.variants.len().max(1))?;
                Op::SetVariantOverride {
                    strip: strip.clone(),
                    variant: variant.clone(),
                    element: id.clone(),
                    patch: (b % 3 != 0).then(|| {
                        io::from_yaml(&format!("{{ rotation: {x}, page: {page} }}")).unwrap()
                    }),
                }
            }
            6 => Op::AddVariant {
                strip: pick.strip(a)?.id.clone(),
                id: VariantId::new(&format!("v{}", b % 3)),
                variant: arrangia_core::model::Variant {
                    content_scale: Some(0.5 + f64::from(b % 3) / 4.0),
                    ..Default::default()
                },
            },
            7 => {
                let s = pick.strip(a)?;
                Op::RemoveVariant {
                    strip: s.id.clone(),
                    id: s
                        .variants
                        .keys()
                        .nth(b as usize % s.variants.len().max(1))?
                        .clone(),
                }
            }
            8 => Op::UpdateStrip {
                id: pick.strip(a)?.id.clone(),
                patch: StripPatch {
                    aspect: (b % 2 == 0).then(|| Aspect::new(1 + a % 30, 1 + b % 40)),
                    count: (b % 3 == 0).then_some(1 + b % 9),
                    ids: (b % 7 == 0).then(|| vec![format!("p{a}"), "second".into()]),
                    backgrounds: (b % 5 == 0).then(|| {
                        vec![Fill::Color {
                            color: arrangia_core::model::ColorRef::Named("accent".into()),
                        }]
                    }),
                },
            },
            9 => {
                let mut s = Strip::new(
                    StripId::new(&format!("s{}", b % 4)),
                    Aspect::new(16, 9),
                    1 + b % 3,
                );
                s.elements.push(text_element(&format!("st-{a}"), 0));
                Op::AddStrip {
                    strip: s,
                    index: Some(a as usize % (p.strips.len() + 1)),
                }
            }
            10 => Op::RemoveStrip {
                id: pick.strip(a)?.id.clone(),
            },
            11 => {
                let t = pick.target(a)?;
                let yaml = match b % 4 {
                    0 => format!("{{ size: [{}, {}] }}", 100 + a % 1000, 100 + b % 1000),
                    1 => "{ gallery: { preset: x, gap: 12 }, screenshots: ~ }".into(),
                    2 => "{ variant: ~, device_map: { a: b } }".into(),
                    _ => format!("{{ group: {} }}", b % 3),
                };
                Op::UpdateTarget {
                    id: t.id.clone(),
                    patch: io::from_yaml(&yaml).unwrap(),
                }
            }
            12 => {
                let s = pick.strip(b)?;
                Op::AddTarget {
                    target: Target::new(
                        TargetId::new(&format!("t{}", a % 4)),
                        s.id.clone(),
                        PixelSize::new(100, 200),
                    ),
                    index: Some(b as usize % (p.targets.len() + 1)),
                }
            }
            13 => Op::RemoveTarget {
                id: pick.target(a)?.id.clone(),
            },
            14 => {
                let mut styles = p.styles.clone();
                if b % 2 == 0 {
                    styles
                        .colors
                        .insert(format!("c{}", a % 3), Rgba::rgb(a as u8, b as u8, 0));
                } else {
                    let name = styles
                        .colors
                        .keys()
                        .nth(a as usize % styles.colors.len().max(1))?
                        .clone();
                    styles.colors.remove(&name);
                }
                Op::SetStyles { styles }
            }
            _ => {
                let mut languages = p.languages.clone();
                if b % 2 == 0 {
                    languages
                        .available
                        .push(Lang::new(["it", "nl", "ja"][a as usize % 3]));
                } else if languages.available.len() > 1 {
                    let i = a as usize % languages.available.len();
                    languages.available.remove(i);
                }
                Op::SetLanguages { languages }
            }
        })
    }

    fn op_inputs() -> impl Strategy<Value = Vec<(u8, u32, u32, f64)>> {
        proptest::collection::vec(
            (
                any::<u8>(),
                any::<u32>(),
                any::<u32>(),
                (-2000i32..2000).prop_map(|v| f64::from(v) / 4.0),
            ),
            1..12,
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

        /// Every single op: inverse restores the state; failed ops change nothing.
        #[test]
        fn apply_then_inverse_restores(inputs in op_inputs()) {
            let mut p = sample();
            for (kind, a, b, x) in inputs {
                let Some(op) = make_op(&p, kind, a, b, x) else { continue };
                let before = p.clone();
                match p.apply(op.clone()) {
                    Ok(inverse) => {
                        let mut undone = p.clone();
                        undone.apply(inverse.clone()).map_err(|e| {
                            TestCaseError::fail(format!("inverse {inverse:?} of {op:?} failed: {e}"))
                        })?;
                        prop_assert_eq!(&undone, &before, "op {:?}", op);
                        prop_assert_eq!(undone.to_yaml().unwrap(), before.to_yaml().unwrap());
                        // The result is still a structurally valid project
                        // that survives a YAML round trip.
                        prop_assert!(p.check_structure().is_ok());
                        let reparsed = io::project_from_yaml(&p.to_yaml().unwrap()).unwrap();
                        let mut expected = p.clone();
                        expected.strings.clear();
                        prop_assert_eq!(reparsed, expected);
                    }
                    Err(_) => prop_assert_eq!(&p, &before, "failed op changed the project: {:?}", op),
                }
            }
        }

        /// A sequence as one transaction: its inverse list restores the start.
        #[test]
        fn transaction_inverse_restores(inputs in op_inputs()) {
            let mut p = sample();
            let start = p.clone();
            // Build a valid sequence by applying ops one by one on a scratch copy.
            let mut scratch = p.clone();
            let mut ops = Vec::new();
            for (kind, a, b, x) in inputs {
                if let Some(op) = make_op(&scratch, kind, a, b, x)
                    && scratch.apply(op.clone()).is_ok()
                {
                    ops.push(op);
                }
            }
            let inverse = p.apply_ops(ops).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(&p, &scratch);
            p.apply_ops(inverse).map_err(|e| TestCaseError::fail(e.to_string()))?;
            prop_assert_eq!(&p, &start);
        }
    }
}
