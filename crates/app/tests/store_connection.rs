//! `store_account_test` with the real store clients against wiremock
//! (in-memory keychain, throwaway keys generated at test time).

use std::sync::Arc;

use arrangia_app::dialogs::NoDialogs;
use arrangia_app::events::RecordingSink;
use arrangia_app::settings::AppPaths;
use arrangia_app::store_test::StoreClientTester;
use arrangia_app::{AppState, CommandError, ErrorCode, Reply, dispatch};
use arrangia_publish::credentials::SecretStore;
use arrangia_publish::{ClientOptions, RetryPolicy};
use aws_lc_rs::encoding::{AsDer, Pkcs8V1Der};
use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
use base64::Engine as _;
use serde_json::{Value, json};
use wiremock::matchers::{header_exists, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ISSUER: &str = "57246542-96fe-1a63-e053-0824d011072a";

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

fn ec_pem() -> String {
    let kp = EcdsaKeyPair::generate(&ECDSA_P256_SHA256_FIXED_SIGNING).unwrap();
    pem(kp.to_pkcs8v1().unwrap().as_ref())
}

fn rsa_pem() -> String {
    let kp = aws_lc_rs::rsa::KeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).unwrap();
    let der: Pkcs8V1Der<'static> = kp.as_der().unwrap();
    pem(der.as_ref())
}

fn tester(apple: Option<String>, token: Option<String>) -> StoreClientTester {
    StoreClientTester {
        apple_base_url: apple,
        google_base_url: None,
        google_token_url: token,
        options: ClientOptions {
            retry: RetryPolicy {
                max_attempts: 1,
                ..RetryPolicy::default()
            },
            request_timeout: std::time::Duration::from_secs(5),
            ..ClientOptions::default()
        },
    }
}

struct Setup {
    _tmp: tempfile::TempDir,
    app: AppState,
    apple_id: String,
    google_id: String,
}

fn setup(tester: StoreClientTester) -> Setup {
    let tmp = tempfile::tempdir().unwrap();
    let app = AppState::new(
        AppPaths::in_dir(&tmp.path().join("app")),
        Arc::new(RecordingSink::default()),
        Arc::new(NoDialogs),
    )
    .with_secret_store(SecretStore::mock())
    .without_watcher();
    app.set_connection_tester(Arc::new(tester));
    let p8 = tmp.path().join("AuthKey_ABCDE12345.p8");
    std::fs::write(&p8, ec_pem()).unwrap();
    let a = call(
        &app,
        "store_account_import",
        json!({"kind": "apple", "path": p8, "issuer_id": ISSUER}),
    );
    let sa = tmp.path().join("sa.json");
    std::fs::write(
        &sa,
        json!({
            "type": "service_account",
            "project_id": "demo",
            "private_key_id": "k1",
            "private_key": rsa_pem(),
            "client_email": "uploader@demo.iam.gserviceaccount.com",
            "token_uri": "https://evil.example/token",
        })
        .to_string(),
    )
    .unwrap();
    let g = call(
        &app,
        "store_account_import",
        json!({"kind": "google", "path": sa}),
    );
    Setup {
        apple_id: a["id"].as_str().unwrap().to_owned(),
        google_id: g["id"].as_str().unwrap().to_owned(),
        app,
        _tmp: tmp,
    }
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

fn reason(e: &CommandError) -> &str {
    e.details.as_ref().unwrap()["reason"].as_str().unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apple_lists_visible_apps() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .and(query_param("limit", "1"))
        .and(header_exists("authorization"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"type": "apps", "id": "1234567890",
                      "attributes": {"name": "Fernwood", "bundleId": "com.example.fernwood"}}],
            "links": {"self": "x", "next": "y"},
            "meta": {"paging": {"total": 3, "limit": 1}}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let s = setup(tester(Some(server.uri()), None));
    let r = call(&s.app, "store_account_test", json!({"id": s.apple_id}));
    assert_eq!(r["ok"], true);
    assert_eq!(r["apps_visible"], 3);
    assert_eq!(r["first_app"], "Fernwood");
    assert_eq!(
        r["summary"],
        "key accepted, 3 apps visible (first: Fernwood)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apple_no_apps_and_rejected_key() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({"data": [], "meta": {"paging": {"total": 0}}})),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    let s = setup(tester(Some(server.uri()), None));
    let r = call(&s.app, "store_account_test", json!({"id": s.apple_id}));
    assert_eq!(r["apps_visible"], 0);
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains("no apps are visible")
    );

    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({"errors": [{
            "status": "401", "code": "NOT_AUTHORIZED",
            "title": "Authentication credentials are missing or invalid.",
            "detail": "Provide a properly configured and signed bearer token."
        }]})))
        .mount(&server)
        .await;
    let e = fail(&s.app, "store_account_test", json!({"id": s.apple_id}));
    assert_eq!(e.code, ErrorCode::Io);
    assert_eq!(reason(&e), "key_rejected");
    assert!(e.message.starts_with("key rejected by App Store Connect"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apple_forbidden_is_a_permission_problem() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({"errors": [{
            "status": "403", "code": "FORBIDDEN_ERROR", "title": "forbidden", "detail": "no"
        }]})))
        .mount(&server)
        .await;
    let s = setup(tester(Some(server.uri()), None));
    let e = fail(&s.app, "store_account_test", json!({"id": s.apple_id}));
    assert_eq!(reason(&e), "no_permission");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn google_fetches_a_token_only() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "ya29.test", "expires_in": 3599, "token_type": "Bearer"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let s = setup(tester(None, Some(format!("{}/token", server.uri()))));
    let r = call(&s.app, "store_account_test", json!({"id": s.google_id}));
    assert_eq!(r["ok"], true);
    assert_eq!(r["client_email"], "uploader@demo.iam.gserviceaccount.com");
    assert!(
        r["summary"]
            .as_str()
            .unwrap()
            .contains("uploader@demo.iam.gserviceaccount.com")
    );
    // Read-only: nothing but the token request.
    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn google_rejected_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant", "error_description": "Invalid JWT Signature."
        })))
        .mount(&server)
        .await;
    let s = setup(tester(None, Some(format!("{}/token", server.uri()))));
    let e = fail(&s.app, "store_account_test", json!({"id": s.google_id}));
    assert_eq!(reason(&e), "key_rejected");
    assert!(e.message.contains("invalid_grant"));
}

#[test]
fn unreachable_store_is_offline() {
    // A port that was free a moment ago: connection refused.
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("http://127.0.0.1:{port}");
    let s = setup(tester(Some(url.clone()), Some(format!("{url}/token"))));
    for id in [&s.apple_id, &s.google_id] {
        let e = fail(&s.app, "store_account_test", json!({"id": id}));
        assert_eq!(reason(&e), "offline", "{}", e.message);
        assert!(e.message.contains("offline"));
    }
}
