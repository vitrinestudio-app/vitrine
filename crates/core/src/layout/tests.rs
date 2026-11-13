use std::collections::{BTreeMap, BTreeSet};

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
fn hyphenation_only_when_enabled_and_only_at_shrink_to() {
    let mut l = layouter();
    let style = headline(104.0);
    let text = "**Mischkulturempfehlungen** für jedes Beet";
    let input = |shrink: f32, lang: Option<&'static str>| TextInput {
        fit: Fit {
            shrink_to: Some(shrink),
            max_lines: None,
        },
        hyphenate: lang,
        ..TextInput::new(text, &style, 840.0)
    };
    // Off: emergency break as before.
    let off = l.layout(&input(90.0, None));
    assert!(matches!(
        off.diagnostics.as_slice(),
        [TextDiagnostic::EmergencyBreak { .. }]
    ));
    // On: hyphenated at `shrink_to` instead, with a visible hyphen.
    let on = l.layout(&input(90.0, Some("de")));
    assert!((on.font_size - 90.0).abs() < f32::EPSILON);
    assert_eq!(
        on.diagnostics,
        [TextDiagnostic::Hyphenated {
            words: vec!["Mischkulturempfehlungen".into()]
        }],
        "{:?}",
        texts(&on)
    );
    assert_eq!(on.lines[0].end, LineEnd::SoftHyphen);
    assert!(texts(&on)[0].ends_with('-'));
    assert!(!on.overflow);
    assert!(on.lines.iter().all(|line| line.width <= 840.01));
    // Shrinking that suffices wins: no hyphenation although enabled.
    let shrunk = l.layout(&input(40.0, Some("de")));
    assert!(shrunk.diagnostics.is_empty(), "{:?}", shrunk.diagnostics);
    assert!(texts(&shrunk).iter().all(|t| !t.ends_with('-')));
    // A text that fits is never hyphenated.
    let fits = l.layout(&TextInput {
        hyphenate: Some("de"),
        ..TextInput::new("Plane deine Beete", &style, 1200.0)
    });
    assert!(fits.diagnostics.is_empty(), "{:?}", fits.diagnostics);
    // Languages without patterns behave as if hyphenation were off.
    let ja = l.layout(&input(90.0, Some("ja")));
    assert_eq!(ja.diagnostics, off.diagnostics);
}

#[test]
fn hyphenation_leaves_english_and_manual_soft_hyphens_alone() {
    let mut l = layouter();
    let style = headline(72.0);
    // English text that fits after shrinking is unaffected.
    let en = l.layout(&TextInput {
        fit: Fit {
            shrink_to: Some(56.0),
            max_lines: Some(2),
        },
        hyphenate: Some("en"),
        ..TextInput::new("Plan your beds in minutes", &style, 840.0)
    });
    assert!(en.diagnostics.is_empty(), "{:?}", en.diagnostics);
    // A word with a manual soft hyphen breaks only there.
    let w = width_of(&mut l, "Mischkultur-", &style)
        .max(width_of(&mut l, "empfehlungen", &style))
        .max(width_of(&mut l, "Gartenplanungs-", &style))
        .max(width_of(&mut l, "werkzeug", &style))
        + 1.0;
    let manual = l.layout(&TextInput {
        fit: Fit {
            shrink_to: Some(72.0),
            max_lines: None,
        },
        hyphenate: Some("de"),
        ..TextInput::new("Mischkultur\\-empfehlungen", &style, w)
    });
    assert_eq!(texts(&manual), ["Mischkultur-", "empfehlungen"]);
    assert!(manual.diagnostics.is_empty(), "{:?}", manual.diagnostics);
    // When hyphenation is needed, words with a manual soft hyphen keep it.
    let mixed = l.layout(&TextInput {
        fit: Fit {
            shrink_to: Some(72.0),
            max_lines: None,
        },
        hyphenate: Some("de"),
        ..TextInput::new(
            "Mischkultur\\-empfehlungen Gartenplanungswerkzeug",
            &style,
            w,
        )
    });
    let lines = texts(&mixed);
    assert_eq!(lines[0], "Mischkultur-", "{lines:?}");
    assert_eq!(
        mixed.diagnostics,
        [TextDiagnostic::Hyphenated {
            words: vec!["Gartenplanungswerkzeug".into()]
        }],
        "{lines:?}"
    );
}

