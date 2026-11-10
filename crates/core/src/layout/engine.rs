//! parley-based implementation of [`TextLayouter`].

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use parley::fontique::{Collection, CollectionOptions, SourceCache};
use parley::{
    Alignment, AlignmentOptions, BreakReason, FontContext, FontFamily, FontFamilyName, FontStyle,
    FontWeight, Layout, LayoutContext, LineHeight, OverflowWrap, PositionedLayoutItem,
    StyleProperty, YieldData,
};
use skrifa::instance::{LocationRef, Size};
use skrifa::raw::types::F2Dot14;
use skrifa::{FontRef, MetadataProvider};

use super::{
    Glyph, GlyphRun, Highlight, LineEnd, Rect, ResolvedSpanStyle, TextDiagnostic, TextEffects,
    TextInput, TextLayout, TextLine, bolder, fonts::FontId, fonts::FontLibrary,
};
use crate::markup::{self, SOFT_HYPHEN};
use crate::model::{Align, Rgba, TextTransform, VAlign};

/// Tolerance for width/height comparisons in lu.
const EPS: f32 = 0.01;

/// Paint of a text range (the parley "brush").
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Paint {
    color: [u8; 4],
    background: Option<[u8; 4]>,
    /// Index into [`Prepared::effects`].
    fx: u16,
}

fn rgba(c: [u8; 4]) -> Rgba {
    Rgba {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

fn arr(c: Rgba) -> [u8; 4] {
    [c.r, c.g, c.b, c.a]
}

/// Style of one span after resolving markup, span styles and fonts.
#[derive(Debug, Clone, PartialEq)]
struct RangeStyle {
    family: Vec<String>,
    weight: u16,
    italic: bool,
    paint: Paint,
}

/// Markup parsed and resolved once per [`TextLayouter::layout`] call.
#[derive(Debug)]
struct Prepared {
    text: String,
    base: RangeStyle,
    ranges: Vec<(std::ops::Range<usize>, RangeStyle)>,
    /// Effect sets: `[0]` from the text style, more from span styles.
    effects: Vec<TextEffects>,
    size: f32,
    line_height: f32,
    tracking: f32,
}

fn family_property(chain: &[String]) -> StyleProperty<'static, Paint> {
    StyleProperty::FontFamily(FontFamily::List(Cow::Owned(
        chain
            .iter()
            .map(|f| FontFamilyName::Named(Cow::Owned(f.clone())))
            .collect(),
    )))
}

fn transform(s: &str, t: TextTransform) -> Cow<'_, str> {
    match t {
        TextTransform::None => Cow::Borrowed(s),
        TextTransform::Uppercase => Cow::Owned(s.to_uppercase()),
        TextTransform::Lowercase => Cow::Owned(s.to_lowercase()),
    }
}

