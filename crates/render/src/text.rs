//! Text rasterisation: the glyph runs of a [`TextLayout`] computed by
//! `arrangia_core::layout` are turned into vector paths from their font
//! outlines (via `skrifa`) and filled like any other shape.
//!
//! Layout is in **lu** and unhinted, so rotation and scaling (lu → px) are
//! just the transform passed to [`draw_text`]; no glyph cache is needed.

use arrangia_core::layout::{FontLibrary, GlyphRun, ResolvedGlow, TextEffects, TextLayout};
use arrangia_core::model::{StrokeJoin, StrokePosition};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::types::F2Dot14;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use tiny_skia::{FillRule, LineJoin, Mask, Path, PathBuilder, Pixmap, Stroke, Transform};

use crate::blur::{ShadowPx, draw_blurred};
use crate::paint::{Rgba, solid};

struct PathPen<'a> {
    pb: &'a mut PathBuilder,
    ox: f32,
    oy: f32,
    /// Horizontal shear for synthetic oblique (tan of the angle).
    shear: f32,
}

impl PathPen<'_> {
    fn pt(&self, x: f32, y: f32) -> (f32, f32) {
        (self.ox + x + y * self.shear, self.oy - y)
    }
}

impl OutlinePen for PathPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.pt(x, y);
        self.pb.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.pt(x, y);
        self.pb.line_to(x, y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (cx0, cy0) = self.pt(cx0, cy0);
        let (x, y) = self.pt(x, y);
        self.pb.quad_to(cx0, cy0, x, y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (cx0, cy0) = self.pt(cx0, cy0);
        let (cx1, cy1) = self.pt(cx1, cy1);
        let (x, y) = self.pt(x, y);
        self.pb.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// Glyph outlines of one paint.
#[derive(Debug, Clone)]
pub struct GlyphPath {
    pub path: Path,
    pub color: Rgba,
    /// Additional stroke width in lu (synthetic bold), if any.
    pub stroke: Option<f32>,
    /// Index into [`TextPaths::effects`].
    pub effects: u16,
}

/// A span highlight (background box).
#[derive(Debug, Clone)]
pub struct HighlightPath {
    pub path: Path,
    pub color: Rgba,
    /// Index into [`TextPaths::effects`]; only the shadow applies.
    pub effects: u16,
}

/// A text layout as paths in lu, relative to the text box origin.
#[derive(Debug, Clone, Default)]
pub struct TextPaths {
    /// Span highlights, drawn first.
    pub highlights: Vec<HighlightPath>,
    pub glyphs: Vec<GlyphPath>,
    /// Outline / glow / shadow sets of the layout ([`TextLayout::effects`]).
    pub effects: Vec<TextEffects>,
}

fn union(a: tiny_skia::Rect, b: tiny_skia::Rect) -> tiny_skia::Rect {
    tiny_skia::Rect::from_ltrb(
        a.left().min(b.left()),
        a.top().min(b.top()),
        a.right().max(b.right()),
        a.bottom().max(b.bottom()),
    )
    .unwrap_or(a)
}

fn outset(r: tiny_skia::Rect, m: f32) -> tiny_skia::Rect {
    if m > 0.0 {
        r.outset(m, m).unwrap_or(r)
    } else {
        r
    }
}

impl TextPaths {
    /// Bounds of all fill paths (lu), `None` if nothing is drawn.
    pub fn bounds(&self) -> Option<tiny_skia::Rect> {
        self.highlights
            .iter()
            .map(|h| h.path.bounds())
            .chain(self.glyphs.iter().map(|g| g.path.bounds()))
            .reduce(union)
    }

    /// Bounds including outline, glow and shadow extents (lu).
    pub fn visual_bounds(&self) -> Option<tiny_skia::Rect> {
        let m = self
            .effects
            .iter()
            .map(TextEffects::visual_outset)
            .fold(0.0, f64::max) as f32;
        self.bounds().map(|b| outset(b, m))
    }
}

fn rgba(c: arrangia_core::model::Rgba) -> Rgba {
    [c.r, c.g, c.b, c.a]
}

/// Synthetic bold stroke width for a font size (similar to Skia's fake bold).
fn embolden_width(size: f32) -> f32 {
    size / 32.0
}

fn draw_run(run: &GlyphRun, fonts: &FontLibrary, pb: &mut PathBuilder) {
    let Some(face) = fonts.face(run.font) else {
        return;
    };
    let Ok(font) = FontRef::from_index(face.data(), face.index) else {
        return;
    };
    let coords: Vec<F2Dot14> = run.coords.iter().map(|&c| F2Dot14::from_bits(c)).collect();
    let outlines = font.outline_glyphs();
    let shear = run.skew.map_or(0.0, |deg| deg.to_radians().tan());
    for g in &run.glyphs {
        if let Some(glyph) = outlines.get(GlyphId::new(g.id)) {
            let mut pen = PathPen {
                pb,
                ox: g.x,
                oy: g.y,
                shear,
            };
            let settings = DrawSettings::unhinted(Size::new(run.size), LocationRef::new(&coords));
            let _ = glyph.draw(settings, &mut pen);
        }
    }
}

/// Converts a layout into paths: one path per paint (colour + synthetic
/// bold + effect set), plus one per highlight.
pub fn to_paths(layout: &TextLayout, fonts: &FontLibrary) -> TextPaths {
    type Key = (Rgba, Option<u32>, u16);
    let mut builders: Vec<(Key, PathBuilder)> = Vec::new();
    for run in &layout.runs {
        let stroke = run.embolden.then(|| embolden_width(run.size));
        let key = (rgba(run.color), stroke.map(f32::to_bits), run.effects);
        let idx = if let Some(i) = builders.iter().position(|(k, _)| *k == key) {
            i
        } else {
            builders.push((key, PathBuilder::new()));
            builders.len() - 1
        };
        draw_run(run, fonts, &mut builders[idx].1);
    }
    TextPaths {
        highlights: layout
            .highlights
            .iter()
            .filter_map(|h| {
                let r = tiny_skia::Rect::from_xywh(h.rect.x, h.rect.y, h.rect.w, h.rect.h)?;
                Some(HighlightPath {
                    path: PathBuilder::from_rect(r),
                    color: rgba(h.color),
                    effects: h.effects,
                })
            })
            .collect(),
        glyphs: builders
            .into_iter()
            .filter_map(|((color, stroke, effects), pb)| {
                pb.finish().map(|path| GlyphPath {
                    path,
                    color,
                    stroke: stroke.map(f32::from_bits),
                    effects,
                })
            })
            .collect(),
        effects: layout.effects.clone(),
    }
}

/// Draws the fills of text paths (highlights, then glyphs) with
/// `transform` (text box origin → pixels, including rotation and the
/// lu → px scale) and an optional clip mask. No outline, glow or shadow:
/// see [`draw_text_with_effects`].
pub fn draw_text(pm: &mut Pixmap, paths: &TextPaths, transform: Transform, mask: Option<&Mask>) {
    for h in &paths.highlights {
        pm.fill_path(&h.path, &solid(h.color), FillRule::Winding, transform, mask);
    }
    draw_glyph_fills(pm, paths, transform, mask);
}

fn draw_glyph_fills(pm: &mut Pixmap, paths: &TextPaths, transform: Transform, mask: Option<&Mask>) {
    for g in &paths.glyphs {
        let paint = solid(g.color);
        pm.fill_path(&g.path, &paint, FillRule::Winding, transform, mask);
        if let Some(width) = g.stroke {
            let stroke = Stroke {
                width,
                ..Stroke::default()
            };
            pm.stroke_path(&g.path, &paint, &stroke, transform, mask);
        }
    }
}

fn line_join(j: StrokeJoin) -> LineJoin {
    match j {
        StrokeJoin::Round => LineJoin::Round,
        StrokeJoin::Miter => LineJoin::Miter,
        StrokeJoin::Bevel => LineJoin::Bevel,
    }
}

/// The silhouette of effect set `fx` (glyphs, optionally highlights)
/// dilated by `grow` lu, painted into a blur mask.
struct Silhouette<'a> {
    paths: &'a TextPaths,
    fx: u16,
    grow: f32,
    highlights: bool,
}

impl Silhouette<'_> {
    fn bounds(&self) -> Option<tiny_skia::Rect> {
        let glyphs = self
            .paths
            .glyphs
            .iter()
            .filter(|g| g.effects == self.fx)
            .map(|g| outset(g.path.bounds(), self.grow + g.stroke.unwrap_or(0.0) / 2.0));
        let hl = self
            .paths
            .highlights
            .iter()
            .filter(|h| self.highlights && h.effects == self.fx)
            .map(|h| outset(h.path.bounds(), self.grow));
        glyphs.chain(hl).reduce(union)
    }

    fn paint(&self, mask: &mut Pixmap, ts: Transform) {
        let mut paint = tiny_skia::Paint::default();
        paint.set_color_rgba8(0, 0, 0, 255);
        paint.anti_alias = true;
        if self.highlights {
            for h in self
                .paths
                .highlights
                .iter()
                .filter(|h| h.effects == self.fx)
            {
                mask.fill_path(&h.path, &paint, FillRule::Winding, ts, None);
            }
        }
        for g in self.paths.glyphs.iter().filter(|g| g.effects == self.fx) {
            mask.fill_path(&g.path, &paint, FillRule::Winding, ts, None);
            let width = 2.0 * self.grow + g.stroke.unwrap_or(0.0);
            if width > 0.0 {
                let st = Stroke {
                    width,
                    line_join: LineJoin::Round,
                    ..Stroke::default()
                };
                mask.stroke_path(&g.path, &paint, &st, ts, None);
            }
        }
    }

    /// Blurred, tinted copy (shadow or glow).
    fn draw(&self, pm: &mut Pixmap, transform: Transform, shadow: ShadowPx) {
        let Some(b) = self.bounds().and_then(|b| b.transform(transform)) else {
            return;
        };
        draw_blurred(pm, b, shadow, |mask, to_mask| {
            self.paint(mask, transform.post_concat(to_mask));
        });
    }
}

