//! Editing: ops transactions, undo/redo, history state, overlay data and
//! the `edit_*` gesture intents (`core::edit`, ADR 0007).
//!
//! Every `edit_*` command takes the view context `{target, lang, variant}`
//! (`target` default: the first target, `lang` default: the project's
//! default language; `variant` missing = the target's own variant, `null` =
//! the base elements, a name = that variant of the target's strip), turns
//! the gesture into ops with a [`core::edit::Editor`](Editor) and applies
//! them as **one** undo step with a readable history label. Commands taking
//! several `ids` plan them one after another on a scratch copy, so each
//! intent sees the previous ones. An intent without ops (e.g. "bring to
//! front" on the topmost element) answers `applied: false` and saves
//! nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use arrangia_core::Project;
use arrangia_core::edit::{
    EditContext, EditError, Editor, Handle, MoveOptions, Overlay, ResizeSpec, ZOrder,
};
use arrangia_core::geometry::Vec2;
use arrangia_core::model::{ElementId, VariantId};
use arrangia_core::ops::Op;
use arrangia_core::resolve::FsAssets;
use serde::{Deserialize, Deserializer, Serialize};

use super::project::check_target;
use super::{AppState, NoArgs, lang_or_default};
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::session::{ApplyResult, HistoryState, Session};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplyOpsArgs {
    /// Applied as one transaction (one undo step).
    pub ops: Vec<Op>,
    /// Undo label (default: derived from the ops).
    #[serde(default)]
    pub label: Option<String>,
}

pub fn apply_ops(app: &AppState, args: ApplyOpsArgs) -> CmdResult<ApplyResult> {
    app.session()?.apply_ops(args.ops, args.label)
}

pub fn undo(app: &AppState, _: NoArgs) -> CmdResult<ApplyResult> {
    app.session()?.undo()
}

pub fn redo(app: &AppState, _: NoArgs) -> CmdResult<ApplyResult> {
    app.session()?.redo()
}

pub fn history_state(app: &AppState, _: NoArgs) -> CmdResult<HistoryState> {
    Ok(app.session()?.history_state())
}

// ----- view context ----------------------------------------------------------

/// `Option<Option<T>>` that tells a missing field (`None`, via
/// `#[serde(default)]`) from an explicit `null` (`Some(None)`).
#[allow(clippy::option_option)]
pub(super) fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

/// What the canvas shows: target × language × variant.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[allow(clippy::option_option)]
pub struct ViewContext {
    /// Default: the first target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Default: the project's default language.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    /// Missing = the target's own variant, `null` = the base elements, a
    /// name = that variant of the target's strip.
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub variant: Option<Option<String>>,
}

