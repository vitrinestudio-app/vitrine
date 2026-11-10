//! Style tokens → concrete values: `@colour` refs, fills, text and span
//! styles, shadow presets.

use std::collections::BTreeMap;

use super::Diags;
use super::scene::{
    DiagnosticCode, ResolvedFill, ResolvedGlow, ResolvedShadowLayer, ResolvedSpanStyle,
    ResolvedStop, ResolvedStroke, ResolvedTextStroke, ResolvedTextStyle,
};
use crate::geometry::Vec2;
use crate::model::{
    ColorRef, Fill, Glow, ImageFillMode, Rgba, ShadowLayer, Stroke, StrokeJoin, Styles, TextProps,
    TextTransform,
};

/// Shown for colour references that do not resolve (loud on purpose).
pub const UNKNOWN_COLOR: Rgba = Rgba::rgb(0xFF, 0x00, 0xFF);

/// Font used when no style sets one (bundled, ADR 0001).
pub const DEFAULT_FONT: &str = "Inter";

/// Text style values used where neither the style nor the element sets one.
pub fn default_text_style() -> ResolvedTextStyle {
    ResolvedTextStyle {
        font: DEFAULT_FONT.to_owned(),
        weight: 400,
        italic: false,
        size: 48.0,
        line_height: 1.2,
        tracking: 0.0,
        color: Rgba::rgb(0, 0, 0),
        transform: TextTransform::None,
        shadow: Vec::new(),
        stroke: None,
        glow: None,
    }
}

/// Built-in definition of the `soft`, `hard` and `none` shadow presets
/// (used unless `styles.shadows` redefines them). `soft` is the two-layer
/// shadow of the Phase 0 spike (ambient + contact).
pub fn builtin_shadow(name: &str) -> Option<Vec<ResolvedShadowLayer>> {
    let layer = |a: u8, blur, y| ResolvedShadowLayer {
        color: Rgba {
            r: 0,
            g: 0,
            b: 0,
            a,
        },
        blur,
        x: 0.0,
        y,
    };
    match name {
        "soft" => Some(vec![layer(0x6B, 48.0, 46.0), layer(0x4D, 10.0, 10.0)]),
        "hard" => Some(vec![layer(0x59, 4.0, 16.0)]),
        "none" => Some(Vec::new()),
        _ => None,
    }
}

/// Resolves style tokens against the project styles.
#[derive(Debug, Clone, Copy)]
pub struct StyleResolver<'a> {
    pub styles: &'a Styles,
}

impl<'a> StyleResolver<'a> {
    pub fn new(styles: &'a Styles) -> Self {
        Self { styles }
    }

    pub fn color(&self, c: &ColorRef, d: &mut Diags) -> Rgba {
        match c {
            ColorRef::Rgba(c) => *c,
            ColorRef::Named(n) => self.styles.colors.get(n).copied().unwrap_or_else(|| {
                d.warn(
                    DiagnosticCode::UnknownColor,
                    format!("unknown colour `@{n}`"),
                );
                UNKNOWN_COLOR
            }),
        }
    }

    pub fn fill(&self, f: &Fill, d: &mut Diags) -> ResolvedFill {
        let stops = |stops: &[crate::model::GradientStop], d: &mut Diags| {
            stops
                .iter()
                .map(|s| ResolvedStop {
                    color: self.color(&s.color, d),
                    pos: s.pos,
                })
                .collect()
        };
        match f {
            Fill::Color { color } => ResolvedFill::Color {
                color: self.color(color, d),
            },
            Fill::LinearGradient { angle, stops: s } => ResolvedFill::LinearGradient {
                angle: *angle,
                stops: stops(s, d),
            },
            Fill::RadialGradient {
                center,
                radius,
                stops: s,
            } => ResolvedFill::RadialGradient {
                center: center.map_or(Vec2::new(0.5, 0.5), |c| Vec2::new(c.x, c.y)),
                radius: radius.unwrap_or(0.5),
                stops: stops(s, d),
            },
            Fill::Image { asset, fit, focus } => ResolvedFill::Image {
                asset: asset.clone(),
                fit: fit.unwrap_or(ImageFillMode::Cover),
                focus: focus.map_or(Vec2::new(0.5, 0.5), |c| Vec2::new(c.x, c.y)),
            },
        }
    }

    pub fn stroke(&self, s: &Stroke, d: &mut Diags) -> ResolvedStroke {
        ResolvedStroke {
            color: self.color(&s.color, d),
            width: s.width,
            // Shapes keep sharp corners unless asked otherwise.
            join: s.join.unwrap_or(StrokeJoin::Miter),
        }
    }

    /// A text outline (lengths clamped to ≥ 0).
    pub fn text_stroke(&self, s: &Stroke, d: &mut Diags) -> ResolvedTextStroke {
        ResolvedTextStroke {
            color: self.color(&s.color, d),
            width: s.width.max(0.0),
            position: s.position.unwrap_or_default(),
            join: s.join.unwrap_or_default(),
        }
    }

