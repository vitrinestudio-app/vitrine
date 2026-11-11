//! The discovery file `<app data dir>/mcp.json` (ADR 0015): how
//! `vitrine mcp` finds the MCP endpoint of a running GUI.
//!
//! ```json
//! {"url": "http://127.0.0.1:53817/mcp", "port": 53817, "token": "…",
//!  "pid": 4711, "version": "0.0.0", "started_at": 1762790405123}
//! ```
//!
//! The GUI writes it when its endpoint starts (Unix permissions `0600`:
//! it holds the bearer token) and removes it when the endpoint stops or
//! the app exits cleanly. A file left behind by a crash is detected by a
//! failing `GET /health` (and a dead `pid`) and removed by the reader.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use vitrine_app::settings::AppPaths;

/// File name in the app data folder.
pub const FILE_NAME: &str = "mcp.json";

/// Contents of the discovery file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discovery {
    /// Streamable HTTP endpoint (`http://127.0.0.1:<port>/mcp`).
    pub url: String,
    pub port: u16,
    /// Bearer token (base64url, 32 random bytes).
    pub token: String,
    /// Process id of the GUI.
    pub pid: u32,
    /// App version of the GUI.
    pub version: String,
    /// Unix time (ms) the endpoint started.
    #[serde(default)]
    pub started_at: u64,
}

/// `<data dir>/mcp.json`.
pub fn path(paths: &AppPaths) -> PathBuf {
    paths.data_dir.join(FILE_NAME)
}

/// Writes `info` to `file` atomically, readable only by the user (Unix).
pub fn write(file: &Path, info: &Discovery) -> std::io::Result<()> {
    let dir = file.parent().unwrap_or(Path::new("."));
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".{FILE_NAME}.{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut f = options.open(&tmp)?;
    f.write_all(&serde_json::to_vec_pretty(info).unwrap_or_default())?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, file)
}

/// Reads the discovery file (`None`: missing or unreadable).
pub fn read(file: &Path) -> Option<Discovery> {
    let bytes = std::fs::read(file).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Removes `file` when it still describes the endpoint with `token` (a
/// newer GUI may have replaced it).
pub fn remove_if_ours(file: &Path, token: &str) {
    if read(file).is_some_and(|d| d.token == token) {
        let _ = std::fs::remove_file(file);
    }
}

/// Is process `pid` alive? `None` when this platform cannot tell.
pub fn pid_alive(pid: u32) -> Option<bool> {
    if pid == std::process::id() {
        return Some(true);
    }
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok()
            .map(|s| s.success())
    }
    #[cfg(windows)]
    {
        let out = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .ok()?;
        Some(String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\"")))
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// `GET /health` with the token: does the endpoint answer as Vitrine Studio?
pub fn healthy(info: &Discovery, timeout: Duration) -> bool {
    let addr = SocketAddr::from(([127, 0, 0, 1], info.port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, timeout) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(timeout));
    let _ = stream.set_write_timeout(Some(timeout));
    let request = format!(
        "GET /health HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nAuthorization: Bearer {}\r\nConnection: close\r\n\r\n",
        info.port, info.token
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut status = String::new();
    if BufReader::new(stream).read_line(&mut status).is_err() {
        return false;
    }
    status.starts_with("HTTP/1.1 200")
}

/// The endpoint of a running GUI: the discovery file in `paths` when its
/// endpoint answers. A stale file (no answer, process gone) is removed.
pub fn find(paths: &AppPaths) -> Option<Discovery> {
    let file = path(paths);
    let info = read(&file)?;
    if healthy(&info, Duration::from_millis(1500)) {
        return Some(info);
    }
    if pid_alive(info.pid) == Some(false) {
        remove_if_ours(&file, &info.token);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(port: u16, pid: u32) -> Discovery {
        Discovery {
            url: format!("http://127.0.0.1:{port}/mcp"),
            port,
            token: "secret".into(),
            pid,
            version: "0.0.0".into(),
            started_at: 1,
        }
    }

    #[test]
    fn write_read_remove() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(tmp.path());
        let file = path(&paths);
        let info = sample(1, std::process::id());
        write(&file, &info).unwrap();
        assert_eq!(read(&file), Some(info.clone()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&file).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        remove_if_ours(&file, "other");
        assert!(file.exists(), "a different token is not ours");
        remove_if_ours(&file, "secret");
        assert!(!file.exists());
    }

    #[test]
    fn stale_files_are_ignored_and_removed_when_the_process_is_gone() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = AppPaths::in_dir(tmp.path());
        let file = path(&paths);
        // A port nobody listens on.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        // Our own pid: alive, so the file stays (maybe a GUI still starting).
        write(&file, &sample(port, std::process::id())).unwrap();
        assert_eq!(find(&paths), None);
        assert!(file.exists());
        // A dead pid: removed.
        #[cfg(unix)]
        {
            let mut child = std::process::Command::new("true").spawn().unwrap();
            let pid = child.id();
            child.wait().unwrap();
            write(&file, &sample(port, pid)).unwrap();
            assert_eq!(pid_alive(pid), Some(false));
            assert_eq!(find(&paths), None);
            assert!(!file.exists());
        }
    }
}