/// Declares an args struct with the view context fields
/// (`target`, `lang`, `variant`) in front of its own fields.
macro_rules! view_args {
    ($(#[$meta:meta])* pub struct $name:ident { $($body:tt)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Deserialize)]
        #[serde(deny_unknown_fields)]
        #[allow(clippy::option_option)]
        pub struct $name {
            #[serde(default)]
            pub target: Option<String>,
            #[serde(default)]
            pub lang: Option<String>,
            #[serde(default, deserialize_with = "double_option")]
            pub variant: Option<Option<String>>,
            $($body)*
        }

        impl $name {
            /// The view context of these arguments.
            pub fn view(&self) -> ViewContext {
                ViewContext {
                    target: self.target.clone(),
                    lang: self.lang.clone(),
                    variant: self.variant.clone(),
                }
            }
        }
    };
}

impl From<EditError> for CommandError {
    fn from(e: EditError) -> Self {
        match e {
            EditError::Resolve(e) => e.into(),
            EditError::Op(e) => e.into(),
            EditError::UnknownElement(_)
            | EditError::WrongStrip { .. }
            | EditError::UnknownVariant { .. } => {
                CommandError::new(ErrorCode::NotFound, e.to_string())
            }
            EditError::Invalid(_) => CommandError::invalid(e.to_string()),
            EditError::NotOnCanvas(_) | EditError::Unsupported(_) | EditError::Unsolvable(_) => {
                CommandError::new(ErrorCode::OpRejected, e.to_string())
            }
        }
    }
}

impl ViewContext {
    /// The edit context in `project` (defaults filled in, ids checked).
    pub fn edit_context(&self, session: &Session, project: &Project) -> CmdResult<EditContext> {
        let target = match &self.target {
            Some(t) => {
                check_target(project, t)?;
                t.clone()
            }
            None => project
                .targets
                .first()
                .map(|t| t.id.to_string())
                .ok_or_else(|| CommandError::not_found("the project has no targets"))?,
        };
        let lang = lang_or_default(session, self.lang.clone());
        let ctx = EditContext::new(&target, &lang)?;
        Ok(match &self.variant {
            None => ctx,
            Some(None) => ctx.base(),
            Some(Some(v)) => ctx.with_variant(VariantId::parse(v).map_err(CommandError::invalid)?),
        })
    }
}

/// Runs `f` with an editor on `project` for `ctx`.
fn with_editor<R>(
    session: &Session,
    project: &Project,
    ctx: &EditContext,
    f: impl FnOnce(&Editor) -> Result<R, EditError>,
) -> CmdResult<R> {
    let fs = FsAssets::new(session.dir());
    let render = session.render_context();
    let editor = Editor::new(project, ctx, render.resolve_options(&fs))?;
    Ok(f(&editor)?)
}

pub fn overlay(app: &AppState, args: OverlayArgs) -> CmdResult<Overlay> {
    let session = app.session()?;
    let project = session.snapshot();
    let ctx = args.view().edit_context(&session, &project)?;
    let render = session.render_context();
    with_editor(&session, &project, &ctx, |editor| {
        render.with_layouter(|l| editor.overlay(l))
    })
}

view_args! {
    /// Overlay (hit-test) data of the view.
    pub struct OverlayArgs {}
}

// ----- planning ----------------------------------------------------------------

/// Ops of one gesture plus the ids it created.
#[derive(Debug, Default)]
pub(super) struct Plan {
    pub ops: Vec<Op>,
    pub created: Vec<String>,
}

/// Plans `f` for each of `ids` in turn on a scratch copy of `project`, so
/// later intents see the effect of earlier ones.
fn plan_each(
    session: &Session,
    project: &Project,
    ctx: &EditContext,
    ids: &[String],
    mut f: impl FnMut(&Editor, &str) -> Result<(Vec<Op>, Option<ElementId>), EditError>,
) -> CmdResult<Plan> {
    let mut scratch = project.clone();
    let mut plan = Plan::default();
    for (i, id) in ids.iter().enumerate() {
        let (ops, created) = with_editor(session, &scratch, ctx, |e| f(e, id))?;
        if ops.is_empty() {
            continue;
        }
        if i + 1 < ids.len() {
            scratch.apply_ops(ops.clone())?;
        }
        plan.ops.extend(ops);
        plan.created.extend(created.map(|c| c.to_string()));
    }
    Ok(plan)
}

/// Parent of every element of the project (by id).
fn parents(project: &Project) -> BTreeMap<String, String> {
    fn walk(list: &[arrangia_core::model::Element], out: &mut BTreeMap<String, String>) {
        for e in list {
            for c in e.children() {
                out.insert(c.id.to_string(), e.id.to_string());
            }
            walk(e.children(), out);
        }
    }
    let mut out = BTreeMap::new();
    for s in &project.strips {
        walk(&s.elements, &mut out);
    }
    out
}

/// `ids` without duplicates and without elements whose enclosing group is
/// selected too (moving or removing the group already covers them).
fn top_level(project: &Project, ids: &[String]) -> CmdResult<Vec<String>> {
    if ids.is_empty() {
        return Err(CommandError::invalid("no element ids given"));
    }
    let parents = parents(project);
    let set: BTreeSet<&str> = ids.iter().map(String::as_str).collect();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for id in ids {
        if project.element(id).is_none() {
            return Err(CommandError::not_found(format!("unknown element `{id}`")));
        }
        let mut p = parents.get(id);
        let mut covered = false;
        while let Some(parent) = p {
            if set.contains(parent.as_str()) {
                covered = true;
                break;
            }
            p = parents.get(parent);
        }
        if !covered && seen.insert(id.clone()) {
            out.push(id.clone());
        }
    }
    Ok(out)
}

/// Position of every element in painting (list) order.
fn list_order(project: &Project) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for s in &project.strips {
        s.walk_elements(&mut |e| {
            let n = out.len();
            out.insert(e.id.to_string(), n);
        });
    }
    out
}