#[allow(clippy::too_many_lines, reason = "one pass over the parsed spans")]
fn prepare(
    input: &TextInput<'_>,
    fonts: &FontLibrary,
    diags: &mut Vec<TextDiagnostic>,
) -> Prepared {
    let style = input.style;
    let parsed = markup::parse(input.markup);
    diags.extend(parsed.diagnostics.iter().map(|d| TextDiagnostic::Markup {
        marker: d.marker.clone(),
        offset: d.offset,
    }));
    let mut unknown_fonts = BTreeSet::new();
    let extra = |family: &str| -> &[String] {
        input
            .fallbacks
            .and_then(|m| {
                m.iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case(family))
                    .map(|(_, v)| v.as_slice())
            })
            .unwrap_or(&[])
    };
    // Missing fallback families are reported once per project by
    // `core::validate`, not per text.
    let mut chain = |family: &str| {
        if !fonts.has_family(family) {
            unknown_fonts.insert(family.to_owned());
        }
        fonts.family_chain_with(family, extra(family))
    };
    let base = RangeStyle {
        family: chain(&style.font),
        weight: style.weight,
        italic: style.italic,
        paint: Paint {
            color: arr(style.color),
            background: None,
            fx: 0,
        },
    };
    let mut effects = vec![style.effects.clone()];
    let mut unknown_spans = BTreeSet::new();
    let empty = BTreeMap::new();
    let span_styles = input.spans.unwrap_or(&empty);
    let mut text = String::new();
    let mut ranges = Vec::new();
    for span in &parsed.spans {
        let named: Option<&ResolvedSpanStyle> = span.style.as_ref().and_then(|n| {
            let s = span_styles.get(n);
            if s.is_none() {
                unknown_spans.insert(n.clone());
            }
            s
        });
        let mut rs = base.clone();
        if let Some(n) = named {
            if let Some(f) = &n.font {
                rs.family = chain(f);
            }
            rs.weight = n.weight.unwrap_or(rs.weight);
            rs.italic = n.italic.unwrap_or(rs.italic);
            if let Some(c) = n.color {
                rs.paint.color = arr(c);
            }
            rs.paint.background = n.background.map(arr);
            if n.has_effects() {
                let fx = TextEffects {
                    stroke: n.stroke.or(style.effects.stroke),
                    glow: n.glow.or(style.effects.glow),
                    shadow: n
                        .shadow
                        .clone()
                        .unwrap_or_else(|| style.effects.shadow.clone()),
                };
                let i = effects.iter().position(|e| *e == fx).unwrap_or_else(|| {
                    effects.push(fx);
                    effects.len() - 1
                });
                rs.paint.fx = u16::try_from(i).unwrap_or(0);
            }
        }
        if span.bold {
            rs.weight = bolder(rs.weight);
        }
        rs.italic |= span.italic;
        let start = text.len();
        text.push_str(&transform(parsed.span_text(span), style.transform));
        ranges.push((start..text.len(), rs));
    }
    diags.extend(
        unknown_spans
            .into_iter()
            .map(|name| TextDiagnostic::UnknownSpanStyle { name }),
    );
    diags.extend(
        unknown_fonts
            .into_iter()
            .map(|family| TextDiagnostic::UnknownFont { family }),
    );
    let finite_pos = |v: f32, d: f32| if v.is_finite() && v > 0.0 { v } else { d };
    Prepared {
        text,
        base,
        ranges,
        effects,
        size: finite_pos(style.size, 48.0),
        line_height: finite_pos(style.line_height, 1.2),
        tracking: if style.tracking.is_finite() {
            style.tracking
        } else {
            0.0
        },
    }
}

/// `p` with soft hyphens at the pattern positions of its words; span
/// ranges are moved accordingly.
fn hyphenate_prepared(
    p: &Prepared,
    h: &super::hyphen::Hyphenator,
) -> (Prepared, Vec<super::hyphen::Inserted>) {
    let (text, inserted, map) = super::hyphen::hyphenate_text(h, &p.text);
    let ranges = p
        .ranges
        .iter()
        .map(|(r, s)| {
            // A soft hyphen inserted right at a range start belongs to it.
            let start = if r.start == 0 { 0 } else { map[r.start] };
            let start = inserted
                .iter()
                .find(|i| i.at + SOFT_HYPHEN.len_utf8() == start)
                .map_or(start, |i| i.at);
            let end = inserted
                .iter()
                .find(|i| i.at + SOFT_HYPHEN.len_utf8() == map[r.end])
                .map_or(map[r.end], |i| i.at);
            (start..end, s.clone())
        })
        .collect();
    (
        Prepared {
            text,
            base: p.base.clone(),
            ranges,
            effects: p.effects.clone(),
            size: p.size,
            line_height: p.line_height,
            tracking: p.tracking,
        },
        inserted,
    )
}

/// Layout at one font size.
#[derive(Debug, Clone)]
struct Attempt {
    size: f32,
    lines: Vec<TextLine>,
    runs: Vec<GlyphRun>,
    highlights: Vec<Highlight>,
    height: f32,
    max_width: f32,
    emergency: Vec<(String, usize)>,
    missing: BTreeSet<char>,
}

/// Hyphen glyph drawn at a soft-hyphen line break.
#[derive(Debug, Clone, Copy)]
struct Hyphen {
    font: FontId,
    glyph: u32,
    advance: f32,
}

/// Lays out texts with the fonts of a [`FontLibrary`] (system fonts are
/// never used). Keep one per thread and reuse it: it caches shaping data.
pub struct TextLayouter {
    fonts: FontLibrary,
    font_cx: FontContext,
    layout_cx: LayoutContext<Paint>,
    ids: HashMap<(u64, u32), FontId>,
}

