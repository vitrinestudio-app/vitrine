//! Text layout with `cosmic-text` (shaping via harfrust, line breaking, bidi).

use std::collections::HashMap;
use std::sync::Arc;

use cosmic_text::{
    Align as CAlign, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Weight, Wrap,
    fontdb,
};

use super::{Align, FontLibrary, GlyphRun, RawLayout, Span, TextEngine, TextStyle};

/// `cosmic-text` engine with a font database containing only bundled fonts.
pub struct CosmicEngine {
    fs: FontSystem,
    ids: HashMap<fontdb::ID, usize>,
}

impl std::fmt::Debug for CosmicEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CosmicEngine").finish_non_exhaustive()
    }
}

impl CosmicEngine {
    pub fn new(fonts: &FontLibrary) -> Self {
        let mut db = fontdb::Database::new();
        let mut ids = HashMap::new();
        for (i, face) in fonts.faces().iter().enumerate() {
            for id in db.load_font_source(fontdb::Source::Binary(Arc::new(face.data))) {
                ids.insert(id, i);
            }
        }
        // Locale only influences fallback ordering; fixed for reproducibility.
        let fs = FontSystem::new_with_locale_and_db("en-US".to_owned(), db);
        Self { fs, ids }
    }
}

fn color(c: [u8; 4]) -> Color {
    Color::rgba(c[0], c[1], c[2], c[3])
}

impl TextEngine for CosmicEngine {
    fn name(&self) -> &'static str {
        "cosmic-text"
    }

    fn layout(
        &mut self,
        spans: &[Span],
        style: &TextStyle,
        size: f32,
        width: f32,
        emergency_break: bool,
    ) -> RawLayout {
        let mut buf = Buffer::new_empty(Metrics::new(size, size * style.line_height));
        buf.set_size(Some(width), None);
        buf.set_wrap(if emergency_break {
            Wrap::WordOrGlyph
        } else {
            Wrap::Word
        });
        let default = Attrs::new()
            .family(Family::Name(style.family))
            .weight(Weight(style.weight))
            .color(color(style.color));
        let align = match style.align {
            Align::Start => CAlign::Left,
            Align::Center => CAlign::Center,
            Align::End => CAlign::Right,
        };
        buf.set_rich_text(
            spans.iter().map(|s| {
                (
                    s.text.as_str(),
                    default
                        .clone()
                        .weight(Weight(style.span_weight(s)))
                        .color(color(style.span_color(s))),
                )
            }),
            &default,
            Shaping::Advanced,
            Some(align),
        );
        buf.shape_until_scroll(&mut self.fs, false);

        let mut out = RawLayout::default();
        for run in buf.layout_runs() {
            out.line_count += 1;
            out.max_line_width = out.max_line_width.max(run.line_w);
            out.height = run.line_top + run.line_height;
            for g in run.glyphs {
                let font = self.ids.get(&g.font_id).copied().unwrap_or(0);
                let c = g
                    .color_opt
                    .map_or(style.color, |c| [c.r(), c.g(), c.b(), c.a()]);
                let x = g.x + g.font_size * g.x_offset;
                let y = run.line_y + g.y - g.font_size * g.y_offset;
                match out.runs.last_mut() {
                    Some(r)
                        if r.font == font
                            && r.color == c
                            && r.size.to_bits() == g.font_size.to_bits() =>
                    {
                        r.glyphs.push((u32::from(g.glyph_id), x, y));
                    }
                    _ => out.runs.push(GlyphRun {
                        font,
                        size: g.font_size,
                        color: c,
                        glyphs: vec![(u32::from(g.glyph_id), x, y)],
                    }),
                }
            }
        }
        out
    }
}