/// "`id`" or "3 elements".
fn subject(ids: &[String]) -> String {
    let unique: BTreeSet<&String> = ids.iter().collect();
    match unique.len() {
        1 => ids[0].clone(),
        n => format!("{n} elements"),
    }
}

/// A gesture shared by the `edit_*` commands and the drag previews
/// (`render_preview_edit`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Gesture {
    /// Move the reference point of `id` to canvas point (`x`, `y`).
    Move {
        id: String,
        x: f64,
        y: f64,
        #[serde(default)]
        keep_page: bool,
    },
    /// Move `ids` by (`dx`, `dy`) canvas lu.
    MoveBy {
        ids: Vec<String>,
        dx: f64,
        dy: f64,
        #[serde(default)]
        keep_page: bool,
    },
    /// Resize `id` (canvas lu) keeping handle `fixed` in place.
    Resize {
        id: String,
        #[serde(default)]
        width: Option<f64>,
        #[serde(default)]
        height: Option<f64>,
        fixed: Handle,
    },
    /// Rotate `id` to `degrees` on the canvas.
    Rotate { id: String, degrees: f64 },
}

fn finite(values: &[f64]) -> CmdResult<()> {
    if values.iter().all(|v| v.is_finite()) {
        Ok(())
    } else {
        Err(CommandError::invalid("coordinates must be finite numbers"))
    }
}

impl Gesture {
    /// History label.
    pub fn label(&self) -> String {
        match self {
            Gesture::Move { id, .. } => format!("Move {id}"),
            Gesture::MoveBy { ids, .. } => format!("Move {}", subject(ids)),
            Gesture::Resize { id, .. } => format!("Resize {id}"),
            Gesture::Rotate { id, degrees } => format!("Rotate {id} to {degrees:.1}°"),
        }
    }

    /// The ops of the gesture on `project`.
    pub(super) fn plan(
        &self,
        session: &Session,
        project: &Project,
        ctx: &EditContext,
    ) -> CmdResult<Plan> {
        match self {
            Gesture::Move {
                id,
                x,
                y,
                keep_page,
            } => {
                finite(&[*x, *y])?;
                let opts = MoveOptions {
                    keep_page: *keep_page,
                };
                let ops = with_editor(session, project, ctx, |e| {
                    e.move_to(id, Vec2::new(*x, *y), opts)
                })?;
                Ok(Plan {
                    ops,
                    created: Vec::new(),
                })
            }
            Gesture::MoveBy {
                ids,
                dx,
                dy,
                keep_page,
            } => {
                finite(&[*dx, *dy])?;
                let ids = top_level(project, ids)?;
                let opts = MoveOptions {
                    keep_page: *keep_page,
                };
                plan_each(session, project, ctx, &ids, |e, id| {
                    Ok((e.move_by(id, Vec2::new(*dx, *dy), opts)?, None))
                })
            }
            Gesture::Resize {
                id,
                width,
                height,
                fixed,
            } => {
                finite(&[width.unwrap_or(0.0), height.unwrap_or(0.0)])?;
                if width.is_some_and(|w| w <= 0.0) || height.is_some_and(|h| h <= 0.0) {
                    return Err(CommandError::invalid("sizes must be positive"));
                }
                let spec = ResizeSpec {
                    width: *width,
                    height: *height,
                };
                let ops = with_editor(session, project, ctx, |e| e.resize(id, spec, *fixed))?;
                Ok(Plan {
                    ops,
                    created: Vec::new(),
                })
            }
            Gesture::Rotate { id, degrees } => {
                finite(&[*degrees])?;
                let ops = with_editor(session, project, ctx, |e| e.rotate_to(id, *degrees))?;
                Ok(Plan {
                    ops,
                    created: Vec::new(),
                })
            }
        }
    }
}

/// Result of an `edit_*` command: the [`ApplyResult`] plus the ids the
/// edit created (duplicates, a new group).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditResult {
    #[serde(flatten)]
    pub result: ApplyResult,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub created: Vec<String>,
}

/// Plans with `f` on the current project and applies the ops as one step.
fn edit(
    app: &AppState,
    view: &ViewContext,
    label: String,
    f: impl FnOnce(&Session, &Project, &EditContext) -> CmdResult<Plan>,
) -> CmdResult<EditResult> {
    let session = app.session()?;
    let project = session.snapshot();
    let ctx = view.edit_context(&session, &project)?;
    let plan = f(&session, &project, &ctx)?;
    let result = if plan.ops.is_empty() {
        session.unchanged()
    } else {
        session.apply_ops(plan.ops, Some(label))?
    };
    Ok(EditResult {
        result,
        created: plan.created,
    })
}