/// Draws text with its effects, in this order: shadows (of glyphs, their
/// outlines and highlights), highlights, glows, outside outlines, glyph
/// fills, centred outlines. Each pass covers all effect sets, so a span's
/// glow never covers a neighbouring span's glyphs. `px_per_lu` scales blur
/// radii and shadow offsets (which stay in canvas space, unrotated).
pub fn draw_text_with_effects(
    pm: &mut Pixmap,
    paths: &TextPaths,
    transform: Transform,
    px_per_lu: f64,
) {
    let sets: Vec<(u16, &TextEffects)> = paths
        .effects
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.is_empty())
        .map(|(i, e)| (u16::try_from(i).unwrap_or(u16::MAX), e))
        .collect();
    let stroke_grow = |e: &TextEffects| e.ink_outset() as f32;
    for &(fx, e) in &sets {
        let sil = Silhouette {
            paths,
            fx,
            grow: stroke_grow(e),
            highlights: true,
        };
        for l in &e.shadow {
            sil.draw(
                pm,
                transform,
                ShadowPx {
                    sigma: l.blur * px_per_lu,
                    dx: l.x * px_per_lu,
                    dy: l.y * px_per_lu,
                    color: rgba(l.color),
                },
            );
        }
    }
    for h in &paths.highlights {
        pm.fill_path(&h.path, &solid(h.color), FillRule::Winding, transform, None);
    }
    for &(fx, e) in &sets {
        if let Some(g) = e.glow.filter(ResolvedGlow::is_visible) {
            let sil = Silhouette {
                paths,
                fx,
                grow: stroke_grow(e) + g.spread as f32,
                highlights: false,
            };
            let [r, gr, b, a] = rgba(g.color);
            let alpha = (f64::from(a) * g.opacity).round().clamp(0.0, 255.0) as u8;
            sil.draw(
                pm,
                transform,
                ShadowPx {
                    sigma: g.radius * px_per_lu,
                    dx: 0.0,
                    dy: 0.0,
                    color: [r, gr, b, alpha],
                },
            );
        }
    }
    let outline = |pm: &mut Pixmap, position: StrokePosition| {
        for &(fx, e) in &sets {
            let Some(s) = e
                .stroke
                .filter(|s| s.is_visible() && s.position == position)
            else {
                continue;
            };
            let paint = solid(rgba(s.color));
            for g in paths.glyphs.iter().filter(|g| g.effects == fx) {
                let k = if position == StrokePosition::Outside {
                    2.0
                } else {
                    1.0
                };
                let st = Stroke {
                    width: k * s.width as f32 + g.stroke.unwrap_or(0.0),
                    line_join: line_join(s.join),
                    miter_limit: 4.0,
                    ..Stroke::default()
                };
                pm.stroke_path(&g.path, &paint, &st, transform, None);
            }
        }
    };
    outline(pm, StrokePosition::Outside);
    draw_glyph_fills(pm, paths, transform, None);
    outline(pm, StrokePosition::Center);
}

