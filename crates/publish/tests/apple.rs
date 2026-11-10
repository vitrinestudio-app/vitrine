//! App Store Connect client against a wiremock server (no real network).

mod common;

use std::sync::Arc;
use std::time::{Duration, Instant};

use arrangia_publish::apple::{AppRef, Client, Config, Platform, md5_hex};
use arrangia_publish::{
    ClientOptions, Error, ErrorKind, HttpEventKind, UploadStep, ignore_progress,
};
use common::{BearerJwt, ProgressLog, ec_key, fast_options, recorder, validation};
use jsonwebtoken::{Algorithm, DecodingKey};
use serde_json::{Value, json};
use wiremock::matchers::{
    body_bytes, body_json, body_partial_json, header, method, path, query_param,
};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ISSUER: &str = "57246542-96fe-1a63-e053-0824d011072a";
const KID: &str = "TESTKEY123";

fn check_claims(h: &jsonwebtoken::Header, c: &Value) -> bool {
    let lifetime = c["exp"].as_u64().unwrap_or(0) - c["iat"].as_u64().unwrap_or(0);
    h.alg == Algorithm::ES256
        && h.kid.as_deref() == Some(KID)
        && h.typ.as_deref() == Some("JWT")
        && c["iss"] == ISSUER
        && c["aud"] == "appstoreconnect-v1"
        && c.get("sub").is_none()
        && lifetime > 0
        && lifetime <= 20 * 60
}

struct Fixture {
    server: MockServer,
    public: DecodingKey,
    client: Client,
}

async fn fixture(options: ClientOptions) -> Fixture {
    let server = MockServer::start().await;
    let (pem, public) = ec_key();
    let config = Config::team(ISSUER, KID, pem).with_base_url(server.uri());
    let client = Client::new(config, options).unwrap();
    Fixture {
        server,
        public,
        client,
    }
}

impl Fixture {
    fn jwt(&self) -> BearerJwt {
        BearerJwt {
            key: self.public.clone(),
            validation: validation(Algorithm::ES256, "appstoreconnect-v1"),
            check: check_claims,
        }
    }
}

fn screenshot(id: &str, state: &str, ops: &Value) -> Value {
    json!({
        "data": {
            "type": "appScreenshots",
            "id": id,
            "attributes": {
                "fileName": "01.png",
                "fileSize": 10,
                "assetDeliveryState": { "state": state, "errors": [] },
                "uploadOperations": ops
            }
        }
    })
}

fn png_bytes() -> Vec<u8> {
    // Content does not matter for the mock; 10 bytes split into 6 + 4.
    b"\x89PNG012345".to_vec()
}

/// Two upload operations against the mock storage host, with a signature
/// in the query string (must never show up in logs).
fn two_parts(server: &MockServer) -> Value {
    json!([
        {
            "method": "PUT",
            "url": format!("{}/blob/part1?sig=SECRET1&Expires=1", server.uri()),
            "length": 6,
            "offset": 0,
            "requestHeaders": [ { "name": "Content-Type", "value": "image/png" },
                                { "name": "x-apple-part", "value": "1" } ]
        },
        {
            "method": "PUT",
            "url": format!("{}/blob/part2?sig=SECRET2&Expires=1", server.uri()),
            "length": 4,
            "offset": 6,
            "requestHeaders": [ { "name": "Content-Type", "value": "image/png" },
                                { "name": "x-apple-part", "value": "2" } ]
        }
    ])
}

async fn mount_reservation(f: &Fixture, id: &str) {
    Mock::given(method("POST"))
        .and(path("/v1/appScreenshots"))
        .and(f.jwt())
        .and(body_partial_json(json!({
            "data": {
                "type": "appScreenshots",
                "attributes": { "fileName": "01.png", "fileSize": 10 },
                "relationships": { "appScreenshotSet": { "data": { "type": "appScreenshotSets", "id": "set-1" } } }
            }
        })))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(screenshot(id, "AWAITING_UPLOAD", &two_parts(&f.server))),
        )
        .expect(1)
        .mount(&f.server)
        .await;
}

