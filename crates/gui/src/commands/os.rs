//! Folder picker and "show in Finder/Explorer" (`std::process`, so the web
//! view keeps `core:default`).

use std::path::PathBuf;

use serde::Deserialize;

use super::AppState;
use crate::error::{CmdResult, CommandError};

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DialogPickFolderArgs {
    #[serde(default)]
    pub title: Option<String>,
}

/// Asks for a folder (`null` when cancelled).
pub fn dialog_pick_folder(
    app: &AppState,
    args: DialogPickFolderArgs,
) -> CmdResult<Option<PathBuf>> {
    let title = args.title.unwrap_or_else(|| "Choose folder".to_owned());
    Ok(app.dialogs().pick_folder(&title))
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevealPathArgs {
    /// Absolute, or relative to the open project.
    pub path: PathBuf,
}

/// Shows a file (selected) or folder in the system file manager.
pub fn reveal_path(app: &AppState, args: RevealPathArgs) -> CmdResult<()> {
    let path = if args.path.is_absolute() {
        args.path
    } else {
        app.session()?.dir().join(args.path)
    };
    if !path.exists() {
        return Err(CommandError::not_found(format!(
            "{} does not exist",
            path.display()
        )));
    }
    let mut cmd = reveal_command(&path);
    cmd.spawn()
        .map_err(|e| CommandError::io(format!("cannot open the file manager: {e}")))?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn reveal_command(path: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new("open");
    if path.is_dir() {
        c.arg(path);
    } else {
        c.arg("-R").arg(path);
    }
    c
}

#[cfg(target_os = "windows")]
fn reveal_command(path: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new("explorer");
    if path.is_dir() {
        c.arg(path);
    } else {
        let mut arg = std::ffi::OsString::from("/select,");
        arg.push(path);
        c.arg(arg);
    }
    c
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn reveal_command(path: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new("xdg-open");
    c.arg(if path.is_dir() {
        path
    } else {
        path.parent().unwrap_or(path)
    });
    c
}
