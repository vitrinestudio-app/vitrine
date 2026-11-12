//! The gallery API in-process (no network): upload with validation,
//! moderation, list/search, detail, download, previews, reports, official
//! flag.

use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use vitrine_api_server::{AppState, Config, StaticToken, router};
use vitrine_core::Project;
use vitrine_core::template::TemplateTags;
use vitrine_core::template::extract::{SaveOptions, extract};

fn app(dir: &Path, auto_approve: bool) -> Router {
    let config = Config {
        data_dir: dir.to_owned(),
        admin_token: Some("admin-secret".into()),
        upload_tokens: vec![
            StaticToken {
                token: "up-1".into(),
                login: "octocat".into(),
                github_id: None,
            },
            StaticToken {
                token: "up-2".into(),
                login: "other".into(),
                github_id: None,
            },
            StaticToken {
                token: "up-official".into(),
                login: "vitrinestudio-app".into(),
                github_id: Some(1000),
            },
        ],
        auto_approve,
        rate_limit_per_hour: 5,
        ..Config::default()
    };
    router(AppState::open(config).unwrap())
}

fn tags() -> TemplateTags {
    TemplateTags {
        style: vec!["minimal".into()],
        color: vec!["light".into()],
        use_: vec!["home-garden".into(), "productivity".into()],
        ..TemplateTags::default()
    }
}

fn package_with(name: &str, version: &str, canvas: &str, samples: bool) -> Vec<u8> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example");
    let project = Project::load(&root).unwrap();
    let opts = SaveOptions {
        name: name.into(),
        version: Some(version.into()),
        canvases: Some(vec![canvas.into()]),
        tags: tags(),
        sample_screenshots: samples,
        ..SaveOptions::default()
    };
    extract(&project, &root, &opts)
        .unwrap()
        .0
        .to_zip_bytes()
        .unwrap()
}

fn package(version: &str) -> Vec<u8> {
    package_with("Fernwood Mac", version, "mac", false)
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
    let (_, v) = json_of(&app, get("/v1/templates?q=fernwood&tag=home-garden")).await;
    let t = &v["templates"][0];
    assert_eq!(t["id"], "fernwood-mac");
    assert_eq!(t["license"], "CC0-1.0");
    assert_eq!(t["uploader"], "octocat");
    assert_eq!(t["official"], false);
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

#[tokio::test]
async fn official_uploads_are_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), true);
    let (s, _) = json_of(&app, upload(Some("up-official"), package("1.0.0"))).await;
    assert_eq!(s, StatusCode::CREATED);
    let (_, v) = json_of(&app, get("/v1/templates/fernwood-mac")).await;
    assert_eq!(v["official"], true);
    assert_eq!(v["uploader"], "vitrinestudio-app");
    // Another static token with a different login cannot take it over.
    assert_eq!(
        send(&app, upload(Some("up-1"), package("2.0.0"))).await.0,
        StatusCode::CONFLICT
    );
}

#[tokio::test]
async fn public_reads_need_no_login_but_are_rate_limited() {
    let tmp = tempfile::tempdir().unwrap();
    let config = Config {
        data_dir: tmp.path().to_owned(),
        read_rate_limit_per_hour: 3,
        ..Config::default()
    };
    let app = router(AppState::open(config).unwrap());
    for _ in 0..3 {
        assert_eq!(send(&app, get("/v1/templates")).await.0, StatusCode::OK);
    }
    let (s, v) = json_of(&app, get("/v1/templates/anything")).await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::TOO_MANY_REQUESTS, Some("rate_limited"))
    );
    // Health is never limited.
    assert_eq!(send(&app, get("/health")).await.0, StatusCode::OK);
}

fn upload_to(uri: &str, token: &str, body: Vec<u8>) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/zip")
        .header("authorization", format!("Bearer {token}"))
        .body(Body::from(body))
        .unwrap()
}

