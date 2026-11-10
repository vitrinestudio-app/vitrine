//! Shared MCP test client: drives `arrangia mcp` over stdio with
//! newline-delimited JSON-RPC, like MCP clients (Claude Code, Claude Desktop, …) do.

#![allow(dead_code, clippy::needless_pass_by_value)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::Duration;

use serde_json::{Value, json};

/// Generous: debug builds render and export slowly.
pub const TIMEOUT: Duration = Duration::from_secs(240);

/// A minimal MCP client over the child's stdio.
pub struct Client {
    pub child: Child,
    pub stdin: Option<ChildStdin>,
    rx: Receiver<Value>,
    next_id: u64,
    /// Notifications received while waiting for responses.
    pub notifications: Vec<Value>,
}

impl Client {
    pub fn start(args: &[&str], app_dir: &Path, cwd: &Path) -> Client {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_arrangia"));
        cmd.args(args).current_dir(cwd);
        Self::spawn(cmd, app_dir, "arrangia-test")
    }

    /// Starts `cmd` (an `arrangia mcp` invocation) and initialises as
    /// `client_name`. Never starts a real GUI: `ARRANGIA_GUI` points to a
    /// missing file unless the caller set it.
    pub fn spawn(mut cmd: Command, app_dir: &Path, client_name: &str) -> Client {
        if !cmd.get_envs().any(|(k, _)| k == "ARRANGIA_GUI") {
            cmd.env("ARRANGIA_GUI", app_dir.join("no-gui-in-tests"));
        }
        let mut child = cmd
            .env("ARRANGIA_APP_DIR", app_dir)
            .env("NO_COLOR", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start arrangia mcp");
        let stdin = child.stdin.take().expect("stdin");
        let stdout = child.stdout.take().expect("stdout");
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if line.trim().is_empty() {
                    continue;
                }
                let v: Value = serde_json::from_str(&line)
                    .unwrap_or_else(|e| panic!("stdout is not JSON-RPC ({e}): {line}"));
                if tx.send(v).is_err() {
                    break;
                }
            }
        });
        let mut c = Client {
            child,
            stdin: Some(stdin),
            rx,
            next_id: 1,
            notifications: Vec::new(),
        };
        let init = c.request(
            "initialize",
            json!({
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": client_name, "version": "1.0"},
            }),
        );
        assert_eq!(init["serverInfo"]["name"], "arrangia", "{init}");
        assert!(
            init["instructions"]
                .as_str()
                .unwrap()
                .contains("render_preview")
        );
        assert!(init["capabilities"]["tools"].is_object());
        assert!(init["capabilities"]["resources"].is_object());
        c.send(&json!({"jsonrpc": "2.0", "method": "notifications/initialized"}));
        c
    }

    /// Closes stdin (the client goes away) and waits for the exit.
    pub fn close(&mut self) -> std::process::ExitStatus {
        self.stdin = None;
        self.child.wait().expect("wait for arrangia mcp")
    }

    pub fn send(&mut self, msg: &Value) {
        let stdin = self.stdin.as_mut().expect("stdin open");
        writeln!(stdin, "{msg}").expect("write to server");
        stdin.flush().expect("flush");
    }

    /// Sends a request and waits for its response; returns `result`.
    pub fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}));
        loop {
            let msg = match self.rx.recv_timeout(TIMEOUT) {
                Ok(m) => m,
                Err(RecvTimeoutError::Timeout) => panic!("no answer to {method}"),
                Err(RecvTimeoutError::Disconnected) => panic!("server exited during {method}"),
            };
            if msg["id"] == json!(id) {
                assert!(msg.get("error").is_none(), "{method}: {msg}");
                return msg["result"].clone();
            }
            self.notifications.push(msg);
        }
    }

    /// Calls a tool; returns the whole `CallToolResult`.
    pub fn call_raw(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({"name": name, "arguments": args}))
    }

    /// Calls a tool that must succeed; returns its structured content.
    pub fn call(&mut self, name: &str, args: Value) -> Value {
        let r = self.call_raw(name, args.clone());
        assert_eq!(r["isError"], false, "{name} {args}: {r}");
        let text = r["content"][0]["text"].as_str().expect("text block");
        assert_ne!(text, "");
        r["structuredContent"].clone()
    }

    /// Calls a tool that must fail; returns its error object.
    pub fn call_err(&mut self, name: &str, args: Value) -> Value {
        let r = self.call_raw(name, args.clone());
        assert_eq!(r["isError"], true, "{name} {args} should fail: {r}");
        r["structuredContent"]["error"].clone()
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Width and height of PNG bytes (IHDR).
pub fn png_size(bytes: &[u8]) -> (u32, u32) {
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n", "not a PNG");
    let be = |i: usize| u32::from_be_bytes(bytes[i..i + 4].try_into().unwrap());
    (be(16), be(20))
}

pub fn path_str(p: &Path) -> &str {
    p.to_str().expect("utf-8 path")
}
