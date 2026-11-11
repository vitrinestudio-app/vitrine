//! # vitrine-gui (library part)
//!
//! The Tauri desktop app (`src/main.rs`, thin `#[tauri::command]` wrappers)
//! and the dev HTTP bridge (`vitrine-devbridge`, feature `devbridge`) on
//! top of the shared application layer `vitrine-app` (ADR 0008, ADR 0013).
//! The modules of `vitrine-app` are re-exported, so `vitrine_gui::commands`
//! etc. keep working.
//!
//! ```text
//! bridge    dev HTTP bridge (feature `devbridge`)
//! ```

#[cfg(feature = "devbridge")]
pub mod bridge;

pub use vitrine_app::{
    Actor, AppInfo, AppState, CommandError, ErrorCode, EventSink, Reply, Session, ViewState, about,
    app_info, commands, dialogs, dispatch, dispatch_as, error, events, preview, session, settings,
    store_test, stores, testutil, upload, view, watch,
};
