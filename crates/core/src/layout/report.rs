//! Serializable layout summary of one text element (used by MCP
//! `layout_report` and `core::validate`).

use serde::{Deserialize, Serialize};

use super::{Rect, TextDiagnostic, TextLayout};

/// Rounds to 1/100 lu so reports are stable across platforms and readable.
fn r2(v: f32) -> f32 {
    (v * 100.0).round() / 100.0
}

fn rect(r: Rect) -> Rect {
    Rect {
        x: r2(r.x),
        y: r2(r.y),
        w: r2(r.w),
        h: r2(r.h),
    }
}

/// Summary of a [`TextLayout`]. Coordinates are relative to the text box's
/// top-left corner; use [`LayoutReport::translated`] to move them onto a page.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutReport {
    /// The text box.
    pub r#box: Rect,
    /// Union of the line boxes.
    pub content: Rect,
    /// Chosen font size in lu.
    pub font_size: f32,
    pub line_count: usize,
    /// Lines as displayed (soft hyphens resolved).
    pub lines: Vec<String>,
    pub overflow: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<TextDiagnostic>,
}

impl LayoutReport {
    pub fn from_layout(l: &TextLayout) -> Self {
        let diagnostics = l
            .diagnostics
            .iter()
            .cloned()
            .map(|d| match d {
                TextDiagnostic::OverflowHeight { height, max_height } => {
                    TextDiagnostic::OverflowHeight {
                        height: r2(height),
                        max_height: r2(max_height),
                    }
                }
                TextDiagnostic::OverflowWidth { width, max_width } => {
                    TextDiagnostic::OverflowWidth {
                        width: r2(width),
                        max_width: r2(max_width),
                    }
                }
                d => d,
            })
            .collect();
        Self {
            r#box: rect(l.r#box),
            content: rect(l.content),
            font_size: r2(l.font_size),
            line_count: l.lines.len(),
            lines: l.lines.iter().map(|line| l.line_text(line)).collect(),
            overflow: l.overflow,
            diagnostics,
        }
    }

    /// The same report with boxes moved by (`dx`, `dy`) lu.
    #[must_use]
    pub fn translated(mut self, dx: f32, dy: f32) -> Self {
        for r in [&mut self.r#box, &mut self.content] {
            r.x = r2(r.x + dx);
            r.y = r2(r.y + dy);
        }
        self
    }
}
