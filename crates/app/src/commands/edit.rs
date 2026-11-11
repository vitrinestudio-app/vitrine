//! Editing: ops transactions, undo/redo, history state, overlay data and
//! the `edit_*` gesture intents (`core::edit`, ADR 0007).
//!
//! Every `edit_*` command takes the view context `{target, lang, variant}`
//! (`target` default: the first target, `lang` default: the project's
//! default language; `variant` missing = the target's own variant, `null` =
//! the base elements, a name = that variant of the target's canvas), turns
//! the gesture into ops with a [`core::edit::Editor`](Editor) and applies
//! them as **one** undo step with a readable history label. Commands taking
//! several `ids` plan them one after another on a scratch copy, so each
//! intent sees the previous ones. An intent without ops (e.g. "bring to
//! front" on the topmost element) answers `applied: false` and saves
//! nothing.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Deserializer, Serialize};
use vitrine_core::Project;
use vitrine_core::edit::{
    ClipItem, EditContext, EditError, EditScope, Editor, Handle, MoveOptions, Overlay, ResizeSpec,
    ZOrder,
};
use vitrine_core::geometry::Vec2;
use vitrine_core::model::{CanvasId, ElementId, VariantId};
use vitrine_core::ops::Op;
use vitrine_core::resolve::FsAssets;

