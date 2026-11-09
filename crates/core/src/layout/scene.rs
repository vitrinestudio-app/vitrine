//! Laying out the texts of a [`ResolvedScene`](crate::resolve::ResolvedScene):
//! converts the resolved (f64, canvas) values into a [`TextInput`] and
//! places the resulting box on the canvas. Shared by `core::validate` and
//! the renderer, so both see the same lines.

use std::collections::BTreeMap;

use super::{Fit, ResolvedSpanStyle, ResolvedTextStyle, TextInput, TextLayout, TextLayouter};
use crate::geometry::{Rect, Vec2};
use crate::model::VAlign;
use crate::resolve;

impl From<&resolve::ResolvedTextStyle> for ResolvedTextStyle {
    fn from(s: &resolve::ResolvedTextStyle) -> Self {
        Self {
            font: s.font.clone(),
            weight: s.weight,
            italic: s.italic,
            size: s.size as f32,
            line_height: s.line_height as f32,
            tracking: s.tracking as f32,
            color: s.color,
            transform: s.transform,
        }
    }
}

impl From<&resolve::ResolvedSpanStyle> for ResolvedSpanStyle {
    fn from(s: &resolve::ResolvedSpanStyle) -> Self {
        Self {
            font: s.font.clone(),
            weight: s.weight,
            italic: s.italic,
            color: s.color,
            background: s.background,
        }
    }
}

/// The scene's span styles in layout form.
pub fn span_styles(
    spans: &BTreeMap<String, resolve::ResolvedSpanStyle>,
) -> BTreeMap<String, ResolvedSpanStyle> {
    spans.iter().map(|(k, v)| (k.clone(), v.into())).collect()
}

/// A laid-out text placed on the canvas.
#[derive(Clone, Debug)]
pub struct PlacedText {
    pub layout: TextLayout,
    /// Canvas position (lu) of the layout box's top-left corner; glyph,
    /// line and content coordinates of [`TextLayout`] are relative to it.
    /// Rotation (element/group) is not applied.
    pub origin: Vec2,
}

impl PlacedText {
    /// The layout box on the canvas (unrotated).
    pub fn box_rect(&self) -> Rect {
        let b = self.layout.r#box;
        Rect::new(
            self.origin.x + f64::from(b.x),
            self.origin.y + f64::from(b.y),
            f64::from(b.w),
            f64::from(b.h),
        )
    }

    /// Union of the line boxes on the canvas (unrotated).
    pub fn content_rect(&self) -> Rect {
        let c = self.layout.content;
        Rect::new(
            self.origin.x + f64::from(c.x),
            self.origin.y + f64::from(c.y),
            f64::from(c.w),
            f64::from(c.h),
        )
    }
}

/// Lay out a resolved text element; `None` if it has no text (key missing
/// in every language of the chain).
///
/// A box with a fixed height is laid out with that height (auto-fit,
/// `valign` inside the box). An `auto_height` box grows with the text and
/// is placed by `valign` within the frame rect: `top` at its top, `center`
/// around its centre, `bottom` on its bottom (a free box without height has
/// a zero-high rect at `at`, so `center` centres the text on `at`).
pub fn layout_text(
    layouter: &mut TextLayouter,
    text: &resolve::ResolvedText,
    spans: &BTreeMap<String, ResolvedSpanStyle>,
) -> Option<PlacedText> {
    let markup = text.text.as_deref()?;
    let style = ResolvedTextStyle::from(&text.style);
    let rect = text.frame.rect;
    let layout = layouter.layout(&TextInput {
        markup,
        style: &style,
        spans: Some(spans),
        width: rect.w as f32,
        height: (!text.frame.auto_height).then_some(rect.h as f32),
        align: text.align,
        valign: text.valign,
        fit: Fit {
            shrink_to: text.shrink_to.map(|v| v as f32),
            max_lines: text.max_lines,
        },
    });
    let y = if text.frame.auto_height {
        let h = f64::from(layout.r#box.h);
        match text.valign {
            VAlign::Top => rect.y,
            VAlign::Center => rect.center().y - h / 2.0,
            VAlign::Bottom => rect.bottom() - h,
        }
    } else {
        rect.y
    };
    Some(PlacedText {
        layout,
        origin: Vec2::new(rect.x, y),
    })
}
