//! Edit intents (ADR 0007): GUI gestures and MCP tools expressed as
//! Operations-API ops, computed in core so that no client touches model
//! fields or re-implements canvas geometry.
//!
//! An [`Editor`] looks at one target × language (× variant, see
//! [`EditContext`]). Its intents (`move_to`, `resize`, `rotate_to`, …)
//! return the ops of **one undo step** without applying them; the caller
//! applies them with [`Project::apply_ops`] or a [`crate::ops::History`].
//!
//! Positions are found by running the real pipeline: candidate ops are
//! applied to a scratch copy of the project, the target is resolved
//! (`core::resolve`) and the element's reference point is read back. A
//! Newton iteration on the element's position parameters (`at`, or the
//! margins of a margin text box) then hits the requested canvas point. The
//! mapping is affine except inside rotated groups (whose pivot follows the
//! children), so one step usually suffices; whatever resolve does with
//! anchors, gaps, grouping, `content_scale`, group offsets and pivots is
//! honoured by construction.
//!
//! With an active variant, element fields are written as variant
//! overrides (merged into an existing override) instead of changing the
//! base element. Structural changes (painting order, grouping, duplicate,
//! remove, ids) always change the base.

// Geometry: e, r, p, q, g, h… are element, resolved element, point,
// parameter, residual, step.
#![allow(clippy::many_single_char_names)]

mod lang;
mod overlay;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub use lang::{DefaultLanguageImpact, LostScreenshot, LostText, default_language_impact};
pub use overlay::{
    Overlay, OverlayElement, OverlayGroup, OverlayHandle, OverlayImage, OverlayPage, OverlayText,
};

use crate::geometry::{Rect, Vec2};
use crate::model::fmt::double_option;
use crate::model::{
    Element, ElementId, ElementKind, ElementKindTag, ElementPatch, GroupProps, Lang, Point,
    Project, Strip, StripId, Target, TextBox, VariantId,
};
use crate::ops::{Op, OpError};
use crate::resolve::{
    ResolveError, ResolveOptions, ResolvedElement, ResolvedKind, ResolvedScene, resolve,
};

/// Convergence threshold of the position solver, lu.
const SOLVE_EPS: f64 = 1e-9;
/// Largest residual accepted after the iteration, lu.
const SOLVE_ACCEPT: f64 = 1e-7;
const SOLVE_MAX_STEPS: usize = 25;
/// Step of the finite-difference Jacobian, lu.
const SOLVE_DIFF_STEP: f64 = 1e-2;

/// What the edits look at and where they are written.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct EditContext {
    pub target: crate::model::TargetId,
    pub lang: Lang,
    /// Variant edits are written to. Missing = the target's own variant,
    /// `null` = the base elements (the target is then shown without its
    /// variant), a name = that variant of the target's strip.
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub variant: Option<Option<VariantId>>,
}

impl EditContext {
    /// Context for `target` in `lang` using the target's own variant.
    pub fn new(target: &str, lang: &str) -> Result<Self, EditError> {
        Ok(Self {
            target: crate::model::TargetId::parse(target).map_err(EditError::Invalid)?,
            lang: Lang::parse(lang).map_err(EditError::Invalid)?,
            variant: None,
        })
    }

    /// Edit the base elements (no variant).
    #[must_use]
    pub fn base(mut self) -> Self {
        self.variant = Some(None);
        self
    }

    /// Edit `variant` of the target's strip.
    #[must_use]
    pub fn with_variant(mut self, variant: VariantId) -> Self {
        self.variant = Some(Some(variant));
        self
    }
}

/// Why an intent could not be turned into ops.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum EditError {
    #[error(transparent)]
    Resolve(#[from] ResolveError),
    #[error(transparent)]
    Op(#[from] OpError),
    #[error("unknown element `{0}`")]
    UnknownElement(String),
    #[error("element `{element}` is not on strip `{strip}` of the target")]
    WrongStrip { element: String, strip: String },
    #[error("unknown variant `{variant}` in strip `{strip}`")]
    UnknownVariant { strip: String, variant: String },
    #[error(
        "element `{0}` is not drawn for this target (unknown device model or no visible children)"
    )]
    NotOnCanvas(String),
    #[error("{0}")]
    Unsupported(String),
    #[error(
        "no position of `{0}` puts it at the requested point (e.g. inside a group rotated by 180°)"
    )]
    Unsolvable(String),
    #[error("{0}")]
    Invalid(String),
}

/// Options of [`Editor::move_to`] / [`Editor::move_by`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MoveOptions {
    /// Keep the home page (manual home page lock). Otherwise the home page
    /// becomes the page (column) under the element's reference point.
    pub keep_page: bool,
}

/// A point of an element's unrotated box: corners, edge centres, centre.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Handle {
    Nw,
    N,
    Ne,
    E,
    Se,
    S,
    Sw,
    W,
    Center,
}

impl Handle {
    /// The eight resize handles, clockwise from the top-left corner.
    pub const RESIZE: [Handle; 8] = [
        Handle::Nw,
        Handle::N,
        Handle::Ne,
        Handle::E,
        Handle::Se,
        Handle::S,
        Handle::Sw,
        Handle::W,
    ];

    /// Position as fractions of the half size: `(-1, -1)` = top-left.
    pub fn unit(self) -> Vec2 {
        let (x, y) = match self {
            Handle::Nw => (-1.0, -1.0),
            Handle::N => (0.0, -1.0),
            Handle::Ne => (1.0, -1.0),
            Handle::E => (1.0, 0.0),
            Handle::Se => (1.0, 1.0),
            Handle::S => (0.0, 1.0),
            Handle::Sw => (-1.0, 1.0),
            Handle::W => (-1.0, 0.0),
            Handle::Center => (0.0, 0.0),
        };
        Vec2::new(x, y)
    }

