//! Events from the backend to the UI. The Tauri app forwards them with
//! `AppHandle::emit`, the dev bridge as server-sent events.
//!
//! | event | payload |
//! |---|---|
//! | `project-changed` | [`crate::session::ProjectChanged`]: reason + what changed |
//! | `project-edited` | [`crate::session::ProjectEdited`]: a step was applied, undone or redone (actor, label, changed ids) |
//! | `project-error` | `{message}`: the folder changed but does not load |
//! | `export-progress` | `{file, done, total}` per exported file |
//! | `export-finished` | `{cancelled, written, errors}` |

use std::sync::{Mutex, PoisonError};

use serde_json::Value;

pub const PROJECT_CHANGED: &str = "project-changed";
pub const PROJECT_ERROR: &str = "project-error";
pub const PROJECT_EDITED: &str = "project-edited";
pub const EXPORT_PROGRESS: &str = "export-progress";
pub const EXPORT_FINISHED: &str = "export-finished";

/// Receives backend events. Must be cheap and non-blocking; called from
/// worker threads (watcher, export).
pub trait EventSink: Send + Sync {
    fn emit(&self, event: &str, payload: Value);
}

/// Drops all events.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _: &str, _: Value) {}
}

/// Keeps all events (tests).
#[derive(Debug, Default)]
pub struct RecordingSink {
    events: Mutex<Vec<(String, Value)>>,
}

impl RecordingSink {
    /// All events so far.
    pub fn events(&self) -> Vec<(String, Value)> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Payloads of the events named `name`.
    pub fn named(&self, name: &str) -> Vec<Value> {
        self.events()
            .into_iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| v)
            .collect()
    }
}

impl EventSink for RecordingSink {
    fn emit(&self, event: &str, payload: Value) {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((event.to_owned(), payload));
    }
}
