//! Text: inline markup, layout via an exchangeable engine, auto-fit and
//! conversion of glyphs to vector paths.
//!
//! Layout happens in **lu** (resolution independent, no hinting), so line
//! breaks are identical in preview and export. Glyphs are converted to
//! `tiny_skia::Path`s from their font outlines (via `skrifa`) and filled like
//! any other vector shape — rotation and scaling stay crisp, and the renderer
//! itself needs no glyph cache or font access.
//!
//! Two engines are implemented for the Phase 0 comparison: [`cosmic::CosmicEngine`]
//! and [`parley_engine::ParleyEngine`] (see `docs/adr/0001-render-pipeline.md`).

pub mod cosmic;
pub mod fonts;
pub mod markup;
pub mod parley_engine;

use std::collections::HashMap;

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use tiny_skia::{Path, PathBuilder};

pub use fonts::FontLibrary;
pub use markup::{Span, parse_markup};

/// Straight (non-premultiplied) RGBA colour.
pub type Rgba = [u8; 4];

/// Horizontal alignment of a text block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// Resolved paragraph style (sizes in lu).
#[derive(Debug, Clone)]
pub struct TextStyle {
    pub family: &'static str,
    pub weight: u16,
    /// Weight used for `**bold**` spans.
    pub bold_weight: u16,
    pub size: f32,
    /// Line height as a multiple of the font size.
    pub line_height: f32,
    pub color: Rgba,
    pub align: Align,
    /// Named span styles (`[name]…[/]`) → colour.
    pub span_colors: HashMap<String, Rgba>,
}

impl TextStyle {
    pub fn span_color(&self, span: &Span) -> Rgba {
        span.style
            .as_deref()
            .and_then(|n| self.span_colors.get(n).copied())
            .unwrap_or(self.color)
    }

    pub fn span_weight(&self, span: &Span) -> u16 {
        if span.bold {
            self.bold_weight
        } else {
            self.weight
        }
    }
}

/// A run of glyphs sharing font, size and colour; positions are baseline
/// origins in lu relative to the text box's top-left corner.
#[derive(Debug, Clone)]
pub struct GlyphRun {
    /// Index into [`FontLibrary`].
    pub font: usize,
    pub size: f32,
    pub color: Rgba,
    pub glyphs: Vec<(u32, f32, f32)>,
}

/// Engine-independent result of laying out one paragraph at one size.
#[derive(Debug, Clone, Default)]
pub struct RawLayout {
    pub runs: Vec<GlyphRun>,
    pub line_count: usize,
    /// Advance width of the widest line (lu). Larger than the box width if a
    /// word could not be broken.
    pub max_line_width: f32,
    /// Total block height (lu).
    pub height: f32,
}

/// A text layout engine. Both implementations produce [`RawLayout`]s that are
/// rendered by the same glyph-outline code.
pub trait TextEngine {
    fn name(&self) -> &'static str;

    /// Lays out `spans` at `size` lu into a box `width` lu wide. With
    /// `emergency_break` words may be broken anywhere (last resort of auto-fit).
    fn layout(
        &mut self,
        spans: &[Span],
        style: &TextStyle,
        size: f32,
        width: f32,
        emergency_break: bool,
    ) -> RawLayout;
}

/// Auto-fit parameters (`fit: { shrink_to, max_lines }`).
#[derive(Debug, Clone, Copy)]
pub struct Fit {
    pub min_size: f32,
    pub max_lines: usize,
    /// Optional maximum block height in lu.
    pub max_height: Option<f32>,
    /// Balance line lengths (like CSS `text-wrap: balance`): after fitting,
    /// the box is narrowed as far as possible without adding a line.
    pub balance: bool,
}

/// Result of [`layout_fit`].
#[derive(Debug, Clone)]
pub struct FittedText {
    pub layout: RawLayout,
    pub size: f32,
    /// `false` if even `min_size` did not fit without breaking inside words.
    pub fits: bool,
}

fn fits(l: &RawLayout, width: f32, fit: &Fit) -> bool {
    l.line_count <= fit.max_lines
        && l.max_line_width <= width + 0.01
        && fit.max_height.is_none_or(|h| l.height <= h)
}