    /// The handle on the other side of the centre.
    #[must_use]
    pub fn opposite(self) -> Handle {
        match self {
            Handle::Nw => Handle::Se,
            Handle::N => Handle::S,
            Handle::Ne => Handle::Sw,
            Handle::E => Handle::W,
            Handle::Se => Handle::Nw,
            Handle::S => Handle::N,
            Handle::Sw => Handle::Ne,
            Handle::W => Handle::E,
            Handle::Center => Handle::Center,
        }
    }
}

/// New size of an element in canvas lu, as seen in the target (content
/// scale and `stretch` included). Missing values stay unchanged.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ResizeSpec {
    pub width: Option<f64>,
    pub height: Option<f64>,
}

/// Painting-order change within the element's list (siblings).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZOrder {
    /// Topmost of its list.
    Front,
    /// Bottommost of its list.
    Back,
    /// One step up.
    Forward,
    /// One step down.
    Backward,
}

/// Where element fields are written.
#[derive(Clone, Debug, PartialEq)]
enum WriteTo {
    Base,
    Variant(VariantId),
}

/// Which point of an element the position solver steers.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Probe {
    /// [`move_point`] (bounding box centre for groups).
    Move,
    /// A point of the unrotated box (bounding box for groups).
    Handle(Handle),
}

/// The free position parameters of an element.
#[derive(Clone, Copy, Debug)]
enum Param {
    /// `at` (devices, images, shapes, free texts, group offsets).
    At(Point),
    /// The margins of a margin text box, shifted together.
    Box(TextBox),
}

impl Param {
    fn of(e: &Element) -> Self {
        match &e.kind {
            ElementKind::Text(t) if t.r#box.is_some() => Param::Box(t.r#box.unwrap_or_default()),
            _ => Param::At(e.at),
        }
    }

    fn patch(self, q: Vec2) -> ElementPatch {
        match self {
            Param::At(at) => ElementPatch {
                at: Some(Some(Point::new(clean(at.x + q.x), clean(at.y + q.y)))),
                ..ElementPatch::default()
            },
            Param::Box(b) => ElementPatch {
                r#box: Some(Some(shift_box(b, q))),
                ..ElementPatch::default()
            },
        }
    }
}

/// Shift a margin box by `d` (lu of the reference page).
fn shift_box(b: TextBox, d: Vec2) -> TextBox {
    let set = |v: Option<f64>, delta: f64| {
        if v.is_none() && delta == 0.0 {
            None
        } else {
            Some(clean(v.unwrap_or(0.0) + delta))
        }
    };
    TextBox {
        left: set(b.left, d.x),
        right: set(b.right, -d.x),
        top: set(b.top, d.y),
        bottom: b.bottom.map(|v| clean(v - d.y)),
    }
}

/// Round away floating-point noise (`499.99999999997` → `500`) so the
/// project file stays readable; far below any visible precision.
fn clean(v: f64) -> f64 {
    if v.is_finite() && v.abs() < 1e9 {
        let r = (v * 1e9).round() / 1e9;
        if r == 0.0 { 0.0 } else { r }
    } else {
        v
    }
}

/// Angle in `(-180, 180]`.
fn normalize_angle(deg: f64) -> f64 {
    let a = deg.rem_euclid(360.0);
    if a > 180.0 { a - 360.0 } else { a }
}

/// Canvas point of `local` (relative to the element centre, unrotated
/// frame) of a resolved element: the renderer draws local coordinates
/// rotated by `rotation` around `center + pivot`.
pub fn local_to_canvas(r: &ResolvedElement, local: Vec2) -> Vec2 {
    let pivot = r.center + r.pivot;
    (pivot + (local - r.pivot)).rotated_around(pivot, r.rotation)
}

/// The point an element is moved by: the centre of the drawn (rotated)
/// box, except for margin text boxes without `bottom` (they grow
/// downwards to the page bottom), where it is the centre of the top edge.
pub fn move_point(r: &ResolvedElement) -> Vec2 {
    local_to_canvas(r, move_local(r))
}

fn move_local(r: &ResolvedElement) -> Vec2 {
    match &r.props {
        ResolvedKind::Text(t) if t.frame.margins && t.frame.auto_height => {
            Vec2::new(0.0, -r.size.y / 2.0)
        }
        _ => Vec2::ZERO,
    }
}

/// An id based on `base` that no element of the project uses: `base`
/// itself, else `base-2`, `base-3` … (a trailing `-N` of `base` is
/// replaced, so duplicating `phone-2` gives `phone-3`).
pub fn unique_id(project: &Project, base: &str) -> ElementId {
    unique_id_except(project, base, &BTreeSet::new())
}

fn unique_id_except(project: &Project, base: &str, taken: &BTreeSet<ElementId>) -> ElementId {
    let free = |id: &str| project.element(id).is_none() && !taken.contains(id);
    let base = if crate::model::is_valid_id(base) {
        base
    } else {
        "element"
    };
    if free(base) {
        return ElementId::new(base);
    }
    let stem = match base.rsplit_once('-') {
        Some((s, n)) if !s.is_empty() && !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
            s
        }
        _ => base,
    };
    let mut n = 2u64;
    loop {
        let id = format!("{stem}-{n}");
        if free(&id) {
            return ElementId::new(&id);
        }
        n += 1;
    }
}

