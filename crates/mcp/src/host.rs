//! The GUI-hosted MCP endpoint (feature `http`, ADR 0015).
//!
//! [`McpHost`] serves [`crate::http::service`] for an [`ArrangiaMcp`] over
//! the GUI's live [`AppState`] (`Host::Gui`) on `127.0.0.1` with hyper, on
//! its own small tokio runtime (works next to Tauri's runtime and in the
//! dev bridge, which has none). Every request must pass
//!
//! - a `Host` header naming the loopback address and our port (DNS
//!   rebinding protection; rmcp checks the host name again),
//! - no `Origin` header, or a loopback one (browsers always send it; web
//!   pages from other origins are refused),
//! - `Authorization: Bearer <token>` (32 random bytes, base64url, new per
//!   start), compared in constant time.
//!
//! Routes: `/mcp` (MCP Streamable HTTP), `/health` (`{name, version,
//! pid}`, used by `arrangia mcp` to tell a live GUI from a stale discovery
//! file). The discovery file ([`crate::discovery`]) is written on start
//! and removed on stop. The host registers itself as the app's
//! [`McpControl`], so the setting "Allow Claude to connect" switches it
//! live, and reports into the app's MCP status (`mcp-status` events).

use std::convert::Infallible;
use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use arrangia_app::commands::AppState;
use arrangia_app::mcp::{McpControl, now_ms};
use arrangia_app::{CommandError, error::CmdResult};
use base64::Engine as _;
use bytes::Bytes;
use http_body_util::{BodyExt, Full, combinators::BoxBody};
use hyper::body::Incoming;
use hyper::header::{AUTHORIZATION, CONTENT_TYPE, HOST, ORIGIN, WWW_AUTHENTICATE};
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use rmcp::transport::StreamableHttpServerConfig;
use tokio_util::sync::CancellationToken;
use tower_service::Service as _;

use crate::discovery::{self, Discovery};
use crate::progress::ProgressRelay;
use crate::server::{ArrangiaMcp, Host};

/// Path of the MCP endpoint.
pub const MCP_PATH: &str = "/mcp";

type Body = BoxBody<Bytes, Infallible>;

/// Options of [`McpHost`].
#[derive(Debug, Clone, Default)]
pub struct HostOptions {
    /// Preferred port (taken or `None`: a free port). Default: the app
    /// setting `mcp_port`.
    pub port: Option<u16>,
    /// Discovery file to write (`None`: none, e.g. tests).
    pub discovery: Option<PathBuf>,
}

/// A running endpoint.
struct Running {
    runtime: tokio::runtime::Runtime,
    cancel: CancellationToken,
    /// Closed when the accept loop (and with it the listener) is gone.
    accept_done: std::sync::mpsc::Receiver<()>,
    addr: SocketAddr,
    token: String,
}

/// The GUI's MCP endpoint (start/stop; see the module docs).
pub struct McpHost {
    app: Arc<AppState>,
    relay: Option<Arc<ProgressRelay>>,
    options: HostOptions,
    running: Mutex<Option<Running>>,
}

impl std::fmt::Debug for McpHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpHost")
            .field("addr", &self.addr())
            .finish_non_exhaustive()
    }
}

