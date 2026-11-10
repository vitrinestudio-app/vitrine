//! Store setup helpers against wiremock (no real network): the Apple app
//! picker (`store_apps_list`, paginated, cached per session) and the Play
//! package check (`store_package_check`, temporary edit deleted again).

use std::sync::Arc;
use std::time::Duration;

use arrangia_app::dialogs::NoDialogs;
use arrangia_app::events::RecordingSink;
use arrangia_app::settings::AppPaths;
use arrangia_app::upload::UploadConfig;
use arrangia_app::{AppState, CommandError, Reply, dispatch};
use arrangia_publish::credentials::SecretStore;
use arrangia_publish::{ClientOptions, RetryPolicy};
use aws_lc_rs::encoding::{AsDer, Pkcs8V1Der};
use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
use base64::Engine as _;
use serde_json::{Value, json};
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ISSUER: &str = "57246542-96fe-1a63-e053-0824d011072a";
const API: &str = "/androidpublisher/v3/applications/com.example.fernwood";

fn pem(der: &[u8]) -> String {
    let b64 = base64::engine::general_purpose::STANDARD.encode(der);
    let mut out = String::from("-----BEGIN PRIVATE KEY-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        out.push_str(std::str::from_utf8(chunk).unwrap());
        out.push('\n');
    }
    out.push_str("-----END PRIVATE KEY-----\n");
    out
}

fn call(app: &AppState, name: &str, args: Value) -> Value {
    match dispatch(app, name, args) {
        Ok(Reply::Json(v)) => v,
        Ok(Reply::Image(_)) => panic!("{name} returned an image"),
        Err(e) => panic!("{name} failed: {e:?}"),
    }
}

fn fail(app: &AppState, name: &str, args: Value) -> CommandError {
    match dispatch(app, name, args) {
        Err(e) => e,
        Ok(v) => panic!("{name} should fail, got {v:?}"),
    }
}

fn reason(e: &CommandError) -> String {
    e.details.as_ref().unwrap()["reason"]
        .as_str()
        .unwrap()
        .to_owned()
}

struct Env {
    _tmp: tempfile::TempDir,
    app: Arc<AppState>,
    server: MockServer,
}

async fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let app = Arc::new(
        AppState::new(
            AppPaths::in_dir(&tmp.path().join("app")),
            Arc::new(RecordingSink::default()),
            Arc::new(NoDialogs),
        )
        .with_secret_store(SecretStore::mock())
        .without_watcher(),
    );
    let server = MockServer::start().await;
    app.upload().set_config(UploadConfig {
        apple_base_url: Some(server.uri()),
        google_base_url: Some(server.uri()),
        google_token_url: Some(format!("{}/token", server.uri())),
        options: ClientOptions {
            retry: RetryPolicy {
                max_attempts: 1,
                ..RetryPolicy::default()
            },
            request_timeout: Duration::from_secs(10),
            ..ClientOptions::default()
        },
        ..UploadConfig::default()
    });
    let kp = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING).unwrap();
    let p8 = tmp.path().join("AuthKey_ABCDE12345.p8");
    std::fs::write(&p8, pem(kp.to_pkcs8v1().unwrap().as_ref())).unwrap();
    call(
        &app,
        "store_account_import",
        json!({"kind": "apple", "path": p8, "issuer_id": ISSUER, "label": "team"}),
    );
    let rsa = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).unwrap();
    let der: Pkcs8V1Der<'static> = rsa.as_der().unwrap();
    let sa = tmp.path().join("sa.json");
    std::fs::write(
        &sa,
        json!({"type": "service_account", "project_id": "demo", "private_key_id": "k1",
               "private_key": pem(der.as_ref()), "client_email": "up@demo.iam.gserviceaccount.com"})
        .to_string(),
    )
    .unwrap();
    call(
        &app,
        "store_account_import",
        json!({"kind": "google", "path": sa, "label": "play"}),
    );
    Env {
        _tmp: tmp,
        app,
        server,
    }
}

