//! GitHub login against a fake GitHub (wiremock, no network): device flow
//! with `authorization_pending`/`slow_down`, session tokens for uploads, ownership by GitHub
//! id across renames, logout, OAuth errors, web flow with state cookie and
//! PKCE.

use std::path::Path;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use vitrine_api_server::{AppState, Config, StaticToken, router};
use vitrine_core::Project;
use vitrine_core::template::extract::{SaveOptions, extract};
use wiremock::matchers::{body_string_contains, header as has_header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn app(dir: &Path, github: &MockServer, secret: Option<&str>) -> Router {
    let config = Config {
        data_dir: dir.to_owned(),
        auto_approve: true,
        github_url: github.uri(),
        github_api_url: github.uri(),
        github_client_id: "test-client".into(),
        github_client_secret: secret.map(str::to_owned),
        public_url: "https://api.example.test".into(),
        upload_tokens: vec![StaticToken {
            token: "static-1".into(),
            login: "tester".into(),
            github_id: None,
        }],
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
        ..SaveOptions::default()
    };
    extract(&project, &root, &opts)
        .unwrap()
        .0
        .to_zip_bytes()
        .unwrap()
}

async fn call(app: &Router, req: Request<Body>) -> (StatusCode, Value, axum::http::HeaderMap) {
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let headers = res.headers().clone();
    let body = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(Value::Null),
        headers,
    )
}

fn post_json(uri: &str, body: &Value) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn with_token(r: axum::http::request::Builder, token: &str) -> axum::http::request::Builder {
    r.header("authorization", format!("Bearer {token}"))
}

fn upload(token: &str, version: &str) -> Request<Body> {
    with_token(Request::post("/v1/templates"), token)
        .header("content-type", "application/zip")
        .body(Body::from(package(version)))
        .unwrap()
}

async fn mount_device_code(github: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/login/device/code"))
        .and(body_string_contains("client_id=test-client"))
        .and(has_header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": "dev-123",
            "user_code": "ABCD-1234",
            "verification_uri": "https://github.com/login/device",
            "expires_in": 900,
            "interval": 5
        })))
        .mount(github)
        .await;
}

/// Token endpoint answers `answers` in order (each once), the last forever.
async fn mount_token_answers(github: &MockServer, answers: &[Value]) {
    for (i, a) in answers.iter().enumerate() {
        let m = Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .and(body_string_contains("device_code=dev-123"))
            .and(body_string_contains(
                "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(a.clone()));
        let m = if i + 1 < answers.len() {
            m.up_to_n_times(1)
        } else {
            m
        };
        m.with_priority(u8::try_from(i + 1).unwrap())
            .mount(github)
            .await;
    }
}

async fn mount_user(github: &MockServer, access: &str, login: &str, id: i64) {
    Mock::given(method("GET"))
        .and(path("/user"))
        .and(has_header(
            "authorization",
            format!("Bearer {access}").as_str(),
        ))
        .and(has_header("x-github-api-version", "2022-11-28"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "login": login, "id": id, "type": "User"
        })))
        .mount(github)
        .await;
}

