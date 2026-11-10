//! The GUI's view (strip, target, language, variant, selection), pushed by
//! the GUI and read by other front ends (see [`crate::view`]).

use super::{AppState, NoArgs};
use crate::error::CmdResult;
use crate::view::ViewState;

/// What the GUI shows in the open project (empty until the GUI sent it).
pub fn view_get(app: &AppState, _: NoArgs) -> CmdResult<ViewState> {
    Ok(app.session()?.view())
}

/// Replaces the stored view of the open project; answers with it.
pub fn view_set(app: &AppState, args: ViewState) -> CmdResult<ViewState> {
    let session = app.session()?;
    session.set_view(args);
    Ok(session.view())
}