async fn mount_parts_ok(f: &Fixture) {
    let bytes = png_bytes();
    Mock::given(method("PUT"))
        .and(path("/blob/part1"))
        .and(query_param("sig", "SECRET1"))
        .and(header("content-type", "image/png"))
        .and(header("x-apple-part", "1"))
        .and(body_bytes(bytes[..6].to_vec()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&f.server)
        .await;
    Mock::given(method("PUT"))
        .and(path("/blob/part2"))
        .and(header("x-apple-part", "2"))
        .and(body_bytes(bytes[6..].to_vec()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&f.server)
        .await;
}

async fn mount_commit(f: &Fixture, id: &str) {
    Mock::given(method("PATCH"))
        .and(path(format!("/v1/appScreenshots/{id}")))
        .and(f.jwt())
        .and(body_json(json!({
            "data": {
                "type": "appScreenshots",
                "id": id,
                "attributes": { "uploaded": true, "sourceFileChecksum": md5_hex(&png_bytes()) }
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(screenshot(
            id,
            "UPLOAD_COMPLETE",
            &json!(null),
        )))
        .expect(1)
        .mount(&f.server)
        .await;
}

/// First poll: still processing; afterwards: `final_state`.
async fn mount_poll(f: &Fixture, id: &str, final_body: Value) {
    Mock::given(method("GET"))
        .and(path(format!("/v1/appScreenshots/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(screenshot(
            id,
            "UPLOAD_COMPLETE",
            &json!(null),
        )))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&f.server)
        .await;
    Mock::given(method("GET"))
        .and(path(format!("/v1/appScreenshots/{id}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(final_body))
        .with_priority(2)
        .mount(&f.server)
        .await;
}

#[tokio::test]
#[allow(clippy::too_many_lines)] // one linear scenario
async fn happy_path_replaces_and_orders_a_set() {
    let (events, opts) = recorder();
    let f = fixture(opts).await;
    let s = &f.server;

    // find app → version → localizations (paginated) → sets.
    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .and(query_param("filter[bundleId]", "com.example.app"))
        .and(f.jwt())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                { "type": "apps", "id": "999", "attributes": { "bundleId": "com.example.app.extra", "name": "Other" } },
                { "type": "apps", "id": "123", "attributes": { "bundleId": "com.example.app", "name": "Example" } }
            ]
        })))
        .expect(1)
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/apps/123/appStoreVersions"))
        .and(query_param("filter[platform]", "IOS"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                { "type": "appStoreVersions", "id": "v1", "attributes": { "platform": "IOS", "versionString": "1.0", "appVersionState": "READY_FOR_DISTRIBUTION" } },
                { "type": "appStoreVersions", "id": "v2", "attributes": { "platform": "IOS", "versionString": "1.1", "appVersionState": "PREPARE_FOR_SUBMISSION" } }
            ]
        })))
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/appStoreVersions/v2/appStoreVersionLocalizations"))
        .and(query_param("cursor", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "type": "appStoreVersionLocalizations", "id": "loc-de", "attributes": { "locale": "de-DE" } } ]
        })))
        .with_priority(1)
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/appStoreVersions/v2/appStoreVersionLocalizations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "type": "appStoreVersionLocalizations", "id": "loc-en", "attributes": { "locale": "en-US" } } ],
            "links": { "next": format!("{}/v1/appStoreVersions/v2/appStoreVersionLocalizations?cursor=2", s.uri()) }
        })))
        .with_priority(2)
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/appStoreVersionLocalizations/loc-de/appScreenshotSets"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "type": "appScreenshotSets", "id": "set-1", "attributes": { "screenshotDisplayType": "APP_IPHONE_67" } } ]
        })))
        .mount(s)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/appScreenshotSets"))
        .and(body_json(json!({
            "data": {
                "type": "appScreenshotSets",
                "attributes": { "screenshotDisplayType": "APP_IPAD_PRO_3GEN_129" },
                "relationships": { "appStoreVersionLocalization": { "data": { "type": "appStoreVersionLocalizations", "id": "loc-de" } } }
            }
        })))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "data": { "type": "appScreenshotSets", "id": "set-2", "attributes": { "screenshotDisplayType": "APP_IPAD_PRO_3GEN_129" } }
        })))
        .expect(1)
        .mount(s)
        .await;
    // Existing screenshot in set-1 is deleted.
    Mock::given(method("GET"))
        .and(path("/v1/appScreenshotSets/set-1/appScreenshots"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [ { "type": "appScreenshots", "id": "old-1", "attributes": { "fileName": "old.png", "assetDeliveryState": { "state": "COMPLETE" } } } ]
        })))
        .mount(s)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/appScreenshots/old-1"))
        .and(f.jwt())
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(s)
        .await;
    mount_reservation(&f, "new-1").await;
    mount_parts_ok(&f).await;
    mount_commit(&f, "new-1").await;
    mount_poll(&f, "new-1", screenshot("new-1", "COMPLETE", &json!(null))).await;
    Mock::given(method("PATCH"))
        .and(path(
            "/v1/appScreenshotSets/set-1/relationships/appScreenshots",
        ))
        .and(f.jwt())
        .and(body_json(json!({ "data": [
            { "type": "appScreenshots", "id": "new-1" },
            { "type": "appScreenshots", "id": "keep-2" }
        ] })))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(s)
        .await;

    let c = &f.client;
    let app = c
        .find_app(&AppRef::BundleId("com.example.app".into()))
        .await
        .unwrap();
    assert_eq!(app.id, "123");
    assert_eq!(app.name, "Example");
    let version = c.editable_version(&app.id, Platform::Ios).await.unwrap();
    assert_eq!(version.id, "v2");
    let locs = c.localizations(&version.id).await.unwrap();
    assert_eq!(
        locs.iter().map(|l| l.locale.as_str()).collect::<Vec<_>>(),
        ["en-US", "de-DE"]
    );
    let set = c.ensure_set("loc-de", "APP_IPHONE_67").await.unwrap();
    assert_eq!(set.id, "set-1");
    let created = c
        .ensure_set("loc-de", "APP_IPAD_PRO_3GEN_129")
        .await
        .unwrap();
    assert_eq!(created.id, "set-2");

    for old in c.list_screenshots(&set.id).await.unwrap() {
        c.delete_screenshot(&old.id).await.unwrap();
    }
    let log = ProgressLog::default();
    let shot = c
        .upload_screenshot(&set.id, "01.png", png_bytes(), &|p| log.push(p))
        .await
        .unwrap();
    assert_eq!(shot.id, "new-1");
    c.reorder(&set.id, &["new-1".into(), "keep-2".into()])
        .await
        .unwrap();

    let steps: Vec<_> = log.all().iter().map(|p| p.step).collect();
    assert_eq!(steps.first(), Some(&UploadStep::Reserve));
    assert_eq!(steps.last(), Some(&UploadStep::Done));
    for s in [
        UploadStep::Transfer,
        UploadStep::Commit,
        UploadStep::Processing,
    ] {
        assert!(steps.contains(&s), "missing {s:?} in {steps:?}");
    }
    let last = log.all().pop().unwrap();
    assert_eq!(
        (last.bytes_sent, last.bytes_total, last.parts),
        (10, 10, Some((2, 2)))
    );

    // Logging hook saw the traffic, without signatures or tokens.
    let events = events.lock().unwrap();
    assert!(events.iter().any(|e| e.url.ends_with("/blob/part1")));
    assert!(
        events
            .iter()
            .any(|e| matches!(e.kind, HttpEventKind::Response { status: 204, .. }))
    );
    let dump = format!("{events:?}");
    assert!(!dump.contains("SECRET"), "signed URL leaked: {dump}");
    assert!(
        !dump.contains("Bearer") && !dump.contains("eyJ"),
        "token leaked"
    );
}