fn gesture(app: &AppState, view: &ViewContext, g: &Gesture) -> CmdResult<EditResult> {
    edit(app, view, g.label(), |s, p, c| g.plan(s, p, c))
}

// ----- edit commands -------------------------------------------------------------

view_args! {
    pub struct EditMoveArgs {
        pub id: String,
        /// Canvas point (lu) for the element's reference point (centre).
        pub x: f64,
        pub y: f64,
        /// Keep the home page instead of following the element centre.
        #[serde(default)]
        pub keep_page: bool,
    }
}

view_args! {
    pub struct EditMoveByArgs {
        pub ids: Vec<String>,
        pub dx: f64,
        pub dy: f64,
        #[serde(default)]
        pub keep_page: bool,
    }
}

view_args! {
    pub struct EditResizeArgs {
        pub id: String,
        /// New size in canvas lu as seen in the target (devices: width only).
        #[serde(default)]
        pub width: Option<f64>,
        #[serde(default)]
        pub height: Option<f64>,
        /// Handle that stays in place (usually opposite the dragged one).
        pub fixed: Handle,
    }
}

view_args! {
    pub struct EditRotateArgs {
        pub id: String,
        pub degrees: f64,
    }
}

view_args! {
    pub struct EditSetPageArgs {
        pub id: String,
        /// New home page (0-based).
        pub page: u32,
        /// Convert the position so the element stays where it is drawn
        /// (default); otherwise it keeps its page-relative position.
        #[serde(default = "yes")]
        pub keep_visual_position: bool,
    }
}

view_args! {
    pub struct EditReorderArgs {
        pub ids: Vec<String>,
        pub order: ZOrder,
    }
}

view_args! {
    pub struct EditDuplicateArgs {
        pub ids: Vec<String>,
        /// Offset of the copies in canvas lu (default 40, 40).
        #[serde(default = "dup_offset")]
        pub dx: f64,
        #[serde(default = "dup_offset")]
        pub dy: f64,
    }
}

view_args! {
    pub struct EditIdsArgs {
        pub ids: Vec<String>,
    }
}

view_args! {
    pub struct EditGroupArgs {
        /// Siblings of one list.
        pub ids: Vec<String>,
        /// Default: a unique `group`.
        #[serde(default)]
        pub group_id: Option<String>,
    }
}

view_args! {
    pub struct EditResetOverrideArgs {
        pub id: String,
        /// YAML field name (e.g. `at`); default: the whole override.
        #[serde(default)]
        pub field: Option<String>,
    }
}

fn yes() -> bool {
    true
}

fn dup_offset() -> f64 {
    40.0
}

pub fn edit_move(app: &AppState, args: EditMoveArgs) -> CmdResult<EditResult> {
    let g = Gesture::Move {
        id: args.id.clone(),
        x: args.x,
        y: args.y,
        keep_page: args.keep_page,
    };
    gesture(app, &args.view(), &g)
}

pub fn edit_move_by(app: &AppState, args: EditMoveByArgs) -> CmdResult<EditResult> {
    let g = Gesture::MoveBy {
        ids: args.ids.clone(),
        dx: args.dx,
        dy: args.dy,
        keep_page: args.keep_page,
    };
    gesture(app, &args.view(), &g)
}

pub fn edit_resize(app: &AppState, args: EditResizeArgs) -> CmdResult<EditResult> {
    let g = Gesture::Resize {
        id: args.id.clone(),
        width: args.width,
        height: args.height,
        fixed: args.fixed,
    };
    gesture(app, &args.view(), &g)
}

pub fn edit_rotate(app: &AppState, args: EditRotateArgs) -> CmdResult<EditResult> {
    let g = Gesture::Rotate {
        id: args.id.clone(),
        degrees: args.degrees,
    };
    gesture(app, &args.view(), &g)
}

