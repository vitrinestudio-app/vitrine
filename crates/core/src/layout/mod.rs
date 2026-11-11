//! Text layout (`docs/KONZEPT.md` §6.1, `docs/adr/0003-text-markup-and-layout.md`):
//! inline markup → shaping, line breaking, soft hyphens, auto-fit →
//! positioned glyph runs in **lu**.
//!
//! Layout never sees pixels: glyph positions are unhinted and in lu, so
//! preview and export break lines identically. `vitrine-render` only
//! rasterises the resulting [`TextLayout`] (glyph outlines as paths).
//!
//! ```
//! use vitrine_core::layout::{Fit, FontLibrary, ResolvedTextStyle, TextInput, TextLayouter};
//!
//! let mut layouter = TextLayouter::new(FontLibrary::bundled());
//! let style = ResolvedTextStyle { size: 72.0, ..ResolvedTextStyle::default() };
//! let layout = layouter.layout(&TextInput {
//!     fit: Fit { shrink_to: Some(40.0), max_lines: Some(2) },
//!     ..TextInput::new("Plane deine Beete **in Minuten**", &style, 840.0)
//! });
//! assert!(!layout.overflow);
//! assert!(layout.lines.len() <= 2);
//! ```

mod engine;
pub mod fonts;
pub mod hyphen;
mod report;
mod scene;

use std::collections::BTreeMap;
use std::fmt;
use std::ops::Range;

use serde::{Deserialize, Serialize};

pub use engine::TextLayouter;
pub use fonts::{FontDirReport, FontError, FontFace, FontId, FontLibrary, FontSource};
pub use report::LayoutReport;
pub use scene::{PlacedText, layout_text, measure_with_bundled_fonts, span_styles};

use crate::model::{Align, ColorRef, Rgba, SpanStyle, TextFit, TextStyle, TextTransform, VAlign};
pub use crate::resolve::{ResolvedGlow, ResolvedShadowLayer, ResolvedTextStroke, TextEffects};

/// Auto-fit step in lu: chosen sizes are `shrink_to + k · FIT_STEP`.
pub const FIT_STEP: f32 = 0.25;

/// A text style with all values resolved (no `@colour` references, no
/// missing fields). Sizes in lu.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedTextStyle {
    pub font: String,
    pub weight: u16,
    pub italic: bool,
    pub size: f32,
    /// Factor of the font size.
    pub line_height: f32,
    /// Letter spacing in em.
    pub tracking: f32,
    pub color: Rgba,
    pub transform: TextTransform,
    /// Outline, glow and shadow of the text (spans may replace them). They
    /// never change line breaking.
    pub effects: TextEffects,
}

impl Default for ResolvedTextStyle {
    fn default() -> Self {
        Self {
            font: fonts::DEFAULT_FAMILY.to_owned(),
            weight: 400,
            italic: false,
            size: 48.0,
            line_height: 1.2,
            tracking: 0.0,
            color: Rgba::rgb(0, 0, 0),
            transform: TextTransform::None,
            effects: TextEffects::default(),
        }
    }
}

fn resolve_color(c: Option<&ColorRef>, colors: &BTreeMap<String, Rgba>) -> Option<Rgba> {
    match c? {
        ColorRef::Rgba(c) => Some(*c),
        ColorRef::Named(n) => colors.get(n).copied(),
    }
}

impl ResolvedTextStyle {
    /// Resolves a model style; missing values (and unknown colour
    /// references, which `core::validate` reports) fall back to the defaults.
    /// Shadow presets are not resolved here (no preset table): use
    /// `resolve::StyleResolver` for complete styles.
    pub fn from_model(style: &TextStyle, colors: &BTreeMap<String, Rgba>) -> Self {
        let d = Self::default();
        let black = Rgba::rgb(0, 0, 0);
        Self {
            font: style.font.clone().unwrap_or(d.font),
            weight: style.weight.unwrap_or(d.weight),
            italic: style.italic.unwrap_or(d.italic),
            size: style.size.map_or(d.size, |v| v as f32),
            line_height: style.line_height.map_or(d.line_height, |v| v as f32),
            tracking: style.tracking.map_or(d.tracking, |v| v as f32),
            color: resolve_color(style.color.as_ref(), colors).unwrap_or(d.color),
            transform: style.transform.unwrap_or(d.transform),
            effects: TextEffects {
                stroke: style.stroke.as_ref().map(|s| ResolvedTextStroke {
                    color: resolve_color(Some(&s.color), colors).unwrap_or(black),
                    width: s.width.max(0.0),
                    position: s.position.unwrap_or_default(),
                    join: s.join.unwrap_or_default(),
                }),
                glow: style.glow.as_ref().map(|g| ResolvedGlow {
                    color: resolve_color(Some(&g.color), colors).unwrap_or(black),
                    radius: g.radius.max(0.0),
                    opacity: g.opacity.unwrap_or(1.0).clamp(0.0, 1.0),
                    spread: g.spread.unwrap_or(0.0).max(0.0),
                }),
                shadow: Vec::new(),
            },
        }
    }
}