use super::project::check_target;
use super::{AppState, NoArgs, lang_or_default};
use crate::error::{CmdResult, CommandError, ErrorCode};
use crate::session::{ApplyResult, Clipboard, HistoryState, Session};

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
    /// name = that variant of the target's canvas.
    #[serde(
        default,
        deserialize_with = "double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub variant: Option<Option<String>>,
    /// `lang`: field edits of text elements go into the language override
    /// of `lang` (inspector toggle "only for <lang>"); default `all`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<EditScope>,
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
            #[serde(default)]
            pub scope: Option<vitrine_core::edit::EditScope>,
            $($body)*
        }

        impl $name {
            /// The view context of these arguments.
            pub fn view(&self) -> ViewContext {
                ViewContext {
                    target: self.target.clone(),
                    lang: self.lang.clone(),
                    variant: self.variant.clone(),
                    scope: self.scope,
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
            | EditError::WrongCanvas { .. }
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
        let mut ctx = EditContext::new(&target, &lang)?;
        ctx.scope = self.scope.unwrap_or_default();
        Ok(match &self.variant {
            None => ctx,
            Some(None) => ctx.base(),
            Some(Some(v)) => ctx.with_variant(VariantId::parse(v).map_err(CommandError::invalid)?),
        })
    }
}

/// Runs `f` with an editor on `project` for `ctx`.
pub(super) fn with_editor<R>(
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

/// The edit context for element `id`: `ctx` when its target shows the
/// element's canvas, else the same context on the first target showing that
/// canvas, so one command can edit elements of several canvases in one undo
/// step (e.g. "bring all titles to front" on the iPhone and iPad canvases).
/// A named variant must exist in that canvas too.
pub(super) fn ctx_for(project: &Project, ctx: &EditContext, id: &str) -> CmdResult<EditContext> {
    let Some((canvas, _)) = project.element(id) else {
        return Ok(ctx.clone()); // the editor reports the unknown element
    };
    if project
        .target(ctx.target.as_str())
        .is_some_and(|t| t.canvas == canvas.id)
    {
        return Ok(ctx.clone());
    }
    let target = project
        .targets
        .iter()
        .find(|t| t.canvas == canvas.id)
        .ok_or_else(|| {
            CommandError::invalid(format!(
                "element `{id}`: no target shows its canvas `{}`",
                canvas.id
            ))
        })?;
    if let Some(Some(v)) = &ctx.variant
        && !canvas.variants.contains_key(v)
    {
        return Err(CommandError::not_found(format!(
            "element `{id}`: canvas `{}` has no variant `{v}`",
            canvas.id
        )));
    }
    let mut out = ctx.clone();
    out.target = target.id.clone();
    Ok(out)
}

/// Plans `f` for each of `ids` in turn on a scratch copy of `project`, so
/// later intents see the effect of earlier ones. Elements of other canvases
/// than the view's are planned on a target showing their canvas
/// ([`ctx_for`]).
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
        let ctx = ctx_for(&scratch, ctx, id)?;
        let (ops, created) = with_editor(session, &scratch, &ctx, |e| f(e, id))?;
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
    fn walk(list: &[vitrine_core::model::Element], out: &mut BTreeMap<String, String>) {
        for e in list {
            for c in e.children() {
                out.insert(c.id.to_string(), e.id.to_string());
            }
            walk(e.children(), out);
        }
    }
    let mut out = BTreeMap::new();
    for s in &project.canvases {
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
    for s in &project.canvases {
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
pub(super) fn edit(
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
        /// The element (or `ids` to rotate several to the same angle).
        #[serde(default)]
        pub id: String,
        #[serde(default)]
        pub ids: Vec<String>,
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
        /// Reset the language override of the view language instead of
        /// the variant override.
        #[serde(default)]
        pub lang_override: bool,
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
    if args.ids.is_empty() {
        let g = Gesture::Rotate {
            id: args.id.clone(),
            degrees: args.degrees,
        };
        return gesture(app, &args.view(), &g);
    }
    let ids: Vec<String> = (!args.id.is_empty())
        .then(|| args.id.clone())
        .into_iter()
        .chain(args.ids.iter().cloned())
        .collect();
    let label = format!("Rotate {} to {:.1}°", subject(&ids), args.degrees);
    edit(app, &args.view(), label, |s, p, c| {
        finite(&[args.degrees])?;
        let ids = top_level(p, &ids)?;
        plan_each(s, p, c, &ids, |e, id| {
            Ok((e.rotate_to(id, args.degrees)?, None))
        })
    })
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

view_args! {
    pub struct EditPasteArgs {
        /// Home page (0-based) of the first copied element; the others
        /// keep their distance in pages. Default: the copied pages.
        #[serde(default)]
        pub page: Option<u32>,
    }
}

/// What the element clipboard holds (`edit_copy`, `edit_clipboard`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClipboardInfo {
    /// Copied top-level elements (0 = empty).
    pub count: usize,
    /// Canvas they were copied from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canvas: Option<String>,
}

fn clipboard_info(session: &Session) -> ClipboardInfo {
    let clip = session.clipboard();
    ClipboardInfo {
        count: clip.as_ref().map_or(0, |c| c.items.len()),
        canvas: clip.as_ref().map(|c| c.canvas.to_string()),
    }
}

/// Copies of the top-level elements of `ids` (all on the view's canvas).
fn copy_items(
    session: &Session,
    project: &Project,
    ctx: &EditContext,
    ids: &[String],
) -> CmdResult<(CanvasId, Vec<ClipItem>)> {
    let ids = top_level(project, ids)?;
    with_editor(session, project, ctx, |e| {
        let items = ids
            .iter()
            .map(|id| e.copy_item(id))
            .collect::<Result<Vec<_>, _>>()?;
        Ok::<_, EditError>((e.canvas().id.clone(), items))
    })
}

fn store_clipboard(session: &Session, canvas: CanvasId, items: Vec<ClipItem>) {
    *session.clipboard() = Some(Clipboard {
        canvas,
        items,
        pastes: 0,
    });
}

/// Copies the elements (with children and variant overrides) to the
/// element clipboard of the open project. Changes nothing.
pub fn edit_copy(app: &AppState, args: EditIdsArgs) -> CmdResult<ClipboardInfo> {
    let session = app.session()?;
    let project = session.snapshot();
    let ctx = args.view().edit_context(&session, &project)?;
    let (canvas, items) = copy_items(&session, &project, &ctx, &args.ids)?;
    store_clipboard(&session, canvas, items);
    Ok(clipboard_info(&session))
}

/// Copies the elements to the clipboard and deletes them (one undo step).
pub fn edit_cut(app: &AppState, args: EditIdsArgs) -> CmdResult<EditResult> {
    let label = format!("Cut {}", subject(&args.ids));
    edit(app, &args.view(), label, |s, p, c| {
        let (canvas, items) = copy_items(s, p, c, &args.ids)?;
        let ids: Vec<String> = items.iter().map(|i| i.element.id.to_string()).collect();
        let plan = plan_each(s, p, c, &ids, |e, id| Ok((e.remove(id)?, None)))?;
        store_clipboard(s, canvas, items);
        Ok(plan)
    })
}

/// Pastes the clipboard into the view's canvas (topmost, new ids; one
/// undo step). Pasting where the originals still are moves the copies by
/// 40 lu per paste, like duplicate.
pub fn edit_paste(app: &AppState, args: EditPasteArgs) -> CmdResult<EditResult> {
    let session = app.session()?;
    let Some(clip) = session.clipboard().clone() else {
        return Err(CommandError::invalid("the clipboard is empty"));
    };
    let label = match clip.items.as_slice() {
        [one] => format!("Paste {}", one.element.id),
        items => format!("Paste {} elements", items.len()),
    };
    let mut overlapped = false;
    let result = edit(app, &args.view(), label, |s, p, c| {
        with_editor(s, p, c, |e| {
            let first = clip.items.iter().map(|i| i.element.page).min().unwrap_or(0);
            overlapped = e.canvas().id == clip.canvas
                && args.page.is_none_or(|pg| pg == first)
                && clip
                    .items
                    .iter()
                    .any(|i| e.canvas().element(i.element.id.as_str()).is_some());
            let step = 40.0 * f64::from(clip.pastes + 1);
            let offset = if overlapped {
                Vec2::new(step, step)
            } else {
                Vec2::ZERO
            };
            let (ops, created) = e.paste(&clip.items, args.page, offset)?;
            Ok::<_, EditError>(Plan {
                ops,
                created: created.iter().map(ToString::to_string).collect(),
            })
        })
    })?;
    if overlapped && let Some(c) = session.clipboard().as_mut() {
        c.pastes += 1;
    }
    Ok(result)
}

/// What the element clipboard holds.
pub fn edit_clipboard(app: &AppState, _: NoArgs) -> CmdResult<ClipboardInfo> {
    let session = app.session()?;
    Ok(clipboard_info(&session))
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
            if args.lang_override {
                e.reset_lang_override(&args.id, args.field.as_deref())
            } else {
                e.reset_override(&args.id, args.field.as_deref())
            }
        })?;
        Ok(Plan {
            ops,
            created: Vec::new(),
        })
    })
}