#[tokio::test]
async fn find_app_by_id_and_missing_bundle() {
    let f = fixture(fast_options()).await;
    Mock::given(method("GET"))
        .and(path("/v1/apps/123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "type": "apps", "id": "123", "attributes": { "bundleId": "com.example.app", "name": "Example" } }
        })))
        .mount(&f.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/apps"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": [] })))
        .mount(&f.server)
        .await;
    let app = f.client.find_app(&AppRef::Id("123".into())).await.unwrap();
    assert_eq!(app.bundle_id, "com.example.app");
    let err = f
        .client
        .find_app(&AppRef::BundleId("com.missing".into()))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::NotFound);
}

#[tokio::test]
async fn part_upload_is_retried_after_server_error() {
    let f = fixture(fast_options()).await;
    mount_reservation(&f, "new-1").await;
    // part1 fails once with 500, then succeeds.
    Mock::given(method("PUT"))
        .and(path("/blob/part1"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .with_priority(1)
        .expect(1)
        .mount(&f.server)
        .await;
    mount_parts_ok(&f).await;
    mount_commit(&f, "new-1").await;
    mount_poll(&f, "new-1", screenshot("new-1", "COMPLETE", &json!(null))).await;
    let shot = f
        .client
        .upload_screenshot("set-1", "01.png", png_bytes(), &ignore_progress)
        .await
        .unwrap();
    assert_eq!(shot.id, "new-1");
}

#[tokio::test]
async fn rejected_part_deletes_the_reservation() {
    let f = fixture(fast_options()).await;
    mount_reservation(&f, "new-1").await;
    Mock::given(method("PUT"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&f.server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/appScreenshots/new-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&f.server)
        .await;
    Mock::given(method("PATCH"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&f.server)
        .await;
    let err = f
        .client
        .upload_screenshot("set-1", "01.png", png_bytes(), &ignore_progress)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Validation, "{err}");
}

#[tokio::test]
async fn rate_limit_waits_for_retry_after() {
    let f = fixture(fast_options()).await;
    Mock::given(method("GET"))
        .and(path("/v1/apps/123"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("retry-after", "1")
                .set_body_json(json!({ "errors": [ { "status": "429", "code": "RATE_LIMIT_EXCEEDED", "title": "t", "detail": "d" } ] })),
        )
        .up_to_n_times(1)
        .with_priority(1)
        .expect(1)
        .mount(&f.server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/apps/123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "type": "apps", "id": "123", "attributes": { "bundleId": "b", "name": "n" } }
        })))
        .with_priority(2)
        .mount(&f.server)
        .await;
    let started = Instant::now();
    f.client.find_app(&AppRef::Id("123".into())).await.unwrap();
    assert!(started.elapsed() >= Duration::from_secs(1));
}

