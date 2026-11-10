//! Google Play client against a wiremock server (no real network).

mod common;

use std::time::Duration;

use arrangia_publish::google::{Client, Config, ImageType};
use arrangia_publish::{ClientOptions, Error, ErrorKind, UploadStep, ignore_progress};
use common::{ProgressLog, fast_options, form_field, recorder, rsa_key, validation, verify};
use jsonwebtoken::{Algorithm, DecodingKey};
use serde_json::{Value, json};
use wiremock::matchers::{body_bytes, header, method, path, query_param, query_param_is_missing};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

const EMAIL: &str = "arrangia-test@example-project.iam.gserviceaccount.com";
const PKG: &str = "com.example.app";
const API: &str = "/androidpublisher/v3/applications/com.example.app";
const UPLOAD: &str = "/upload/androidpublisher/v3/applications/com.example.app";

/// Token request with a valid RS256 assertion signed by the test key.
struct TokenRequest(DecodingKey);

impl Match for TokenRequest {
    fn matches(&self, request: &Request) -> bool {
        let ct_ok = request
            .headers
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("application/x-www-form-urlencoded"));
        let grant_ok = form_field(&request.body, "grant_type").as_deref()
            == Some("urn:ietf:params:oauth:grant-type:jwt-bearer");
        let Some(assertion) = form_field(&request.body, "assertion") else {
            return false;
        };
        ct_ok
            && grant_ok
            && verify(
                &assertion,
                &self.0,
                &validation(Algorithm::RS256, "https://oauth2.googleapis.com/token"),
                check_assertion,
            )
    }
}

fn check_assertion(h: &jsonwebtoken::Header, c: &Value) -> bool {
    let lifetime = c["exp"].as_u64().unwrap_or(0) - c["iat"].as_u64().unwrap_or(0);
    h.alg == Algorithm::RS256
        && h.kid.as_deref() == Some("key-id-1")
        && c["iss"] == EMAIL
        && c["scope"] == "https://www.googleapis.com/auth/androidpublisher"
        && lifetime > 0
        && lifetime <= 3600
}

struct Fixture {
    server: MockServer,
    public: DecodingKey,
    client: Client,
}

fn service_account_json(pem: &str) -> String {
    json!({
        "type": "service_account",
        "project_id": "example-project",
        "private_key_id": "key-id-1",
        "private_key": pem,
        "client_email": EMAIL,
        "client_id": "1",
        // Ignored on purpose: the configured token URL is used.
        "token_uri": "https://attacker.example.com/token"
    })
    .to_string()
}

async fn fixture(options: ClientOptions) -> Fixture {
    let server = MockServer::start().await;
    let (pem, public) = rsa_key();
    let config = Config::new(service_account_json(&pem), PKG)
        .with_endpoints(server.uri(), format!("{}/token", server.uri()));
    let client = Client::new(config, options).unwrap();
    Fixture {
        server,
        public,
        client,
    }
}

impl Fixture {
    async fn mount_token(&self, token: &str, expires_in: u64, times: u64) {
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(TokenRequest(self.public.clone()))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "access_token": token, "expires_in": expires_in, "token_type": "Bearer"
            })))
            .expect(times)
            .mount(&self.server)
            .await;
    }
}