impl std::fmt::Debug for TextLayouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextLayouter")
            .field("fonts", &self.fonts)
            .finish_non_exhaustive()
    }
}

impl TextLayouter {
    pub fn new(fonts: FontLibrary) -> Self {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let mut ids = HashMap::new();
        let mut registered = BTreeSet::new();
        for (i, face) in fonts.faces().iter().enumerate() {
            ids.insert((face.blob.id(), face.index), FontId(i as u32));
            if registered.insert(face.blob.id()) {
                collection.register_fonts(face.blob.clone(), None);
            }
        }
        Self {
            fonts,
            font_cx: FontContext {
                collection,
                source_cache: SourceCache::default(),
            },
            layout_cx: LayoutContext::new(),
            ids,
        }
    }

    pub fn fonts(&self) -> &FontLibrary {
        &self.fonts
    }

    /// Lays out a text: markup, shaping, line breaking with soft hyphens,
    /// auto-fit and alignment. Never panics on any input.
    pub fn layout(&mut self, input: &TextInput<'_>) -> TextLayout {
        let mut diagnostics = Vec::new();
        let p = prepare(input, &self.fonts, &mut diagnostics);
        let width = if input.width.is_finite() {
            input.width.max(0.0)
        } else {
            f32::MAX
        };
        let max_height = input.height.filter(|h| h.is_finite()).map(|h| h.max(0.0));
        let max_lines = input.fit.max_lines.map(|m| m as usize);
        let hi = p.size;
        let lo = input
            .fit
            .shrink_to
            .filter(|s| s.is_finite() && *s > 0.0)
            .map_or(hi, |s| s.min(hi));

        let fits = |a: &Attempt| {
            max_lines.is_none_or(|m| a.lines.len() <= m)
                && max_height.is_none_or(|h| a.height <= h + EPS)
                && a.max_width <= width + EPS
        };

        let first = self.attempt(&p, input.align, width, hi, false);
        let mut hyphenated: Option<(Prepared, Vec<super::hyphen::Inserted>)> = None;
        let chosen = if fits(&first) {
            first
        } else {
            let smallest = if lo < hi {
                self.attempt(&p, input.align, width, lo, false)
            } else {
                first
            };
            let hyphenator = input.hyphenate.and_then(super::hyphen::hyphenator);
            if !fits(&smallest)
                && let Some(h) = hyphenator
            {
                // Automatic hyphenation, only at `shrink_to` (ADR 0009).
                let (hp, inserted) = hyphenate_prepared(&p, h);
                let a = self.attempt(&hp, input.align, width, lo, false);
                let a = if fits(&a) {
                    a
                } else {
                    self.attempt(&hp, input.align, width, lo, true)
                };
                hyphenated = Some((hp, inserted));
                a
            } else if fits(&smallest) {
                // Largest k with fits(lo + k·step); k = n + 1 stands for `hi`
                // (known not to fit). Every returned layout was checked.
                let n = ((hi - lo) / super::FIT_STEP).floor() as u32;
                let (mut good, mut bad) = (0u32, n + 1);
                let mut best = smallest;
                while bad - good > 1 {
                    let mid = good + (bad - good) / 2;
                    let size = lo + mid as f32 * super::FIT_STEP;
                    let a = self.attempt(&p, input.align, width, size, false);
                    if fits(&a) {
                        good = mid;
                        best = a;
                    } else {
                        bad = mid;
                    }
                }
                best
            } else {
                // Last resort: break inside words at the smallest size.
                self.attempt(&p, input.align, width, lo, true)
            }
        };
        match hyphenated {
            None => finish(chosen, &p, input, width, max_height, max_lines, diagnostics),
            Some((hp, inserted)) => {
                let mut words: Vec<String> = Vec::new();
                for l in &chosen.lines {
                    if l.end != LineEnd::SoftHyphen {
                        continue;
                    }
                    let t = hp.text[l.range.clone()].trim_end();
                    let at = l.range.start + t.len() - SOFT_HYPHEN.len_utf8();
                    if let Some(i) = inserted.iter().find(|i| i.at == at)
                        && !words.contains(&i.word)
                    {
                        words.push(i.word.clone());
                    }
                }
                if !words.is_empty() {
                    diagnostics.push(TextDiagnostic::Hyphenated { words });
                }
                finish(
                    chosen,
                    &hp,
                    input,
                    width,
                    max_height,
                    max_lines,
                    diagnostics,
                )
            }
        }
    }