pub fn edit_set_page(app: &AppState, args: EditSetPageArgs) -> CmdResult<EditResult> {
    let label = format!("Set home page of {} to page {}", args.id, args.page + 1);
    edit(app, &args.view(), label, |s, p, c| {
        let ops = with_editor(s, p, c, |e| {
            e.set_home_page(&args.id, args.page, args.keep_visual_position)
        })?;
        Ok(Plan {
            ops,
            created: Vec::new(),
        })
    })
}

pub fn edit_reorder(app: &AppState, args: EditReorderArgs) -> CmdResult<EditResult> {
    let verb = match args.order {
        ZOrder::Front => "Bring to front",
        ZOrder::Back => "Send to back",
        ZOrder::Forward => "Bring forward",
        ZOrder::Backward => "Send backward",
    };
    let label = format!("{verb}: {}", subject(&args.ids));
    edit(app, &args.view(), label, |s, p, c| {
        // Keep the relative order of the selection: elements moving up are
        // handled topmost first for single steps and bottommost first for
        // "front" (and the mirror image going down).
        let mut ids = top_level(p, &args.ids)?;
        let order = list_order(p);
        ids.sort_by_key(|id| order.get(id).copied().unwrap_or(0));
        if matches!(args.order, ZOrder::Forward | ZOrder::Back) {
            ids.reverse();
        }
        plan_each(s, p, c, &ids, |e, id| {
            Ok((e.reorder_z(id, args.order)?, None))
        })
    })
}

pub fn edit_duplicate(app: &AppState, args: EditDuplicateArgs) -> CmdResult<EditResult> {
    let label = format!("Duplicate {}", subject(&args.ids));
    edit(app, &args.view(), label, |s, p, c| {
        finite(&[args.dx, args.dy])?;
        let ids = top_level(p, &args.ids)?;
        plan_each(s, p, c, &ids, |e, id| {
            let (ops, new_id) = e.duplicate(id, Vec2::new(args.dx, args.dy))?;
            Ok((ops, Some(new_id)))
        })
    })
}

pub fn edit_remove(app: &AppState, args: EditIdsArgs) -> CmdResult<EditResult> {
    let label = format!("Delete {}", subject(&args.ids));
    edit(app, &args.view(), label, |s, p, c| {
        let ids = top_level(p, &args.ids)?;
        plan_each(s, p, c, &ids, |e, id| Ok((e.remove(id)?, None)))
    })
}

pub fn edit_group(app: &AppState, args: EditGroupArgs) -> CmdResult<EditResult> {
    let label = format!("Group {}", subject(&args.ids));
    edit(app, &args.view(), label, |s, p, c| {
        let ids = top_level(p, &args.ids)?;
        let ids: Vec<ElementId> = ids.iter().map(|i| ElementId::new(i)).collect();
        let (ops, group) = with_editor(s, p, c, |e| e.group(&ids, args.group_id.as_deref()))?;
        Ok(Plan {
            ops,
            created: vec![group.to_string()],
        })
    })
}

pub fn edit_ungroup(app: &AppState, args: EditIdsArgs) -> CmdResult<EditResult> {
    let label = format!("Ungroup {}", subject(&args.ids));
    edit(app, &args.view(), label, |s, p, c| {
        let ids = top_level(p, &args.ids)?;
        plan_each(s, p, c, &ids, |e, id| Ok((e.ungroup(id)?, None)))
    })
}

pub fn edit_reset_override(app: &AppState, args: EditResetOverrideArgs) -> CmdResult<EditResult> {
    let label = match &args.field {
        Some(f) => format!("Reset {f} of {}", args.id),
        None => format!("Reset overrides of {}", args.id),
    };
    edit(app, &args.view(), label, |s, p, c| {
        let ops = with_editor(s, p, c, |e| {
            e.reset_override(&args.id, args.field.as_deref())
        })?;
        Ok(Plan {
            ops,
            created: Vec::new(),
        })
    })
}

/// A scratch copy of the current project with `gesture` applied (drag
/// previews; memoised per gesture and revision, never saved).
pub(super) fn preview_project(
    session: &Session,
    view: &ViewContext,
    gesture: &Gesture,
) -> CmdResult<Arc<Project>> {
    let key = serde_json::to_string(&(view, gesture)).unwrap_or_default();
    session.edit_preview(key, |project| {
        let ctx = view.edit_context(session, project)?;
        let plan = gesture.plan(session, project, &ctx)?;
        let mut scratch = project.clone();
        scratch.apply_ops(plan.ops)?;
        Ok(scratch)
    })
}