/// Runs a full device login as `login`/`id` and returns the session token.
async fn device_login(app: &Router, github: &MockServer, login: &str, id: i64) -> String {
    github.reset().await;
    mount_device_code(github).await;
    mount_token_answers(
        github,
        &[json!({"access_token": "gho_x", "token_type": "bearer", "scope": ""})],
    )
    .await;
    mount_user(github, "gho_x", login, id).await;
    let (s, _, _) = call(
        app,
        Request::post("/v1/auth/github/device")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    let (s, v, _) = call(
        app,
        post_json("/v1/auth/github/token", &json!({"device_code": "dev-123"})),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    v["token"].as_str().unwrap().to_owned()
}

#[tokio::test]
#[allow(clippy::too_many_lines)]
async fn device_flow_sessions_and_ownership() {
    let tmp = tempfile::tempdir().unwrap();
    let github = MockServer::start().await;
    let app = app(tmp.path(), &github, None);

    mount_device_code(&github).await;
    mount_token_answers(
        &github,
        &[
            json!({"error": "authorization_pending", "error_description": "pending"}),
            json!({"error": "slow_down", "interval": 10}),
            json!({"access_token": "gho_x", "token_type": "bearer", "scope": ""}),
        ],
    )
    .await;
    mount_user(&github, "gho_x", "octocat", 1).await;

    let (s, v, _) = call(
        &app,
        Request::post("/v1/auth/github/device")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["user_code"], "ABCD-1234");
    assert_eq!(v["device_code"], "dev-123");
    assert_eq!(v["verification_uri"], "https://github.com/login/device");
    assert_eq!(
        (v["interval"].as_u64(), v["expires_in"].as_u64()),
        (Some(5), Some(900))
    );

    let poll = || post_json("/v1/auth/github/token", &json!({"device_code": "dev-123"}));
    let (s, v, _) = call(&app, poll()).await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("authorization_pending"))
    );
    let (s, v, _) = call(&app, poll()).await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("slow_down"))
    );
    assert_eq!(v["interval"], 10);
    let (s, v, _) = call(&app, poll()).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["login"], "octocat");
    assert_eq!(v["github_id"], 1);
    assert_eq!(v["official"], false);
    let token = v["token"].as_str().unwrap().to_owned();
    assert!(token.starts_with("vss_"));
    assert!(
        !v.to_string().contains("gho_x"),
        "the GitHub token is never passed on"
    );
    assert!(v["expires_at"].as_i64().unwrap() > 1_700_000_000);

    // The session token uploads and identifies.
    let (s, v, _) = call(&app, upload(&token, "1.0.0")).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let (s, v, _) = call(
        &app,
        with_token(Request::get("/v1/auth/me"), &token)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(
        (v["login"].as_str(), v["kind"].as_str()),
        (Some("octocat"), Some("session"))
    );

    // Renamed GitHub account (same id) keeps its template; the uploader
    // name follows.
    let renamed = device_login(&app, &github, "octocat-renamed", 1).await;
    let (s, v, _) = call(&app, upload(&renamed, "1.1.0")).await;
    assert_eq!(s, StatusCode::CREATED, "{v}");
    let (_, v, _) = call(
        &app,
        Request::get("/v1/templates/fernwood-mac")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(v["uploader"], "octocat-renamed");
    assert_eq!(v["version"], "1.1.0");
    // Someone who later took the old login (other id) cannot.
    let impostor = device_login(&app, &github, "octocat", 2).await;
    let (s, v, _) = call(&app, upload(&impostor, "2.0.0")).await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::CONFLICT, Some("id_taken"))
    );
    // Nor a static token of another login.
    let (s, _, _) = call(&app, upload("static-1", "2.0.0")).await;
    assert_eq!(s, StatusCode::CONFLICT);

    // Logout revokes the session; static tokens cannot be revoked.
    let logout = |t: &str| {
        with_token(Request::post("/v1/auth/logout"), t)
            .body(Body::empty())
            .unwrap()
    };
    let (s, v, _) = call(&app, logout(&renamed)).await;
    assert_eq!((s, v["revoked"].as_bool()), (StatusCode::OK, Some(true)));
    assert_eq!(
        call(&app, upload(&renamed, "3.0.0")).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        call(&app, logout(&renamed)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let (s, v, _) = call(&app, logout("static-1")).await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("static_token"))
    );
    // The first session is still valid.
    let (s, _, _) = call(
        &app,
        with_token(Request::get("/v1/auth/me"), &token)
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(s, StatusCode::OK);
}