    fn build(&mut self, p: &Prepared, size: f32, emergency: bool) -> Layout<Paint> {
        let mut b = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, &p.text, 1.0, false);
        b.push_default(family_property(&p.base.family));
        b.push_default(StyleProperty::FontSize(size));
        b.push_default(StyleProperty::FontWeight(FontWeight::new(f32::from(
            p.base.weight,
        ))));
        if p.base.italic {
            b.push_default(StyleProperty::FontStyle(FontStyle::Italic));
        }
        b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            p.line_height,
        )));
        if p.tracking != 0.0 {
            b.push_default(StyleProperty::LetterSpacing(p.tracking * size));
        }
        b.push_default(StyleProperty::Brush(p.base.paint));
        if emergency {
            b.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));
        }
        for (range, rs) in &p.ranges {
            if rs.family != p.base.family {
                b.push(family_property(&rs.family), range.clone());
            }
            if rs.weight != p.base.weight {
                b.push(
                    StyleProperty::FontWeight(FontWeight::new(f32::from(rs.weight))),
                    range.clone(),
                );
            }
            if rs.italic != p.base.italic {
                let s = if rs.italic {
                    FontStyle::Italic
                } else {
                    FontStyle::Normal
                };
                b.push(StyleProperty::FontStyle(s), range.clone());
            }
            if rs.paint != p.base.paint {
                b.push(StyleProperty::Brush(rs.paint), range.clone());
            }
        }
        b.build(&p.text)
    }

    /// The hyphen to draw if `line` ends at a soft hyphen (and is not the
    /// last line).
    fn soft_hyphen(
        &self,
        text: &str,
        line: &parley::Line<'_, Paint>,
        is_last: bool,
        tracking: f32,
    ) -> Option<Hyphen> {
        if is_last || line.break_reason() != BreakReason::Regular {
            return None;
        }
        let t = &text[line.text_range()];
        if !t.trim_end().ends_with(SOFT_HYPHEN) {
            return None;
        }
        let run = line.runs().last()?;
        let fd = run.font();
        let font = *self.ids.get(&(fd.data.id(), fd.index))?;
        let f = FontRef::from_index(fd.data.data(), fd.index).ok()?;
        let charmap = f.charmap();
        let gid = charmap.map('-').or_else(|| charmap.map('\u{2010}'))?;
        let coords: Vec<F2Dot14> = run
            .normalized_coords()
            .iter()
            .map(|c| F2Dot14::from_bits(c.to_bits()))
            .collect();
        let advance = f
            .glyph_metrics(Size::new(run.font_size()), LocationRef::new(&coords))
            .advance_width(gid)?;
        Some(Hyphen {
            font,
            glyph: gid.to_u32(),
            advance: advance + tracking * run.font_size(),
        })
    }

    #[allow(
        clippy::too_many_lines,
        clippy::many_single_char_names,
        reason = "one linear pass over parley's lines"
    )]
    fn attempt(
        &mut self,
        p: &Prepared,
        align: Align,
        width: f32,
        size: f32,
        emergency: bool,
    ) -> Attempt {
        let mut layout = self.build(p, size, emergency);
        let tracking = p.tracking;

        // Break lines; a line ending at a soft hyphen must leave room for the
        // hyphen glyph, otherwise it is re-broken with less width.
        let mut narrowed: BTreeMap<usize, f32> = BTreeMap::new();
        for _ in 0..64 {
            break_lines(&mut layout, width, &narrowed);
            let n = layout.len();
            let violation = layout.lines().enumerate().find_map(|(i, line)| {
                if narrowed.contains_key(&i) {
                    return None;
                }
                let hy = self.soft_hyphen(&p.text, &line, i + 1 == n, tracking)?;
                let m = line.metrics();
                (m.advance - m.hanging_advance + hy.advance > width + EPS)
                    .then_some((i, (width - hy.advance).max(0.0)))
            });
            match violation {
                Some((i, w)) => {
                    narrowed.insert(i, w);
                }
                None => break,
            }
        }
        layout.align(Alignment::Start, AlignmentOptions::default());

        let factor = match align {
            Align::Left => 0.0,
            Align::Center => 0.5,
            Align::Right => 1.0,
        };
        let mut out = Attempt {
            size,
            lines: Vec::new(),
            runs: Vec::new(),
            highlights: Vec::new(),
            height: layout.height(),
            max_width: 0.0,
            emergency: Vec::new(),
            missing: BTreeSet::new(),
        };
        let n = layout.len();
        for (i, line) in layout.lines().enumerate() {
            let m = *line.metrics();
            let hyphen = self.soft_hyphen(&p.text, &line, i + 1 == n, tracking);
            let content_w = m.advance - m.hanging_advance;
            let w = content_w + hyphen.map_or(0.0, |h| h.advance);
            let dx = (factor * (width - w)).max(0.0);
            out.max_width = out.max_width.max(w);
            let end = if hyphen.is_some() {
                LineEnd::SoftHyphen
            } else if i + 1 == n {
                LineEnd::End
            } else {
                match line.break_reason() {
                    BreakReason::Explicit => LineEnd::Explicit,
                    BreakReason::Emergency => LineEnd::Emergency,
                    _ => LineEnd::Wrap,
                }
            };
            if line.break_reason() == BreakReason::Emergency && i + 1 < n {
                out.emergency
                    .push((word_at(&p.text, line.text_range().end), i));
            }
            let mut last: Option<GlyphRun> = None;
            let mut line_end_x = 0.0f32;
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else {
                    continue;
                };
                let run = gr.run();
                for c in run.clusters() {
                    if c.glyphs().any(|g| g.id == 0) {
                        out.missing
                            .extend(p.text[c.text_range()].chars().filter(|ch| {
                                !ch.is_whitespace() && !ch.is_control() && *ch != SOFT_HYPHEN
                            }));
                    }
                }
                let fd = run.font();
                let Some(&font) = self.ids.get(&(fd.data.id(), fd.index)) else {
                    continue;
                };
                let synth = run.synthesis();
                let paint = gr.style().brush;
                line_end_x = line_end_x.max(gr.offset() + gr.advance());
                if let Some(bg) = paint.background {
                    let rect = Rect {
                        x: gr.offset() + dx,
                        y: m.content_block_min_coord,
                        w: gr.advance(),
                        h: m.content_block_max_coord - m.content_block_min_coord,
                    };
                    match out.highlights.last_mut() {
                        Some(h)
                            if h.color == rgba(bg)
                                && h.effects == paint.fx
                                && (h.rect.y - rect.y).abs() < EPS
                                && (h.rect.right() - rect.x).abs() < EPS =>
                        {
                            h.rect.w = rect.right() - h.rect.x;
                        }
                        _ => out.highlights.push(Highlight {
                            rect,
                            color: rgba(bg),
                            effects: paint.fx,
                        }),
                    }
                }
                let glyph_run = GlyphRun {
                    font,
                    size: run.font_size(),
                    color: rgba(paint.color),
                    effects: paint.fx,
                    skew: synth.skew(),
                    embolden: synth.embolden(),
                    coords: run
                        .normalized_coords()
                        .iter()
                        .map(|c| c.to_bits())
                        .collect(),
                    glyphs: gr
                        .positioned_glyphs()
                        .map(|g| Glyph {
                            id: g.id,
                            x: g.x + dx,
                            y: g.y,
                        })
                        .collect(),
                };
                if let Some(prev) = last.replace(glyph_run) {
                    out.runs.push(prev);
                }
            }
            if let (Some(h), Some(run)) = (hyphen, last.as_ref()) {
                out.runs.push(GlyphRun {
                    font: h.font,
                    glyphs: vec![Glyph {
                        id: h.glyph,
                        x: line_end_x + dx,
                        y: m.baseline,
                    }],
                    ..run.clone()
                });
            }
            out.runs.extend(last);
            out.lines.push(TextLine {
                range: line.text_range(),
                x: dx,
                top: m.block_min_coord,
                baseline: m.baseline,
                height: m.line_height,
                width: w,
                end,
            });
        }
        out
    }
}

