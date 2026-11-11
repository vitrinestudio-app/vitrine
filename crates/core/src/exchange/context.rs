//! Context for translators: which text elements use a key, where, and an
//! approximate length limit derived from the text box.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::PAGE_WIDTH_LU;
use crate::model::{Element, ElementKind, Project};
use crate::resolve::default_text_style;

/// Average glyph advance in em used for the length hint. Inter averages
/// ≈ 0.5 em for mixed-case text; a little more keeps the hint on the safe
/// side for bold headlines.
pub const AVG_CHAR_EM: f64 = 0.55;

/// One text element using a key (base element, variants ignored).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyUsage {
    pub element: String,
    pub canvas: String,
    /// 1-based page number (as shown in the GUI).
    pub page: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<String>,
    /// Text box width in lu (`None` for a free box without width).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_width: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u32>,
    /// ≈ characters per line at the full font size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chars_per_line: Option<u32>,
    /// ≈ characters per line at the auto-fit minimum (`shrink_to`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chars_per_line_min: Option<u32>,
}

impl KeyUsage {
    /// ≈ maximum characters before the text overflows (lines × chars at the
    /// minimum size); `None` without box width or line limit.
    pub fn max_chars(&self) -> Option<u32> {
        let per_line = self.chars_per_line_min.or(self.chars_per_line)?;
        Some(per_line * self.max_lines?)
    }

    /// One line for a note: `title-plan (canvas phone, page 1, style
    /// headline): box 860 lu, max 2 lines, ≈ 17 chars/line (≈ 24 at the
    /// minimum size)`.
    pub fn describe(&self) -> String {
        let mut s = format!(
            "{} (canvas {}, page {}",
            self.element, self.canvas, self.page
        );
        if let Some(st) = &self.style {
            let _ = write!(s, ", style {st}");
        }
        s.push(')');
        let mut facts = Vec::new();
        if let Some(w) = self.box_width {
            facts.push(format!("box {} lu", w.round()));
        }
        if let Some(l) = self.max_lines {
            facts.push(format!("max {l} line{}", if l == 1 { "" } else { "s" }));
        }
        match (self.chars_per_line, self.chars_per_line_min) {
            (Some(a), Some(b)) if b > a => {
                facts.push(format!("≈ {a} chars/line (≈ {b} at the minimum size)"));
            }
            (Some(a), _) => facts.push(format!("≈ {a} chars/line")),
            _ => {}
        }
        if !facts.is_empty() {
            s.push_str(": ");
            s.push_str(&facts.join(", "));
        }
        s
    }
}

/// Context of one key.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyContext {
    pub usages: Vec<KeyUsage>,
}

impl KeyContext {
    /// Smallest [`KeyUsage::max_chars`] of all usages.
    pub fn max_chars(&self) -> Option<u32> {
        self.usages.iter().filter_map(KeyUsage::max_chars).min()
    }

    /// Note lines (one per usage, or "not used").
    pub fn lines(&self) -> Vec<String> {
        if self.usages.is_empty() {
            return vec!["Not used by any text element.".to_owned()];
        }
        self.usages.iter().map(KeyUsage::describe).collect()
    }
}

fn chars(width: f64, size: f64) -> Option<u32> {
    (width > 0.0 && size > 0.0).then(|| {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (width / (size * AVG_CHAR_EM)).floor().max(1.0) as u32;
        n
    })
}

fn usage(p: &Project, canvas: &str, e: &Element) -> Option<(String, KeyUsage)> {
    let ElementKind::Text(t) = &e.kind else {
        return None;
    };
    let size = t
        .font_size
        .or_else(|| {
            t.style
                .as_ref()
                .and_then(|s| p.styles.text.get(s))
                .and_then(|s| s.size)
        })
        .unwrap_or_else(|| default_text_style().size);
    let box_width = match &t.r#box {
        Some(b) => Some(PAGE_WIDTH_LU - b.left.unwrap_or(0.0) - b.right.unwrap_or(0.0)),
        None => e.width,
    };
    let fit = t.fit.unwrap_or_default();
    let per_line = box_width.and_then(|w| chars(w, size));
    let per_line_min = box_width
        .zip(fit.shrink_to.filter(|&m| m < size))
        .and_then(|(w, m)| chars(w, m));
    Some((
        t.text.to_string(),
        KeyUsage {
            element: e.id.to_string(),
            canvas: canvas.to_owned(),
            page: e.page + 1,
            style: t.style.clone(),
            box_width,
            max_lines: fit.max_lines,
            chars_per_line: per_line,
            chars_per_line_min: per_line_min,
        },
    ))
}

/// Context of every key used by a text element.
pub fn key_contexts(p: &Project) -> BTreeMap<String, KeyContext> {
    let mut out: BTreeMap<String, KeyContext> = BTreeMap::new();
    for s in &p.canvases {
        s.walk_elements(&mut |e| {
            if let Some((key, u)) = usage(p, s.id.as_str(), e) {
                out.entry(key).or_default().usages.push(u);
            }
        });
    }
    out
}

/// All keys: used by a text element or present in any language.
pub fn all_keys(p: &Project) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for s in &p.canvases {
        s.walk_elements(&mut |e| {
            if let ElementKind::Text(t) = &e.kind {
                keys.insert(t.text.to_string());
            }
        });
    }
    for map in p.strings.values() {
        keys.extend(map.keys().map(ToString::to_string));
    }
    keys
}