/// A resolved span style (`[name]…[/]`); `None` = inherit.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ResolvedSpanStyle {
    pub font: Option<String>,
    pub weight: Option<u16>,
    pub italic: Option<bool>,
    pub color: Option<Rgba>,
    /// Highlight colour drawn behind the span.
    pub background: Option<Rgba>,
    /// Replace the text's outline / glow / shadow within the span.
    pub stroke: Option<ResolvedTextStroke>,
    pub glow: Option<ResolvedGlow>,
    pub shadow: Option<Vec<ResolvedShadowLayer>>,
}

impl ResolvedSpanStyle {
    /// Like [`ResolvedTextStyle::from_model`]: outline and glow are
    /// resolved, shadow presets are not.
    pub fn from_model(style: &SpanStyle, colors: &BTreeMap<String, Rgba>) -> Self {
        let base = ResolvedTextStyle::from_model(
            &TextStyle {
                stroke: style.stroke.clone(),
                glow: style.glow.clone(),
                ..TextStyle::default()
            },
            colors,
        );
        Self {
            font: style.font.clone(),
            weight: style.weight,
            italic: style.italic,
            color: resolve_color(style.color.as_ref(), colors),
            background: resolve_color(style.background.as_ref(), colors),
            stroke: base.effects.stroke,
            glow: base.effects.glow,
            shadow: None,
        }
    }

    /// Does the span replace any text effect?
    pub fn has_effects(&self) -> bool {
        self.stroke.is_some() || self.glow.is_some() || self.shadow.is_some()
    }
}

/// Auto-fit settings (`fit: { shrink_to, max_lines }`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Fit {
    /// Smallest allowed size in lu; `None` = no shrinking.
    pub shrink_to: Option<f32>,
    pub max_lines: Option<u32>,
}

impl From<&TextFit> for Fit {
    fn from(f: &TextFit) -> Self {
        Self {
            shrink_to: f.shrink_to.map(|v| v as f32),
            max_lines: f.max_lines,
        }
    }
}

/// Input of [`TextLayouter::layout`]. All lengths in lu.
#[derive(Clone, Debug)]
pub struct TextInput<'a> {
    /// The text for one language, with inline markup (§3.5).
    pub markup: &'a str,
    pub style: &'a ResolvedTextStyle,
    /// Named span styles; unknown names are reported and ignored.
    pub spans: Option<&'a BTreeMap<String, ResolvedSpanStyle>>,
    /// Box width.
    pub width: f32,
    /// Box height; `None` = the box grows with the text.
    pub height: Option<f32>,
    pub align: Align,
    /// Vertical alignment inside a box with `height`.
    pub valign: VAlign,
    pub fit: Fit,
    /// Language of the text when automatic hyphenation is on (used only
    /// when the text does not fit at `fit.shrink_to`, ADR 0009); `None` =
    /// off. Languages without bundled patterns are not hyphenated.
    pub hyphenate: Option<&'a str>,
    /// Font fallbacks of the project (`styles.fonts`): family → families
    /// tried for missing characters before the library's own fallbacks.
    pub fallbacks: Option<&'a BTreeMap<String, Vec<String>>>,
}

impl<'a> TextInput<'a> {
    /// Left/top aligned text without spans, height or auto-fit.
    pub fn new(markup: &'a str, style: &'a ResolvedTextStyle, width: f32) -> Self {
        Self {
            markup,
            style,
            spans: None,
            width,
            height: None,
            align: Align::Left,
            valign: VAlign::Top,
            fit: Fit::default(),
            hyphenate: None,
            fallbacks: None,
        }
    }
}

/// Axis-aligned rectangle in lu.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

/// A glyph: id in its run's font and baseline origin in lu, relative to the
/// box's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

/// Glyphs sharing font, size and paint.
#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub font: FontId,
    /// Font size in lu.
    pub size: f32,
    pub color: Rgba,
    /// Index into [`TextLayout::effects`].
    pub effects: u16,
    /// Synthetic oblique angle in degrees (italic requested, no italic face).
    pub skew: Option<f32>,
    /// Synthetic bold requested (weight far above the face's).
    pub embolden: bool,
    /// Normalised variation coordinates (`F2Dot14` bits) for variable fonts.
    pub coords: Vec<i16>,
    pub glyphs: Vec<Glyph>,
}

/// Background highlight of a span style, drawn behind the glyphs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Highlight {
    pub rect: Rect,
    pub color: Rgba,
    /// Index into [`TextLayout::effects`] (only its shadow applies).
    pub effects: u16,
}

/// Why a line ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnd {
    /// Regular break at a space or other break opportunity.
    Wrap,
    /// Break at a soft hyphen; a visible hyphen is drawn.
    SoftHyphen,
    /// Hard break (`\n`).
    Explicit,
    /// Emergency break inside a word, without hyphen.
    Emergency,
    /// Last line.
    End,
}