/// Which override [`element_reset_override`] resets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverrideScope {
    /// A variant override (`variants.<v>.overrides.<id>`).
    Variant,
    /// A language override of a text element (`langs.<lang>`).
    Lang,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementResetOverrideArgs {
    #[serde(default)]
    pub id: Option<String>,
    /// Several elements, also of different canvases (one undo step).
    #[serde(default)]
    pub ids: Vec<String>,
    pub scope: OverrideScope,
    /// `variant`: the variant (default: per canvas, the variant of `target`
    /// when it shows the canvas, else of the first target showing it).
    #[serde(default)]
    pub variant: Option<String>,
    /// `lang`: the language whose override is reset (exactly this one,
    /// not its prefix languages). Required for `lang`.
    #[serde(default)]
    pub lang: Option<String>,
    /// Fields (YAML names) to reset; missing = the whole override.
    #[serde(default)]
    pub fields: Option<Vec<String>>,
    /// See `variant`.
    #[serde(default)]
    pub target: Option<String>,
}

/// The variant of `canvas` an override reset without explicit variant
/// works on: that of `target` when it shows the canvas, else that of the
/// first target showing the canvas.
fn default_variant(
    project: &Project,
    target: Option<&str>,
    canvas: &vitrine_core::model::Canvas,
) -> CmdResult<VariantId> {
    let t = target
        .and_then(|t| project.target(t))
        .filter(|t| t.canvas == canvas.id)
        .or_else(|| project.targets.iter().find(|t| t.canvas == canvas.id));
    t.and_then(|t| t.variant.clone()).ok_or_else(|| {
        CommandError::invalid(format!(
            "no variant given and the target showing canvas `{}` has none; pass `variant`",
            canvas.id
        ))
    })
}