#[tokio::test]
async fn long_retry_after_is_reported_not_awaited() {
    let f = fixture(fast_options()).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "3600"))
        .expect(1)
        .mount(&f.server)
        .await;
    let err = f
        .client
        .find_app(&AppRef::Id("1".into()))
        .await
        .unwrap_err();
    assert!(
        matches!(err, Error::RateLimited { retry_after: Some(d) } if d == Duration::from_secs(3600)),
        "{err:?}"
    );
    assert!(err.is_transient());
}

#[tokio::test]
async fn rate_limit_without_header_retries_with_backoff_then_gives_up() {
    let f = fixture(fast_options()).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429))
        .expect(4)
        .mount(&f.server)
        .await;
    let err = f
        .client
        .find_app(&AppRef::Id("1".into()))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::RateLimited);
}

#[tokio::test]
async fn processing_failure_is_reported_and_cleaned_up() {
    let f = fixture(fast_options()).await;
    mount_reservation(&f, "new-1").await;
    mount_parts_ok(&f).await;
    mount_commit(&f, "new-1").await;
    mount_poll(
        &f,
        "new-1",
        json!({ "data": { "type": "appScreenshots", "id": "new-1", "attributes": {
            "assetDeliveryState": { "state": "FAILED", "errors": [
                { "code": "IMAGE_INCORRECT_DIMENSIONS", "description": "The dimensions of one or more screenshots are wrong." }
            ] }
        } } }),
    )
    .await;
    Mock::given(method("DELETE"))
        .and(path("/v1/appScreenshots/new-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&f.server)
        .await;
    let err = f
        .client
        .upload_screenshot("set-1", "01.png", png_bytes(), &ignore_progress)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Processing);
    assert_eq!(err.details()[0].code, "IMAGE_INCORRECT_DIMENSIONS");
    assert!(err.to_string().contains("dimensions"));
}

#[tokio::test]
async fn processing_timeout() {
    let mut opts = fast_options();
    opts.poll.timeout = Duration::from_millis(50);
    let f = fixture(opts).await;
    mount_reservation(&f, "new-1").await;
    mount_parts_ok(&f).await;
    mount_commit(&f, "new-1").await;
    mount_poll(
        &f,
        "new-1",
        screenshot("new-1", "UPLOAD_COMPLETE", &json!(null)),
    )
    .await;
    let err = f
        .client
        .upload_screenshot("set-1", "01.png", png_bytes(), &ignore_progress)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Timeout);
}

#[tokio::test]
async fn no_editable_version_is_a_conflict() {
    let f = fixture(fast_options()).await;
    Mock::given(method("GET"))
        .and(path("/v1/apps/123/appStoreVersions"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                { "type": "appStoreVersions", "id": "v1", "attributes": { "platform": "IOS", "versionString": "1.0", "appVersionState": "READY_FOR_DISTRIBUTION" } },
                { "type": "appStoreVersions", "id": "v2", "attributes": { "platform": "IOS", "versionString": "1.1", "appVersionState": "WAITING_FOR_REVIEW" } }
            ]
        })))
        .mount(&f.server)
        .await;
    let err = f
        .client
        .editable_version("123", Platform::Ios)
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Conflict);
    assert!(err.to_string().contains("1.1 WAITING_FOR_REVIEW"), "{err}");
}

