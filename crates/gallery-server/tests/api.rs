//! The gallery API in-process (no network): upload with validation,
//! moderation, list/search, detail, download, previews, reports.

use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use vitrine_core::Project;
use vitrine_core::template::extract::{SaveOptions, extract};
use vitrine_gallery_server::{AppState, Config, router};

fn app(dir: &Path, auto_approve: bool) -> Router {
    let config = Config {
        data_dir: dir.to_owned(),
        admin_token: Some("admin-secret".into()),
        upload_tokens: [
            ("up-1".to_owned(), "octocat".to_owned()),
            ("up-2".to_owned(), "other".to_owned()),
        ]
        .into_iter()
        .collect(),
        auto_approve,
        rate_limit_per_hour: 5,
        ..Config::default()
    };
    router(AppState::open(config).unwrap())
}

fn package(version: &str) -> Vec<u8> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example");
    let project = Project::load(&root).unwrap();
    let opts = SaveOptions {
        name: "Fernwood Mac".into(),
        version: Some(version.into()),
        canvases: Some(vec!["mac".into()]),
        tags: vec!["garden".into()],
        ..SaveOptions::default()
    };
    extract(&project, &root, &opts)
        .unwrap()
        .0
        .to_zip_bytes()
        .unwrap()
}

async fn send(app: &Router, req: Request<Body>) -> (StatusCode, Vec<u8>) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let body = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, body)
}

async fn json_of(app: &Router, req: Request<Body>) -> (StatusCode, Value) {
    let (s, b) = send(app, req).await;
    (s, serde_json::from_slice(&b).unwrap_or(Value::Null))
}

fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

fn upload(token: Option<&str>, body: Vec<u8>) -> Request<Body> {
    let mut r = Request::post("/v1/templates").header("content-type", "application/zip");
    if let Some(t) = token {
        r = r.header("authorization", format!("Bearer {t}"));
    }
    r.body(Body::from(body)).unwrap()
}

fn admin(uri: &str) -> Request<Body> {
    Request::post(uri)
        .header("authorization", "Bearer admin-secret")
        .header("content-type", "application/json")
        .body(Body::from(r#"{"note":"ok"}"#))
        .unwrap()
}

#[tokio::test]
#[allow(clippy::many_single_char_names)]
async fn upload_moderate_list_download() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), false);

    let (s, v) = json_of(&app, get("/health")).await;
    assert_eq!((s, v["status"].as_str()), (StatusCode::OK, Some("ok")));

    // Auth required; garbage refused with a reason.
    assert_eq!(
        send(&app, upload(None, package("1.0.0"))).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (s, v) = json_of(&app, upload(Some("up-1"), b"not a zip".to_vec())).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["code"], "invalid_template");

    // Valid upload → pending, invisible until approved.
    let (s, v) = json_of(&app, upload(Some("up-1"), package("1.0.0"))).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["status"], "pending");
    let (_, v) = json_of(&app, get("/v1/templates")).await;
    assert_eq!(v["templates"].as_array().unwrap().len(), 0);
    assert_eq!(
        send(&app, get("/v1/templates/fernwood-mac")).await.0,
        StatusCode::NOT_FOUND
    );

    // Same version again, other uploader.
    assert_eq!(
        send(&app, upload(Some("up-1"), package("1.0.0"))).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        send(&app, upload(Some("up-2"), package("2.0.0"))).await.0,
        StatusCode::CONFLICT
    );

    // Moderation.
    let unauth = Request::get("/v1/admin/queue").body(Body::empty()).unwrap();
    assert_eq!(send(&app, unauth).await.0, StatusCode::UNAUTHORIZED);
    let q = Request::get("/v1/admin/queue")
        .header("authorization", "Bearer admin-secret")
        .body(Body::empty())
        .unwrap();
    let (_, v) = json_of(&app, q).await;
    assert_eq!(v["pending"][0]["id"], "fernwood-mac");
    let (s, _) = json_of(&app, admin("/v1/admin/templates/fernwood-mac/approve")).await;
    assert_eq!(s, StatusCode::OK);

    // Public now: list, search, detail, preview, download.
    let (_, v) = json_of(&app, get("/v1/templates?q=fernwood&tag=garden")).await;
    let t = &v["templates"][0];
    assert_eq!(t["id"], "fernwood-mac");
    assert_eq!(t["license"], "CC0-1.0");
    assert!(
        t["canvases"][0]["preview_url"]
            .as_str()
            .unwrap()
            .ends_with("/previews/mac")
    );
    let (_, v) = json_of(&app, get("/v1/templates?q=nothing-like-this")).await;
    assert_eq!(v["templates"].as_array().unwrap().len(), 0);
    let (s, png) = send(&app, get("/v1/templates/fernwood-mac/previews/mac")).await;
    assert_eq!(s, StatusCode::OK);
    assert!(png.starts_with(b"\x89PNG"));
    let (s, zip) = send(&app, get("/v1/templates/fernwood-mac/download")).await;
    assert_eq!(s, StatusCode::OK);
    let pkg = vitrine_core::template::Package::from_zip_bytes(
        &zip,
        vitrine_core::template::Limits::default(),
    )
    .unwrap();
    assert_eq!(pkg.manifest.id, "fernwood-mac");
    let (_, v) = json_of(&app, get("/v1/templates/fernwood-mac")).await;
    assert_eq!(v["downloads"], 1);

    // Abuse report.
    let r = Request::post("/v1/templates/fernwood-mac/report")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"reason": "copyright", "message": "mine"}).to_string(),
        ))
        .unwrap();
    assert_eq!(send(&app, r).await.0, StatusCode::ACCEPTED);
    let bad = Request::post("/v1/templates/fernwood-mac/report")
        .header("content-type", "application/json")
        .body(Body::from(json!({"reason": "boring"}).to_string()))
        .unwrap();
    assert_eq!(send(&app, bad).await.0, StatusCode::BAD_REQUEST);

    // Reject hides it again.
    json_of(&app, admin("/v1/admin/templates/fernwood-mac/reject")).await;
    assert_eq!(
        send(&app, get("/v1/templates/fernwood-mac")).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn auto_approve_and_rate_limit() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), true);
    let (s, v) = json_of(&app, upload(Some("up-1"), package("1.0.0"))).await;
    assert_eq!(
        (s, v["status"].as_str()),
        (StatusCode::CREATED, Some("approved"))
    );
    // Five uploads per hour for the same client (one used above).
    let mut last = StatusCode::OK;
    for i in 0..5 {
        last = send(
            &app,
            upload(Some("up-1"), package(&format!("1.0.{}", i + 1))),
        )
        .await
        .0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    // Unknown admin action, moderation token checks.
    assert_eq!(
        send(&app, admin("/v1/admin/templates/fernwood-mac/delete"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