#[tokio::test]
async fn tags_filters_and_taxonomy() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), true);
    let (s, v) = json_of(&app, upload(Some("up-1"), package("1.0.0"))).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let (_, v) = json_of(&app, get("/v1/templates/fernwood-mac")).await;
    assert_eq!(v["tags"]["style"], json!(["minimal"]));
    assert_eq!(v["tags"]["use"], json!(["home-garden", "productivity"]));
    assert_eq!(v["devices"], json!(["mac"]));
    assert_eq!(v["samples"], 0);

    let count = |v: &Value| v["templates"].as_array().unwrap().len();
    for (q, n) in [
        ("style=minimal", 1),
        ("style=bold", 0),
        ("style=bold,minimal", 1),
        ("style=bold&style=minimal&color=light", 1),
        ("use=games", 0),
        ("use=games&use=productivity", 1),
        ("devices=mac", 1),
        ("devices=phone", 0),
        ("tag=mac", 1),
    ] {
        let (s, v) = json_of(&app, get(&format!("/v1/templates?{q}"))).await;
        assert_eq!((s, count(&v)), (StatusCode::OK, n), "{q}");
    }
    let (s, v) = json_of(&app, get("/v1/templates?style=fancy")).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
    assert_eq!(v["code"], "invalid_filter");
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("allowed: minimal, bold")
    );

    let (s, v) = json_of(&app, get("/v1/tags")).await;
    assert_eq!(s, StatusCode::OK);
    let facets = v["facets"].as_array().unwrap();
    assert_eq!(facets.len(), 4);
    assert_eq!(facets[0]["id"], "style");
    assert_eq!(facets[0]["values"][0]["label"]["de"], "Minimalistisch");
    assert_eq!(facets[3]["derived"], true);
}

#[tokio::test]
async fn uploads_need_complete_tags() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), true);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example");
    let project = Project::load(&root).unwrap();
    let opts = SaveOptions {
        name: "No tags".into(),
        canvases: Some(vec!["mac".into()]),
        ..SaveOptions::default()
    };
    let bytes = extract(&project, &root, &opts)
        .unwrap()
        .0
        .to_zip_bytes()
        .unwrap();
    let (s, v) = json_of(&app, upload(Some("up-1"), bytes)).await;
    assert_eq!(s, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(v["code"], "invalid_tags");
    assert!(
        v["error"]
            .as_str()
            .unwrap()
            .contains("at least 1 style tag"),
        "{v}"
    );
}

