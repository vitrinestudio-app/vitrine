//! Editing: ops transactions, undo/redo, history state.

use arrangia_core::ops::Op;
use serde::Deserialize;

use super::{AppState, NoArgs};
use crate::error::CmdResult;
use crate::session::{ApplyResult, HistoryState};

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
