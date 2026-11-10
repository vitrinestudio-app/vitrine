//! `arrangia mcp` as a stdio ↔ Streamable HTTP proxy to a running GUI
//! (feature `proxy`, ADR 0015).
//!
//! Claude Desktop starts `arrangia mcp` and speaks newline-delimited
//! JSON-RPC over stdio. The proxy forwards every message unchanged as one
//! `POST /mcp` to the GUI's endpoint (bearer token from the discovery
//! file) and writes every JSON-RPC message of the answer – a JSON body or
//! the events of an SSE stream (progress notifications, then the result)
//! – back to stdout. It never looks into tool calls, so `clientInfo`
//! (e.g. Claude Desktop's) reaches the GUI as sent and the GUI's history
//! names the real client.
//!
//! - `initialize` is forwarded first and alone (it returns the
//!   `Mcp-Session-Id` of legacy-session protocol versions); the answer's
//!   `protocolVersion` (or a request's `_meta` protocol version) becomes
//!   the `MCP-Protocol-Version` header; `Mcp-Method`/`Mcp-Name` are set
//!   for protocol versions that route on them.
//! - Other messages are forwarded concurrently (a long `export` does not
//!   block `render_preview`).
//! - The GUI restarted (connection refused, unknown session): the proxy
//!   reads the discovery file again, replays `initialize` +
//!   `notifications/initialized` and retries the message once.
//! - An HTTP error answers a request with a JSON-RPC error, so the client
//!   never waits forever.
//! - stdin closed: in-flight requests finish (up to 60 s), the session is
//!   closed (`DELETE`).
//!
//! Server-initiated messages outside a request (the optional `GET` SSE
//! stream) are not forwarded; Arrangia sends none.

use std::sync::Arc;
use std::time::Duration;

use arrangia_app::settings::AppPaths;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, mpsc};
use tokio::task::JoinSet;

use crate::discovery::{self, Discovery};

const HEADER_SESSION_ID: &str = "Mcp-Session-Id";
const HEADER_PROTOCOL_VERSION: &str = "MCP-Protocol-Version";
const HEADER_METHOD: &str = "Mcp-Method";
const HEADER_NAME: &str = "Mcp-Name";
const META_PROTOCOL_VERSION: &str = "io.modelcontextprotocol/protocolVersion";

/// Where the proxy sends to, and the session it holds.
#[derive(Debug, Clone, Default)]
struct Upstream {
    url: String,
    token: String,
    session: Option<String>,
    protocol: Option<String>,
    /// The client's `initialize` request (replayed after a GUI restart).
    initialize: Option<Value>,
}

/// One forwarding failure.
enum Failure {
    /// The GUI is gone or forgot the session: rediscover and retry.
    Reconnect(String),
    /// Anything else (answered to the client as a JSON-RPC error).
    Fatal(String),
}

struct Proxy {
    http: reqwest::Client,
    paths: AppPaths,
    up: Mutex<Upstream>,
    out: mpsc::UnboundedSender<String>,
}

/// Runs the proxy until stdin closes.
pub async fn run(paths: AppPaths, gui: Discovery) -> Result<(), String> {
    let http = reqwest::Client::builder()
        .no_proxy()
        .connect_timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("cannot create the HTTP client: {e}"))?;
    let (out, mut out_rx) = mpsc::unbounded_channel::<String>();
    let writer = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        while let Some(line) = out_rx.recv().await {
            if stdout.write_all(line.as_bytes()).await.is_err()
                || stdout.write_all(b"\n").await.is_err()
                || stdout.flush().await.is_err()
            {
                break;
            }
        }
    });
    let proxy = Arc::new(Proxy {
        http,
        paths,
        up: Mutex::new(Upstream {
            url: gui.url,
            token: gui.token,
            ..Upstream::default()
        }),
        out,
    });
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    let mut tasks = JoinSet::new();
    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                proxy.emit(&json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": format!("parse error: {e}")},
                }));
                continue;
            }
        };
        if msg["method"] == "initialize" {
            // Alone and first: later messages need its session id.
            proxy.up.lock().await.initialize = Some(msg.clone());
            proxy.forward(msg).await;
        } else {
            let p = proxy.clone();
            tasks.spawn(async move { p.forward(msg).await });
        }
        while tasks.try_join_next().is_some() {}
    }
    let _ = tokio::time::timeout(Duration::from_secs(60), async {
        while tasks.join_next().await.is_some() {}
    })
    .await;
    proxy.close_session().await;
    drop(proxy);
    let _ = writer.await;
    Ok(())
}

/// `value` as an HTTP header value (SEP-2243: Base64 sentinel when it
/// would not survive as a header).
fn header_value(value: &str) -> String {
    let plain = !value.is_empty()
        && !value.starts_with([' ', '\t'])
        && !value.ends_with([' ', '\t'])
        && !value.starts_with("=?base64?")
        && value.bytes().all(|b| (0x20..0x7f).contains(&b));
    if plain {
        value.to_owned()
    } else {
        use base64::Engine as _;
        format!(
            "=?base64?{}?=",
            base64::engine::general_purpose::STANDARD.encode(value)
        )
    }
}