#[tokio::test]
async fn sprites_are_rendered_and_cached() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), true);
    let (s, _) = json_of(&app, upload(Some("up-1"), package("1.0.0"))).await;
    assert_eq!(s, StatusCode::CREATED);
    let (_, v) = json_of(&app, get("/v1/templates/fernwood-mac")).await;
    let c = &v["canvases"][0];
    assert_eq!(c["page_aspect"], c["aspect"]);
    let sprite = &c["sprite"];
    assert_eq!(sprite["page_height"], 600);
    assert_eq!(
        sprite["width"].as_u64().unwrap(),
        sprite["page_width"].as_u64().unwrap() * c["pages"].as_u64().unwrap()
    );
    let url = sprite["url"].as_str().unwrap();
    assert!(
        url.contains("/v1/templates/fernwood-mac/sprites/mac.webp?v="),
        "{url}"
    );
    assert!(
        sprite["url_2x"]
            .as_str()
            .unwrap()
            .contains("/sprites/mac@2x.webp?v=")
    );
    let path = url.split_once("https://api.vitrinestudio.app").unwrap().1;

    let res = app.clone().oneshot(get(path)).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(res.headers()["content-type"], "image/webp");
    assert!(
        res.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    let etag = res.headers()["etag"].clone();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(body.starts_with(b"RIFF"));
    // Unversioned URL: short cache; If-None-Match → 304.
    let res = app
        .clone()
        .oneshot(get("/v1/templates/fernwood-mac/sprites/mac@2x.webp"))
        .await
        .unwrap();
    assert!(
        res.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("max-age=3600")
    );
    let req = Request::get("/v1/templates/fernwood-mac/sprites/mac.webp")
        .header("if-none-match", etag)
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(&app, req).await.0, StatusCode::NOT_MODIFIED);
    assert_eq!(
        send(&app, get("/v1/templates/fernwood-mac/sprites/nope.webp"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // The admin can render them again.
    let (s, v) = json_of(&app, admin("/v1/admin/templates/fernwood-mac/rerender")).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["sprites"][0]["id"], "mac");
    assert_eq!(
        send(&app, admin("/v1/admin/templates/unknown/rerender"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn sample_screenshots_need_the_uploaders_confirmation() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app(tmp.path(), true);
    let bytes = package_with("Fernwood Phone", "1.0.0", "phone", true);
    let pkg = vitrine_core::template::Package::from_zip_bytes(
        &bytes,
        vitrine_core::template::Limits::default(),
    )
    .unwrap();
    let n = pkg.manifest.samples.len();
    assert!(n > 0);

    // Not confirmed: replaced by placeholders, as before.
    let (s, v) = json_of(&app, upload(Some("up-1"), bytes.clone())).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["samples"], 0);
    assert_eq!(v["samples_removed"], n);
    let (_, zip) = send(&app, get("/v1/templates/fernwood-phone/download")).await;
    let stored = vitrine_core::template::Package::from_zip_bytes(
        &zip,
        vitrine_core::template::Limits::default(),
    )
    .unwrap();
    assert_eq!(stored.manifest.samples.len(), 0);

    // Confirmed: kept byte for byte.
    let bytes = package_with("Fernwood Phone", "1.0.1", "phone", true);
    let (s, v) = json_of(
        &app,
        upload_to("/v1/templates?confirm_own_screenshots=true", "up-1", bytes),
    )
    .await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    assert_eq!(v["samples"], n);
    let (_, zip) = send(&app, get("/v1/templates/fernwood-phone/download")).await;
    let stored = vitrine_core::template::Package::from_zip_bytes(
        &zip,
        vitrine_core::template::Limits::default(),
    )
    .unwrap();
    assert_eq!(stored.manifest.samples, pkg.manifest.samples);
    for s in &pkg.manifest.samples {
        assert_eq!(stored.file(s), pkg.file(s), "{s}");
    }
    let (_, v) = json_of(&app, get("/v1/templates/fernwood-phone")).await;
    assert_eq!(v["samples"], n);
    assert_eq!(v["devices"], json!(["phone", "banner"]));
}

#[tokio::test]
async fn an_old_database_is_migrated_and_backfilled() {
    let tmp = tempfile::tempdir().unwrap();
    // A schema-0 database with one approved template (free tag list) and
    // its stored package, as the first deployment wrote them.
    {
        let db = rusqlite::Connection::open(tmp.path().join("api.sqlite")).unwrap();
        db.execute_batch(
            "CREATE TABLE templates (
                 id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT NOT NULL DEFAULT '',
                 author TEXT NOT NULL DEFAULT '', tags TEXT NOT NULL DEFAULT '[]',
                 version TEXT NOT NULL, uploader TEXT NOT NULL, uploader_id INTEGER,
                 status TEXT NOT NULL, canvases TEXT NOT NULL DEFAULT '[]',
                 fonts TEXT NOT NULL DEFAULT '[]', size INTEGER NOT NULL,
                 downloads INTEGER NOT NULL DEFAULT 0, created_at INTEGER NOT NULL,
                 updated_at INTEGER NOT NULL, moderation_note TEXT NOT NULL DEFAULT '');
             INSERT INTO templates (id, name, tags, version, uploader, status, canvases, size, created_at, updated_at)
             VALUES ('fernwood-mac', 'Fernwood Mac', '[\"garden\", \"minimal\", \"dark\"]', '1.0.0', 'octocat',
                     'approved', '[{\"id\":\"mac\",\"aspect\":\"16:10\",\"pages\":1}]', 1, 1, 1);",
        )
        .unwrap();
        let dir = tmp.path().join("templates/fernwood-mac");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("template.vitrine-template"), package("1.0.0")).unwrap();
    }
    let config = Config {
        data_dir: tmp.path().to_owned(),
        ..Config::default()
    };
    let state = AppState::open(config.clone()).unwrap();
    // Opening twice is fine (idempotent migration).
    drop(AppState::open(config).unwrap());
    let app = router(state.clone());
    // Migrated: the old list sorted into facets ("garden" dropped).
    let (s, v) = json_of(&app, get("/v1/templates/fernwood-mac")).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(
        v["tags"],
        json!({"style": ["minimal"], "color": ["dark"], "use": []})
    );
    // Backfilled from the stored package (its tags win).
    vitrine_api_server::backfill(state).await;
    let (_, v) = json_of(&app, get("/v1/templates/fernwood-mac")).await;
    assert_eq!(v["tags"]["color"], json!(["light"]));
    assert_eq!(v["devices"], json!(["mac"]));
    assert!(v["canvases"][0]["sprite"]["url"].is_string(), "{v}");
    let (s, v) = json_of(&app, get("/v1/templates?style=minimal&devices=mac")).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(v["templates"].as_array().unwrap().len(), 1);
}