/// Removes fields (or all) of variant or language overrides of elements –
/// the explicit "back to the base value". Nothing set: `applied: false`.
pub fn element_reset_override(
    app: &AppState,
    mut args: ElementResetOverrideArgs,
) -> CmdResult<EditResult> {
    use vitrine_core::model::{ElementPatch, Lang};

    // Duplicates would reset a field twice.
    if let Some(fields) = &mut args.fields {
        let mut seen = BTreeSet::new();
        fields.retain(|f| seen.insert(f.clone()));
    }
    let mut ids: Vec<String> = args.id.iter().chain(&args.ids).cloned().collect();
    let mut seen = BTreeSet::new();
    ids.retain(|i| seen.insert(i.clone()));
    if ids.is_empty() {
        return Err(CommandError::invalid("give `id` or `ids`"));
    }
    if let Some(t) = &args.target {
        check_target(&app.session()?.snapshot(), t)?;
    }
    if let Some(fields) = &args.fields {
        if fields.is_empty() {
            return Err(CommandError::invalid(
                "`fields` is empty; omit it to reset the whole override",
            ));
        }
        if let Some(f) = fields.iter().find(|f| !ElementPatch::is_field_name(f)) {
            return Err(CommandError::invalid(format!(
                "unknown element field `{f}` (allowed: {})",
                ElementPatch::field_names().join(", ")
            )));
        }
    }
    let lang = match (args.scope, &args.lang) {
        (OverrideScope::Lang, None) => {
            return Err(CommandError::invalid("scope `lang` needs `lang`"));
        }
        (OverrideScope::Lang, Some(l)) => Some(Lang::parse(l).map_err(CommandError::invalid)?),
        (OverrideScope::Variant, _) => None,
    };
    let what = match &args.fields {
        Some(f) => f.join(", "),
        None => "overrides".to_owned(),
    };
    let whose = match (&lang, &args.variant) {
        (Some(l), _) => format!(" ({l})"),
        (None, Some(v)) => format!(" ({v})"),
        (None, None) => String::new(),
    };
    let label = format!("Reset {what} of {}{whose}", subject(&ids));
    let view = ViewContext {
        target: args.target.clone(),
        ..ViewContext::default()
    };
    edit(app, &view, label, |_, p, _| {
        let mut ops = Vec::new();
        for id in &ids {
            ops.extend(match &lang {
                Some(l) => reset_lang_ops(p, id, l, args.fields.as_deref())?,
                None => reset_variant_ops(p, id, &args)?,
            });
        }
        Ok(Plan {
            ops,
            created: Vec::new(),
        })
    })
}

/// Ops resetting `fields` (all: `None`) of the `lang` override of `id`.
fn reset_lang_ops(
    p: &Project,
    id: &str,
    lang: &vitrine_core::model::Lang,
    fields: Option<&[String]>,
) -> CmdResult<Vec<Op>> {
    let (_, e) = p
        .element(id)
        .ok_or_else(|| CommandError::not_found(format!("unknown element `{id}`")))?;
    let t = e.text_props().ok_or_else(|| {
        CommandError::invalid(format!(
            "element `{id}` is not a text: only texts have language overrides"
        ))
    })?;
    let Some(current) = t.langs.get(lang) else {
        return Ok(Vec::new());
    };
    Ok(match fields {
        None => vec![Op::SetLangOverride {
            element: e.id.clone(),
            lang: lang.clone(),
            patch: None,
        }],
        Some(fields) => {
            let set = current.fields();
            fields
                .iter()
                .filter(|f| set.contains(&f.as_str()))
                .map(|f| Op::ResetLangOverrideField {
                    element: e.id.clone(),
                    lang: lang.clone(),
                    field: f.clone(),
                })
                .collect()
        }
    })
}

/// Ops resetting the fields of `args` in the variant override of `id`.
fn reset_variant_ops(p: &Project, id: &str, args: &ElementResetOverrideArgs) -> CmdResult<Vec<Op>> {
    let (canvas, e) = p
        .element(id)
        .ok_or_else(|| CommandError::not_found(format!("unknown element `{id}`")))?;
    let v = match &args.variant {
        Some(v) => VariantId::parse(v).map_err(CommandError::invalid)?,
        None => default_variant(p, args.target.as_deref(), canvas)?,
    };
    let Some(var) = canvas.variants.get(&v) else {
        return Err(CommandError::not_found(format!(
            "canvas `{}` has no variant `{v}`",
            canvas.id
        )));
    };
    if !var.overrides.contains_key(&e.id) {
        return Ok(Vec::new());
    }
    let fields: Vec<Option<&str>> = match &args.fields {
        None => vec![None],
        Some(f) => f.iter().map(|f| Some(f.as_str())).collect(),
    };
    let mut ops = Vec::new();
    for f in fields {
        ops.extend(vitrine_core::edit::reset_override(
            p, &canvas.id, &v, &e.id, f,
        )?);
    }
    Ok(ops)
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