/// Ops that remove field `field` (YAML name, e.g. `at`) from the override
/// of `element` in `variant`, or the whole override (`None`). Empty if
/// there is nothing to reset.
pub fn reset_override(
    project: &Project,
    strip: &StripId,
    variant: &VariantId,
    element: &ElementId,
    field: Option<&str>,
) -> Result<Vec<Op>, EditError> {
    let s = project
        .strip(strip.as_str())
        .ok_or_else(|| EditError::Op(OpError::UnknownStrip(strip.to_string())))?;
    let v = s
        .variants
        .get(variant)
        .ok_or_else(|| EditError::UnknownVariant {
            strip: strip.to_string(),
            variant: variant.to_string(),
        })?;
    let Some(patch) = v.overrides.get(element) else {
        return Ok(Vec::new());
    };
    Ok(match field {
        None => vec![Op::SetVariantOverride {
            strip: strip.clone(),
            variant: variant.clone(),
            element: element.clone(),
            patch: None,
        }],
        Some(f) => {
            if !ElementPatch::is_field_name(f) {
                return Err(EditError::Invalid(format!("unknown element field `{f}`")));
            }
            if patch.fields().contains(&f) {
                vec![Op::ResetOverrideField {
                    strip: strip.clone(),
                    variant: variant.clone(),
                    element: element.clone(),
                    field: f.to_owned(),
                }]
            } else {
                Vec::new()
            }
        }
    })
}

/// Edit intents for one target × language × variant.
pub struct Editor<'a> {
    project: &'a Project,
    options: ResolveOptions<'a>,
    target: &'a Target,
    strip: &'a Strip,
    lang: Lang,
    variant: Option<VariantId>,
    /// The project as edits see it: the target shows `variant`.
    view: Project,
}

impl std::fmt::Debug for Editor<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Editor")
            .field("target", &self.target.id)
            .field("lang", &self.lang)
            .field("variant", &self.variant)
            .finish_non_exhaustive()
    }
}

impl<'a> Editor<'a> {
    pub fn new(
        project: &'a Project,
        ctx: &EditContext,
        options: ResolveOptions<'a>,
    ) -> Result<Self, EditError> {
        let target = project
            .target(ctx.target.as_str())
            .ok_or_else(|| ResolveError::UnknownTarget(ctx.target.to_string()))?;
        let strip =
            project
                .strip(target.strip.as_str())
                .ok_or_else(|| ResolveError::UnknownStrip {
                    target: target.id.to_string(),
                    strip: target.strip.to_string(),
                })?;
        let variant = match &ctx.variant {
            None => target
                .variant
                .clone()
                .filter(|v| strip.variants.contains_key(v)),
            Some(v) => v.clone(),
        };
        if let Some(v) = &variant
            && !strip.variants.contains_key(v)
        {
            return Err(EditError::UnknownVariant {
                strip: strip.id.to_string(),
                variant: v.to_string(),
            });
        }
        let mut view = project.clone();
        if let Some(t) = view.targets.iter_mut().find(|t| t.id == target.id) {
            t.variant.clone_from(&variant);
        }
        Ok(Self {
            project,
            options,
            target,
            strip,
            lang: ctx.lang.clone(),
            variant,
            view,
        })
    }

    /// The variant edits are written to (`None` = base elements).
    pub fn variant(&self) -> Option<&VariantId> {
        self.variant.as_ref()
    }

    /// The strip shown by the target.
    pub fn strip(&self) -> &Strip {
        self.strip
    }

    /// The scene as the editor sees it (target with the edit variant).
    pub fn scene(&self) -> Result<ResolvedScene, EditError> {
        Ok(resolve(
            &self.view,
            self.target.id.as_str(),
            self.lang.as_str(),
            &self.options,
        )?)
    }

    fn write_to(&self) -> WriteTo {
        self.variant.clone().map_or(WriteTo::Base, WriteTo::Variant)
    }

    // ----- lookups -----------------------------------------------------------

