//! Export progress for MCP progress notifications.
//!
//! The export command reports progress through the app's [`EventSink`]
//! (`export-progress`). [`ProgressRelay`] is an `EventSink` that passes
//! every event on to an inner sink (the GUI's, or none when headless) and
//! copies `export-progress` payloads to the listeners of running `export`
//! tool calls, which turn them into `notifications/progress`.

use std::sync::{Arc, Mutex, PoisonError};

use serde_json::Value;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use vitrine_app::EventSink;
use vitrine_app::events::{EXPORT_PROGRESS, NullSink};

/// Event sink that forwards everything and taps `export-progress`.
pub struct ProgressRelay {
    inner: Arc<dyn EventSink>,
    listeners: Mutex<Vec<UnboundedSender<Value>>>,
}

impl std::fmt::Debug for ProgressRelay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgressRelay").finish_non_exhaustive()
    }
}

impl Default for ProgressRelay {
    fn default() -> Self {
        Self::new(Arc::new(NullSink))
    }
}

impl ProgressRelay {
    /// Forwards all events to `inner`.
    pub fn new(inner: Arc<dyn EventSink>) -> Self {
        Self {
            inner,
            listeners: Mutex::new(Vec::new()),
        }
    }

    /// Receives the `export-progress` payloads (`{file, done, total}`) from
    /// now on until the receiver is dropped.
    pub fn subscribe(&self) -> UnboundedReceiver<Value> {
        let (tx, rx) = unbounded_channel();
        self.listeners
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tx);
        rx
    }
}

impl EventSink for ProgressRelay {
    fn emit(&self, event: &str, payload: Value) {
        if event == EXPORT_PROGRESS {
            self.listeners
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .retain(|tx| tx.send(payload.clone()).is_ok());
        }
        self.inner.emit(event, payload);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vitrine_app::events::RecordingSink;

    use super::*;

    #[test]
    fn forwards_and_taps_progress() {
        let inner = Arc::new(RecordingSink::default());
        let relay = ProgressRelay::new(inner.clone());
        let mut rx = relay.subscribe();
        relay.emit(EXPORT_PROGRESS, json!({"done": 1, "total": 2}));
        relay.emit("project-edited", json!({}));
        assert_eq!(rx.try_recv().unwrap()["done"], 1);
        assert!(rx.try_recv().is_err());
        assert_eq!(inner.events().len(), 2);
        drop(rx);
        relay.emit(EXPORT_PROGRESS, json!({"done": 2, "total": 2}));
        assert!(relay.listeners.lock().unwrap().is_empty());
    }
}