    /// A text glow (lengths clamped to ≥ 0, opacity to 0..1).
    pub fn glow(&self, g: &Glow, d: &mut Diags) -> ResolvedGlow {
        ResolvedGlow {
            color: self.color(&g.color, d),
            radius: g.radius.max(0.0),
            opacity: g.opacity.unwrap_or(1.0).clamp(0.0, 1.0),
            spread: g.spread.unwrap_or(0.0).max(0.0),
        }
    }

    /// Layers of a shadow preset (`None` = no shadow). Project presets win
    /// over the built-in ones.
    pub fn shadow(&self, name: Option<&str>, d: &mut Diags) -> Vec<ResolvedShadowLayer> {
        let Some(name) = name else {
            return Vec::new();
        };
        if let Some(p) = self.styles.shadows.get(name) {
            return p.layers.iter().map(|l| self.shadow_layer(l, d)).collect();
        }
        builtin_shadow(name).unwrap_or_else(|| {
            d.warn(
                DiagnosticCode::UnknownShadow,
                format!("unknown shadow preset `{name}`"),
            );
            Vec::new()
        })
    }

    fn shadow_layer(self, l: &ShadowLayer, d: &mut Diags) -> ResolvedShadowLayer {
        ResolvedShadowLayer {
            color: self.color(&l.color, d),
            blur: l.blur,
            x: l.x,
            y: l.y,
        }
    }

    /// Text style of a text element: defaults ← named style ← element
    /// overrides (`color`, `font_size`, `stroke`, `glow`, `shadow`).
    pub fn text_style(&self, t: &TextProps, d: &mut Diags) -> ResolvedTextStyle {
        let mut out = default_text_style();
        if let Some(name) = &t.style {
            match self.styles.text.get(name) {
                Some(s) => {
                    if let Some(v) = &s.font {
                        out.font.clone_from(v);
                    }
                    out.weight = s.weight.unwrap_or(out.weight);
                    out.italic = s.italic.unwrap_or(out.italic);
                    out.size = s.size.unwrap_or(out.size);
                    out.line_height = s.line_height.unwrap_or(out.line_height);
                    out.tracking = s.tracking.unwrap_or(out.tracking);
                    if let Some(c) = &s.color {
                        out.color = self.color(c, d);
                    }
                    out.transform = s.transform.unwrap_or(out.transform);
                    out.shadow = self.shadow(s.shadow.as_deref(), d);
                    out.stroke = s.stroke.as_ref().map(|v| self.text_stroke(v, d));
                    out.glow = s.glow.as_ref().map(|v| self.glow(v, d));
                }
                None => d.warn(
                    DiagnosticCode::UnknownTextStyle,
                    format!("unknown text style `{name}`"),
                ),
            }
        }
        if let Some(c) = &t.color {
            out.color = self.color(c, d);
        }
        if let Some(s) = t.font_size {
            out.size = s;
        }
        if let Some(s) = &t.stroke {
            out.stroke = Some(self.text_stroke(s, d));
        }
        if let Some(g) = &t.glow {
            out.glow = Some(self.glow(g, d));
        }
        if t.shadow.is_some() {
            out.shadow = self.shadow(t.shadow.as_deref(), d);
        }
        out
    }

    /// Is automatic hyphenation on for a text element (element value,
    /// else its style's, else off)?
    pub fn hyphenate(&self, t: &TextProps) -> bool {
        t.hyphenate
            .or_else(|| {
                t.style
                    .as_ref()
                    .and_then(|n| self.styles.text.get(n))
                    .and_then(|s| s.hyphenate)
            })
            .unwrap_or(false)
    }