    fn base_element(&self, id: &str) -> Result<&'a Element, EditError> {
        let (strip, e) = self
            .project
            .element(id)
            .ok_or_else(|| EditError::UnknownElement(id.to_owned()))?;
        if strip.id != self.strip.id {
            return Err(EditError::WrongStrip {
                element: id.to_owned(),
                strip: self.strip.id.to_string(),
            });
        }
        Ok(e)
    }

    /// Element `id` of `project` with the override of `to` applied.
    fn effective_in(project: &Project, strip: &StripId, id: &str, to: &WriteTo) -> Element {
        let Some(s) = project.strip(strip.as_str()) else {
            unreachable!("strip checked by the caller")
        };
        let e = s
            .element(id)
            .cloned()
            .unwrap_or_else(|| unreachable!("element checked by the caller"));
        match to {
            WriteTo::Base => e,
            WriteTo::Variant(v) => s
                .variants
                .get(v)
                .and_then(|v| v.overrides.get(id))
                .and_then(|p| e.patched(p).ok())
                .map_or(e, |(p, _)| p),
        }
    }

    fn effective(&self, id: &str) -> Result<Element, EditError> {
        self.base_element(id)?;
        Ok(Self::effective_in(
            self.project,
            &self.strip.id,
            id,
            &self.write_to(),
        ))
    }

    /// Op writing `patch` for element `id`, as an override when writing to
    /// a variant (merged into the existing override of `project`).
    fn write_op(&self, project: &Project, id: &ElementId, patch: ElementPatch, to: &WriteTo) -> Op {
        match to {
            WriteTo::Base => Op::UpdateElement {
                id: id.clone(),
                patch,
            },
            WriteTo::Variant(v) => {
                let existing = project
                    .strip(self.strip.id.as_str())
                    .and_then(|s| s.variants.get(v))
                    .and_then(|v| v.overrides.get(id))
                    .cloned()
                    .unwrap_or_default();
                Op::SetVariantOverride {
                    strip: self.strip.id.clone(),
                    variant: v.clone(),
                    element: id.clone(),
                    patch: Some(existing.merged(&patch)),
                }
            }
        }
    }

    fn write(&self, id: &ElementId, patch: ElementPatch) -> Vec<Op> {
        if patch.is_empty() {
            return Vec::new();
        }
        vec![self.write_op(self.project, id, patch, &self.write_to())]
    }

    fn is_group(e: &Element) -> bool {
        matches!(e.kind, ElementKind::Group(_))
    }

    // ----- probing -----------------------------------------------------------

    /// Size of the drawn box: images without `height` take the asset's
    /// aspect when its pixel size is known.
    fn drawn_size(&self, r: &ResolvedElement) -> Vec2 {
        if let ResolvedKind::Image(img) = &r.props
            && img.auto_height
            && let Some((w, h)) = self.options.assets.dimensions(&img.asset)
            && w > 0
        {
            return Vec2::new(r.size.x, r.size.x * f64::from(h) / f64::from(w));
        }
        r.size
    }

    /// Corners of the drawn box on the canvas (nw, ne, se, sw).
    fn quad(&self, r: &ResolvedElement) -> [Vec2; 4] {
        let s = self.drawn_size(r) * 0.5;
        [
            Handle::Nw.unit(),
            Handle::Ne.unit(),
            Handle::Se.unit(),
            Handle::Sw.unit(),
        ]
        .map(|u| local_to_canvas(r, Vec2::new(u.x * s.x, u.y * s.y)))
    }

    /// Axis-aligned bounds of all drawn descendants of group `id`.
    fn group_bounds(&self, scene: &ResolvedScene, id: &str) -> Option<Rect> {
        let mut out: Option<Rect> = None;
        for r in scene
            .elements
            .iter()
            .filter(|r| r.groups.iter().any(|g| g.as_str() == id))
        {
            for p in self.quad(r) {
                let pr = Rect::new(p.x, p.y, 0.0, 0.0);
                out = Some(out.map_or(pr, |o| o.union(&pr)));
            }
        }
        out
    }

    fn probe(&self, scene: &ResolvedScene, id: &str, probe: Probe) -> Option<Vec2> {
        if let Some(r) = scene.element(id) {
            let local = match probe {
                Probe::Move => move_local(r),
                Probe::Handle(h) => {
                    let s = self.drawn_size(r) * 0.5;
                    let u = h.unit();
                    Vec2::new(u.x * s.x, u.y * s.y)
                }
            };
            return Some(local_to_canvas(r, local));
        }
        let b = self.group_bounds(scene, id)?;
        let u = match probe {
            Probe::Move => Vec2::ZERO,
            Probe::Handle(h) => h.unit(),
        };
        Some(b.center() + Vec2::new(u.x * b.w / 2.0, u.y * b.h / 2.0))
    }

    /// Resolve `project` with element `id` (and its groups) forced visible.
    fn resolve_focused(&self, project: &mut Project, id: &str) -> Result<ResolvedScene, EditError> {
        force_visible(project, &self.strip.id, id);
        Ok(resolve(
            project,
            self.target.id.as_str(),
            self.lang.as_str(),
            &self.options,
        )?)
    }

    /// Current canvas point of `probe` on element `id` (after `pre`).
    fn current(&self, pre: &[Op], id: &str, probe: Probe) -> Result<Vec2, EditError> {
        let mut p = self.view.clone();
        p.apply_ops(pre.iter().cloned())?;
        let scene = self.resolve_focused(&mut p, id)?;
        self.probe(&scene, id, probe)
            .ok_or_else(|| EditError::NotOnCanvas(id.to_owned()))
    }

    // ----- the position solver ----------------------------------------------

    /// Ops: `pre`, then one write of `fixed` plus the position of `id`
    /// chosen so that `probe` lands on `goal`.
    fn solve(
        &self,
        pre: Vec<Op>,
        id: &ElementId,
        fixed: &ElementPatch,
        probe: Probe,
        goal: Vec2,
        to: &WriteTo,
    ) -> Result<Vec<Op>, EditError> {
        let mut scratch = self.view.clone();
        scratch.apply_ops(pre.iter().cloned())?;
        let start = Self::effective_in(&scratch, &self.strip.id, id.as_str(), to);
        let (start, _) = start
            .patched(fixed)
            .map_err(|e| EditError::Invalid(format!("element `{id}`: {e}")))?;
        let param = Param::of(&start);
        let op_for = |q: Vec2| self.write_op(&scratch, id, fixed.merged(&param.patch(q)), to);
        let eval = |q: Vec2| -> Result<Vec2, EditError> {
            let mut p = scratch.clone();
            p.apply(op_for(q))?;
            let scene = self.resolve_focused(&mut p, id.as_str())?;
            let at = self
                .probe(&scene, id.as_str(), probe)
                .ok_or_else(|| EditError::NotOnCanvas(id.to_string()))?;
            Ok(at - goal)
        };
        let mut q = Vec2::ZERO;
        let mut g = eval(q)?;
        for _ in 0..SOLVE_MAX_STEPS {
            if g.x.hypot(g.y) < SOLVE_EPS {
                break;
            }
            let h = SOLVE_DIFF_STEP;
            let gx = eval(q + Vec2::new(h, 0.0))?;
            let gy = eval(q + Vec2::new(0.0, h))?;
            let (a, c) = ((gx.x - g.x) / h, (gx.y - g.y) / h);
            let (b, d) = ((gy.x - g.x) / h, (gy.y - g.y) / h);
            let det = a * d - b * c;
            if det.abs() < 1e-9 {
                return Err(EditError::Unsolvable(id.to_string()));
            }
            let dq = Vec2::new((d * g.x - b * g.y) / det, (a * g.y - c * g.x) / det);
            q = q - dq;
            g = eval(q)?;
        }
        if g.x.hypot(g.y) > SOLVE_ACCEPT {
            return Err(EditError::Unsolvable(id.to_string()));
        }
        let mut ops = pre;
        ops.push(op_for(q));
        Ok(ops)
    }

    /// Home page (column) under canvas x: the page containing it, else the
    /// nearest one (gaps, outside the canvas).
    fn page_at(scene: &ResolvedScene, x: f64) -> u32 {
        let pages = &scene.frame.pages;
        pages
            .iter()
            .find(|p| x >= p.rect.x && x < p.rect.right())
            .or_else(|| {
                pages.iter().min_by(|a, b| {
                    let da = (a.rect.x - x).abs().min((a.rect.right() - x).abs());
                    let db = (b.rect.x - x).abs().min((b.rect.right() - x).abs());
                    da.total_cmp(&db)
                })
            })
            .map_or(0, |p| p.index)
    }

    // ----- intents -----------------------------------------------------------

    /// Move element `id` so that its [`move_point`] (bounding box centre
    /// for groups) lands on canvas point `point` in this target. The home
    /// page follows the point unless `keep_page`; groups keep theirs (it
    /// only matters for empty groups). Page backgrounds only change their
    /// home page.
    pub fn move_to(&self, id: &str, point: Vec2, opts: MoveOptions) -> Result<Vec<Op>, EditError> {
        let e = self.effective(id)?;
        let scene = self.scene()?;
        let mut fixed = ElementPatch::default();
        if !opts.keep_page && !Self::is_group(&e) {
            let page = Self::page_at(&scene, point.x);
            if page != e.page {
                fixed.page = Some(Some(page));
            }
        }
        if matches!(e.kind, ElementKind::Background(_)) {
            return Ok(self.write(&e.id, fixed));
        }
        self.solve(
            Vec::new(),
            &e.id,
            &fixed,
            Probe::Move,
            point,
            &self.write_to(),
        )
    }

    /// Canvas point of element `id` that [`Editor::move_to`] places: its
    /// [`move_point`], for groups the centre of the bounds of their drawn
    /// descendants. Hidden elements are measured as if visible.
    pub fn reference_point(&self, id: &str) -> Result<Vec2, EditError> {
        self.base_element(id)?;
        self.current(&[], id, Probe::Move)
    }

    /// Move element `id` by `delta` canvas lu.
    pub fn move_by(&self, id: &str, delta: Vec2, opts: MoveOptions) -> Result<Vec<Op>, EditError> {
        let from = self.reference_point(id)?;
        self.move_to(id, from + delta, opts)
    }

    /// Resize element `id` to `size` (canvas lu) keeping handle `fixed`
    /// (usually the one opposite the dragged handle) in place.
    ///
    /// - devices: width only, the height follows the model (a height alone
    ///   is converted with the current aspect); `content_scale` is undone;
    /// - images, shapes, free text boxes: width and height (`stretch`
    ///   anchors undone); an image without `height` keeps following its
    ///   asset's aspect unless a height is given;
    /// - margin text boxes: the margins move (left/right for the width,
    ///   top/bottom for the height; a box without `bottom` gets one only
    ///   when a height is given);
    /// - groups and page backgrounds cannot be resized.
    pub fn resize(&self, id: &str, size: ResizeSpec, fixed: Handle) -> Result<Vec<Op>, EditError> {
        let e = self.effective(id)?;
        let scene = self.scene_focused(id)?;
        let r = scene
            .element(id)
            .ok_or_else(|| EditError::NotOnCanvas(id.to_owned()))?;
        for v in [size.width, size.height].into_iter().flatten() {
            if !(v.is_finite() && v >= 0.0) {
                return Err(EditError::Invalid(format!(
                    "size must be a non-negative number, got {v}"
                )));
            }
        }
        let layout = scene.frame.layout;
        let sy = if e.effective_anchor_y() == crate::model::AnchorY::Stretch {
            layout.stretch_factor()
        } else {
            1.0
        };
        let cur = self.drawn_size(r);
        let mut patch = ElementPatch::default();
        match &e.kind {
            ElementKind::Group(_) | ElementKind::Background(_) => {
                return Err(EditError::Unsupported(format!(
                    "{} elements cannot be resized",
                    e.kind_tag().as_str()
                )));
            }
            ElementKind::Device(_) => {
                let k = match &r.props {
                    ResolvedKind::Device(d) => d.content_scale,
                    _ => 1.0,
                };
                let w = match (size.width, size.height) {
                    (Some(w), _) => w,
                    (None, Some(h)) if cur.y > 0.0 => h * cur.x / cur.y,
                    _ => return Ok(Vec::new()),
                };
                patch.width = Some(Some(clean(w / k)));
            }
            ElementKind::Text(t) if t.r#box.is_some() => {
                let b = t.r#box.unwrap_or_default();
                let mut nb = b;
                if let Some(w) = size.width {
                    let dw = w - cur.x;
                    let u = fixed.unit().x;
                    // The fixed side keeps its margin; the other one moves.
                    let (dl, dr) = (f64::midpoint(u, 1.0) * dw, f64::midpoint(1.0, -u) * dw);
                    nb.left = Some(clean(b.left.unwrap_or(0.0) - dl));
                    nb.right = Some(clean(b.right.unwrap_or(0.0) - dr));
                }
                if let Some(h) = size.height {
                    let dh = (h - cur.y) / sy;
                    let u = fixed.unit().y;
                    let (dt, db) = (f64::midpoint(u, 1.0) * dh, f64::midpoint(1.0, -u) * dh);
                    let top = b.top.unwrap_or(0.0);
                    // A box without `bottom` reaches to the page bottom.
                    let bottom = b.bottom.unwrap_or_else(|| {
                        layout.reference_page_height
                            - layout.unanchor_y(layout.page_height, e.effective_anchor_y())
                    });
                    nb.top = Some(clean(top - dt));
                    nb.bottom = Some(clean(bottom - db));
                }
                if nb != b {
                    patch.r#box = Some(Some(nb));
                }
            }
            ElementKind::Text(_) | ElementKind::Image(_) | ElementKind::Shape(_) => {
                if let Some(w) = size.width {
                    patch.width = Some(Some(clean(w)));
                }
                if let Some(h) = size.height {
                    patch.height = Some(Some(clean(h / sy)));
                }
            }
        }
        if patch.is_empty() {
            return Ok(Vec::new());
        }
        let goal = self
            .probe(&scene, id, Probe::Handle(fixed))
            .unwrap_or(r.center);
        self.solve(
            Vec::new(),
            &e.id,
            &patch,
            Probe::Handle(fixed),
            goal,
            &self.write_to(),
        )
    }

    fn scene_focused(&self, id: &str) -> Result<ResolvedScene, EditError> {
        let mut p = self.view.clone();
        self.resolve_focused(&mut p, id)
    }

    /// Sum of the rotations of the groups enclosing `id` (effective values).
    fn ancestors_rotation(&self, id: &str) -> f64 {
        ancestors(self.strip, id)
            .iter()
            .map(|g| {
                Self::effective_in(
                    self.project,
                    &self.strip.id,
                    g.id.as_str(),
                    &self.write_to(),
                )
                .rotation
            })
            .sum()
    }

    /// Rotate element `id` to `degrees` on the canvas (enclosing groups'
    /// rotations included) around its pivot. Stored normalised to
    /// `(-180, 180]`.
    pub fn rotate_to(&self, id: &str, degrees: f64) -> Result<Vec<Op>, EditError> {
        let e = self.effective(id)?;
        if !degrees.is_finite() {
            return Err(EditError::Invalid(format!("invalid angle {degrees}")));
        }
        if matches!(e.kind, ElementKind::Background(_)) {
            return Err(EditError::Unsupported(
                "page backgrounds cannot be rotated".into(),
            ));
        }
        let own = clean(normalize_angle(degrees - self.ancestors_rotation(id)));
        if (own - e.rotation).abs() < 1e-12 {
            return Ok(Vec::new());
        }
        Ok(self.write(
            &e.id,
            ElementPatch {
                rotation: Some((own != 0.0).then_some(own)),
                ..ElementPatch::default()
            },
        ))
    }

    /// Set the rotation pivot of element `id` to canvas point `pivot`
    /// (`None` = the element centre). With `keep_visual_position` the
    /// element is moved so that it stays where it is drawn now (otherwise a
    /// rotated element jumps, since it now turns around another point).
    pub fn set_pivot(
        &self,
        id: &str,
        pivot: Option<Vec2>,
        keep_visual_position: bool,
    ) -> Result<Vec<Op>, EditError> {
        let e = self.effective(id)?;
        if Self::is_group(&e) || matches!(e.kind, ElementKind::Background(_)) {
            return Err(EditError::Unsupported(format!(
                "the pivot of {} elements cannot be set on the canvas",
                e.kind_tag().as_str()
            )));
        }
        let scene = self.scene_focused(id)?;
        let r = scene
            .element(id)
            .ok_or_else(|| EditError::NotOnCanvas(id.to_owned()))?;
        let k = match &r.props {
            ResolvedKind::Device(d) => d.content_scale,
            _ => 1.0,
        };
        // The stored offset is relative to the default pivot (the content
        // centre of auto-height texts, else the element centre).
        let base = match &r.props {
            ResolvedKind::Text(t) => t.default_pivot,
            _ => Vec2::ZERO,
        };
        // Drawn centre V = c + p − R·p; the new world pivot P = c' + p'
        // keeps V when p' = R⁻¹(P − V).
        let v = local_to_canvas(r, Vec2::ZERO);
        let new = pivot.map(|p| {
            let local = (p - v).rotated_around(Vec2::ZERO, -r.rotation) * (1.0 / k) - base;
            Point::new(clean(local.x), clean(local.y))
        });
        let new = new.filter(|p| !p.is_zero());
        if new == e.pivot {
            return Ok(Vec::new());
        }
        let fixed = ElementPatch {
            pivot: Some(new),
            ..ElementPatch::default()
        };
        if !keep_visual_position {
            return Ok(self.write(&e.id, fixed));
        }
        let goal = move_point(r);
        self.solve(
            Vec::new(),
            &e.id,
            &fixed,
            Probe::Move,
            goal,
            &self.write_to(),
        )
    }

    /// Make `page` the home page of element `id`. With
    /// `keep_visual_position` the position is converted so the element
    /// stays where it is drawn; otherwise it keeps its page-relative
    /// position and moves with the page.
    pub fn set_home_page(
        &self,
        id: &str,
        page: u32,
        keep_visual_position: bool,
    ) -> Result<Vec<Op>, EditError> {
        let e = self.effective(id)?;
        if page >= self.strip.page.count {
            return Err(EditError::Op(OpError::PageOutOfRange {
                element: id.to_owned(),
                strip: self.strip.id.to_string(),
                page,
                count: self.strip.page.count,
            }));
        }
        if page == e.page {
            return Ok(Vec::new());
        }
        let fixed = ElementPatch {
            page: Some(Some(page)),
            ..ElementPatch::default()
        };
        if !keep_visual_position
            || Self::is_group(&e)
            || matches!(e.kind, ElementKind::Background(_))
        {
            return Ok(self.write(&e.id, fixed));
        }
        let goal = self.current(&[], id, Probe::Move)?;
        self.solve(
            Vec::new(),
            &e.id,
            &fixed,
            Probe::Move,
            goal,
            &self.write_to(),
        )
    }

    /// Change the painting order of `id` among its siblings (list order;
    /// `z` is a separate field). Always changes the base.
    pub fn reorder_z(&self, id: &str, order: ZOrder) -> Result<Vec<Op>, EditError> {
        let e = self.base_element(id)?;
        let siblings = siblings(self.strip, id);
        let len = siblings.len();
        let Some(i) = siblings.iter().position(|s| s.id == e.id) else {
            return Err(EditError::UnknownElement(id.to_owned()));
        };
        let to = match order {
            ZOrder::Front => len - 1,
            ZOrder::Back => 0,
            ZOrder::Forward => (i + 1).min(len - 1),
            ZOrder::Backward => i.saturating_sub(1),
        };
        Ok(if to == i {
            Vec::new()
        } else {
            vec![Op::MoveElement {
                id: e.id.clone(),
                index: to,
            }]
        })
    }

    /// Duplicate `id` (with children and variant overrides; new unique ids
    /// via [`unique_id`]) right above the original, moved by `offset`
    /// canvas lu (in the base and in overrides that set the position).
    /// Returns the ops and the new id.
    pub fn duplicate(&self, id: &str, offset: Vec2) -> Result<(Vec<Op>, ElementId), EditError> {
        let e = self.base_element(id)?;
        let mut copy = e.clone();
        let mut taken = BTreeSet::new();
        let mut renames = Vec::new();
        rename_tree(self.project, &mut copy, &mut taken, &mut renames);
        let new_id = copy.id.clone();
        // Offset: `at` (or the margins) of the copy itself.
        let sy = {
            let scene = self.scene()?;
            if e.effective_anchor_y() == crate::model::AnchorY::Stretch {
                scene.frame.layout.stretch_factor()
            } else {
                1.0
            }
        };
        let d = Vec2::new(offset.x, offset.y / sy);
        let shift = |el: &Element| -> ElementPatch {
            match Param::of(el) {
                p @ Param::At(_) if Self::is_group(el) => p.patch(offset),
                p => p.patch(d),
            }
        };
        if offset != Vec2::ZERO && !matches!(copy.kind, ElementKind::Background(_)) {
            let (moved, _) = copy.patched(&shift(&copy)).map_err(EditError::Invalid)?;
            copy = moved;
        }
        let mut overrides = crate::ops::VariantOverrides::new();
        for (vid, v) in &self.strip.variants {
            for (old, new) in &renames {
                if let Some(p) = v.overrides.get(old) {
                    let mut p = p.clone();
                    if old.as_str() == id && offset != Vec2::ZERO {
                        let base = self.base_element(id)?;
                        if let Ok((eff, _)) = base.patched(&p) {
                            let moved = shift(&eff);
                            if p.at.is_some() {
                                p.at = moved.at;
                            }
                            if p.r#box.is_some() {
                                p.r#box = moved.r#box;
                            }
                        }
                    }
                    overrides
                        .entry(vid.clone())
                        .or_default()
                        .insert(new.clone(), p);
                }
            }
        }
        let parent = parent_of(self.strip, id);
        let index = siblings(self.strip, id)
            .iter()
            .position(|s| s.id.as_str() == id)
            .map(|i| i + 1);
        Ok((
            vec![Op::AddElement {
                strip: self.strip.id.clone(),
                parent: parent.map(|p| p.id.clone()),
                index,
                element: copy,
                overrides,
            }],
            new_id,
        ))
    }

    /// Remove `id` with its children and their variant overrides.
    pub fn remove(&self, id: &str) -> Result<Vec<Op>, EditError> {
        let e = self.base_element(id)?;
        Ok(vec![Op::RemoveElement { id: e.id.clone() }])
    }

    /// Put `ids` (siblings of one list) into a new group at the position of
    /// the lowest of them. The group has no offset or rotation, so nothing
    /// moves. `group_id` defaults to a unique `group`. Returns the ops and
    /// the group id.
    pub fn group(
        &self,
        ids: &[ElementId],
        group_id: Option<&str>,
    ) -> Result<(Vec<Op>, ElementId), EditError> {
        let Some(first) = ids.first() else {
            return Err(EditError::Invalid("nothing to group".into()));
        };
        let parent = parent_of(self.strip, first.as_str()).map(|p| p.id.clone());
        let siblings = siblings(self.strip, first.as_str());
        let mut members = Vec::new();
        for id in ids {
            let e = self.base_element(id.as_str())?;
            if parent_of(self.strip, id.as_str()).map(|p| p.id.clone()) != parent {
                return Err(EditError::Invalid(format!(
                    "`{id}` is not in the same group as `{first}`"
                )));
            }
            let i = siblings
                .iter()
                .position(|s| s.id == e.id)
                .ok_or_else(|| EditError::UnknownElement(id.to_string()))?;
            if !members.iter().any(|(_, m): &(usize, ElementId)| m == id) {
                members.push((i, e.id.clone()));
            }
        }
        members.sort();
        let gid = match group_id {
            Some(g) => ElementId::parse(g).map_err(EditError::Invalid)?,
            None => unique_id(self.project, "group"),
        };
        let page = self.base_element(first.as_str())?.page;
        let group = Element {
            id: gid.clone(),
            page,
            at: Point::ZERO,
            width: None,
            height: None,
            rotation: 0.0,
            pivot: None,
            anchor_y: None,
            opacity: 1.0,
            z: 0,
            clip: None,
            visible: true,
            visible_in: None,
            kind: ElementKind::Group(GroupProps::default()),
        };
        let mut ops = vec![Op::AddElement {
            strip: self.strip.id.clone(),
            parent: parent.clone(),
            index: Some(members[0].0),
            element: group,
            overrides: crate::ops::VariantOverrides::new(),
        }];
        for (_, id) in &members {
            ops.push(Op::MoveElementTo {
                id: id.clone(),
                parent: Some(gid.clone()),
                index: None,
            });
        }
        Ok((ops, gid))
    }

    /// Dissolve group `id`: its children take its place in the parent list
    /// and keep their drawn position and rotation in this target (the
    /// group's offset and rotation are folded into the base of each child,
    /// opacity multiplied, `z` added). Overrides of the group itself are
    /// dropped (restored by undo).
    pub fn ungroup(&self, id: &str) -> Result<Vec<Op>, EditError> {
        let g = self.base_element(id)?;
        let ElementKind::Group(props) = &g.kind else {
            return Err(EditError::Invalid(format!("`{id}` is not a group")));
        };
        let parent = parent_of(self.strip, id).map(|p| p.id.clone());
        let index = siblings(self.strip, id)
            .iter()
            .position(|s| s.id.as_str() == id)
            .unwrap_or(0);
        // Base geometry: the target without variant.
        let base_view = Editor {
            project: self.project,
            options: self.options,
            target: self.target,
            strip: self.strip,
            lang: self.lang.clone(),
            variant: None,
            view: {
                let mut v = self.project.clone();
                if let Some(t) = v.targets.iter_mut().find(|t| t.id == self.target.id) {
                    t.variant = None;
                }
                v
            },
        };
        let mut goals = Vec::new();
        for c in &props.children {
            let goal = if matches!(c.kind, ElementKind::Background(_)) {
                None
            } else {
                Some(base_view.current(&[], c.id.as_str(), Probe::Move)?)
            };
            goals.push(goal);
        }
        let mut ops = Vec::new();
        for (i, c) in props.children.iter().enumerate() {
            ops.push(Op::MoveElementTo {
                id: c.id.clone(),
                parent: parent.clone(),
                index: Some(index + i + 1),
            });
        }
        ops.push(Op::RemoveElement { id: g.id.clone() });
        for (c, goal) in props.children.iter().zip(goals) {
            let rotation = clean(normalize_angle(c.rotation + g.rotation));
            let fixed = ElementPatch {
                rotation: Some((rotation != 0.0).then_some(rotation)),
                opacity: Some(Some(c.opacity * g.opacity)),
                z: Some(Some(c.z + g.z)),
                ..ElementPatch::default()
            };
            if let Some(goal) = goal {
                ops = base_view.solve(ops, &c.id, &fixed, Probe::Move, goal, &WriteTo::Base)?;
            } else {
                ops.push(Op::UpdateElement {
                    id: c.id.clone(),
                    patch: fixed,
                });
            }
        }
        Ok(ops)
    }

    /// Reset field `field` of the override of `id` in the edit variant (or
    /// the whole override). Empty without an edit variant or override.
    pub fn reset_override(&self, id: &str, field: Option<&str>) -> Result<Vec<Op>, EditError> {
        let e = self.base_element(id)?;
        match &self.variant {
            None => Ok(Vec::new()),
            Some(v) => reset_override(self.project, &self.strip.id, v, &e.id, field),
        }
    }

    /// Fields of `id` overridden by the edit variant (YAML names).
    pub fn overridden_fields(&self, id: &str) -> Vec<&'static str> {
        self.variant
            .as_ref()
            .and_then(|v| self.strip.variants.get(v))
            .and_then(|v| v.overrides.get(id))
            .map(ElementPatch::fields)
            .unwrap_or_default()
    }

    /// Kind of element `id`.
    pub fn kind(&self, id: &str) -> Option<ElementKindTag> {
        self.strip.element(id).map(Element::kind_tag)
    }
}

