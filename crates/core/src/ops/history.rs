//! In-memory undo/redo history of transactions.

use super::{Op, OpError};
use crate::model::Project;

/// One undo step: the ops that were applied and the ops that undo them.
#[derive(Clone, Debug, PartialEq)]
struct Step {
    forward: Vec<Op>,
    backward: Vec<Op>,
}

/// Undo/redo stacks. Every [`History::apply`] call is one undo step,
/// regardless of how many ops it contains.
#[derive(Clone, Debug)]
pub struct History {
    undo: Vec<Step>,
    redo: Vec<Step>,
    limit: usize,
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

impl History {
    /// Default number of undo steps kept.
    pub const DEFAULT_LIMIT: usize = 500;

    pub fn new() -> Self {
        Self::with_limit(Self::DEFAULT_LIMIT)
    }

    /// History keeping at most `limit` undo steps (oldest are dropped).
    pub fn with_limit(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Apply ops as one transaction and record it. Clears the redo stack.
    pub fn apply(&mut self, project: &mut Project, ops: Vec<Op>) -> Result<(), OpError> {
        let backward = project.apply_ops(ops.clone())?;
        self.undo.push(Step {
            forward: ops,
            backward,
        });
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
        Ok(())
    }

    /// Undo the last transaction. Returns `false` if there is nothing to undo.
    /// If the undo fails (project changed in between), the step stays on
    /// the stack and the project is unchanged.
    pub fn undo(&mut self, project: &mut Project) -> Result<bool, OpError> {
        let Some(step) = self.undo.pop() else {
            return Ok(false);
        };
        match project.apply_ops(step.backward.clone()) {
            Ok(forward) => {
                self.redo.push(Step {
                    forward,
                    backward: step.backward,
                });
                Ok(true)
            }
            Err(e) => {
                self.undo.push(step);
                Err(e)
            }
        }
    }

    /// Redo the last undone transaction. Returns `false` if there is nothing to redo.
    pub fn redo(&mut self, project: &mut Project) -> Result<bool, OpError> {
        let Some(step) = self.redo.pop() else {
            return Ok(false);
        };
        match project.apply_ops(step.forward.clone()) {
            Ok(backward) => {
                self.undo.push(Step {
                    forward: step.forward,
                    backward,
                });
                Ok(true)
            }
            Err(e) => {
                self.redo.push(step);
                Err(e)
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Forget all steps (e.g. after reloading the project from disk).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