#[test]
fn project_font_fallbacks_extend_the_chain() {
    let mut l = layouter();
    let style = headline(60.0);
    let mut fb = BTreeMap::new();
    fb.insert("Inter Display".to_owned(), vec!["Noto Sans JP".to_owned()]);
    let out = l.layout(&TextInput {
        fallbacks: Some(&fb),
        ..TextInput::new("こんにちは", &style, 900.0)
    });
    // The fallback family is not installed: glyphs still missing (the
    // unknown family is reported once per project by validation).
    assert!(
        !out.diagnostics
            .iter()
            .any(|d| matches!(d, TextDiagnostic::UnknownFont { .. }))
    );
    assert_eq!(
        l.fonts()
            .family_chain_with("Inter Display", &fb["Inter Display"]),
        ["Inter Display", "Noto Sans JP", "Inter"]
    );
    assert!(
        out.diagnostics
            .iter()
            .any(|d| matches!(d, TextDiagnostic::MissingGlyphs { .. }))
    );
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

/// A padded span that wraps gets one box per line fragment; runs inside a
/// fragment still merge, and the line layout does not change.
#[test]
fn padded_highlight_per_line_fragment() {
    let mut l = layouter();
    let style = headline(60.0);
    let with = |padding: (f32, f32)| {
        let mut spans = BTreeMap::new();
        spans.insert(
            "tag".to_owned(),
            ResolvedSpanStyle {
                background: Some(Rgba::rgb(20, 20, 20)),
                padding,
                radius: 12.0,
                ..ResolvedSpanStyle::default()
            },
        );
        spans
    };
    let text = "Plan [tag]your **whole** garden in minutes[/] now";
    let plain = with((0.0, 0.0));
    let padded = with((16.0, 6.0));
    let a = l.layout(&TextInput {
        spans: Some(&plain),
        ..TextInput::new(text, &style, 420.0)
    });
    let b = l.layout(&TextInput {
        spans: Some(&padded),
        ..TextInput::new(text, &style, 420.0)
    });
    assert!(a.lines.len() >= 2);
    assert_eq!(a.lines, b.lines, "padding keeps the line breaks");
    assert_eq!(a.highlights.len(), b.highlights.len());
    let lines_with_span = b
        .lines
        .iter()
        .filter(|ln| {
            b.highlights
                .iter()
                .any(|h| h.rect.y < ln.top + ln.height && h.rect.bottom() > ln.top)
        })
        .count();
    assert_eq!(b.highlights.len(), lines_with_span, "one box per fragment");
    for (ha, hb) in a.highlights.iter().zip(&b.highlights) {
        assert!((hb.rect.x - (ha.rect.x - 16.0)).abs() < 0.01);
        assert!((hb.rect.w - (ha.rect.w + 32.0)).abs() < 0.01);
        assert!((hb.rect.h - (ha.rect.h + 12.0)).abs() < 0.01);
        assert!((hb.radius - 12.0).abs() < f32::EPSILON);
    }
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

#[test]
fn span_effects_replace_the_text_effects_without_changing_lines() {
    use crate::model::{StrokeJoin, StrokePosition};
    let mut l = layouter();
    let stroke = |w: f64| ResolvedTextStroke {
        color: Rgba::rgb(255, 255, 255),
        width: w,
        position: StrokePosition::Outside,
        join: StrokeJoin::Round,
    };
    let plain = headline(60.0);
    let style = ResolvedTextStyle {
        effects: TextEffects {
            stroke: Some(stroke(4.0)),
            ..TextEffects::default()
        },
        ..headline(60.0)
    };
    let glow = ResolvedGlow {
        color: Rgba::rgb(255, 0, 0),
        radius: 10.0,
        opacity: 0.5,
        spread: 2.0,
    };
    let mut spans = BTreeMap::new();
    spans.insert(
        "pop".to_owned(),
        ResolvedSpanStyle {
            glow: Some(glow),
            ..ResolvedSpanStyle::default()
        },
    );
    spans.insert(
        "bare".to_owned(),
        ResolvedSpanStyle {
            stroke: Some(stroke(0.0)),
            ..ResolvedSpanStyle::default()
        },
    );
    let text = "Mehr [pop]Ernte[/] im [bare]Beet[/] und [pop]Garten[/]";
    let input = |s| TextInput {
        spans: Some(&spans),
        ..TextInput::new(text, s, 400.0)
    };
    let a = l.layout(&input(&style));
    let b = l.layout(&input(&plain));
    assert_eq!(texts(&a), texts(&b), "effects never change line breaking");
    // [0] = style, [1] = pop (outline inherited + glow), [2] = bare.
    assert_eq!(a.effects.len(), 3);
    assert_eq!(a.effects[0].stroke, Some(stroke(4.0)));
    assert_eq!(a.effects[1].stroke, Some(stroke(4.0)));
    assert_eq!(a.effects[1].glow, Some(glow));
    assert_eq!(a.effects[2].stroke, Some(stroke(0.0)));
    assert!(a.effects[2].is_empty());
    let used: BTreeSet<u16> = a.runs.iter().map(|r| r.effects).collect();
    assert_eq!(used, BTreeSet::from([0, 1, 2]));
    // Outsets: ink = outline 4, visual = glow 2 + 3 · 10.
    assert_eq!(a.effect_outsets(), (4.0, 32.0));
    assert_eq!(b.effect_outsets(), (0.0, 32.0));
}

fn styled(font: &str, weight: u16, italic: bool) -> ResolvedTextStyle {
    ResolvedTextStyle {
        font: font.into(),
        weight,
        italic,
        size: 60.0,
        ..ResolvedTextStyle::default()
    }
}

#[test]
fn bundled_families_are_registered_on_first_use() {
    let mut l = layouter();
    assert!(l.loaded_bundled_families().is_empty(), "nothing at start");
    l.layout(&TextInput::new(
        "Hello",
        &styled("Inter", 400, false),
        900.0,
    ));
    assert_eq!(l.loaded_bundled_families(), ["inter"]);
    let spans = BTreeMap::from([(
        "mono".to_owned(),
        ResolvedSpanStyle {
            font: Some("JetBrains Mono".into()),
            ..ResolvedSpanStyle::default()
        },
    )]);
    let out = l.layout(&TextInput {
        spans: Some(&spans),
        ..TextInput::new(
            "Hello [mono]code[/]",
            &styled("Montserrat", 700, false),
            900.0,
        )
    });
    assert_eq!(
        l.loaded_bundled_families(),
        ["inter", "jetbrains mono", "montserrat"]
    );
    let fams: BTreeSet<&str> = out
        .runs
        .iter()
        .map(|r| l.fonts().face(r.font).unwrap().family.as_str())
        .collect();
    assert_eq!(fams, BTreeSet::from(["JetBrains Mono", "Montserrat"]));
}

/// Every bundled family lays out a line with its own face (no fallback
/// to Inter, no missing glyphs).
#[test]
fn every_bundled_family_lays_out_with_its_own_face() {
    let mut l = layouter();
    for fam in bundled::FAMILIES {
        let out = l.layout(&TextInput::new(
            "PLANT plant 123",
            &styled(fam.family, 400, false),
            2000.0,
        ));
        assert!(
            out.diagnostics.is_empty(),
            "{}: {:?}",
            fam.family,
            out.diagnostics
        );
        for r in &out.runs {
            assert_eq!(l.fonts().face(r.font).unwrap().family, fam.family);
            assert!(
                r.glyphs.iter().all(|g| g.id != 0),
                "{}: .notdef",
                fam.family
            );
        }
    }
}

/// One variable file serves every weight: the `wght` axis follows the
/// requested weight (heavier = wider), clamped to the font's range.
#[test]
fn variable_fonts_follow_the_requested_weight() {
    let mut l = layouter();
    let w = |l: &mut TextLayouter, font: &str, weight: u16| {
        width_of(l, "Headline text", &styled(font, weight, false))
    };
    let (w300, w700, w900) = (
        w(&mut l, "Montserrat", 300),
        w(&mut l, "Montserrat", 700),
        w(&mut l, "Montserrat", 900),
    );
    assert!(w300 < w700 && w700 < w900, "{w300} {w700} {w900}");
    let out = l.layout(&TextInput::new(
        "Bold",
        &styled("Montserrat", 700, false),
        900.0,
    ));
    assert!(!out.runs[0].coords.is_empty(), "wght coordinate set");
    assert!(!out.runs[0].embolden, "no synthetic bold");
    // Lora stops at 700: 900 looks like 700.
    let (l400, l700, l900) = (
        w(&mut l, "Lora", 400),
        w(&mut l, "Lora", 700),
        w(&mut l, "Lora", 900),
    );
    assert!(
        (l900 - l700).abs() < 0.01 && l700 > l400,
        "{l400} {l700} {l900}"
    );
    // Italic picks the italic file, not a synthetic slant.
    let out = l.layout(&TextInput::new(
        "Italic",
        &styled("Playfair Display", 400, true),
        900.0,
    ));
    let face = l.fonts().face(out.runs[0].font).unwrap();
    assert!(face.italic && out.runs[0].skew.is_none());
}

/// Bundled script fonts (ADR 0028): RTL, Indic and Thai texts lay out
/// without missing glyphs or emergency breaks; Indic conjuncts shape into
/// fewer glyphs than characters; the script font joins the chain only for
/// texts that need it (lazy registration stays lazy for Latin).
#[test]
fn complex_scripts_use_the_bundled_script_fonts() {
    let mut l = layouter();
    let style = headline(60.0);
    for (lang, text, family) in [
        ("ar", "خطط حديقتك في دقائق وتابع كل شيء", "Noto Sans Arabic"),
        ("he", "תכננו את הגינה שלכם בדקות", "Noto Sans Hebrew"),
        (
            "hi",
            "मिनटों में अपने बगीचे की योजना बनाएं",
            "Noto Sans Devanagari",
        ),
        (
            "bn",
            "মিনিটের মধ্যে আপনার বাগানের পরিকল্পনা করুন",
            "Noto Sans Bengali",
        ),
        (
            "ta",
            "நிமிடங்களில் உங்கள் தோட்டத்தைத் திட்டமிடுங்கள்",
            "Noto Sans Tamil",
        ),
        (
            "th",
            "วางแผนสวนของคุณได้ภายในไม่กี่นาทีและติดตามทุกอย่าง",
            "Noto Sans Thai",
        ),
    ] {
        let out = l.layout(&TextInput {
            lang: Some(lang),
            ..TextInput::new(text, &style, 600.0)
        });
        assert!(out.diagnostics.is_empty(), "{lang}: {:?}", out.diagnostics);
        assert!(out.lines.len() >= 2, "{lang}: wraps");
        assert!(
            out.lines.iter().all(|x| x.end != LineEnd::Emergency),
            "{lang}"
        );
        let fams: BTreeSet<&str> = out
            .runs
            .iter()
            .filter_map(|r| l.fonts().face(r.font))
            .map(|f| f.family.as_str())
            .collect();
        assert!(fams.contains(family), "{lang}: {fams:?}");
    }
    // Devanagari "क्षत्रिय": conjuncts → fewer glyphs than characters.
    let out = l.layout(&TextInput::new("क्षत्रिय", &style, 600.0));
    let glyphs: usize = out.runs.iter().map(|r| r.glyphs.len()).sum();
    assert!(glyphs < "क्षत्रिय".chars().count(), "{glyphs} glyphs");
    // Latin texts never register a script font.
    let mut fresh = layouter();
    fresh.layout(&TextInput::new("Hello", &style, 600.0));
    assert!(
        fresh
            .loaded_bundled_families()
            .iter()
            .all(|f| !f.starts_with("noto")),
        "{:?}",
        fresh.loaded_bundled_families()
    );
}

/// RTL lines are aligned like LTR ones: the model's alignment is physical
/// and applied once (parley's `Start` alignment right-aligned RTL lines
/// before, and centred text was pushed out of its box).
#[test]
fn rtl_lines_follow_the_physical_alignment() {
    let mut l = layouter();
    let style = headline(60.0);
    let text = "خطط حديقتك في دقائق وتابع كل شيء";
    for align in [Align::Left, Align::Center, Align::Right] {
        let out = l.layout(&TextInput {
            align,
            lang: Some("ar"),
            ..TextInput::new(text, &style, 600.0)
        });
        // Visible glyphs (the hanging trailing space may sit left of 0).
        let xs: Vec<f32> = out
            .runs
            .iter()
            .filter(|r| {
                l.fonts()
                    .face(r.font)
                    .is_some_and(|f| f.family == "Noto Sans Arabic")
            })
            .flat_map(|r| r.glyphs.iter().map(|g| g.x))
            .collect();
        let (min, max) = xs
            .iter()
            .fold((f32::MAX, f32::MIN), |(a, b), x| (a.min(*x), b.max(*x)));
        assert!(min >= -0.5 && max <= 600.0, "{align:?}: {min}..{max}");
        match align {
            Align::Left => assert!(min < 40.0, "left: starts at {min}"),
            Align::Right => assert!(max > 520.0, "right: ends at {max}"),
            Align::Center => assert!(min > 20.0 && max < 580.0, "center: {min}..{max}"),
        }
    }
}

/// Korean breaks between words (spaces), not between syllables.
#[test]
fn korean_keeps_words_together() {
    let mut l = layouter();
    let style = headline(60.0);
    let text = "몇 분 만에 정원을 계획하세요";
    let words: Vec<&str> = text.split(' ').collect();
    let out = l.layout(&TextInput {
        lang: Some("ko"),
        ..TextInput::new(text, &style, 300.0)
    });
    assert!(out.lines.len() > 1);
    for line in texts(&out) {
        for w in line.split_whitespace() {
            assert!(words.contains(&w), "{w:?} split inside a word");
        }
    }
}
