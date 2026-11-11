//! "Install command line tool" (ADR 0026): makes the `vitrine` CLI that
//! ships inside the app reachable as `vitrine` in a terminal.
//!
//! | platform | CLI in the bundle | how it is installed |
//! |---|---|---|
//! | macOS | `Vitrine Studio.app/Contents/MacOS/vitrine` | symlink `/usr/local/bin/vitrine` (admin prompt via `osascript` when the folder is not writable) |
//! | Windows | `vitrine.exe` next to the app | install folder added to the user `PATH` (`HKCU\Environment`, via PowerShell); also offered by the NSIS installer |
//! | Linux deb/rpm | `/usr/bin/vitrine` | nothing to do (package) |
//! | Linux `AppImage` | inside the image (temporary mount) | not possible: use the standalone CLI archive |
//! | other (dev builds) | next to the GUI binary | symlink in `~/.local/bin` (Linux) |
//!
//! Used by the GUI (`cli_install_*` commands, the user confirms first) and
//! by `vitrine install-cli` (the NSIS installer hook runs it). The link
//! folder can be redirected with [`LINK_DIR_ENV`] (tests, dev bridge), so
//! tests never touch `/usr/local/bin` or the real `PATH`.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::error::{CmdResult, CommandError};

/// Overrides the folder of the `vitrine` symlink (macOS/Linux).
pub const LINK_DIR_ENV: &str = "VITRINE_CLI_LINK_DIR";

/// How the CLI is made available on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// A `vitrine` symlink in a folder on `PATH`.
    Symlink,
    /// The CLI's folder in the user `PATH` (Windows).
    UserPath,
    /// Installed by the system package (deb/rpm): already on `PATH`.
    Package,
    /// Not possible from this build (`AppImage`).
    Unavailable,
}

/// What [`status`] found.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub method: Method,
    /// The CLI binary of this installation.
    pub cli_path: PathBuf,
    pub cli_exists: bool,
    /// `vitrine` is installed (symlink to this CLI / folder on `PATH` /
    /// package).
    pub installed: bool,
    /// The symlink (`Symlink`) or folder (`UserPath`) that is (or would be)
    /// written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<PathBuf>,
    /// Another file already has the symlink's name (not replaced).
    pub conflict: bool,
    /// Why it is not possible (`Unavailable`), or a hint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// The method of this platform/installation for the CLI at `cli`.
pub fn method_for(cli: &Path) -> Method {
    #[cfg(target_os = "macos")]
    {
        let _ = cli;
        Method::Symlink
    }
    #[cfg(windows)]
    {
        let _ = cli;
        Method::UserPath
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::env::var_os("APPIMAGE").is_some() {
            Method::Unavailable
        } else if cli.starts_with("/usr/") {
            Method::Package
        } else {
            Method::Symlink
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = cli;
        Method::Unavailable
    }
}

/// Folder of the symlink: `link_dir` (override), [`LINK_DIR_ENV`], else
/// `/usr/local/bin` (macOS) or `~/.local/bin` (Linux).
pub fn link_dir(link_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(d) = link_dir {
        return Some(d.to_owned());
    }
    if let Some(d) = std::env::var_os(LINK_DIR_ENV).filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(d));
    }
    if cfg!(target_os = "macos") {
        Some(PathBuf::from("/usr/local/bin"))
    } else {
        directories::BaseDirs::new().and_then(|b| b.executable_dir().map(Path::to_owned))
    }
}

/// Name of the CLI binary on this platform.
fn cli_file_name() -> String {
    format!("vitrine{}", std::env::consts::EXE_SUFFIX)
}

/// Current state for the CLI at `cli`.
pub fn status(cli: &Path, link_dir_override: Option<&Path>) -> Status {
    let method = method_for(cli);
    let mut out = Status {
        method,
        cli_path: cli.to_owned(),
        cli_exists: cli.is_file(),
        installed: false,
        location: None,
        conflict: false,
        note: None,
    };
    match method {
        Method::Symlink => {
            let Some(dir) = link_dir(link_dir_override) else {
                out.note = Some("no folder for the symlink (no home folder)".into());
                return out;
            };
            let link = dir.join(cli_file_name());
            match link_state(&link, cli) {
                LinkState::Missing => {}
                LinkState::Ours => out.installed = true,
                LinkState::Other => out.conflict = true,
            }
            out.location = Some(link);
        }
        Method::UserPath => {
            let dir = cli.parent().map(Path::to_owned);
            if let Some(d) = &dir {
                match user_path(d, PathMode::Status) {
                    Ok(has) => out.installed = has,
                    Err(e) => out.note = Some(e),
                }
            }
            out.location = dir;
        }
        Method::Package => {
            out.installed = true;
            out.location = Some(cli.to_owned());
        }
        Method::Unavailable => {
            out.note = Some(
                "this build (AppImage) cannot install the command line tool; \
                 download the standalone `vitrine` archive from the release page"
                    .into(),
            );
        }
    }
    out
}