/// One laid-out line.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    /// Byte range in [`TextLayout::text`].
    pub range: Range<usize>,
    /// Left edge of the visible content (after alignment).
    pub x: f32,
    /// Top of the line box.
    pub top: f32,
    pub baseline: f32,
    pub height: f32,
    /// Visible width (without trailing spaces, with the soft-hyphen glyph).
    pub width: f32,
    pub end: LineEnd,
}

/// Problems found while laying out a text. `core::validate` turns them into
/// warnings/errors for a language and page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TextDiagnostic {
    /// Unbalanced inline markup, rendered literally.
    Markup { marker: String, offset: usize },
    /// `[name]` does not name a span style.
    UnknownSpanStyle { name: String },
    /// Font family not in the font library; the default family is used.
    UnknownFont { family: String },
    /// Characters without a glyph in any font of the family chain.
    MissingGlyphs { text: String },
    /// A word had to be broken without hyphen (`line` is 0-based).
    EmergencyBreak { word: String, line: usize },
    /// Automatic hyphenation broke these words (the text did not fit at
    /// `shrink_to` otherwise).
    Hyphenated { words: Vec<String> },
    /// More lines than `max_lines` even at `shrink_to`.
    OverflowLines { lines: usize, max_lines: u32 },
    /// Taller than the box even at `shrink_to`.
    OverflowHeight { height: f32, max_height: f32 },
    /// A line is wider than the box (a single glyph cluster too wide).
    OverflowWidth { width: f32, max_width: f32 },
}

impl TextDiagnostic {
    /// Does this diagnostic mean the text does not fit its box?
    pub fn is_overflow(&self) -> bool {
        matches!(
            self,
            Self::OverflowLines { .. } | Self::OverflowHeight { .. } | Self::OverflowWidth { .. }
        )
    }
}

impl fmt::Display for TextDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Markup { marker, offset } => {
                write!(f, "unbalanced markup `{marker}` at byte {offset}")
            }
            Self::UnknownSpanStyle { name } => write!(f, "unknown span style `{name}`"),
            Self::UnknownFont { family } => write!(f, "font family `{family}` not available"),
            Self::MissingGlyphs { text } => write!(f, "no glyphs for `{text}`"),
            Self::EmergencyBreak { word, line } => write!(
                f,
                "`{word}` broken without hyphen in line {} (add a soft hyphen `\\-`)",
                line + 1
            ),
            Self::Hyphenated { words } => {
                write!(f, "hyphenated automatically: {}", words.join(", "))
            }
            Self::OverflowLines { lines, max_lines } => {
                write!(f, "text needs {lines} lines, max_lines is {max_lines}")
            }
            Self::OverflowHeight { height, max_height } => {
                write!(f, "text is {height} lu high, box is {max_height} lu")
            }
            Self::OverflowWidth { width, max_width } => {
                write!(f, "line is {width} lu wide, box is {max_width} lu")
            }
        }
    }
}

/// Result of laying out one text element for one language.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLayout {
    /// The laid-out plain text (markup removed, text transform applied).
    pub text: String,
    /// The box: origin (0, 0), given width, given or content height.
    pub r#box: Rect,
    /// Union of the line boxes (after alignment).
    pub content: Rect,
    /// Chosen font size in lu (≤ style size, ≥ `shrink_to` unless overflowing).
    pub font_size: f32,
    pub lines: Vec<TextLine>,
    pub runs: Vec<GlyphRun>,
    pub highlights: Vec<Highlight>,
    /// Effect sets referenced by runs and highlights: `[0]` is the text
    /// style's, further entries come from span styles that replace an
    /// effect. Never empty.
    pub effects: Vec<TextEffects>,
    /// The text does not fit its box/`max_lines` even at `shrink_to`.
    pub overflow: bool,
    pub diagnostics: Vec<TextDiagnostic>,
}

impl TextLayout {
    /// Text of a line as displayed (soft hyphens removed, `-` added when
    /// broken at one).
    pub fn line_text(&self, line: &TextLine) -> String {
        let mut s: String = self.text[line.range.clone()]
            .trim_end()
            .chars()
            .filter(|&c| c != crate::markup::SOFT_HYPHEN)
            .collect();
        if line.end == LineEnd::SoftHyphen {
            s.push('-');
        }
        s
    }

    /// How far outline / glow / shadow reach beyond the glyphs, lu:
    /// `(ink, visual)` where `ink` counts only the hard-edged outline.
    pub fn effect_outsets(&self) -> (f32, f32) {
        self.effects.iter().fold((0.0, 0.0), |(i, v), e| {
            (
                i.max(e.ink_outset() as f32),
                v.max(e.visual_outset() as f32),
            )
        })
    }

    /// Serializable summary (§8 `layout_report`, validation).
    pub fn report(&self) -> LayoutReport {
        LayoutReport::from_layout(self)
    }
}

/// CSS `bolder`: the weight used for `**bold**` relative to `weight`.
pub fn bolder(weight: u16) -> u16 {
    match weight {
        0..350 => 400,
        350..550 => 700,
        _ => 900,
    }
}

#[cfg(test)]
mod tests;