/// Diagnostics, vertical alignment and the final [`TextLayout`].
#[allow(clippy::too_many_arguments, reason = "internal helper")]
fn finish(
    mut a: Attempt,
    p: &Prepared,
    input: &TextInput<'_>,
    width: f32,
    max_height: Option<f32>,
    max_lines: Option<usize>,
    mut diagnostics: Vec<TextDiagnostic>,
) -> TextLayout {
    if !a.missing.is_empty() {
        diagnostics.push(TextDiagnostic::MissingGlyphs {
            text: a.missing.iter().collect(),
        });
    }
    diagnostics.extend(
        a.emergency
            .iter()
            .map(|(word, line)| TextDiagnostic::EmergencyBreak {
                word: word.clone(),
                line: *line,
            }),
    );
    if let Some(m) = max_lines
        && a.lines.len() > m
    {
        diagnostics.push(TextDiagnostic::OverflowLines {
            lines: a.lines.len(),
            max_lines: m as u32,
        });
    }
    if let Some(h) = max_height
        && a.height > h + EPS
    {
        diagnostics.push(TextDiagnostic::OverflowHeight {
            height: a.height,
            max_height: h,
        });
    }
    if a.max_width > width + EPS {
        diagnostics.push(TextDiagnostic::OverflowWidth {
            width: a.max_width,
            max_width: width,
        });
    }
    let overflow = diagnostics.iter().any(TextDiagnostic::is_overflow);

    let box_h = max_height.unwrap_or(a.height);
    let dy = match (max_height, input.valign) {
        (None, _) | (_, VAlign::Top) => 0.0,
        (Some(h), VAlign::Center) => ((h - a.height) / 2.0).max(0.0),
        (Some(h), VAlign::Bottom) => (h - a.height).max(0.0),
    };
    if dy != 0.0 {
        for l in &mut a.lines {
            l.top += dy;
            l.baseline += dy;
        }
        for r in &mut a.runs {
            for g in &mut r.glyphs {
                g.y += dy;
            }
        }
        for h in &mut a.highlights {
            h.rect.y += dy;
        }
    }
    let content = a
        .lines
        .iter()
        .map(|l| Rect {
            x: l.x,
            y: l.top,
            w: l.width,
            h: l.height,
        })
        .reduce(|u, r| {
            let x = u.x.min(r.x);
            let y = u.y.min(r.y);
            Rect {
                x,
                y,
                w: u.right().max(r.right()) - x,
                h: u.bottom().max(r.bottom()) - y,
            }
        })
        .unwrap_or(Rect {
            x: 0.0,
            y: dy,
            w: 0.0,
            h: 0.0,
        });
    TextLayout {
        text: p.text.clone(),
        r#box: Rect {
            x: 0.0,
            y: 0.0,
            w: width,
            h: box_h,
        },
        content,
        font_size: a.size,
        lines: a.lines,
        runs: a.runs,
        highlights: a.highlights,
        effects: p.effects.clone(),
        overflow,
        diagnostics,
    }
}

