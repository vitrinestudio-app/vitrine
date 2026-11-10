//! Dev HTTP bridge (feature `devbridge`, binary `arrangia-devbridge`): the
//! unchanged UI in a normal browser, for automated and visual checks
//! (docs/adr/0008). Never part of the release bundle.
//!
//! | route | |
//! |---|---|
//! | `GET /…` | static files from `crates/gui/ui/` |
//! | `POST /cmd/<name>` | JSON body = command args → JSON result, or for image commands the bare PNG/RGBA with `X-Width`, `X-Height`, `X-Format`, `X-Origin-X`, `X-Origin-Y`, `X-Scale` |
//! | `GET /events` | server-sent events (`event: <name>`, `data: <json>`) |
//!
//! Errors answer with a non-2xx status and the JSON `CommandError`. The
//! server binds `127.0.0.1` only. It is a minimal HTTP/1.1 server on
//! `std::net`: one thread per connection and one request per connection
//! (`Connection: close`). Keep-alive connections are deliberately not
//! supported: with `tiny_http` a burst of new connections (page reload
//! after a long export) could be queued behind pool threads blocked on idle
//! keep-alive sockets and stall for minutes.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;

use crate::commands::{AppState, Reply, dispatch};
use crate::error::{CommandError, ErrorCode};
use crate::events::EventSink;

/// Largest accepted request body.
const MAX_BODY: u64 = 16 * 1024 * 1024;
/// SSE keep-alive comment interval (also how fast a dead stream's thread
/// notices the client is gone).
const KEEPALIVE: Duration = Duration::from_secs(5);
/// Longest wait for a client to send its request.
const READ_TIMEOUT: Duration = Duration::from_secs(30);
/// Longest request head (request line + headers).
const MAX_HEAD: u64 = 64 * 1024;

/// Fans events out to all connected `/events` clients.
#[derive(Debug, Default)]
pub struct SseHub {
    clients: Mutex<Vec<Sender<String>>>,
}

impl SseHub {
    fn subscribe(&self) -> Receiver<String> {
        let (tx, rx) = mpsc::channel();
        self.clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(tx);
        rx
    }

    /// Number of connected clients.
    pub fn clients(&self) -> usize {
        self.clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}

impl EventSink for SseHub {
    fn emit(&self, event: &str, payload: Value) {
        let msg = format!("event: {event}\ndata: {payload}\n\n");
        self.clients
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .retain(|c| c.send(msg.clone()).is_ok());
    }
}

/// A running bridge.
pub struct BridgeServer {
    addr: SocketAddr,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for BridgeServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeServer")
            .field("addr", &self.addr)
            .finish_non_exhaustive()
    }
}

impl BridgeServer {
    /// `http://127.0.0.1:<port>/`.
    pub fn url(&self) -> String {
        format!("http://{}/", self.addr)
    }

    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Blocks until the server stops.
    pub fn join(mut self) {
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }

    /// Stops accepting requests.
    pub fn shutdown(mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop.
        let _ = TcpStream::connect(self.addr);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Starts the bridge on `127.0.0.1:port` (`0` = a free port), serving
/// `ui_dir` and the commands of `app`; `hub` must be the event sink `app`
/// was created with.
pub fn serve(
    app: Arc<AppState>,
    hub: Arc<SseHub>,
    ui_dir: PathBuf,
    port: u16,
) -> std::io::Result<BridgeServer> {
    let listener = TcpListener::bind(("127.0.0.1", port))?;
    let addr = listener.local_addr()?;
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let thread = std::thread::Builder::new()
        .name("devbridge-accept".into())
        .spawn(move || {
            for stream in listener.incoming() {
                if stopped.load(Ordering::SeqCst) {
                    break;
                }
                let Ok(stream) = stream else { continue };
                let (app, hub, ui) = (app.clone(), hub.clone(), ui_dir.clone());
                let _ = std::thread::Builder::new()
                    .name("devbridge-conn".into())
                    .spawn(move || connection(&app, &hub, &ui, stream));
            }
        })?;
    Ok(BridgeServer {
        addr,
        stop,
        thread: Some(thread),
    })
}

/// A parsed request.
struct Request {
    method: String,
    url: String,
    body: Vec<u8>,
}

/// An answer: status, headers, body.
struct Response {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Response {
    fn new(status: u16, content_type: &str, body: Vec<u8>) -> Self {
        Self {
            status,
            headers: vec![
                ("Content-Type", content_type.to_owned()),
                ("Cache-Control", "no-store".to_owned()),
            ],
            body,
        }
    }

    fn text(status: u16, body: &str) -> Self {
        Self::new(
            status,
            "text/plain; charset=utf-8",
            body.as_bytes().to_vec(),
        )
    }

    fn write(&self, w: &mut impl Write, head_only: bool) -> std::io::Result<()> {
        let mut head = format!(
            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
            self.status,
            reason(self.status),
            self.body.len()
        );
        for (k, v) in &self.headers {
            head.push_str(k);
            head.push_str(": ");
            head.push_str(v);
            head.push_str("\r\n");
        }
        head.push_str("\r\n");
        w.write_all(head.as_bytes())?;
        if !head_only {
            w.write_all(&self.body)?;
        }
        w.flush()
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        413 => "Payload Too Large",
        422 => "Unprocessable Entity",
        499 => "Client Closed Request",
        _ => "Error",
    }
}

/// Reads one request (`Err(response)`: answer this and close).
fn read_request(stream: &TcpStream) -> Result<Request, Option<Response>> {
    let mut reader = BufReader::new(stream.take(MAX_HEAD + MAX_BODY));
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) | Err(_) => return Err(None),
        Ok(_) => {}
    }
    let mut parts = line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return Err(Some(Response::text(400, "bad request line")));
    };
    let (method, url) = (method.to_owned(), target.to_owned());
    let mut length: u64 = 0;
    let mut head_len = line.len() as u64;
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return Err(None),
            Ok(n) => head_len += n as u64,
        }
        if head_len > MAX_HEAD {
            return Err(Some(Response::text(400, "request head too large")));
        }
        let l = line.trim_end();
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':')
            && k.trim().eq_ignore_ascii_case("content-length")
        {
            length = v
                .trim()
                .parse()
                .map_err(|_| Some(Response::text(400, "bad content-length")))?;
        }
    }
    if length > MAX_BODY {
        return Err(Some(Response::text(413, "request body too large")));
    }
    let mut body = vec![0; usize::try_from(length).unwrap_or(0)];
    reader.read_exact(&mut body).map_err(|_| None)?;
    Ok(Request { method, url, body })
}

