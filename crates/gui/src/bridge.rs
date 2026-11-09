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
//! server binds `127.0.0.1` only and handles every request on its own
//! thread (`tiny_http`).

use std::io::{Read, Write};
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::Value;
use tiny_http::{Header, Method, Request, Response, Server};

use crate::commands::{AppState, Reply, dispatch};
use crate::error::{CommandError, ErrorCode};
use crate::events::EventSink;

/// Largest accepted request body.
const MAX_BODY: u64 = 16 * 1024 * 1024;
/// SSE keep-alive comment interval.
const KEEPALIVE: Duration = Duration::from_secs(15);

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
    server: Arc<Server>,
    addr: SocketAddr,
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
        self.server.unblock();
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
    let server = Server::http(("127.0.0.1", port)).map_err(std::io::Error::other)?;
    let addr = server
        .server_addr()
        .to_ip()
        .ok_or_else(|| std::io::Error::other("not an IP listener"))?;
    let server = Arc::new(server);
    let accept = server.clone();
    let thread = std::thread::Builder::new()
        .name("devbridge-accept".into())
        .spawn(move || {
            while let Ok(request) = accept.recv() {
                let (app, hub, ui) = (app.clone(), hub.clone(), ui_dir.clone());
                let _ = std::thread::Builder::new()
                    .name("devbridge-request".into())
                    .spawn(move || handle(&app, &hub, &ui, request));
            }
        })?;
    Ok(BridgeServer {
        server,
        addr,
        thread: Some(thread),
    })
}

fn header(name: &str, value: &str) -> Header {
    Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("valid header")
}

fn status_of(code: ErrorCode) -> u16 {
    match code {
        ErrorCode::InvalidArgs => 400,
        ErrorCode::UnknownCommand | ErrorCode::NotFound => 404,
        ErrorCode::NoProject | ErrorCode::Conflict | ErrorCode::Busy => 409,
        ErrorCode::OpRejected | ErrorCode::Load => 422,
        ErrorCode::Cancelled => 499,
        ErrorCode::Io | ErrorCode::Render | ErrorCode::Internal => 500,
    }
}

fn json_response(status: u16, body: &impl serde::Serialize) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_data(serde_json::to_vec(body).unwrap_or_default())
        .with_status_code(status)
        .with_header(header("Content-Type", "application/json"))
        .with_header(header("Cache-Control", "no-store"))
}

fn handle(app: &AppState, hub: &SseHub, ui: &Path, mut request: Request) {
    let url = request.url().split('?').next().unwrap_or("/").to_owned();
    let method = request.method().clone();
    if let (Method::Post, Some(name)) = (&method, url.strip_prefix("/cmd/")) {
        let name = name.to_owned();
        let mut body = Vec::new();
        if let Err(e) = request.as_reader().take(MAX_BODY).read_to_end(&mut body) {
            let _ = request.respond(json_response(
                400,
                &CommandError::invalid(format!("cannot read body: {e}")),
            ));
            return;
        }
        let args: Value = if body.iter().all(u8::is_ascii_whitespace) {
            Value::Null
        } else {
            match serde_json::from_slice(&body) {
                Ok(v) => v,
                Err(e) => {
                    let _ = request.respond(json_response(400, &CommandError::from(e)));
                    return;
                }
            }
        };
        let _ = match dispatch(app, &name, args) {
            Ok(Reply::Json(v)) => request.respond(json_response(200, &v)),
            Ok(Reply::Image(img)) => {
                let mut r = Response::from_data(img.data.clone())
                    .with_header(header("Content-Type", img.format.content_type()))
                    .with_header(header("Cache-Control", "no-store"));
                for (k, v) in img.http_headers() {
                    r = r.with_header(header(k, &v));
                }
                request.respond(r)
            }
            Err(e) => request.respond(json_response(status_of(e.code), &e)),
        };
        return;
    }
    if method == Method::Get && url == "/events" {
        stream_events(hub, request);
        return;
    }
    if method == Method::Get || method == Method::Head {
        let _ = serve_file(ui, &url, request);
        return;
    }
    let _ = request.respond(Response::from_string("method not allowed").with_status_code(405));
}

/// Writes the SSE stream until the client goes away.
fn stream_events(hub: &SseHub, request: Request) {
    let rx = hub.subscribe();
    let mut w = request.into_writer();
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

fn serve_file(ui: &Path, url: &str, request: Request) -> std::io::Result<()> {
    let rel = url.trim_start_matches('/');
    let rel = if rel.is_empty() { "index.html" } else { rel };
    let rel_path = Path::new(rel);
    if !rel_path
        .components()
        .all(|c| matches!(c, Component::Normal(_)))
    {
        return request.respond(Response::from_string("bad path").with_status_code(400));
    }
    let path = ui.join(rel_path);
    match std::fs::read(&path) {
        Ok(data) => request.respond(
            Response::from_data(data)
                .with_header(header("Content-Type", content_type(&path)))
                .with_header(header("Cache-Control", "no-store")),
        ),
        Err(_) => request.respond(Response::from_string("not found").with_status_code(404)),
    }
}
