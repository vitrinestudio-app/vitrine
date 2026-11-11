//! "Install command line tool" (App settings → Command line tool; ADR 0026):
//! makes the bundled `vitrine` CLI available in terminals
//! ([`crate::cli_install`]).

use serde::Deserialize;

use super::{AppState, NoArgs};
use crate::claude_desktop;
use crate::cli_install::{self, Status};
use crate::error::{CmdResult, CommandError, ErrorCode};

/// Where the CLI is and whether `vitrine` is installed for terminals.
pub fn cli_install_status(app: &AppState, _: NoArgs) -> CmdResult<Status> {
    Ok(cli_install::status(
        &claude_desktop::cli_path(),
        app.cli_link_dir(),
    ))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CliInstallArgs {
    /// The user confirmed in the app (required).
    #[serde(default)]
    pub confirm: bool,
    /// Remove instead of install.
    #[serde(default)]
    pub remove: bool,
}

/// Installs (or removes) `vitrine` for terminals. macOS asks for the
/// administrator password when `/usr/local/bin` is not writable.
pub fn cli_install(app: &AppState, args: CliInstallArgs) -> CmdResult<Status> {
    if !args.confirm {
        return Err(CommandError::new(
            ErrorCode::NotConfirmed,
            "installing the command line tool needs the user's confirmation",
        ));
    }
    cli_install::apply(
        &claude_desktop::cli_path(),
        app.cli_link_dir(),
        args.remove,
        true,
    )
}

#[cfg(all(test, unix))]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::dialogs::NoDialogs;
    use crate::events::RecordingSink;
    use crate::settings::AppPaths;

    #[test]
    fn needs_confirmation_and_uses_the_link_dir_override() {
        let tmp = tempfile::tempdir().unwrap();
        let bin = tmp.path().join("bin");
        let app = AppState::new(
            AppPaths::in_dir(&tmp.path().join("app")),
            Arc::new(RecordingSink::default()),
            Arc::new(NoDialogs),
        )
        .with_cli_link_dir(bin.clone());
        let e = cli_install(
            &app,
            CliInstallArgs {
                confirm: false,
                remove: false,
            },
        )
        .unwrap_err();
        assert_eq!(e.code, ErrorCode::NotConfirmed);
        let s = cli_install_status(&app, NoArgs {}).unwrap();
        if s.method == cli_install::Method::Symlink {
            assert_eq!(s.location, Some(bin.join("vitrine")));
        }
    }
}