/// Give `e` and its descendants unique ids (recorded in `taken`); records
/// `(old, new)` pairs.
fn rename_tree(
    project: &Project,
    e: &mut Element,
    taken: &mut BTreeSet<ElementId>,
    renames: &mut Vec<(ElementId, ElementId)>,
) {
    let new = unique_id_except(project, e.id.as_str(), taken);
    taken.insert(new.clone());
    renames.push((e.id.clone(), new.clone()));
    e.id = new;
    if let Some(children) = e.children_mut() {
        for c in children {
            rename_tree(project, c, taken, renames);
        }
    }
}

/// Groups enclosing `id`, outermost first.
fn ancestors<'s>(strip: &'s Strip, id: &str) -> Vec<&'s Element> {
    fn walk<'s>(list: &'s [Element], id: &str, path: &mut Vec<&'s Element>) -> bool {
        for e in list {
            if e.id.as_str() == id {
                return true;
            }
            path.push(e);
            if walk(e.children(), id, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::new();
    walk(&strip.elements, id, &mut path);
    path
}

fn parent_of<'s>(strip: &'s Strip, id: &str) -> Option<&'s Element> {
    ancestors(strip, id).last().copied()
}

/// The list holding `id` (strip elements or the parent group's children).
fn siblings<'s>(strip: &'s Strip, id: &str) -> &'s [Element] {
    parent_of(strip, id).map_or(&strip.elements, |p| p.children())
}

/// Make element `id` and its enclosing groups visible in `project` (base
/// and all overrides), so a hidden element can still be measured.
fn force_visible(project: &mut Project, strip: &StripId, id: &str) {
    fn walk(list: &mut [Element], chain: &[ElementId]) {
        for e in list {
            if chain.contains(&e.id) {
                e.visible = true;
                e.visible_in = None;
            }
            if let Some(c) = e.children_mut() {
                walk(c, chain);
            }
        }
    }
    let Some(s) = project.strips.iter_mut().find(|s| &s.id == strip) else {
        return;
    };
    let mut chain: Vec<ElementId> = ancestors(s, id).iter().map(|e| e.id.clone()).collect();
    chain.push(ElementId::new(id));
    walk(&mut s.elements, &chain);
    for v in s.variants.values_mut() {
        for c in &chain {
            if let Some(p) = v.overrides.get_mut(c) {
                p.visible = None;
                p.visible_in = None;
            }
        }
    }
}
