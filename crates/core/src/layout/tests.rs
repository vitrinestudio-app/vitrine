use std::collections::BTreeMap;

use proptest::prelude::*;

use super::*;

fn layouter() -> TextLayouter {
    TextLayouter::new(FontLibrary::bundled())
}

fn headline(size: f32) -> ResolvedTextStyle {
    ResolvedTextStyle {
        font: "Inter Display".into(),
        weight: 700,
        size,
        line_height: 1.08,
        ..ResolvedTextStyle::default()
    }
}

fn texts(l: &TextLayout) -> Vec<String> {
    l.lines.iter().map(|line| l.line_text(line)).collect()
}

fn width_of(l: &mut TextLayouter, s: &str, style: &ResolvedTextStyle) -> f32 {
    l.layout(&TextInput::new(s, style, 10_000.0)).lines[0].width
}

#[test]
fn short_text_is_one_line_long_text_wraps() {
    let mut l = layouter();
    let style = headline(72.0);
    let one = l.layout(&TextInput::new("Plane deine Beete", &style, 840.0));
    assert_eq!(texts(&one), ["Plane deine Beete"]);
    assert!(!one.overflow && one.diagnostics.is_empty());
    assert!((one.font_size - 72.0).abs() < f32::EPSILON);

    let two = l.layout(&TextInput::new(
        "Plane deine Beete in Minuten und behalte alles im Blick",
        &style,
        840.0,
    ));
    assert_eq!(two.lines.len(), 3, "{:?}", texts(&two));
    assert!(two.lines.iter().all(|line| line.width <= 840.0));
    assert!(two.lines.windows(2).all(|w| w[1].top > w[0].top));
    assert!((two.r#box.h - two.content.h).abs() < 0.01);
}

#[test]
fn explicit_breaks() {
    let mut l = layouter();
    let style = headline(60.0);
    let out = l.layout(&TextInput::new("Eins\\nZwei\nDrei", &style, 900.0));
    assert_eq!(texts(&out), ["Eins", "Zwei", "Drei"]);
    assert_eq!(out.lines[0].end, LineEnd::Explicit);
    assert_eq!(out.lines[2].end, LineEnd::End);
}

#[test]
fn auto_fit_shrinks_long_german_text() {
    let mut l = layouter();
    let style = headline(104.0);
    let fit = Fit {
        shrink_to: Some(56.0),
        max_lines: Some(2),
    };
    let input = |markup| TextInput {
        align: Align::Center,
        fit,
        ..TextInput::new(markup, &style, 840.0)
    };
    let en = l.layout(&input("Plan your beds **in minutes**"));
    let de = l.layout(&input(
        "Behalte jede [accent]Ernte[/] auf einen Blick im Griff",
    ));
    assert!(!en.overflow && !de.overflow);
    assert!(en.lines.len() <= 2 && de.lines.len() <= 2);
    assert!(
        de.font_size < en.font_size,
        "{} vs {}",
        de.font_size,
        en.font_size
    );
    assert!(de.font_size >= 56.0 && de.font_size < 104.0);
    // Chosen sizes lie on the FIT_STEP grid above shrink_to.
    let k = (de.font_size - 56.0) / FIT_STEP;
    assert!((k - k.round()).abs() < 1e-3);
    // One step larger would not fit any more.
    let bigger = l.layout(&TextInput {
        fit: Fit::default(),
        ..input("Behalte jede [accent]Ernte[/] auf einen Blick im Griff")
    });
    assert!(bigger.lines.len() > 2 || bigger.font_size > de.font_size);
    let style_up = headline(de.font_size + FIT_STEP);
    let up = l.layout(&TextInput {
        align: Align::Center,
        ..TextInput::new(
            "Behalte jede [accent]Ernte[/] auf einen Blick im Griff",
            &style_up,
            840.0,
        )
    });
    assert!(up.lines.len() > 2, "{:?}", texts(&up));
}

#[test]
fn auto_fit_respects_box_height() {
    let mut l = layouter();
    let style = headline(80.0);
    let out = l.layout(&TextInput {
        height: Some(120.0),
        fit: Fit {
            shrink_to: Some(20.0),
            max_lines: None,
        },
        ..TextInput::new("Mischkultur für jedes Beet in deinem Garten", &style, 600.0)
    });
    assert!(!out.overflow);
    assert!(out.content.h <= 120.0 + 0.01);
    assert!(out.font_size < 80.0);
}

#[test]
fn max_lines_and_overflow() {
    let mut l = layouter();
    let style = headline(72.0);
    let text = "Plane deine Beete in Minuten und behalte alles im Blick";
    let fitted = l.layout(&TextInput {
        fit: Fit {
            shrink_to: Some(30.0),
            max_lines: Some(1),
        },
        ..TextInput::new(text, &style, 840.0)
    });
    assert_eq!(fitted.lines.len(), 1);
    assert!(!fitted.overflow);

    let over = l.layout(&TextInput {
        fit: Fit {
            shrink_to: Some(60.0),
            max_lines: Some(1),
        },
        ..TextInput::new(text, &style, 840.0)
    });
    assert!(over.overflow);
    assert!((over.font_size - 60.0).abs() < f32::EPSILON);
    assert!(
        over.diagnostics
            .iter()
            .any(|d| matches!(d, TextDiagnostic::OverflowLines { max_lines: 1, .. }))
    );
}

#[test]
fn soft_hyphen_breaks_with_visible_hyphen() {
    let mut l = layouter();
    let style = headline(104.0);
    let out = l.layout(&TextInput::new(
        "Misch\\-kultur\\-empfehlungen für jedes Beet",
        &style,
        840.0,
    ));
    assert_eq!(
        texts(&out),
        ["Mischkultur-", "empfehlungen", "für jedes Beet"]
    );
    assert_eq!(out.lines[0].end, LineEnd::SoftHyphen);
    assert!(out.diagnostics.is_empty(), "{:?}", out.diagnostics);
    // The hyphen glyph sits at the end of line 0.
    let hyphen_x = out
        .runs
        .iter()
        .flat_map(|r| &r.glyphs)
        .filter(|g| (g.y - out.lines[0].baseline).abs() < 0.01)
        .map(|g| g.x)
        .fold(f32::MIN, f32::max);
    let line = &out.lines[0];
    assert!(hyphen_x > line.x + line.width * 0.9 && hyphen_x < line.x + line.width);
}

#[test]
fn soft_hyphen_is_invisible_without_break() {
    let mut l = layouter();
    let style = headline(72.0);
    let plain = width_of(&mut l, "Mischkultur", &style);
    let shy = width_of(&mut l, "Misch\\-kul\u{AD}tur", &style);
    assert!((plain - shy).abs() < 0.01, "{plain} vs {shy}");
    let out = l.layout(&TextInput::new("Misch\\-kultur", &style, 2000.0));
    assert_eq!(texts(&out), ["Mischkultur"]);
    let dash = l.layout(&TextInput::new("-", &style, 2000.0)).runs[0].glyphs[0].id;
    assert!(
        out.runs
            .iter()
            .flat_map(|r| &r.glyphs)
            .all(|g| g.id != dash)
    );
}

#[test]
fn soft_hyphen_line_leaves_room_for_the_hyphen() {
    let mut l = layouter();
    let style = headline(72.0);
    let w0 = width_of(&mut l, "Mischkultur", &style);
    let hyphen = width_of(&mut l, "Mischkultur-", &style) - w0;
    assert!(hyphen > 5.0);
    // "Mischkultur" fits, "Mischkultur-" does not → break at the first SHY.
    let width = w0 + hyphen / 2.0;
    let out = l.layout(&TextInput::new("Misch\\-kultur\\-beet", &style, width));
    assert_eq!(texts(&out), ["Misch-", "kulturbeet"]);
    assert!(out.lines.iter().all(|line| line.width <= width + 0.01));
}

#[test]
fn emergency_break_is_reported() {
    let mut l = layouter();
    let style = headline(104.0);
    let out = l.layout(&TextInput {
        fit: Fit {
            shrink_to: Some(90.0),
            max_lines: None,
        },
        ..TextInput::new("**Mischkulturempfehlungen** für jedes Beet", &style, 840.0)
    });
    assert!((out.font_size - 90.0).abs() < f32::EPSILON);
    assert_eq!(out.lines[0].end, LineEnd::Emergency);
    assert_eq!(
        out.diagnostics,
        [TextDiagnostic::EmergencyBreak {
            word: "Mischkulturempfehlungen".into(),
            line: 0
        }]
    );
    assert!(!out.overflow, "an emergency break alone is no overflow");
    assert!(out.lines.iter().all(|line| line.width <= 840.01));

    // Auto-fit prefers shrinking over breaking words.
    let shrunk = l.layout(&TextInput {
        fit: Fit {
            shrink_to: Some(40.0),
            max_lines: None,
        },
        ..TextInput::new("**Mischkulturempfehlungen** für jedes Beet", &style, 840.0)
    });
    assert_eq!(shrunk.diagnostics, Vec::<TextDiagnostic>::new());
    assert!(shrunk.font_size < 90.0);
}

#[test]
fn horizontal_alignment() {
    let mut l = layouter();
    let style = headline(60.0);
    let run = |l: &mut TextLayouter, align| {
        l.layout(&TextInput {
            align,
            ..TextInput::new("Ernte im Blick", &style, 900.0)
        })
    };
    let left = run(&mut l, Align::Left);
    let center = run(&mut l, Align::Center);
    let right = run(&mut l, Align::Right);
    let w = left.lines[0].width;
    assert!(left.lines[0].x.abs() < 0.01);
    assert!((center.lines[0].x - (900.0 - w) / 2.0).abs() < 0.01);
    assert!((right.lines[0].x + w - 900.0).abs() < 0.01);
    let first_x = |t: &TextLayout| t.runs[0].glyphs[0].x;
    assert!((first_x(&right) - first_x(&left) - (900.0 - w)).abs() < 0.01);
}

#[test]
fn vertical_alignment_in_fixed_box() {
    let mut l = layouter();
    let style = headline(60.0);
    let run = |l: &mut TextLayouter, valign| {
        l.layout(&TextInput {
            valign,
            height: Some(400.0),
            ..TextInput::new("Ernte", &style, 900.0)
        })
    };
    let top = run(&mut l, VAlign::Top);
    let mid = run(&mut l, VAlign::Center);
    let bottom = run(&mut l, VAlign::Bottom);
    let h = top.content.h;
    assert!((top.r#box.h - 400.0).abs() < f32::EPSILON);
    assert!(top.content.y.abs() < 0.01);
    assert!((mid.content.y - (400.0 - h) / 2.0).abs() < 0.01);
    assert!((bottom.content.bottom() - 400.0).abs() < 0.01);
}

#[test]
fn spans_bold_italic_and_highlight() {
    let mut l = layouter();
    let style = headline(60.0);
    let mut spans = BTreeMap::new();
    spans.insert(
        "accent".to_owned(),
        ResolvedSpanStyle {
            color: Some(Rgba::rgb(255, 198, 92)),
            background: Some(Rgba::rgb(20, 20, 20)),
            ..ResolvedSpanStyle::default()
        },
    );
    let out = l.layout(&TextInput {
        spans: Some(&spans),
        ..TextInput::new(
            "Mit [accent]Misch kultur[/] **mehr** _Ernte_ [nope]x[/]",
            &style,
            2000.0,
        )
    });
    assert_eq!(
        out.diagnostics,
        [TextDiagnostic::UnknownSpanStyle {
            name: "nope".into()
        }]
    );
    assert_eq!(out.highlights.len(), 1, "adjacent runs are merged");
    assert_eq!(out.highlights[0].color, Rgba::rgb(20, 20, 20));
    assert!(out.runs.iter().any(|r| r.color == Rgba::rgb(255, 198, 92)));
    let fonts = l.fonts();
    let weight = |r: &GlyphRun| fonts.face(r.font).unwrap().weight;
    assert!(out.runs.iter().any(|r| weight(r) == 900), "bold = Black");
    assert!(
        out.runs.iter().any(|r| r.skew.is_some()),
        "italic is synthesised"
    );
    assert!(out.runs.iter().all(|r| !r.embolden));
}

#[test]
fn markup_and_font_problems_are_diagnosed() {
    let mut l = layouter();
    let style = ResolvedTextStyle {
        font: "Comic Sans".into(),
        ..ResolvedTextStyle::default()
    };
    let out = l.layout(&TextInput::new("**open 日本", &style, 900.0));
    assert_eq!(texts(&out), ["**open 日本"]);
    assert_eq!(
        out.diagnostics,
        [
            TextDiagnostic::Markup {
                marker: "**".into(),
                offset: 0
            },
            TextDiagnostic::UnknownFont {
                family: "Comic Sans".into()
            },
            TextDiagnostic::MissingGlyphs {
                text: "日本".into()
            },
        ]
    );
    // Falls back to Inter.
    let fam = &l.fonts().face(out.runs[0].font).unwrap().family;
    assert_eq!(fam, "Inter");
}

#[test]
fn uppercase_transform() {
    let mut l = layouter();
    let style = ResolvedTextStyle {
        transform: TextTransform::Uppercase,
        ..ResolvedTextStyle::default()
    };
    let out = l.layout(&TextInput::new("Straße **neu**", &style, 900.0));
    assert_eq!(texts(&out), ["STRASSE NEU"]);
}

#[test]
fn style_from_model() {
    use crate::model::{ColorRef, TextStyle};
    let mut colors = BTreeMap::new();
    colors.insert("text".to_owned(), Rgba::rgb(1, 2, 3));
    let s = ResolvedTextStyle::from_model(
        &TextStyle {
            font: Some("Inter Display".into()),
            weight: Some(800),
            size: Some(72.0),
            line_height: Some(1.08),
            color: Some(ColorRef::Named("text".into())),
            ..TextStyle::default()
        },
        &colors,
    );
    assert_eq!(s.color, Rgba::rgb(1, 2, 3));
    assert_eq!(s.weight, 800);
    assert!((s.size - 72.0).abs() < f32::EPSILON);
    assert_eq!(bolder(400), 700);
    assert_eq!(bolder(700), 900);
}

#[test]
fn layout_is_deterministic() {
    let style = headline(90.0);
    let input = TextInput {
        align: Align::Center,
        fit: Fit {
            shrink_to: Some(50.0),
            max_lines: Some(2),
        },
        ..TextInput::new(
            "Dein Garten, jetzt auch [accent]auf dem iPad[/]",
            &style,
            840.0,
        )
    };
    let a = layouter().layout(&input);
    let b = layouter().layout(&input);
    assert_eq!(a, b);
}

#[test]
fn layout_report_snapshot() {
    let mut l = layouter();
    let style = headline(104.0);
    let reports: Vec<LayoutReport> = [
        ("Plane deine Beete **in Minuten**", Some(2)),
        ("Misch\\-kultur\\-empfehlungen für jedes Beet", Some(3)),
        ("**Mischkulturempfehlungen** für jedes Beet", Some(1)),
    ]
    .into_iter()
    .map(|(text, max_lines)| {
        l.layout(&TextInput {
            align: Align::Center,
            fit: Fit {
                shrink_to: Some(80.0),
                max_lines,
            },
            ..TextInput::new(text, &style, 840.0)
        })
        .report()
        .translated(80.0, 140.0)
    })
    .collect();
    insta::assert_yaml_snapshot!(reports);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn layout_never_panics(
        text in "(\\*\\*|_|\\[a\\]|\\[/\\]|\\\\-|\\\\n|[a-zA-Zäß ]{1,12}|日){0,12}",
        width in prop_oneof![Just(0.0f32), Just(f32::NAN), 1.0f32..2000.0],
        shrink in proptest::option::of(1.0f32..100.0),
        max_lines in proptest::option::of(0u32..4),
        height in proptest::option::of(0.0f32..500.0),
    ) {
        let mut l = layouter();
        let style = headline(64.0);
        let out = l.layout(&TextInput {
            height,
            align: Align::Center,
            valign: VAlign::Center,
            fit: Fit { shrink_to: shrink, max_lines },
            ..TextInput::new(&text, &style, width)
        });
        prop_assert!(out.font_size <= 64.0);
        prop_assert_eq!(out.overflow, out.diagnostics.iter().any(TextDiagnostic::is_overflow));
    }
}