/// A new random bearer token (32 bytes, base64url without padding).
pub fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| format!("no random numbers: {e}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

impl McpHost {
    /// A host for `app` (not started). `relay` must be (or wrap) the app's
    /// event sink for `export` progress notifications.
    pub fn new(
        app: Arc<AppState>,
        relay: Option<Arc<ProgressRelay>>,
        options: HostOptions,
    ) -> Arc<Self> {
        Arc::new(Self {
            app,
            relay,
            options,
            running: Mutex::new(None),
        })
    }

    /// Creates the host, registers it as the app's [`McpControl`] and
    /// starts it when the setting `mcp_enabled` is on (default). A start
    /// error is reported in the MCP status, not returned.
    pub fn install(
        app: &Arc<AppState>,
        relay: Option<Arc<ProgressRelay>>,
        options: HostOptions,
    ) -> Arc<Self> {
        let host = Self::new(app.clone(), relay, options);
        app.set_mcp_control(host.clone());
        let enabled = app.settings_store().load().map_or(true, |s| s.mcp_enabled);
        app.update_mcp_status(|s| {
            s.hosted = true;
            s.enabled = enabled;
        });
        if enabled && let Err(e) = host.start() {
            eprintln!("arrangia: MCP endpoint not started: {e}");
        }
        host
    }

    /// Address while running.
    pub fn addr(&self) -> Option<SocketAddr> {
        self.lock().as_ref().map(|r| r.addr)
    }

    /// Bearer token while running.
    pub fn token(&self) -> Option<String> {
        self.lock().as_ref().map(|r| r.token.clone())
    }

    /// `http://127.0.0.1:<port>/mcp` while running.
    pub fn url(&self) -> Option<String> {
        self.addr().map(|a| format!("http://{a}{MCP_PATH}"))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<Running>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn bind(&self) -> std::io::Result<TcpListener> {
        let preferred = self.options.port.or_else(|| {
            self.app
                .settings_store()
                .load()
                .ok()
                .and_then(|s| s.mcp_port)
        });
        if let Some(port) = preferred.filter(|p| *p != 0)
            && let Ok(l) = TcpListener::bind(("127.0.0.1", port))
        {
            return Ok(l);
        }
        TcpListener::bind(("127.0.0.1", 0))
    }

    /// Starts listening (no-op when running). Writes the discovery file.
    pub fn start(&self) -> Result<SocketAddr, String> {
        let mut running = self.lock();
        if let Some(r) = running.as_ref() {
            return Ok(r.addr);
        }
        let result = self.start_locked();
        match result {
            Ok(r) => {
                let addr = r.addr;
                let url = format!("http://{addr}{MCP_PATH}");
                self.app.set_mcp_token(Some(r.token.clone()));
                *running = Some(r);
                drop(running);
                let discovery = self.options.discovery.clone();
                self.app.update_mcp_status(|s| {
                    s.hosted = true;
                    s.enabled = true;
                    s.running = true;
                    s.error = None;
                    s.url = Some(url);
                    s.port = Some(addr.port());
                    s.discovery_file = discovery;
                });
                Ok(addr)
            }
            Err(e) => {
                drop(running);
                self.app.update_mcp_status(|s| {
                    s.hosted = true;
                    s.running = false;
                    s.error = Some(e.clone());
                });
                Err(e)
            }
        }
    }

    fn start_locked(&self) -> Result<Running, String> {
        let token = new_token()?;
        let listener = self
            .bind()
            .map_err(|e| format!("cannot listen on 127.0.0.1: {e}"))?;
        let addr = listener.local_addr().map_err(|e| e.to_string())?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("arrangia-mcp")
            .enable_all()
            .build()
            .map_err(|e| format!("cannot start the async runtime: {e}"))?;
        let cancel = CancellationToken::new();
        let server = ArrangiaMcp::new(self.app.clone(), self.relay.clone(), Host::Gui);
        let config = StreamableHttpServerConfig::default()
            .with_cancellation_token(cancel.child_token())
            .with_sse_keep_alive(Some(std::time::Duration::from_secs(15)));
        let service = crate::http::service(server, config);
        let gate = Arc::new(Gate {
            token: token.clone(),
            port: addr.port(),
        });
        let accept_cancel = cancel.clone();
        let (done_tx, accept_done) = std::sync::mpsc::channel::<()>();
        runtime.spawn(async move {
            // Dropped (closing the channel) after the listener.
            let _done = done_tx;
            let Ok(listener) = tokio::net::TcpListener::from_std(listener) else {
                return;
            };
            loop {
                let stream = tokio::select! {
                    () = accept_cancel.cancelled() => break,
                    r = listener.accept() => match r {
                        Ok((stream, _)) => stream,
                        Err(_) => continue,
                    },
                };
                let (service, gate, cancel) =
                    (service.clone(), gate.clone(), accept_cancel.clone());
                tokio::spawn(async move {
                    let svc = hyper::service::service_fn(move |req: Request<Incoming>| {
                        let (mut service, gate) = (service.clone(), gate.clone());
                        async move {
                            if let Some(denied) = gate.check(&req) {
                                return Ok::<_, Infallible>(denied);
                            }
                            match req.uri().path() {
                                "/health" => Ok(health()),
                                MCP_PATH => service.call(req).await,
                                _ => Ok(plain(StatusCode::NOT_FOUND, "not found")),
                            }
                        }
                    });
                    let conn = hyper::server::conn::http1::Builder::new()
                        .serve_connection(TokioIo::new(stream), svc);
                    tokio::pin!(conn);
                    tokio::select! {
                        _ = conn.as_mut() => {}
                        () = cancel.cancelled() => {}
                    }
                });
            }
        });
        if let Some(file) = &self.options.discovery {
            let info = Discovery {
                url: format!("http://{addr}{MCP_PATH}"),
                port: addr.port(),
                token: token.clone(),
                pid: std::process::id(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
                started_at: now_ms(),
            };
            if let Err(e) = discovery::write(file, &info) {
                cancel.cancel();
                runtime.shutdown_background();
                return Err(format!("cannot write {}: {e}", file.display()));
            }
        }
        Ok(Running {
            runtime,
            cancel,
            accept_done,
            addr,
            token,
        })
    }

    /// Stops listening, ends all MCP sessions and removes the discovery
    /// file (no-op when stopped).
    pub fn stop(&self) {
        let Some(r) = self.lock().take() else {
            return;
        };
        if let Some(file) = &self.options.discovery {
            discovery::remove_if_ours(file, &r.token);
        }
        r.cancel.cancel();
        // The port is free once the accept loop has dropped the listener.
        let _ = r
            .accept_done
            .recv_timeout(std::time::Duration::from_secs(2));
        r.runtime
            .shutdown_timeout(std::time::Duration::from_millis(500));
        self.app.set_mcp_token(None);
        self.app
            .update_mcp_status(arrangia_app::mcp::McpStatus::stopped);
    }
}

impl McpControl for McpHost {
    fn set_enabled(&self, enabled: bool) -> CmdResult<()> {
        if enabled {
            self.start().map(|_| ()).map_err(CommandError::io)?;
        } else {
            self.stop();
        }
        self.app.update_mcp_status(|s| s.enabled = enabled);
        Ok(())
    }
}

/// Host, Origin and token checks.
struct Gate {
    token: String,
    port: u16,
}

/// `a == b` without an early exit on the first difference.
fn same_secret(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Is `authority` (`host[:port]`) a loopback name (with `port` when
/// given)?
fn loopback_authority(authority: &str, port: Option<u16>) -> bool {
    let (host, p) = match authority.rsplit_once(':') {
        Some((h, p)) if !h.is_empty() && !p.contains(']') => (h, Some(p)),
        _ => (authority, None),
    };
    let host_ok = matches!(host, "127.0.0.1" | "localhost" | "[::1]");
    let port_ok = match (port, p) {
        (Some(want), Some(p)) => p.parse::<u16>().ok() == Some(want),
        (Some(_), None) => false,
        (None, _) => true,
    };
    host_ok && port_ok
}

impl Gate {
    /// `Some(response)` when the request is refused.
    fn check<B>(&self, req: &Request<B>) -> Option<Response<Body>> {
        let header = |name| req.headers().get(name).and_then(|v| v.to_str().ok());
        if !header(HOST).is_some_and(|h| loopback_authority(h, Some(self.port))) {
            return Some(plain(StatusCode::FORBIDDEN, "forbidden: host"));
        }
        if let Some(origin) = req.headers().get(ORIGIN) {
            let ok = origin
                .to_str()
                .ok()
                .and_then(|o| o.strip_prefix("http://"))
                .is_some_and(|a| loopback_authority(a, None));
            if !ok {
                return Some(plain(StatusCode::FORBIDDEN, "forbidden: origin"));
            }
        }
        let given = header(AUTHORIZATION)
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or_default();
        if !same_secret(given.trim().as_bytes(), self.token.as_bytes()) {
            let mut r = plain(
                StatusCode::UNAUTHORIZED,
                "unauthorized: bearer token required",
            );
            r.headers_mut().insert(
                WWW_AUTHENTICATE,
                hyper::header::HeaderValue::from_static("Bearer realm=\"arrangia\""),
            );
            return Some(r);
        }
        None
    }
}

fn plain(status: StatusCode, text: &'static str) -> Response<Body> {
    let mut r = Response::new(Full::new(Bytes::from_static(text.as_bytes())).boxed());
    *r.status_mut() = status;
    r.headers_mut().insert(
        CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    r
}

fn health() -> Response<Body> {
    let body = serde_json::json!({
        "name": crate::SERVER_NAME,
        "version": env!("CARGO_PKG_VERSION"),
        "pid": std::process::id(),
    });
    let mut r = Response::new(Full::new(Bytes::from(body.to_string())).boxed());
    r.headers_mut().insert(
        CONTENT_TYPE,
        hyper::header::HeaderValue::from_static("application/json"),
    );
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorities() {
        assert!(loopback_authority("127.0.0.1:4100", Some(4100)));
        assert!(loopback_authority("localhost:4100", Some(4100)));
        assert!(loopback_authority("[::1]:4100", Some(4100)));
        assert!(!loopback_authority("127.0.0.1:4101", Some(4100)));
        assert!(!loopback_authority("127.0.0.1", Some(4100)));
        assert!(!loopback_authority("evil.example:4100", Some(4100)));
        assert!(!loopback_authority(
            "127.0.0.1.evil.example:4100",
            Some(4100)
        ));
        assert!(loopback_authority("localhost:3000", None));
        assert!(loopback_authority("[::1]", None));
        assert!(!loopback_authority("example.com", None));
        assert!(same_secret(b"abc", b"abc"));
        assert!(!same_secret(b"abc", b"abd") && !same_secret(b"abc", b"ab"));
        let t = new_token().unwrap();
        assert_eq!(t.len(), 43);
        assert_ne!(t, new_token().unwrap());
    }
}