fn connection(app: &AppState, hub: &SseHub, ui: &Path, mut stream: TcpStream) {
    let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
    let _ = stream.set_nodelay(true);
    let request = match read_request(&stream) {
        Ok(r) => r,
        Err(Some(resp)) => {
            let _ = resp.write(&mut stream, false);
            return;
        }
        Err(None) => return,
    };
    let path = request.url.split('?').next().unwrap_or("/").to_owned();
    if request.method == "GET" && path == "/events" {
        stream_events(hub, stream);
        return;
    }
    let head_only = request.method == "HEAD";
    let response = handle(app, ui, &request, &path);
    let _ = response.write(&mut stream, head_only);
    let _ = stream.shutdown(Shutdown::Both);
}

fn status_of(code: ErrorCode) -> u16 {
    match code {
        ErrorCode::InvalidArgs => 400,
        ErrorCode::UnknownCommand | ErrorCode::NotFound => 404,
        ErrorCode::NoProject | ErrorCode::Conflict | ErrorCode::Busy => 409,
        ErrorCode::OpRejected | ErrorCode::Load => 422,
        ErrorCode::Cancelled => 499,
        ErrorCode::NotImplemented => 501,
        ErrorCode::NotConfirmed | ErrorCode::Forbidden => 403,
        ErrorCode::Io | ErrorCode::Render | ErrorCode::Keychain | ErrorCode::Internal => 500,
    }
}

fn json_response(status: u16, body: &impl serde::Serialize) -> Response {
    Response::new(
        status,
        "application/json",
        serde_json::to_vec(body).unwrap_or_default(),
    )
}

fn handle(app: &AppState, ui: &Path, request: &Request, path: &str) -> Response {
    if let ("POST", Some(name)) = (request.method.as_str(), path.strip_prefix("/cmd/")) {
        let args: Value = if request.body.iter().all(u8::is_ascii_whitespace) {
            Value::Null
        } else {
            match serde_json::from_slice(&request.body) {
                Ok(v) => v,
                Err(e) => return json_response(400, &CommandError::from(e)),
            }
        };
        return match dispatch(app, name, args) {
            Ok(Reply::Json(v)) => json_response(200, &v),
            Ok(Reply::Image(img)) => {
                let mut r = Response::new(200, img.format.content_type(), img.data.clone());
                for (k, v) in img.http_headers() {
                    r.headers.push((k, v));
                }
                r
            }
            Err(e) => json_response(status_of(e.code), &e),
        };
    }
    if request.method == "GET" || request.method == "HEAD" {
        return serve_file(ui, path);
    }
    Response::text(405, "method not allowed")
}

/// Writes the SSE stream until the client goes away.
fn stream_events(hub: &SseHub, mut w: TcpStream) {
    let rx = hub.subscribe();
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nConnection: keep-alive\r\n\r\n: connected\n\n";
    if w.write_all(head.as_bytes())
        .and_then(|()| w.flush())
        .is_err()
    {
        return;
    }
    loop {
        let msg = match rx.recv_timeout(KEEPALIVE) {
            Ok(m) => m,
            Err(RecvTimeoutError::Timeout) => ": keep-alive\n\n".to_owned(),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if w.write_all(msg.as_bytes())
            .and_then(|()| w.flush())
            .is_err()
        {
            return;
        }
    }
}

fn content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
    {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}

fn serve_file(ui: &Path, url: &str) -> Response {
    let rel = url.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let rel_path = Path::new(rel);
    if !rel_path
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return Response::text(400, "bad path");
    }
    let path = ui.join(rel_path);
    match std::fs::read(&path) {
        Ok(data) => Response::new(200, content_type(&path), data),
        Err(_) => Response::text(404, "not found"),
    }
}
