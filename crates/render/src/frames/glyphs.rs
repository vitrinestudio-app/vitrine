//! Single-line labels inside frames (browser URL, tab title, placeholder
//! key) drawn from the bundled Inter outlines. No shaping beyond the
//! character map and advances: these are short Latin strings.

use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use tiny_skia::{Path, PathBuilder};

static INTER: &[u8] = include_bytes!("../../../../assets/fonts/inter/Inter-Regular.ttf");

struct Pen<'a> {
    pb: &'a mut PathBuilder,
    ox: f32,
}

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(self.ox + x, -y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(self.ox + x, -y);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.pb.quad_to(self.ox + cx0, -cy0, self.ox + x, -y);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.pb
            .cubic_to(self.ox + cx0, -cy0, self.ox + cx1, -cy1, self.ox + x, -y);
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

/// A laid-out label: outline with the baseline at y = 0, starting at x = 0.
#[derive(Debug, Clone)]
pub struct Label {
    pub path: Path,
    pub width: f32,
    /// Cap height (for vertical centring).
    pub cap: f32,
}

/// Lays out `text` at `size`, truncated with an ellipsis to `max_width`.
pub fn label(text: &str, size: f32, max_width: f32) -> Option<Label> {
    let font = FontRef::new(INTER).ok()?;
    let loc = LocationRef::default();
    let metrics = font.glyph_metrics(Size::new(size), loc);
    let cmap = font.charmap();
    let glyph = |c: char| cmap.map(c).unwrap_or(GlyphId::NOTDEF);
    let advance = |g: GlyphId| metrics.advance_width(g).unwrap_or(size * 0.5);
    let mut glyphs: Vec<(GlyphId, f32)> = text.chars().map(|c| (glyph(c), 0.0)).collect();
    for g in &mut glyphs {
        g.1 = advance(g.0);
    }
    let total = |gs: &[(GlyphId, f32)]| gs.iter().map(|g| g.1).sum::<f32>();
    if total(&glyphs) > max_width {
        let ell = glyph('…');
        let ell_w = advance(ell);
        while !glyphs.is_empty() && total(&glyphs) + ell_w > max_width {
            glyphs.pop();
        }
        if glyphs.is_empty() {
            return None;
        }
        glyphs.push((ell, ell_w));
    }
    let outlines = font.outline_glyphs();
    let mut pb = PathBuilder::new();
    let mut x = 0.0;
    for (g, adv) in &glyphs {
        if let Some(o) = outlines.get(*g) {
            let mut pen = Pen { pb: &mut pb, ox: x };
            let _ = o.draw(DrawSettings::unhinted(Size::new(size), loc), &mut pen);
        }
        x += adv;
    }
    let cap = font
        .metrics(Size::new(size), loc)
        .cap_height
        .unwrap_or(size * 0.73);
    Some(Label {
        path: pb.finish()?,
        width: x,
        cap,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncates_with_ellipsis() {
        let full = label("example.com/some/long/path", 13.0, 1000.0).expect("label");
        let short = label("example.com/some/long/path", 13.0, 60.0).expect("label");
        assert!(short.width <= 60.0 && short.width < full.width);
        assert!(full.cap > 8.0 && full.cap < 11.0);
    }
}