/// `Mcp-Name` of a request (tool/prompt name, resource uri, task id).
fn name_of(msg: &Value) -> Option<&str> {
    let method = msg["method"].as_str()?;
    let key = match method {
        "tools/call" | "prompts/get" => "name",
        "resources/read" | "resources/subscribe" | "resources/unsubscribe" => "uri",
        "tasks/get" | "tasks/update" | "tasks/cancel" => "taskId",
        _ => return None,
    };
    msg["params"][key].as_str()
}

/// Splits an SSE stream into the `data` of its events.
#[derive(Debug, Default)]
struct SseParser {
    buf: String,
    data: Vec<String>,
}

impl SseParser {
    /// Feeds bytes; returns the complete events' data.
    fn feed(&mut self, chunk: &str) -> Vec<String> {
        self.buf.push_str(chunk);
        let mut events = Vec::new();
        while let Some(i) = self.buf.find('\n') {
            let line: String = self.buf.drain(..=i).collect();
            let line = line.trim_end_matches(['\n', '\r']);
            if line.is_empty() {
                let data = self.data.join("\n");
                self.data.clear();
                // Priming events carry no data.
                if !data.trim().is_empty() {
                    events.push(data);
                }
            } else if let Some(d) = line.strip_prefix("data:") {
                self.data.push(d.strip_prefix(' ').unwrap_or(d).to_owned());
            }
            // `event:`, `id:`, `retry:` and `:` comments are not needed.
        }
        events
    }

    /// The events still buffered when the stream ends.
    fn finish(&mut self) -> Vec<String> {
        let mut rest = std::mem::take(&mut self.buf);
        rest.push_str("\n\n");
        self.feed(&rest)
    }
}

impl Proxy {
    fn emit(&self, msg: &Value) {
        let _ = self.out.send(msg.to_string());
    }