fn image(id: &str) -> Value {
    json!({ "id": id, "url": format!("https://lh3.example/{id}"), "sha1": "aa", "sha256": "bb" })
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // one linear scenario
async fn happy_path_replaces_listing_images() {
    let (events, opts) = recorder();
    let f = fixture(opts).await;
    let s = &f.server;
    f.mount_token("tok-1", 3600, 1).await;
    Mock::given(method("POST"))
        .and(path(format!("{API}/edits")))
        .and(header("authorization", "Bearer tok-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "id": "e1", "expiryTimeSeconds": "1900000000" })),
        )
        .expect(1)
        .mount(s)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!(
            "{API}/edits/e1/listings/de-DE/phoneScreenshots"
        )))
        .and(header("authorization", "Bearer tok-1"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "deleted": [image("old-1"), image("old-2")] })),
        )
        .expect(1)
        .mount(s)
        .await;
    for i in 1..=3u8 {
        Mock::given(method("POST"))
            .and(path(format!(
                "{UPLOAD}/edits/e1/listings/de-DE/phoneScreenshots"
            )))
            .and(query_param("uploadType", "media"))
            .and(header("content-type", "image/png"))
            .and(header("authorization", "Bearer tok-1"))
            .and(body_bytes(vec![i; 16]))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({ "image": image(&format!("new-{i}")) })),
            )
            .expect(1)
            .mount(s)
            .await;
    }
    Mock::given(method("GET"))
        .and(path(format!(
            "{API}/edits/e1/listings/de-DE/phoneScreenshots"
        )))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "images": [image("new-1"), image("new-2"), image("new-3")]
        })))
        .mount(s)
        .await;
    Mock::given(method("POST"))
        .and(path(format!("{API}/edits/e1:commit")))
        .and(query_param("changesNotSentForReview", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "e1" })))
        .expect(1)
        .mount(s)
        .await;

    let c = &f.client;
    assert_eq!(c.client_email(), EMAIL);
    let edit = c.insert_edit().await.unwrap();
    assert_eq!(edit.id, "e1");
    let deleted = c
        .delete_all(&edit.id, "de-DE", ImageType::PhoneScreenshots)
        .await
        .unwrap();
    assert_eq!(deleted.len(), 2);
    let log = ProgressLog::default();
    let mut ids = Vec::new();
    for i in 1..=3u8 {
        let img = c
            .upload_image(
                &edit.id,
                "de-DE",
                ImageType::PhoneScreenshots,
                vec![i; 16],
                "image/png",
                &|p| log.push(p),
            )
            .await
            .unwrap();
        ids.push(img.id);
    }
    assert_eq!(ids, ["new-1", "new-2", "new-3"]);
    let listed = c
        .list_images(&edit.id, "de-DE", ImageType::PhoneScreenshots)
        .await
        .unwrap();
    assert_eq!(
        listed.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        ["new-1", "new-2", "new-3"]
    );
    c.commit(&edit.id, true).await.unwrap();

    let last = log.all().pop().unwrap();
    assert_eq!(last.step, UploadStep::Done);
    assert_eq!((last.bytes_sent, last.bytes_total), (16, 16));
    assert_eq!(last.asset_id.as_deref(), Some("new-3"));

    let dump = format!("{:?}", events.lock().unwrap());
    assert!(
        !dump.contains("tok-1") && !dump.contains("assertion"),
        "secret leaked: {dump}"
    );
    assert!(dump.contains("/token"));
}

#[tokio::test]
async fn commit_without_flag_and_abort() {
    let f = fixture(fast_options()).await;
    f.mount_token("tok-1", 3600, 1).await;
    Mock::given(method("POST"))
        .and(path(format!("{API}/edits/e1:commit")))
        .and(query_param_is_missing("changesNotSentForReview"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "id": "e1" })))
        .expect(1)
        .mount(&f.server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("{API}/edits/e2")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&f.server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(format!("{API}/edits/gone")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&f.server)
        .await;
    f.client.commit("e1", false).await.unwrap();
    f.client.delete_edit("e2").await.unwrap();
    f.client.delete_edit("gone").await.unwrap();
}

#[tokio::test]
async fn delete_all_with_empty_body() {
    let f = fixture(fast_options()).await;
    f.mount_token("tok-1", 3600, 1).await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&f.server)
        .await;
    let deleted = f
        .client
        .delete_all("e1", "en-US", ImageType::FeatureGraphic)
        .await
        .unwrap();
    assert_eq!(deleted.len(), 0);
}

#[tokio::test]
async fn upload_is_retried_on_429_and_503_but_not_500() {
    let f = fixture(fast_options()).await;
    f.mount_token("tok-1", 3600, 1).await;
    let url = format!("{UPLOAD}/edits/e1/listings/en-US/tenInchScreenshots");
    Mock::given(method("POST"))
        .and(path(url.clone()))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&f.server)
        .await;
    Mock::given(method("POST"))
        .and(path(url.clone()))
        .respond_with(ResponseTemplate::new(503))
        .up_to_n_times(1)
        .with_priority(2)
        .mount(&f.server)
        .await;
    Mock::given(method("POST"))
        .and(path(url.clone()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "image": image("i1") })))
        .up_to_n_times(1)
        .with_priority(3)
        .mount(&f.server)
        .await;
    let img = f
        .client
        .upload_image(
            "e1",
            "en-US",
            ImageType::TenInchScreenshots,
            vec![1; 4],
            "image/jpeg",
            &ignore_progress,
        )
        .await
        .unwrap();
    assert_eq!(img.id, "i1");

    // A 500 on a non-idempotent upload is not repeated (could duplicate).
    Mock::given(method("POST"))
        .and(path(url))
        .respond_with(ResponseTemplate::new(500))
        .with_priority(4)
        .expect(1)
        .mount(&f.server)
        .await;
    let err = f
        .client
        .upload_image(
            "e1",
            "en-US",
            ImageType::TenInchScreenshots,
            vec![1; 4],
            "image/png",
            &ignore_progress,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            Error::Transport {
                status: Some(500),
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn idempotent_calls_retry_server_errors() {
    let f = fixture(fast_options()).await;
    f.mount_token("tok-1", 3600, 1).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(502))
        .up_to_n_times(2)
        .with_priority(1)
        .expect(2)
        .mount(&f.server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "images": [] })))
        .with_priority(2)
        .mount(&f.server)
        .await;
    let images = f
        .client
        .list_images("e1", "en-US", ImageType::Icon)
        .await
        .unwrap();
    assert_eq!(images.len(), 0);
}