fn app_json(id: &str, name: &str, bundle: &str) -> Value {
    json!({"type": "apps", "id": id, "attributes": {
        "name": name, "bundleId": bundle, "sku": format!("SKU-{id}"), "primaryLocale": "en-US"}})
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apple_app_picker_pages_caches_and_refreshes() {
    let e = env().await;
    let next = format!("{}/v1/apps?cursor=2", e.server.uri());
    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .and(query_param_is_missing("cursor"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [app_json("2", "Zinnia", "com.example.zinnia")],
            "links": {"next": next}
        })))
        .expect(2)
        .mount(&e.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .and(query_param("cursor", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [app_json("1", "Fernwood", "com.example.fernwood")]
        })))
        .expect(2)
        .mount(&e.server)
        .await;

    let list = call(&e.app, "store_apps_list", json!({"account": "apple-team"}));
    assert_eq!(list["cached"], false);
    let apps = list["apps"].as_array().unwrap();
    assert_eq!(apps.len(), 2);
    assert_eq!(apps[0]["name"], "Fernwood");
    assert_eq!(apps[0]["id"], "1");
    assert_eq!(apps[0]["bundle_id"], "com.example.fernwood");
    assert_eq!(apps[0]["sku"], "SKU-1");
    assert_eq!(apps[0]["primary_locale"], "en-US");

    // Second call: from the session cache, no request.
    let again = call(&e.app, "store_apps_list", json!({"account": "apple-team"}));
    assert_eq!(again["cached"], true);
    assert_eq!(again["apps"], list["apps"]);
    // refresh asks App Store Connect again (expect(2) above).
    let fresh = call(
        &e.app,
        "store_apps_list",
        json!({"account": "apple-team", "refresh": true}),
    );
    assert_eq!(fresh["cached"], false);

    // A Google account cannot list Apple apps.
    let err = fail(&e.app, "store_apps_list", json!({"account": "google-play"}));
    assert_eq!(err.code, arrangia_app::ErrorCode::InvalidArgs);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apple_app_picker_maps_errors_like_the_connection_test() {
    let e = env().await;
    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({
            "errors": [{"status": "401", "code": "NOT_AUTHORIZED", "title": "x", "detail": "y"}]
        })))
        .mount(&e.server)
        .await;
    let err = fail(&e.app, "store_apps_list", json!({"account": "apple-team"}));
    assert_eq!(err.code, arrangia_app::ErrorCode::Io);
    assert_eq!(reason(&err), "key_rejected");
    // Nothing was cached.
    let err = fail(&e.app, "store_apps_list", json!({"account": "apple-team"}));
    assert_eq!(reason(&err), "key_rejected");
}

async fn mount_token(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            json!({"access_token": "tok", "expires_in": 3600, "token_type": "Bearer"}),
        ))
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn play_package_check_uses_a_temporary_edit() {
    let e = env().await;
    mount_token(&e.server).await;
    Mock::given(method("POST"))
        .and(path(format!("{API}/edits")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": "e1"})))
        .expect(1)
        .mount(&e.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("{API}/edits/e1/details")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"defaultLanguage": "de-DE"})))
        .mount(&e.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("{API}/edits/e1/listings/de-DE")))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"language": "de-DE", "title": "Fernwood"})),
        )
        .mount(&e.server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("{API}/edits/e1")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&e.server)
        .await;
    let r = call(
        &e.app,
        "store_package_check",
        json!({"account": "google-play", "package_name": " com.example.fernwood "}),
    );
    assert_eq!(r["package_name"], "com.example.fernwood");
    assert_eq!(r["title"], "Fernwood");
    assert_eq!(r["default_language"], "de-DE");
    let reqs = e.server.received_requests().await.unwrap();
    assert!(!reqs.iter().any(|r| r.url.path().contains(":commit")));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn play_package_check_reports_unknown_packages_and_missing_access() {
    let e = env().await;
    mount_token(&e.server).await;
    Mock::given(method("POST"))
        .and(path(format!("{API}/edits")))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "error": {"code": 404, "message": "Package not found", "status": "NOT_FOUND"}
        })))
        .mount(&e.server)
        .await;
    let err = fail(
        &e.app,
        "store_package_check",
        json!({"account": "google-play", "package_name": "com.example.fernwood"}),
    );
    assert_eq!(reason(&err), "not_found", "{err:?}");

    let other = "/androidpublisher/v3/applications/com.example.other/edits";
    Mock::given(method("POST"))
        .and(path(other))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({
            "error": {"code": 403, "message": "The caller does not have permission", "status": "PERMISSION_DENIED"}
        })))
        .mount(&e.server)
        .await;
    let err = fail(
        &e.app,
        "store_package_check",
        json!({"account": "google-play", "package_name": "com.example.other"}),
    );
    assert_eq!(reason(&err), "no_permission", "{err:?}");
}