    /// All span styles with concrete colours.
    pub fn spans(&self, d: &mut Diags) -> BTreeMap<String, ResolvedSpanStyle> {
        self.styles
            .spans
            .iter()
            .map(|(name, s)| {
                (
                    name.clone(),
                    ResolvedSpanStyle {
                        font: s.font.clone(),
                        weight: s.weight,
                        italic: s.italic,
                        color: s.color.as_ref().map(|c| self.color(c, d)),
                        background: s.background.as_ref().map(|c| self.color(c, d)),
                        stroke: s.stroke.as_ref().map(|v| self.text_stroke(v, d)),
                        glow: s.glow.as_ref().map(|v| self.glow(v, d)),
                        shadow: s.shadow.as_deref().map(|name| self.shadow(Some(name), d)),
                    },
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ShadowPreset, StringKey, TextStyle};

    fn styles() -> Styles {
        let mut s = Styles::default();
        s.colors.insert("text".into(), Rgba::rgb(1, 2, 3));
        s.text.insert(
            "headline".into(),
            TextStyle {
                weight: Some(800),
                size: Some(72.0),
                color: Some(ColorRef::parse("@text").unwrap()),
                shadow: Some("soft".into()),
                ..TextStyle::default()
            },
        );
        s.shadows.insert(
            "soft".into(),
            ShadowPreset {
                layers: vec![ShadowLayer {
                    color: ColorRef::parse("@text").unwrap(),
                    blur: 5.0,
                    x: 1.0,
                    y: 2.0,
                }],
            },
        );
        s
    }

    fn text(style: Option<&str>) -> TextProps {
        TextProps {
            text: StringKey::new("k"),
            style: style.map(String::from),
            r#box: None,
            align: None,
            valign: None,
            fit: None,
            color: None,
            font_size: None,
            stroke: None,
            glow: None,
            shadow: None,
            hyphenate: None,
            langs: BTreeMap::new(),
        }
    }

    #[test]
    fn text_style_cascade() {
        let s = styles();
        let r = StyleResolver::new(&s);
        let mut d = Diags::default();
        let mut t = text(Some("headline"));
        let st = r.text_style(&t, &mut d);
        assert_eq!(
            (st.weight, st.size, st.color),
            (800, 72.0, Rgba::rgb(1, 2, 3))
        );
        assert_eq!(st.font, DEFAULT_FONT);
        assert_eq!(st.shadow.len(), 1, "project preset redefines `soft`");
        t.font_size = Some(50.0);
        t.color = Some(ColorRef::parse("#FFFFFF").unwrap());
        let st = r.text_style(&t, &mut d);
        assert_eq!((st.size, st.color), (50.0, Rgba::rgb(255, 255, 255)));
        assert_eq!(d.items.len(), 0);
        let st = r.text_style(&text(Some("nope")), &mut d);
        assert_eq!(st, default_text_style());
        assert_eq!(d.items[0].code, DiagnosticCode::UnknownTextStyle);
    }

    #[test]
    fn text_effects_cascade_style_element_span() {
        use crate::model::{Glow, SpanStyle, StrokeJoin, StrokePosition};
        let mut s = styles();
        let headline = s.text.get_mut("headline").unwrap();
        headline.stroke = Some(Stroke::new(ColorRef::parse("@text").unwrap(), 3.0));
        headline.glow = Some(Glow {
            color: ColorRef::parse("#FF000080").unwrap(),
            radius: 10.0,
            opacity: None,
            spread: Some(-1.0),
        });
        s.spans.insert(
            "pop".into(),
            SpanStyle {
                stroke: Some(Stroke {
                    position: Some(StrokePosition::Center),
                    join: Some(StrokeJoin::Bevel),
                    ..Stroke::new(ColorRef::parse("@nope").unwrap(), 2.0)
                }),
                shadow: Some("none".into()),
                ..SpanStyle::default()
            },
        );
        let r = StyleResolver::new(&s);
        let mut d = Diags::default();
        let mut t = text(Some("headline"));
        let st = r.text_style(&t, &mut d);
        let stroke = st.stroke.unwrap();
        assert_eq!(
            (stroke.color, stroke.width, stroke.position, stroke.join),
            (
                Rgba::rgb(1, 2, 3),
                3.0,
                StrokePosition::Outside,
                StrokeJoin::Round
            )
        );
        let glow = st.glow.unwrap();
        assert_eq!((glow.opacity, glow.spread), (1.0, 0.0), "defaults, clamped");
        assert_eq!(st.shadow.len(), 1);
        // Element overrides win over the style.
        t.stroke = Some(Stroke::new(ColorRef::parse("#FFFFFF").unwrap(), 0.0));
        t.shadow = Some("none".into());
        let st = r.text_style(&t, &mut d);
        assert!(!st.stroke.unwrap().is_visible());
        assert_eq!(st.shadow.len(), 0);
        assert!(st.glow.is_some(), "not overridden");
        assert_eq!(d.items.len(), 0);
        // Span styles carry their own effects; unknown colours are reported.
        let spans = r.spans(&mut d);
        let pop = &spans["pop"];
        assert_eq!(pop.stroke.unwrap().join, StrokeJoin::Bevel);
        assert_eq!(pop.stroke.unwrap().color, UNKNOWN_COLOR);
        assert_eq!(pop.shadow.as_deref(), Some(&[][..]));
        assert_eq!(d.items[0].code, DiagnosticCode::UnknownColor);
    }

    #[test]
    fn colors_and_shadows() {
        let s = styles();
        let r = StyleResolver::new(&s);
        let mut d = Diags::default();
        assert_eq!(
            r.color(&ColorRef::parse("@nope").unwrap(), &mut d),
            UNKNOWN_COLOR
        );
        assert_eq!(r.shadow(Some("hard"), &mut d).len(), 1);
        assert_eq!(r.shadow(Some("none"), &mut d).len(), 0);
        assert_eq!(r.shadow(Some("weird"), &mut d).len(), 0);
        let codes: Vec<_> = d.items.iter().map(|x| x.code).collect();
        assert_eq!(
            codes,
            [DiagnosticCode::UnknownColor, DiagnosticCode::UnknownShadow]
        );
    }
}