#[tokio::test]
async fn validation_error_carries_detail() {
    let f = fixture(fast_options()).await;
    f.mount_token("tok-1", 3600, 1).await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({ "error": {
            "code": 400, "message": "Image is too small.", "status": "INVALID_ARGUMENT",
            "errors": [ { "reason": "imageTooSmall", "message": "Image is too small." } ]
        } })))
        .mount(&f.server)
        .await;
    let err = f
        .client
        .upload_image(
            "e1",
            "en-US",
            ImageType::PhoneScreenshots,
            vec![1; 4],
            "image/png",
            &ignore_progress,
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    assert_eq!(err.details()[0].code, "imageTooSmall");
}

#[tokio::test]
async fn local_checks_before_upload() {
    let f = fixture(fast_options()).await;
    let too_big = vec![0u8; arrangia_publish::google::MAX_IMAGE_BYTES + 1];
    let err = f
        .client
        .upload_image(
            "e1",
            "en-US",
            ImageType::PhoneScreenshots,
            too_big,
            "image/png",
            &ignore_progress,
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation);
    let err = f
        .client
        .upload_image(
            "e1",
            "en-US",
            ImageType::PhoneScreenshots,
            vec![1],
            "text/plain",
            &ignore_progress,
        )
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Config);
    assert!(f.server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn invalid_grant_is_an_auth_error() {
    let f = fixture(fast_options()).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant", "error_description": "Invalid JWT Signature."
        })))
        .expect(1)
        .mount(&f.server)
        .await;
    let err = f.client.insert_edit().await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Auth {
                status: Some(400),
                ..
            }
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("Invalid JWT Signature"));
}

#[tokio::test]
async fn missing_permission_is_an_auth_error() {
    let f = fixture(fast_options()).await;
    f.mount_token("tok-1", 3600, 1).await;
    Mock::given(method("POST"))
        .and(path(format!("{API}/edits")))
        .respond_with(ResponseTemplate::new(403).set_body_json(json!({ "error": {
            "code": 403, "message": "The caller does not have permission", "status": "PERMISSION_DENIED"
        } })))
        .mount(&f.server)
        .await;
    let err = f.client.insert_edit().await.unwrap_err();
    assert!(
        matches!(
            err,
            Error::Auth {
                status: Some(403),
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn token_is_cached_and_refreshed_on_expiry_and_401() {
    let f = fixture(fast_options()).await;
    // expires_in below the refresh margin: every call fetches a new token.
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "access_token": "short", "expires_in": 30 })),
        )
        .expect(2)
        .mount(&f.server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "images": [] })))
        .mount(&f.server)
        .await;
    f.client
        .list_images("e1", "en-US", ImageType::Icon)
        .await
        .unwrap();
    f.client
        .list_images("e1", "en-US", ImageType::Icon)
        .await
        .unwrap();

    // A 401 forces exactly one refetch, then the new token is reused.
    let g = fixture(fast_options()).await;
    g.mount_token("long", 3600, 2).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&g.server)
        .await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "images": [] })))
        .with_priority(2)
        .mount(&g.server)
        .await;
    g.client
        .list_images("e1", "en-US", ImageType::Icon)
        .await
        .unwrap();
    g.client
        .list_images("e1", "en-US", ImageType::Icon)
        .await
        .unwrap();
}

#[tokio::test]
async fn edit_state_error_is_a_conflict() {
    let f = fixture(fast_options()).await;
    f.mount_token("tok-1", 3600, 1).await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({ "error": {
            "code": 400, "message": "This Edit has been deleted.", "status": "FAILED_PRECONDITION"
        } })))
        .mount(&f.server)
        .await;
    let err = f.client.commit("e1", false).await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Conflict);
}

#[tokio::test]
async fn rate_limit_with_long_retry_after() {
    let f = fixture(fast_options()).await;
    f.mount_token("tok-1", 3600, 1).await;
    Mock::given(method("POST"))
        .and(path(format!("{API}/edits")))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "600"))
        .expect(1)
        .mount(&f.server)
        .await;
    let err = f.client.insert_edit().await.unwrap_err();
    assert!(
        matches!(err, Error::RateLimited { retry_after: Some(d) } if d == Duration::from_secs(600))
    );
}

#[test]
fn bad_service_account_files_are_config_errors() {
    let err = Client::new(Config::new("{}", PKG), fast_options()).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Config);
    let json =
        json!({ "type": "authorized_user", "client_email": "a", "private_key": "b" }).to_string();
    let err = Client::new(Config::new(json, PKG), fast_options()).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Config);
    let json =
        json!({ "type": "service_account", "client_email": "a", "private_key": "SECRETKEY" })
            .to_string();
    let err = Client::new(Config::new(json, PKG), fast_options()).unwrap_err();
    assert!(!err.to_string().contains("SECRETKEY"));
}
