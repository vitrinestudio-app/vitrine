//! Text layout with `parley` (fontique + harfrust shaping, ICU line breaking).

use std::collections::HashMap;
use std::sync::Arc;

use parley::fontique::{Blob, Collection, CollectionOptions, SourceCache};
use parley::{
    Alignment, AlignmentOptions, FontContext, FontFamily, FontWeight, LayoutContext, LineHeight,
    OverflowWrap, PositionedLayoutItem, StyleProperty,
};

use super::{Align, FontLibrary, GlyphRun, RawLayout, Rgba, Span, TextEngine, TextStyle};

/// `parley` engine with a font collection containing only bundled fonts
/// (system font discovery disabled).
pub struct ParleyEngine {
    font_cx: FontContext,
    layout_cx: LayoutContext<Rgba>,
    blobs: HashMap<u64, usize>,
}

impl std::fmt::Debug for ParleyEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParleyEngine").finish_non_exhaustive()
    }
}

impl ParleyEngine {
    pub fn new(fonts: &FontLibrary) -> Self {
        let mut collection = Collection::new(CollectionOptions {
            shared: false,
            system_fonts: false,
        });
        let mut blobs = HashMap::new();
        for (i, face) in fonts.faces().iter().enumerate() {
            let blob: Blob<u8> = Blob::new(Arc::new(face.data));
            blobs.insert(blob.id(), i);
            collection.register_fonts(blob, None);
        }
        Self {
            font_cx: FontContext {
                collection,
                source_cache: SourceCache::default(),
            },
            layout_cx: LayoutContext::new(),
            blobs,
        }
    }
}

impl TextEngine for ParleyEngine {
    fn name(&self) -> &'static str {
        "parley"
    }

    fn layout(
        &mut self,
        spans: &[Span],
        style: &TextStyle,
        size: f32,
        width: f32,
        emergency_break: bool,
    ) -> RawLayout {
        let text: String = spans.iter().map(|s| s.text.as_str()).collect();
        let mut b = self
            .layout_cx
            .ranged_builder(&mut self.font_cx, &text, 1.0, false);
        b.push_default(StyleProperty::FontFamily(FontFamily::named(style.family)));
        b.push_default(StyleProperty::FontSize(size));
        b.push_default(StyleProperty::FontWeight(FontWeight::new(f32::from(
            style.weight,
        ))));
        b.push_default(StyleProperty::LineHeight(LineHeight::FontSizeRelative(
            style.line_height,
        )));
        b.push_default(StyleProperty::Brush(style.color));
        if emergency_break {
            b.push_default(StyleProperty::OverflowWrap(OverflowWrap::Anywhere));
        }
        let mut pos = 0;
        for s in spans {
            let range = pos..pos + s.text.len();
            pos = range.end;
            if s.bold {
                b.push(
                    StyleProperty::FontWeight(FontWeight::new(f32::from(style.bold_weight))),
                    range.clone(),
                );
            }
            let c = style.span_color(s);
            if c != style.color {
                b.push(StyleProperty::Brush(c), range);
            }
        }
        let mut layout = b.build(&text);
        layout.break_all_lines(Some(width));
        let align = match style.align {
            Align::Start => Alignment::Start,
            Align::Center => Alignment::Center,
            Align::End => Alignment::End,
        };
        layout.align(align, AlignmentOptions::default());

        let mut out = RawLayout {
            height: layout.height(),
            ..RawLayout::default()
        };
        for line in layout.lines() {
            out.line_count += 1;
            let m = line.metrics();
            out.max_line_width = out.max_line_width.max(m.advance - m.hanging_advance);
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(gr) = item else {
                    continue;
                };
                let run = gr.run();
                let font = self.blobs.get(&run.font().data.id()).copied().unwrap_or(0);
                let color = gr.style().brush;
                out.runs.push(GlyphRun {
                    font,
                    size: run.font_size(),
                    color,
                    glyphs: gr.positioned_glyphs().map(|g| (g.id, g.x, g.y)).collect(),
                });
            }
        }
        out
    }
}
