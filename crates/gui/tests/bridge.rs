//! Dev HTTP bridge end to end: a real server on a free port, plain HTTP/1.1
//! over a `TcpStream`.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use arrangia_gui::bridge::{BridgeServer, SseHub, serve};
use arrangia_gui::commands::AppState;
use arrangia_gui::dialogs::NoDialogs;
use arrangia_gui::settings::AppPaths;
use arrangia_gui::testutil::{copy_project, example_fixture};

struct Reply {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> serde_json::Value {
        serde_json::from_slice(&self.body).expect("JSON body")
    }
}

fn request(addr: SocketAddr, method: &str, path: &str, body: &str) -> Reply {
    let mut s = TcpStream::connect(addr).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    write!(
        s,
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    let mut raw = Vec::new();
    s.read_to_end(&mut raw).unwrap();
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':'))
        .map(|(k, v)| (k.trim().to_owned(), v.trim().to_owned()))
        .collect();
    let mut body = raw[split + 4..].to_vec();
    if headers
        .iter()
        .any(|(k, v)| k.eq_ignore_ascii_case("transfer-encoding") && v.contains("chunked"))
    {
        body = dechunk(&body);
    }
    Reply {
        status,
        headers,
        body,
    }
}

fn dechunk(mut data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let line_end = data.windows(2).position(|w| w == b"\r\n").unwrap();
        let size =
            usize::from_str_radix(std::str::from_utf8(&data[..line_end]).unwrap().trim(), 16)
                .unwrap();
        data = &data[line_end + 2..];
        if size == 0 {
            return out;
        }
        out.extend_from_slice(&data[..size]);
        data = &data[size + 2..];
    }
}

fn start() -> (tempfile::TempDir, PathBuf, BridgeServer) {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("project");
    copy_project(&example_fixture(), &dir).unwrap();
    let hub = Arc::new(SseHub::default());
    let app = Arc::new(
        AppState::new(
            AppPaths::in_dir(&tmp.path().join("app")),
            hub.clone(),
            Arc::new(NoDialogs),
        )
        .without_watcher(),
    );
    let ui = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("ui");
    let server = serve(app, hub, ui, 0).unwrap();
    (tmp, dir, server)
}

#[test]
fn serves_ui_commands_and_images() {
    let (_tmp, dir, server) = start();
    let addr = server.addr();
    assert!(addr.ip().is_loopback());
    assert_eq!(server.url(), format!("http://{addr}/"));

    let index = request(addr, "GET", "/", "");
    assert_eq!(index.status, 200);
    assert!(String::from_utf8_lossy(&index.body).contains("<html"));
    assert_eq!(
        request(addr, "GET", "/js/transport.js", "").header("content-type"),
        Some("text/javascript; charset=utf-8")
    );
    assert_eq!(request(addr, "GET", "/../Cargo.toml", "").status, 400);

    let none = request(addr, "POST", "/cmd/project_summary", "");
    assert_eq!(none.status, 409);
    assert_eq!(none.json()["code"], "no_project");

    let open = request(
        addr,
        "POST",
        "/cmd/project_open",
        &serde_json::json!({ "dir": dir }).to_string(),
    );
    assert_eq!(open.status, 200, "{}", String::from_utf8_lossy(&open.body));
    assert_eq!(open.json()["name"], "Fernwood");

    let png = request(
        addr,
        "POST",
        "/cmd/render_image",
        r#"{"target": "ios-iphone-69", "image": 0, "max_edge": 400}"#,
    );
    assert_eq!(png.status, 200);
    assert_eq!(png.header("content-type"), Some("image/png"));
    assert_eq!(png.header("x-height"), Some("400"));
    assert!(png.body.starts_with(b"\x89PNG\r\n\x1a\n"));
    let decoded = png::Decoder::new(std::io::Cursor::new(&png.body))
        .read_info()
        .unwrap();
    assert_eq!(decoded.info().height, 400);
    assert_eq!(
        png.header("x-width"),
        Some(decoded.info().width.to_string().as_str())
    );

    let rgba = request(
        addr,
        "POST",
        "/cmd/render_image",
        r#"{"target": "ios-iphone-69", "max_edge": 300, "format": "rgba"}"#,
    );
    let w: usize = rgba.header("x-width").unwrap().parse().unwrap();
    let h: usize = rgba.header("x-height").unwrap().parse().unwrap();
    assert_eq!(h, 300);
    assert_eq!(rgba.body.len(), w * h * 4);
    assert_eq!(rgba.header("x-format"), Some("rgba"));

    let bad = request(addr, "POST", "/cmd/render_image", r#"{"image": 99}"#);
    assert_eq!(bad.status, 404);
    assert_eq!(bad.json()["code"], "not_found");
    assert_eq!(request(addr, "POST", "/cmd/nope", "").status, 404);
    server.shutdown();
}

#[test]
fn streams_events() {
    let (_tmp, dir, server) = start();
    let addr = server.addr();
    request(
        addr,
        "POST",
        "/cmd/project_open",
        &serde_json::json!({ "dir": dir }).to_string(),
    );
    let mut sse = TcpStream::connect(addr).unwrap();
    sse.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
    write!(sse, "GET /events HTTP/1.1\r\nHost: {addr}\r\n\r\n").unwrap();
    let mut reader = BufReader::new(sse);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    assert!(line.starts_with("HTTP/1.1 200"), "{line}");
    // Wait until the stream is registered (": connected" comment).
    loop {
        line.clear();
        reader.read_line(&mut line).unwrap();
        if line.starts_with(": connected") {
            break;
        }
    }
    let export = request(
        addr,
        "POST",
        "/cmd/export",
        &serde_json::json!({
            "targets": ["blog-header"], "langs": ["en"], "out_dir": dir.join("out")
        })
        .to_string(),
    );
    assert_eq!(export.status, 200);
    let mut events = Vec::new();
    while events.len() < 2 {
        line.clear();
        reader.read_line(&mut line).unwrap();
        if let Some(name) = line.strip_prefix("event: ") {
            events.push(name.trim().to_owned());
        }
    }
    assert_eq!(events, ["export-progress", "export-finished"]);
    server.shutdown();
}
