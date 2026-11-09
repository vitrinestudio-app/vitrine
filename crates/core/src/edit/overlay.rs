//! Hit-test and overlay data for the canvas view: element quads, handles,
//! page/column, gap and output image rects of one target × language, all
//! in canvas lu. Built from `core::resolve` and `core::layout`, so it
//! matches what the renderer draws.

use serde::Serialize;

use super::{EditError, Editor, Handle, local_to_canvas, move_point};
use crate::geometry::{Rect, Vec2};
use crate::layout::{TextLayouter, layout_text, span_styles};
use crate::model::{ElementId, ElementKindTag, Lang, StripId, TargetId, VariantId};
use crate::resolve::{ResolvedKind, ResolvedScene};

/// Everything the GUI draws on top of the rendered preview.
#[derive(Clone, Debug, Serialize)]
pub struct Overlay {
    pub target: TargetId,
    pub strip: StripId,
    pub lang: Lang,
    /// Variant shown and edited (`None` = base elements).
    pub variant: Option<VariantId>,
    /// The whole canvas (pages and gaps).
    pub canvas: Rect,
    pub pages: Vec<OverlayPage>,
    /// Gaps between export images (hatched in the editor).
    pub gaps: Vec<Rect>,
    /// Output images (page groups or the banner).
    pub images: Vec<OverlayImage>,
    /// Drawn elements in painting order (bottom to top); groups flattened.
    pub elements: Vec<OverlayElement>,
    /// Groups with at least one drawn descendant, outermost first.
    pub groups: Vec<OverlayGroup>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OverlayPage {
    pub index: u32,
    pub id: String,
    /// Page (column) rect on the canvas.
    pub rect: Rect,
    pub image: Option<u32>,
    pub column: u32,
}

#[derive(Clone, Debug, Serialize)]
pub struct OverlayImage {
    pub index: u32,
    pub pages: Vec<u32>,
    /// Canvas rect covered by the image.
    pub rect: Rect,
}

#[derive(Clone, Debug, Serialize)]
pub struct OverlayElement {
    pub id: ElementId,
    pub kind: ElementKindTag,
    /// Enclosing groups, outermost first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ElementId>,
    /// Home page.
    pub page: u32,
    /// Effective layer.
    pub z: i32,
    /// Corners of the drawn box on the canvas: nw, ne, se, sw (rotated).
    pub quad: [Vec2; 4],
    /// The unrotated box (centre and size as drawn).
    pub rect: Rect,
    /// Degrees on the canvas (groups included).
    pub rotation: f64,
    /// Rotation pivot on the canvas.
    pub pivot: Vec2,
    /// Point that `move_to` places (see [`super::move_point`]).
    pub move_point: Vec2,
    /// Resize handles on the canvas (empty if not resizable).
    pub handles: Vec<OverlayHandle>,
    /// Devices resize with a fixed aspect (width only).
    pub aspect_locked: bool,
    /// Fields overridden by the edit variant (YAML names).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overridden: Vec<&'static str>,
    /// Clip rect (text: its home page/column).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip: Option<Rect>,
    /// The centre lies in a gap: not part of any page export.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub in_gap: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<OverlayText>,
}

#[derive(Clone, Debug, Serialize)]
pub struct OverlayHandle {
    pub handle: Handle,
    pub at: Vec2,
}

/// Laid-out text of a text element.
#[derive(Clone, Debug, Serialize)]
pub struct OverlayText {
    /// Box from margins rather than a free box.
    pub margins: bool,
    pub auto_height: bool,
    /// Union of the line boxes, unrotated, on the canvas.
    pub content: Rect,
    /// `content` rotated with the element: nw, ne, se, sw.
    pub content_quad: [Vec2; 4],
    pub lines: usize,
    /// Font size after auto-fit, lu.
    pub font_size: f64,
    pub overflow: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct OverlayGroup {
    pub id: ElementId,
    /// Enclosing groups, outermost first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<ElementId>,
    /// Drawn descendants.
    pub members: Vec<ElementId>,
    /// Axis-aligned bounds of the members' quads.
    pub bounds: Rect,
    /// Point that `move_to` places (centre of `bounds`).
    pub move_point: Vec2,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub overridden: Vec<&'static str>,
}

fn rect_quad(r: Rect) -> [Vec2; 4] {
    [
        Vec2::new(r.x, r.y),
        Vec2::new(r.right(), r.y),
        Vec2::new(r.right(), r.bottom()),
        Vec2::new(r.x, r.bottom()),
    ]
}

impl Editor<'_> {
    /// Overlay data of the editor's target × language × variant. Texts are
    /// laid out with `layouter` (use the project's fonts so lines match the
    /// render).
    pub fn overlay(&self, layouter: &mut TextLayouter) -> Result<Overlay, EditError> {
        let scene = self.scene()?;
        Ok(self.overlay_for(&scene, layouter))
    }