/// Breaks lines with `width`, or a narrower width for the line indices in
/// `narrowed` (room for a soft-hyphen glyph).
fn break_lines(layout: &mut Layout<Paint>, width: f32, narrowed: &BTreeMap<usize, f32>) {
    let mut br = layout.break_lines();
    let state = br.state_mut();
    state.set_layout_max_advance(width);
    state.set_line_max_advance(narrowed.get(&0).copied().unwrap_or(width));
    let mut line = 0;
    while let Some(y) = br.break_next() {
        match y {
            YieldData::LineBreak(_) => {
                line += 1;
                br.state_mut()
                    .set_line_max_advance(narrowed.get(&line).copied().unwrap_or(width));
            }
            YieldData::InlineBoxBreak(_) => {
                // No inline boxes are ever added; place them as empty boxes
                // to guarantee progress anyway.
                let x = br.state().line_x();
                br.state_mut().append_inline_box_to_line(x, 0.0, 0.0);
            }
            YieldData::MaxHeightExceeded(_) => {}
        }
    }
    br.finish();
}

/// The whitespace-delimited word around byte `at`, soft hyphens removed.
fn word_at(text: &str, at: usize) -> String {
    let at = at.min(text.len());
    let start = text[..at].rfind(char::is_whitespace).map_or(0, |i| {
        i + text[i..].chars().next().map_or(1, char::len_utf8)
    });
    let end = text[at..]
        .find(char::is_whitespace)
        .map_or(text.len(), |i| at + i);
    text[start..end]
        .chars()
        .filter(|&c| c != SOFT_HYPHEN)
        .collect()
}