#[tokio::test]
async fn device_flow_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let github = MockServer::start().await;
    let app = app(tmp.path(), &github, None);
    let poll = || post_json("/v1/auth/github/token", &json!({"device_code": "dev-123"}));
    for (answer, status, code) in [
        ("expired_token", StatusCode::BAD_REQUEST, "expired_token"),
        ("access_denied", StatusCode::FORBIDDEN, "access_denied"),
        (
            "incorrect_device_code",
            StatusCode::BAD_REQUEST,
            "incorrect_device_code",
        ),
        (
            "device_flow_disabled",
            StatusCode::SERVICE_UNAVAILABLE,
            "device_flow_disabled",
        ),
        (
            "unsupported_grant_type",
            StatusCode::BAD_GATEWAY,
            "github_error",
        ),
    ] {
        github.reset().await;
        mount_token_answers(&github, &[json!({"error": answer})]).await;
        let (s, v, _) = call(&app, poll()).await;
        assert_eq!(
            (s, v["code"].as_str()),
            (status, Some(code)),
            "{answer}: {v}"
        );
    }
    // GitHub down → 502.
    github.reset().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&github)
        .await;
    let (s, v, _) = call(
        &app,
        Request::post("/v1/auth/github/device")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::BAD_GATEWAY, Some("github_error"))
    );
    // Malformed body.
    let (s, _, _) = call(
        &app,
        post_json("/v1/auth/github/token", &json!({"device_code": ""})),
    )
    .await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[allow(clippy::many_single_char_names)]
async fn web_flow_with_state_cookie_and_pkce() {
    let tmp = tempfile::tempdir().unwrap();
    let github = MockServer::start().await;

    // Without the client secret the web flow is off.
    let off = app(&tmp.path().join("off"), &github, None);
    let (s, v, _) = call(
        &off,
        Request::get("/v1/auth/github/login")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, Some("web_login_disabled"))
    );

    let app = app(&tmp.path().join("on"), &github, Some("shh"));
    let (s, _, h) = call(
        &app,
        Request::get("/v1/auth/github/login")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(s, StatusCode::FOUND);
    let location = reqwest::Url::parse(h[header::LOCATION].to_str().unwrap()).unwrap();
    assert_eq!(location.path(), "/login/oauth/authorize");
    let q: std::collections::HashMap<_, _> = location.query_pairs().into_owned().collect();
    assert_eq!(q["client_id"], "test-client");
    assert_eq!(
        q["redirect_uri"],
        "https://api.example.test/v1/auth/github/callback"
    );
    assert_eq!(q["code_challenge_method"], "S256");
    assert!(!q.contains_key("scope"));
    let state = q["state"].clone();
    let cookie = h[header::SET_COOKIE].to_str().unwrap().to_owned();
    assert!(
        cookie.starts_with(&format!("vitrine_oauth_state={state};")),
        "{cookie}"
    );
    assert!(
        cookie.contains("HttpOnly") && cookie.contains("Secure") && cookie.contains("SameSite=Lax")
    );

    Mock::given(method("POST"))
        .and(path("/login/oauth/access_token"))
        .and(body_string_contains("client_secret=shh"))
        .and(body_string_contains("code=gh-code"))
        .and(body_string_contains("code_verifier="))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"access_token": "gho_web", "token_type": "bearer"})),
        )
        .mount(&github)
        .await;
    mount_user(&github, "gho_web", "webuser", 5).await;

    let callback = |cookie: Option<&str>| {
        let mut r = Request::get(format!(
            "/v1/auth/github/callback?code=gh-code&state={state}"
        ));
        if let Some(c) = cookie {
            r = r.header(header::COOKIE, c);
        }
        r.body(Body::empty()).unwrap()
    };
    // Without this browser's cookie: refused (login CSRF), state kept.
    let (s, v, _) = call(&app, callback(None)).await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_state"))
    );
    let (s, v, _) = call(&app, callback(Some("other=1; vitrine_oauth_state=wrong"))).await;
    assert_eq!(
        (s, v["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_state"))
    );
    let good = format!("vitrine_oauth_state={state}");
    let (s, v, h) = call(&app, callback(Some(&good))).await;
    assert_eq!(s, StatusCode::OK, "{v}");
    assert_eq!(v["login"], "webuser");
    assert!(v["token"].as_str().unwrap().starts_with("vss_"));
    assert!(
        h[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("Max-Age=0")
    );
    // States are single use.
    let (s, _, _) = call(&app, callback(Some(&good))).await;
    assert_eq!(s, StatusCode::BAD_REQUEST);
}