#[tokio::test]
async fn state_error_from_api_is_a_conflict() {
    let f = fixture(fast_options()).await;
    Mock::given(method("DELETE"))
        .respond_with(
            ResponseTemplate::new(409).set_body_json(json!({ "errors": [ {
            "status": "409", "code": "STATE_ERROR", "title": "The request cannot be fulfilled",
            "detail": "Screenshots cannot be changed in the current state."
        } ] })),
        )
        .mount(&f.server)
        .await;
    let err = f.client.delete_screenshot("x").await.unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Conflict);
    assert_eq!(err.details()[0].code, "STATE_ERROR");
}

#[tokio::test]
async fn delete_of_missing_screenshot_is_ok() {
    let f = fixture(fast_options()).await;
    Mock::given(method("DELETE"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&f.server)
        .await;
    f.client.delete_screenshot("gone").await.unwrap();
}

#[tokio::test]
async fn auth_failure_after_one_resign() {
    let f = fixture(fast_options()).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "errors": [ {
            "status": "401", "code": "NOT_AUTHORIZED", "title": "Authentication credentials are missing or invalid.",
            "detail": "Provide a properly configured and signed bearer token."
        } ] })))
        .expect(2)
        .mount(&f.server)
        .await;
    let err = f
        .client
        .find_app(&AppRef::Id("1".into()))
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            Error::Auth {
                status: Some(401),
                ..
            }
        ),
        "{err:?}"
    );
}

#[tokio::test]
async fn token_is_resigned_after_401_and_cached_otherwise() {
    let f = fixture(fast_options()).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&f.server)
        .await;
    Mock::given(method("GET"))
        .and(f.jwt())
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "type": "apps", "id": "1", "attributes": { "bundleId": "b", "name": "n" } }
        })))
        .with_priority(2)
        .mount(&f.server)
        .await;
    f.client.find_app(&AppRef::Id("1".into())).await.unwrap();
    f.client.find_app(&AppRef::Id("1".into())).await.unwrap();
    let requests = f.server.received_requests().await.unwrap();
    let tokens: Vec<_> = requests
        .iter()
        .map(|r| {
            r.headers
                .get("authorization")
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        })
        .collect();
    assert_eq!(tokens.len(), 3);
    // Second and third request share the re-signed, cached token.
    assert_eq!(tokens[1], tokens[2]);
}

#[tokio::test]
async fn individual_key_uses_sub_user() {
    let server = MockServer::start().await;
    let (pem, public) = ec_key();
    let client = Client::new(
        Config::individual(KID, pem).with_base_url(server.uri()),
        fast_options(),
    )
    .unwrap();
    Mock::given(method("GET"))
        .and(BearerJwt {
            key: public,
            validation: validation(Algorithm::ES256, "appstoreconnect-v1"),
            check: |_, c| c["sub"] == "user" && c.get("iss").is_none(),
        })
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": { "type": "apps", "id": "1", "attributes": { "bundleId": "b", "name": "n" } }
        })))
        .expect(1)
        .mount(&server)
        .await;
    client.find_app(&AppRef::Id("1".into())).await.unwrap();
}

#[tokio::test]
async fn invalid_key_is_a_config_error() {
    let err = Client::new(Config::team(ISSUER, KID, "not a key"), fast_options()).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Config);
    assert!(!err.to_string().contains("not a key"));
}

#[tokio::test]
async fn cancellation_interrupts_a_retry_wait() {
    let mut opts = fast_options();
    opts.retry.max_retry_after = Duration::from_secs(60);
    let f = fixture(opts).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "30"))
        .mount(&f.server)
        .await;
    let cancel = f.client.options().cancel.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        cancel.cancel();
    });
    let started = Instant::now();
    let err = f
        .client
        .find_app(&AppRef::Id("1".into()))
        .await
        .unwrap_err();
    assert!(matches!(err, Error::Cancelled));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn foreign_pagination_links_are_not_followed() {
    let f = fixture(fast_options()).await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [],
            "links": { "next": "https://evil.example.com/v1/apps?cursor=2" }
        })))
        .mount(&f.server)
        .await;
    let err = f
        .client
        .find_app(&AppRef::BundleId("x".into()))
        .await
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::Transport);
}

#[test]
fn options_are_shareable() {
    // Observer closures must be Send + Sync (executor runs on tokio).
    let opts = ClientOptions {
        observer: Some(Arc::new(|_: &arrangia_publish::HttpEvent| {})),
        ..ClientOptions::default()
    };
    let _ = format!("{opts:?}");
}