    /// Writes a JSON body (one message or a batch) to stdout.
    fn emit_text(&self, text: &str) {
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Array(items)) => items.iter().for_each(|m| self.emit(m)),
            Ok(Value::Null) | Err(_) if text.trim().is_empty() => {}
            Ok(m) => self.emit(&m),
            Err(_) => {}
        }
    }

    /// Forwards `msg`; on a GUI restart reconnects and retries once.
    async fn forward(&self, msg: Value) {
        let first = self.send(&msg).await;
        let result = match first {
            Err(Failure::Reconnect(why)) => match self.reconnect(&msg).await {
                Ok(()) => self.send(&msg).await,
                Err(e) => Err(Failure::Fatal(format!("{why}; {e}"))),
            },
            other => other,
        };
        if let Err(Failure::Reconnect(e) | Failure::Fatal(e)) = result
            && msg.get("id").is_some_and(|id| !id.is_null())
            && msg.get("method").is_some()
        {
            self.emit(&json!({
                "jsonrpc": "2.0", "id": msg["id"],
                "error": {"code": -32603, "message": format!("Arrangia GUI: {e}")},
            }));
        }
    }

    /// Finds the GUI again (discovery file) and replays `initialize`.
    async fn reconnect(&self, failed: &Value) -> Result<(), String> {
        let paths = self.paths.clone();
        let gui = tokio::task::spawn_blocking(move || discovery::find(&paths))
            .await
            .ok()
            .flatten()
            .ok_or("no running Arrangia GUI found (is it still open?)")?;
        let init = {
            let mut up = self.up.lock().await;
            up.url = gui.url;
            up.token = gui.token;
            up.session = None;
            up.initialize.clone()
        };
        if failed["method"] == "initialize" {
            return Ok(());
        }
        if let Some(init) = init {
            // Answer dropped: the client already has one.
            self.post(&init, false)
                .await
                .map_err(|_| "re-initialising failed")?;
            let note = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
            self.post(&note, false)
                .await
                .map_err(|_| "re-initialising failed")?;
        }
        Ok(())
    }

    async fn send(&self, msg: &Value) -> Result<(), Failure> {
        self.post(msg, true).await
    }

    /// POSTs `msg`; with `relay` the answer goes to stdout.
    async fn post(&self, msg: &Value, relay: bool) -> Result<(), Failure> {
        let up = self.up.lock().await.clone();
        let mut req = self
            .http
            .post(&up.url)
            .bearer_auth(&up.token)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream")
            .body(msg.to_string());
        if let Some(s) = &up.session {
            req = req.header(HEADER_SESSION_ID, s);
        }
        let version = msg["params"]["_meta"][META_PROTOCOL_VERSION]
            .as_str()
            .map(str::to_owned)
            .or(up.protocol.clone());
        if let Some(v) = &version
            && msg["method"] != "initialize"
        {
            req = req.header(HEADER_PROTOCOL_VERSION, v);
        }
        if let Some(m) = msg["method"].as_str() {
            req = req.header(HEADER_METHOD, header_value(m));
            if let Some(n) = name_of(msg) {
                req = req.header(HEADER_NAME, header_value(n));
            }
        }
        let resp = req.send().await.map_err(|e| {
            if e.is_connect() {
                Failure::Reconnect(format!("cannot reach the GUI: {e}"))
            } else {
                Failure::Fatal(format!("request failed: {e}"))
            }
        })?;
        let status = resp.status();
        if msg["method"] == "initialize"
            && let Some(s) = resp
                .headers()
                .get(HEADER_SESSION_ID)
                .and_then(|v| v.to_str().ok())
        {
            self.up.lock().await.session = Some(s.to_owned());
        }
        if status == reqwest::StatusCode::NOT_FOUND && up.session.is_some() {
            return Err(Failure::Reconnect(
                "the GUI does not know this session".into(),
            ));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            // A JSON-RPC error body (e.g. header mismatch) goes through.
            if relay && serde_json::from_str::<Value>(&body).is_ok_and(|v| v.get("error").is_some())
            {
                self.emit_text(&body);
                return Ok(());
            }
            return Err(Failure::Fatal(format!(
                "HTTP {status}: {}",
                body.chars().take(300).collect::<String>()
            )));
        }
        if status == reqwest::StatusCode::ACCEPTED {
            return Ok(());
        }
        let sse = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|c| c.starts_with("text/event-stream"));
        if sse {
            return self.read_sse(resp, msg, relay).await;
        }
        let body = resp
            .text()
            .await
            .map_err(|e| Failure::Fatal(format!("reading the answer: {e}")))?;
        if msg["method"] == "initialize" {
            self.note_initialize(&body).await;
        }
        if relay {
            self.emit_text(&body);
        }
        Ok(())
    }

    /// Relays the events of an SSE answer as they arrive.
    async fn read_sse(
        &self,
        mut resp: reqwest::Response,
        msg: &Value,
        relay: bool,
    ) -> Result<(), Failure> {
        let mut parser = SseParser::default();
        let mut pending = Vec::new();
        loop {
            match resp.chunk().await {
                Ok(Some(bytes)) => {
                    pending.extend_from_slice(&bytes);
                    // Only complete UTF-8 goes to the parser.
                    let valid = match std::str::from_utf8(&pending) {
                        Ok(_) => pending.len(),
                        Err(e) => e.valid_up_to(),
                    };
                    let text = String::from_utf8_lossy(&pending[..valid]).into_owned();
                    pending.drain(..valid);
                    for data in parser.feed(&text) {
                        self.relay_data(msg, relay, &data).await;
                    }
                }
                Ok(None) => break,
                Err(e) => return Err(Failure::Fatal(format!("stream broke: {e}"))),
            }
        }
        for data in parser.finish() {
            self.relay_data(msg, relay, &data).await;
        }
        Ok(())
    }

    async fn relay_data(&self, msg: &Value, relay: bool, data: &str) {
        if msg["method"] == "initialize" {
            self.note_initialize(data).await;
        }
        if relay {
            self.emit_text(data);
        }
    }

    /// Remembers the negotiated protocol version.
    async fn note_initialize(&self, body: &str) {
        if let Ok(v) = serde_json::from_str::<Value>(body)
            && let Some(p) = v["result"]["protocolVersion"].as_str()
        {
            self.up.lock().await.protocol = Some(p.to_owned());
        }
    }

    /// Ends the MCP session (legacy protocol versions).
    async fn close_session(&self) {
        let up = self.up.lock().await.clone();
        let Some(session) = up.session else { return };
        let mut req = self
            .http
            .delete(&up.url)
            .bearer_auth(&up.token)
            .header(HEADER_SESSION_ID, session)
            .timeout(Duration::from_secs(3));
        if let Some(v) = &up.protocol {
            req = req.header(HEADER_PROTOCOL_VERSION, v);
        }
        let _ = req.send().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_events_split_across_chunks() {
        let mut p = SseParser::default();
        let none: Vec<String> = Vec::new();
        assert_eq!(p.feed("id: 1\nretry: 3000\ndata:\n\n"), none);
        assert_eq!(p.feed("event: message\ndata: {\"a\""), none);
        assert_eq!(
            p.feed(":1}\n\n: keep-alive\n\ndata: x\r\n"),
            vec!["{\"a\":1}"]
        );
        assert_eq!(p.feed("data: y\n\n"), vec!["x\ny"]);
        p.feed("data: tail");
        assert_eq!(p.finish(), vec!["tail"]);
    }

    #[test]
    fn standard_headers() {
        let call = json!({"method": "tools/call", "params": {"name": "element_add"}});
        assert_eq!(name_of(&call), Some("element_add"));
        let read = json!({"method": "resources/read", "params": {"uri": "arrangia://x"}});
        assert_eq!(name_of(&read), Some("arrangia://x"));
        assert_eq!(name_of(&json!({"method": "tools/list"})), None);
        assert_eq!(header_value("tools/call"), "tools/call");
        assert_eq!(header_value("é"), "=?base64?w6k=?=");
        assert_eq!(header_value(" x"), "=?base64?IHg=?=");
    }
}