    /// Overlay data for an already resolved scene of this editor.
    #[allow(clippy::too_many_lines)]
    pub fn overlay_for(&self, scene: &ResolvedScene, layouter: &mut TextLayouter) -> Overlay {
        let spans = span_styles(&scene.spans);
        let frame = &scene.frame;
        let mut elements = Vec::new();
        for r in &scene.elements {
            let kind = self.kind(r.id.as_str()).unwrap_or(ElementKindTag::Shape);
            let mut size = self.drawn_size(r);
            let mut center = r.center;
            let mut text = None;
            if let ResolvedKind::Text(t) = &r.props {
                if let Some(placed) = layout_text(layouter, t, &spans) {
                    let content = placed.content_rect();
                    // Auto-height boxes: the drawn box is the layout box.
                    if t.frame.auto_height {
                        let b = placed.box_rect();
                        size = Vec2::new(b.w, b.h);
                        center = b.center();
                    }
                    let to_canvas = |p: Vec2| local_to_canvas(r, p - r.center);
                    text = Some(OverlayText {
                        margins: t.frame.margins,
                        auto_height: t.frame.auto_height,
                        content,
                        content_quad: rect_quad(content).map(to_canvas),
                        lines: placed.layout.lines.len(),
                        font_size: f64::from(placed.layout.font_size),
                        overflow: placed.layout.overflow,
                    });
                } else if t.frame.auto_height {
                    // No text: a free box shrinks to its centre line.
                    size = Vec2::new(t.frame.rect.w, 0.0);
                }
            }
            // Local box relative to the resolved centre (the pivot frame).
            let off = center - r.center;
            let local = |u: Vec2| off + Vec2::new(u.x * size.x / 2.0, u.y * size.y / 2.0);
            let quad = [Handle::Nw, Handle::Ne, Handle::Se, Handle::Sw]
                .map(|h| local_to_canvas(r, local(h.unit())));
            let resizable = !matches!(kind, ElementKindTag::Background | ElementKindTag::Group);
            let handles = if resizable {
                Handle::RESIZE
                    .iter()
                    .map(|&h| OverlayHandle {
                        handle: h,
                        at: local_to_canvas(r, local(h.unit())),
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let visual_center = local_to_canvas(r, off);
            elements.push(OverlayElement {
                id: r.id.clone(),
                kind,
                groups: r.groups.clone(),
                page: r.page,
                z: r.z,
                quad,
                rect: Rect::centered(r.center + off, size.x, size.y),
                rotation: r.rotation,
                pivot: r.center + r.pivot,
                move_point: move_point(r),
                handles,
                aspect_locked: kind == ElementKindTag::Device,
                overridden: self.overridden_fields(r.id.as_str()),
                clip: r.clip,
                in_gap: frame.gaps.iter().any(|g| g.contains(visual_center)),
                text,
            });
        }

        let mut groups: Vec<OverlayGroup> = Vec::new();
        for r in &scene.elements {
            for (i, g) in r.groups.iter().enumerate() {
                if groups.iter().any(|o| &o.id == g) {
                    continue;
                }
                let members: Vec<ElementId> = scene
                    .elements
                    .iter()
                    .filter(|m| m.groups.contains(g))
                    .map(|m| m.id.clone())
                    .collect();
                let bounds = elements
                    .iter()
                    .filter(|e| members.contains(&e.id))
                    .flat_map(|e| e.quad)
                    .map(|p| Rect::new(p.x, p.y, 0.0, 0.0))
                    .reduce(|a, b| a.union(&b))
                    .unwrap_or_default();
                groups.push(OverlayGroup {
                    id: g.clone(),
                    groups: r.groups[..i].to_vec(),
                    members,
                    bounds,
                    move_point: self
                        .group_bounds(scene, g.as_str())
                        .map_or(bounds.center(), |b| b.center()),
                    overridden: self.overridden_fields(g.as_str()),
                });
            }
        }

        Overlay {
            target: scene.target.clone(),
            strip: scene.strip.clone(),
            lang: scene.lang.clone(),
            variant: self.variant.clone(),
            canvas: frame.canvas,
            pages: frame
                .pages
                .iter()
                .map(|p| OverlayPage {
                    index: p.index,
                    id: p.id.clone(),
                    rect: p.rect,
                    image: p.image,
                    column: p.column,
                })
                .collect(),
            gaps: frame.gaps.clone(),
            images: frame
                .images
                .iter()
                .map(|i| OverlayImage {
                    index: i.index,
                    pages: i.pages.clone(),
                    rect: i.source,
                })
                .collect(),
            elements,
            groups,
        }
    }
}