/// Lays out text at the largest size in `[fit.min_size, style.size]` that fits
/// into `max_lines` lines without overflowing the box (binary search, 0.25 lu
/// precision). If nothing fits, falls back to `min_size` with emergency word
/// breaking. Optionally balances the line lengths afterwards.
pub fn layout_fit(
    engine: &mut dyn TextEngine,
    spans: &[Span],
    style: &TextStyle,
    width: f32,
    fit: &Fit,
) -> FittedText {
    let mut fitted = fit_size(engine, spans, style, width, fit);
    if fit.balance && fitted.fits && fitted.layout.line_count > 1 {
        let lines = fitted.layout.line_count;
        let (mut lo, mut hi) = (width / lines as f32, width);
        let mut best: Option<(f32, RawLayout)> = None;
        while hi - lo > 1.0 {
            let mid = f32::midpoint(lo, hi);
            let l = engine.layout(spans, style, fitted.size, mid, false);
            if l.line_count == lines && l.max_line_width <= mid + 0.01 {
                hi = mid;
                best = Some((mid, l));
            } else {
                lo = mid;
            }
        }
        if let Some((w, mut l)) = best {
            let dx = match style.align {
                Align::Start => 0.0,
                Align::Center => (width - w) / 2.0,
                Align::End => width - w,
            };
            for run in &mut l.runs {
                for g in &mut run.glyphs {
                    g.1 += dx;
                }
            }
            fitted.layout = l;
        }
    }
    fitted
}

fn fit_size(
    engine: &mut dyn TextEngine,
    spans: &[Span],
    style: &TextStyle,
    width: f32,
    fit: &Fit,
) -> FittedText {
    let first = engine.layout(spans, style, style.size, width, false);
    if fits(&first, width, fit) {
        return FittedText {
            layout: first,
            size: style.size,
            fits: true,
        };
    }
    let smallest = engine.layout(spans, style, fit.min_size, width, false);
    if !fits(&smallest, width, fit) {
        let layout = engine.layout(spans, style, fit.min_size, width, true);
        return FittedText {
            layout,
            size: fit.min_size,
            fits: false,
        };
    }
    let (mut lo, mut hi) = (fit.min_size, style.size);
    let mut best = (fit.min_size, smallest);
    while hi - lo > 0.25 {
        let mid = f32::midpoint(lo, hi);
        let l = engine.layout(spans, style, mid, width, false);
        if fits(&l, width, fit) {
            lo = mid;
            best = (mid, l);
        } else {
            hi = mid;
        }
    }
    FittedText {
        layout: best.1,
        size: best.0,
        fits: true,
    }
}

struct PathPen<'a> {
    pb: &'a mut PathBuilder,
    ox: f32,
    oy: f32,
}

impl OutlinePen for PathPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.ox + x, self.oy - y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.ox + x, self.oy - y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.pb
            .quad_to(self.ox + cx0, self.oy - cy0, self.ox + x, self.oy - y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.pb.cubic_to(
            self.ox + cx0,
            self.oy - cy0,
            self.ox + cx1,
            self.oy - cy1,
            self.ox + x,
            self.oy - y,
        );
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Glyph outlines merged into one path per colour, in lu relative to the
/// text box origin.
#[derive(Debug, Clone, Default)]
pub struct TextPaths {
    pub paths: Vec<(Path, Rgba)>,
}

/// Converts laid-out glyphs into vector paths (one path per colour).
pub fn to_paths(layout: &RawLayout, fonts: &FontLibrary) -> TextPaths {
    let mut builders: Vec<(Rgba, PathBuilder)> = Vec::new();
    for run in &layout.runs {
        let Ok(font) = FontRef::from_index(fonts.data(run.font), 0) else {
            continue;
        };
        let outlines = font.outline_glyphs();
        let idx = if let Some(i) = builders.iter().position(|(c, _)| *c == run.color) {
            i
        } else {
            builders.push((run.color, PathBuilder::new()));
            builders.len() - 1
        };
        let pb = &mut builders[idx].1;
        for &(gid, x, y) in &run.glyphs {
            if let Some(glyph) = outlines.get(GlyphId::new(gid)) {
                let mut pen = PathPen { pb, ox: x, oy: y };
                let settings = DrawSettings::unhinted(Size::new(run.size), LocationRef::default());
                let _ = glyph.draw(settings, &mut pen);
            }
        }
    }
    TextPaths {
        paths: builders
            .into_iter()
            .filter_map(|(c, pb)| pb.finish().map(|p| (p, c)))
            .collect(),
    }
}