/// Convenience: [`to_paths`] + [`draw_text`].
pub fn draw_layout(
    pm: &mut Pixmap,
    layout: &TextLayout,
    fonts: &FontLibrary,
    transform: Transform,
    mask: Option<&Mask>,
) {
    draw_text(pm, &to_paths(layout, fonts), transform, mask);
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrangia_core::layout::{ResolvedTextStyle, TextInput, TextLayouter};

    fn ink(pm: &Pixmap) -> usize {
        pm.data()
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .count()
    }

    #[test]
    fn draws_glyphs_and_italic_leans_right() {
        let mut l = TextLayouter::new(FontLibrary::bundled());
        let style = ResolvedTextStyle {
            size: 80.0,
            ..ResolvedTextStyle::default()
        };
        let upright = l.layout(&TextInput::new("I", &style, 500.0));
        let italic = l.layout(&TextInput::new("_I_", &style, 500.0));
        let a = to_paths(&upright, l.fonts()).bounds().unwrap();
        let b = to_paths(&italic, l.fonts()).bounds().unwrap();
        assert!((a.height() - b.height()).abs() < 0.5);
        assert!(b.width() > a.width() + 10.0, "{a:?} vs {b:?}");

        let mut pm = Pixmap::new(200, 120).unwrap();
        draw_layout(&mut pm, &upright, l.fonts(), Transform::identity(), None);
        assert!(ink(&pm) > 200);
        // Scaled 2x (lu → px) covers about four times the pixels.
        let mut pm2 = Pixmap::new(400, 240).unwrap();
        draw_layout(
            &mut pm2,
            &upright,
            l.fonts(),
            Transform::from_scale(2.0, 2.0),
            None,
        );
        let ratio = ink(&pm2) as f32 / ink(&pm) as f32;
        assert!((3.0..5.0).contains(&ratio), "{ratio}");
    }

    #[test]
    fn highlights_are_drawn() {
        let mut l = TextLayouter::new(FontLibrary::bundled());
        let style = ResolvedTextStyle::default();
        let mut spans = std::collections::BTreeMap::new();
        spans.insert(
            "hl".to_owned(),
            arrangia_core::layout::ResolvedSpanStyle {
                background: Some(arrangia_core::model::Rgba::rgb(255, 0, 0)),
                ..Default::default()
            },
        );
        let layout = l.layout(&TextInput {
            spans: Some(&spans),
            ..TextInput::new("a [hl]bc[/] d", &style, 500.0)
        });
        let paths = to_paths(&layout, l.fonts());
        assert_eq!(paths.highlights.len(), 1);
        assert_eq!(paths.highlights[0].color, [255, 0, 0, 255]);
    }
}