/// Installs (`remove == false`) or removes the CLI. `elevate`: may ask
/// for administrator rights (macOS password prompt) when the link folder
/// is not writable. Returns the new status.
pub fn apply(
    cli: &Path,
    link_dir_override: Option<&Path>,
    remove: bool,
    elevate: bool,
) -> CmdResult<Status> {
    let before = status(cli, link_dir_override);
    match before.method {
        Method::Package => return Ok(before),
        Method::Unavailable => {
            return Err(CommandError::invalid(
                before.note.unwrap_or_else(|| "not available".into()),
            ));
        }
        Method::Symlink | Method::UserPath => {}
    }
    if !remove && !before.cli_exists {
        return Err(CommandError::not_found(format!(
            "the command line tool is missing: {}",
            cli.display()
        )));
    }
    if before.installed != remove {
        // Already in the wanted state.
        return Ok(before);
    }
    let location = before
        .location
        .clone()
        .ok_or_else(|| CommandError::io(before.note.clone().unwrap_or_default()))?;
    match before.method {
        Method::Symlink => {
            if before.conflict {
                return Err(CommandError::invalid(format!(
                    "{} already exists and is not Vitrine Studio's command line tool; \
                     remove it first",
                    location.display()
                )));
            }
            symlink_apply(cli, &location, remove, elevate)?;
        }
        Method::UserPath => {
            let mode = if remove {
                PathMode::Remove
            } else {
                PathMode::Add
            };
            user_path(&location, mode).map_err(CommandError::io)?;
        }
        Method::Package | Method::Unavailable => {}
    }
    Ok(status(cli, link_dir_override))
}

enum LinkState {
    Missing,
    Ours,
    Other,
}

fn link_state(link: &Path, cli: &Path) -> LinkState {
    let Ok(meta) = std::fs::symlink_metadata(link) else {
        return LinkState::Missing;
    };
    if !meta.file_type().is_symlink() {
        return LinkState::Other;
    }
    let same = match (std::fs::canonicalize(link), std::fs::canonicalize(cli)) {
        (Ok(a), Ok(b)) => a == b,
        // A dangling link to our (moved) CLI path still counts as ours.
        _ => std::fs::read_link(link).is_ok_and(|t| t == cli),
    };
    if same {
        LinkState::Ours
    } else if std::fs::read_link(link)
        .is_ok_and(|t| t.to_string_lossy().contains("Vitrine Studio.app"))
    {
        // An older copy of the app (moved/renamed): ours to replace.
        LinkState::Ours
    } else {
        LinkState::Other
    }
}

#[cfg(unix)]
fn symlink_apply(cli: &Path, link: &Path, remove: bool, elevate: bool) -> CmdResult<()> {
    let direct = (|| -> std::io::Result<()> {
        if std::fs::symlink_metadata(link).is_ok() {
            std::fs::remove_file(link)?;
        }
        if !remove {
            if let Some(dir) = link.parent() {
                std::fs::create_dir_all(dir)?;
            }
            std::os::unix::fs::symlink(cli, link)?;
        }
        Ok(())
    })();
    match direct {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && elevate => {
            elevated_symlink(cli, link, remove)
        }
        Err(e) => Err(CommandError::io(format!("{}: {e}", link.display()))),
    }
}

#[cfg(not(unix))]
fn symlink_apply(_cli: &Path, link: &Path, _remove: bool, _elevate: bool) -> CmdResult<()> {
    Err(CommandError::invalid(format!(
        "symlinks are not used on this platform ({})",
        link.display()
    )))
}

