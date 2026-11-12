//! System fonts stored in the project (ADR 0028): status, store/update,
//! remove, and a rescan of the installed fonts.

use serde::{Deserialize, Serialize};
use vitrine_core::layout::stored::{self, FontStorePlan};
use vitrine_core::layout::system;

use super::{AppState, NoArgs};
use crate::error::{CmdResult, CommandError, ErrorCode};

/// [`fonts_embed_status`]: what is stored and what an update would do.
#[derive(Debug, Clone, Serialize)]
pub struct FontsEmbedStatus {
    #[serde(flatten)]
    pub plan: FontStorePlan,
    /// System fonts are on (app setting); off = nothing to store.
    pub system_enabled: bool,
    /// Size of the whole stored folder (fonts, manifest, notice).
    pub folder_bytes: u64,
}

fn index_if_enabled(app: &AppState) -> std::sync::Arc<system::SystemFontIndex> {
    let on = app.session().is_ok_and(|s| s.system_fonts());
    if on {
        system::index()
    } else {
        std::sync::Arc::new(system::SystemFontIndex::default())
    }
}

/// What "Store used system fonts in the project" would do now (dry run).
pub fn fonts_embed_status(app: &AppState, _: NoArgs) -> CmdResult<FontsEmbedStatus> {
    let session = app.session()?;
    let dir = session.dir().to_owned();
    let index = index_if_enabled(app);
    let project = session.snapshot();
    let plan = stored::plan(&project, &dir, &index);
    Ok(FontsEmbedStatus {
        system_enabled: session.system_fonts(),
        folder_bytes: vitrine_core::frame_import::embed::dir_bytes(&plan.dir),
        plan,
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FontsEmbedArgs {
    /// Only report what would happen.
    #[serde(default)]
    pub dry_run: bool,
}

/// Stores (or updates) the system fonts the project uses in
/// `fonts/system/` and re-renders with them.
pub fn fonts_embed(app: &AppState, args: FontsEmbedArgs) -> CmdResult<FontStorePlan> {
    let session = app.session()?;
    let dir = session.dir().to_owned();
    let index = index_if_enabled(app);
    let project = session.snapshot();
    if args.dry_run {
        return Ok(stored::plan(&project, &dir, &index));
    }
    let plan =
        stored::apply(&project, &dir, &index).map_err(|e| CommandError::new(ErrorCode::Io, e))?;
    session.fonts_changed();
    Ok(plan)
}

/// Removes the system fonts stored in the project; returns the bytes
/// freed.
pub fn fonts_unembed(app: &AppState, _: NoArgs) -> CmdResult<u64> {
    let session = app.session()?;
    let bytes = stored::remove(session.dir()).map_err(|e| CommandError::new(ErrorCode::Io, e))?;
    session.fonts_changed();
    Ok(bytes)
}

/// Rescans the installed system fonts (fonts installed while the app
/// runs) and re-renders; returns the number of installed families.
pub fn fonts_refresh(app: &AppState, _: NoArgs) -> CmdResult<usize> {
    let n = system::refresh().len();
    if let Ok(session) = app.session() {
        session.fonts_changed();
    }
    Ok(n)
}
