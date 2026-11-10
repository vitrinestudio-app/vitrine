//! # arrangia-gui (library part)
//!
//! The Tauri desktop app (`src/main.rs`, thin `#[tauri::command]` wrappers)
//! and the dev HTTP bridge (`arrangia-devbridge`, feature `devbridge`) on
//! top of the shared application layer `arrangia-app` (ADR 0008, ADR 0013).
//! The modules of `arrangia-app` are re-exported, so `arrangia_gui::commands`
//! etc. keep working.
//!
//! ```text
//! bridge    dev HTTP bridge (feature `devbridge`)
//! ```

#[cfg(feature = "devbridge")]
pub mod bridge;

pub use arrangia_app::{
    AppInfo, AppState, CommandError, ErrorCode, EventSink, Reply, Session, app_info, commands,
    dialogs, dispatch, error, events, preview, session, settings, testutil, watch,
};