/// macOS: the same symlink change with administrator rights (the system
/// asks for the password; cancelling is an error).
#[cfg(target_os = "macos")]
fn elevated_symlink(cli: &Path, link: &Path, remove: bool) -> CmdResult<()> {
    let dir = link.parent().unwrap_or(Path::new("/usr/local/bin"));
    // Paths go in as arguments (`quoted form of`), never into the script text.
    let script = if remove {
        r#"on run argv
do shell script "rm -f " & quoted form of item 3 of argv with prompt "Vitrine Studio wants to remove its command line tool (vitrine)." with administrator privileges
end run"#
    } else {
        r#"on run argv
do shell script "mkdir -p " & quoted form of item 1 of argv & " && ln -sfn " & quoted form of item 2 of argv & " " & quoted form of item 3 of argv with prompt "Vitrine Studio wants to install its command line tool (vitrine)." with administrator privileges
end run"#
    };
    let out = std::process::Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .arg(dir)
        .arg(cli)
        .arg(link)
        .output()
        .map_err(|e| CommandError::io(format!("cannot run osascript: {e}")))?;
    if out.status.success() {
        Ok(())
    } else {
        let msg = String::from_utf8_lossy(&out.stderr);
        if msg.contains("-128") {
            Err(CommandError::new(
                crate::error::ErrorCode::Cancelled,
                "cancelled at the password prompt",
            ))
        } else {
            Err(CommandError::io(format!(
                "cannot change {}: {}",
                link.display(),
                msg.trim()
            )))
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn elevated_symlink(_cli: &Path, link: &Path, _remove: bool) -> CmdResult<()> {
    Err(CommandError::io(format!(
        "no permission to change {} (choose a folder you own with {LINK_DIR_ENV})",
        link.display()
    )))
}

#[derive(Debug, Clone, Copy)]
enum PathMode {
    Status,
    Add,
    Remove,
}

/// Windows: reads or changes the user `PATH` (`HKCU\Environment\Path`,
/// kept as `REG_EXPAND_SZ` and never truncated) through PowerShell, then
/// broadcasts the change so new terminals see it. Answers whether `dir`
/// is (now) in it.
#[cfg(windows)]
fn user_path(dir: &Path, mode: PathMode) -> Result<bool, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const SCRIPT: &str = r"$ErrorActionPreference = 'Stop'
$d = $env:VITRINE_PATH_DIR.TrimEnd('\')
$k = [Microsoft.Win32.Registry]::CurrentUser.CreateSubKey('Environment')
$p = [string]$k.GetValue('Path', '', 'DoNotExpandEnvironmentNames')
$parts = @($p.Split(';') | Where-Object { $_ -ne '' })
$has = @($parts | Where-Object { $_.TrimEnd('\') -ieq $d }).Count -gt 0
function Notify { [Environment]::SetEnvironmentVariable('VITRINE_PATH_REFRESH', '1', 'User'); [Environment]::SetEnvironmentVariable('VITRINE_PATH_REFRESH', $null, 'User') }
switch ($env:VITRINE_PATH_MODE) {
  'add' { if (-not $has) { $k.SetValue('Path', (($parts + $d) -join ';'), 'ExpandString'); Notify }; $has = $true }
  'remove' { if ($has) { $rest = @($parts | Where-Object { $_.TrimEnd('\') -ine $d }); $k.SetValue('Path', ($rest -join ';'), 'ExpandString'); Notify }; $has = $false }
}
if ($has) { 'yes' } else { 'no' }";
    let mode = match mode {
        PathMode::Status => "status",
        PathMode::Add => "add",
        PathMode::Remove => "remove",
    };
    let out = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            SCRIPT,
        ])
        .env("VITRINE_PATH_DIR", dir)
        .env("VITRINE_PATH_MODE", mode)
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("cannot run PowerShell: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cannot change the user PATH: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim() == "yes")
}

#[cfg(not(windows))]
fn user_path(_dir: &Path, _mode: PathMode) -> Result<bool, String> {
    Err("the user PATH is only changed on Windows".into())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn fake_cli(dir: &Path) -> PathBuf {
        let cli = dir.join("app").join("vitrine");
        std::fs::create_dir_all(cli.parent().unwrap()).unwrap();
        std::fs::write(&cli, b"#!/bin/sh\n").unwrap();
        cli
    }

    #[test]
    fn symlink_install_and_remove_in_an_override_folder() {
        if method_for(Path::new("/opt/x/vitrine")) != Method::Symlink {
            return; // AppImage environment
        }
        let tmp = tempfile::tempdir().unwrap();
        let cli = fake_cli(tmp.path());
        let bin = tmp.path().join("bin");

        let s = status(&cli, Some(&bin));
        assert!(!s.installed && !s.conflict && s.cli_exists);
        assert_eq!(s.location.as_deref(), Some(bin.join("vitrine").as_path()));

        let s = apply(&cli, Some(&bin), false, false).unwrap();
        assert!(s.installed, "{s:?}");
        assert_eq!(std::fs::read_link(bin.join("vitrine")).unwrap(), cli);
        // Idempotent.
        assert!(apply(&cli, Some(&bin), false, false).unwrap().installed);

        let s = apply(&cli, Some(&bin), true, false).unwrap();
        assert!(!s.installed);
        assert!(std::fs::symlink_metadata(bin.join("vitrine")).is_err());
    }

    #[test]
    fn a_foreign_file_is_never_replaced() {
        if method_for(Path::new("/opt/x/vitrine")) != Method::Symlink {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let cli = fake_cli(tmp.path());
        let bin = tmp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(bin.join("vitrine"), b"someone else's").unwrap();

        let s = status(&cli, Some(&bin));
        assert!(s.conflict && !s.installed);
        assert!(apply(&cli, Some(&bin), false, false).is_err());
        assert!(apply(&cli, Some(&bin), true, false).unwrap().conflict);
        assert_eq!(
            std::fs::read(bin.join("vitrine")).unwrap(),
            b"someone else's"
        );
    }

    #[test]
    fn missing_cli_is_an_error() {
        if method_for(Path::new("/opt/x/vitrine")) != Method::Symlink {
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let cli = tmp.path().join("nope").join("vitrine");
        let e = apply(&cli, Some(&tmp.path().join("bin")), false, false).unwrap_err();
        assert!(e.message.contains("missing"), "{}", e.message);
    }
}
