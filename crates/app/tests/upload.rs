//! Upload plan + executor end to end (Phase 5.2/5.5) against stateful fake
//! store servers (wiremock, no real network), with an in-memory keychain
//! and throwaway keys generated at test time.

// Scripted scenarios with fake servers read best long and terse.
#![allow(
    clippy::many_single_char_names,
    clippy::too_many_lines,
    clippy::match_bool,
    clippy::collapsible_if,
    clippy::assert_is_empty,
    clippy::manual_let_else,
    clippy::format_collect
)]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use arrangia_app::dialogs::NoDialogs;
use arrangia_app::events::RecordingSink;
use arrangia_app::settings::AppPaths;
use arrangia_app::upload::policy::{self, McpHost};
use arrangia_app::upload::{UploadConfig, UploadPlan};
use arrangia_app::{Actor, AppState, CommandError, ErrorCode, Reply, dispatch, dispatch_as};
use arrangia_publish::credentials::SecretStore;
use arrangia_publish::{ClientOptions, PollPolicy, RetryPolicy};
use aws_lc_rs::encoding::{AsDer, Pkcs8V1Der};
use aws_lc_rs::signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair};
use base64::Engine as _;
use serde_json::{Value, json};
use sha2::Digest as _;
use wiremock::matchers::any;
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

const ISSUER: &str = "57246542-96fe-1a63-e053-0824d011072a";
const PACKAGE: &str = "com.example.fernwood";

// ----- keys ---------------------------------------------------------------------------

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

fn sha256_hex(b: &[u8]) -> String {
    sha2::Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

// ----- fake App Store Connect ---------------------------------------------------------

#[derive(Debug, Clone)]
struct Shot {
    set: String,
    file_name: String,
    size: u64,
    checksum: Option<String>,
    state: String,
    received: u64,
}

#[derive(Debug, Default)]
struct Asc {
    base: String,
    editable: bool,
    /// (id, locale)
    locs: Vec<(String, String)>,
    /// (id, loc id, display type)
    sets: Vec<(String, String, String)>,
    shots: BTreeMap<String, Shot>,
    order: BTreeMap<String, Vec<String>>,
    next: u32,
    /// Reservations into sets of this locale fail with 409.
    fail_locale: Option<String>,
    /// Upload parts wait this long.
    part_delay: Option<Duration>,
    /// Requests seen: `METHOD path`.
    seen: Vec<String>,
    /// Reservations per locale.
    reserves: BTreeMap<String, u32>,
    /// Files (by name) Apple never finishes processing (`UPLOAD_COMPLETE`).
    stuck: Vec<String>,
    /// Files (by name) Apple fails to process (`FAILED` with errors).
    failing: Vec<String>,
}

impl Asc {
    fn id(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}{}", self.next)
    }

    fn shot_doc(&self, id: &str) -> Value {
        let s = &self.shots[id];
        let ops = if s.state == "AWAITING_UPLOAD" {
            json!([{"method": "PUT", "url": format!("{}/upload/{id}", self.base),
                    "length": s.size, "offset": 0, "requestHeaders": [
                        {"name": "Content-Type", "value": "image/png"}]}])
        } else {
            Value::Null
        };
        let errors = if s.state == "FAILED" {
            json!([{"code": "IMAGE_INCORRECT_DIMENSIONS",
                    "description": "The dimensions of one or more screenshots are wrong."}])
        } else {
            json!([])
        };
        json!({"type": "appScreenshots", "id": id, "attributes": {
            "fileName": s.file_name, "fileSize": s.size,
            "sourceFileChecksum": s.checksum,
            "assetDeliveryState": {"state": s.state, "errors": errors},
            "uploadOperations": ops}})
    }

    fn locale_of_set(&self, set: &str) -> Option<String> {
        let loc = &self.sets.iter().find(|(id, ..)| id == set)?.1;
        self.locs
            .iter()
            .find(|(id, _)| id == loc)
            .map(|(_, l)| l.clone())
    }

    /// Screenshot checksums of `locale`'s set of `display_type`, in order.
    fn checksums(&self, locale: &str, display_type: &str) -> Vec<String> {
        let Some((loc, _)) = self.locs.iter().find(|(_, l)| l == locale) else {
            return Vec::new();
        };
        let Some((set, ..)) = self
            .sets
            .iter()
            .find(|(_, l, d)| l == loc && d == display_type)
        else {
            return Vec::new();
        };
        self.order[set]
            .iter()
            .map(|id| self.shots[id].checksum.clone().unwrap_or_default())
            .collect()
    }

    /// Seeds `locale` with a set holding one old screenshot.
    fn seed(&mut self, locale: &str, display_type: &str) {
        let loc = self.id("loc-");
        self.locs.push((loc.clone(), locale.to_owned()));
        let set = self.id("set-");
        self.sets.push((set.clone(), loc, display_type.to_owned()));
        let shot = self.id("old-");
        self.shots.insert(
            shot.clone(),
            Shot {
                set: set.clone(),
                file_name: "old.png".into(),
                size: 10,
                checksum: Some("0".repeat(32)),
                state: "COMPLETE".into(),
                received: 10,
            },
        );
        self.order.insert(set, vec![shot]);
    }

    /// Ids of `locale`'s set of `display_type`, in order.
    fn ids(&self, locale: &str, display_type: &str) -> Vec<String> {
        let Some((loc, _)) = self.locs.iter().find(|(_, l)| l == locale) else {
            return Vec::new();
        };
        self.sets
            .iter()
            .find(|(_, l, d)| l == loc && d == display_type)
            .map(|(set, ..)| self.order[set].clone())
            .unwrap_or_default()
    }

    /// Lets Apple finish processing everything.
    fn finish_processing(&mut self) {
        self.stuck.clear();
        for s in self.shots.values_mut() {
            if s.state == "UPLOAD_COMPLETE" {
                s.state = "COMPLETE".into();
            }
        }
    }

    /// Puts a processed screenshot with `checksum` into `locale`'s set.
    fn seed_shot(&mut self, locale: &str, display_type: &str, checksum: &str) -> String {
        if self.ids(locale, display_type).is_empty() && !self.locs.iter().any(|(_, l)| l == locale)
        {
            self.seed(locale, display_type);
        }
        let loc = self
            .locs
            .iter()
            .find(|(_, l)| l == locale)
            .unwrap()
            .0
            .clone();
        let set = self
            .sets
            .iter()
            .find(|(_, l, d)| *l == loc && d == display_type)
            .unwrap()
            .0
            .clone();
        let id = self.id("shot-");
        self.shots.insert(
            id.clone(),
            Shot {
                set: set.clone(),
                file_name: "kept.png".into(),
                size: 10,
                checksum: Some(checksum.to_owned()),
                state: "COMPLETE".into(),
                received: 10,
            },
        );
        self.order.get_mut(&set).unwrap().push(id.clone());
        id
    }
}

#[derive(Clone)]
struct FakeAsc(Arc<Mutex<Asc>>);

fn ok(v: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(v)
}

impl Respond for FakeAsc {
    #[allow(clippy::too_many_lines)]
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let mut a = self.0.lock().unwrap();
        let method = req.method.as_str().to_owned();
        let path = req.url.path().to_owned();
        a.seen.push(format!("{method} {path}"));
        let seg: Vec<&str> = path.trim_start_matches('/').split('/').collect();
        let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
        match (method.as_str(), seg.as_slice()) {
            ("GET", ["v1", "apps", id]) => ok(json!({"data": {"type": "apps", "id": id,
                "attributes": {"name": "Fernwood", "bundleId": PACKAGE}}})),
            ("GET", ["v1", "apps", _, "appStoreVersions"]) => {
                let state = if a.editable {
                    "PREPARE_FOR_SUBMISSION"
                } else {
                    "READY_FOR_DISTRIBUTION"
                };
                ok(json!({"data": [{"type": "appStoreVersions", "id": "ver-1", "attributes": {
                    "platform": "IOS", "versionString": "1.0", "appVersionState": state}}]}))
            }
            ("GET", ["v1", "appStoreVersions", _, "appStoreVersionLocalizations"]) => {
                let data: Vec<Value> = a
                    .locs
                    .iter()
                    .map(|(id, l)| json!({"type": "appStoreVersionLocalizations", "id": id,
                                          "attributes": {"locale": l}}))
                    .collect();
                ok(json!({ "data": data }))
            }
            ("POST", ["v1", "appStoreVersionLocalizations"]) => {
                let locale = body["data"]["attributes"]["locale"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let id = a.id("loc-");
                a.locs.push((id.clone(), locale.clone()));
                ResponseTemplate::new(201).set_body_json(json!({"data": {
                    "type": "appStoreVersionLocalizations", "id": id,
                    "attributes": {"locale": locale}}}))
            }
            ("GET", ["v1", "appStoreVersionLocalizations", loc, "appScreenshotSets"]) => {
                let data: Vec<Value> = a
                    .sets
                    .iter()
                    .filter(|(_, l, _)| l == loc)
                    .map(|(id, _, d)| json!({"type": "appScreenshotSets", "id": id,
                                             "attributes": {"screenshotDisplayType": d}}))
                    .collect();
                ok(json!({ "data": data }))
            }
            ("POST", ["v1", "appScreenshotSets"]) => {
                let d = body["data"]["attributes"]["screenshotDisplayType"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let loc = body["data"]["relationships"]["appStoreVersionLocalization"]["data"]
                    ["id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let id = a.id("set-");
                a.sets.push((id.clone(), loc, d.clone()));
                a.order.insert(id.clone(), Vec::new());
                ResponseTemplate::new(201).set_body_json(json!({"data": {
                    "type": "appScreenshotSets", "id": id,
                    "attributes": {"screenshotDisplayType": d}}}))
            }
            ("GET", ["v1", "appScreenshotSets", set, "appScreenshots"]) => {
                let ids = a.order.get(*set).cloned().unwrap_or_default();
                let data: Vec<Value> = ids.iter().map(|id| a.shot_doc(id)).collect();
                ok(json!({ "data": data }))
            }
            ("PATCH", ["v1", "appScreenshotSets", set, "relationships", "appScreenshots"]) => {
                let ids: Vec<String> = body["data"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|d| d["id"].as_str().unwrap().to_owned())
                    .collect();
                a.order.insert((*set).to_owned(), ids);
                ResponseTemplate::new(204)
            }
            ("POST", ["v1", "appScreenshots"]) => {
                let set = body["data"]["relationships"]["appScreenshotSet"]["data"]["id"]
                    .as_str()
                    .unwrap()
                    .to_owned();
                let locale = a.locale_of_set(&set).unwrap_or_default();
                *a.reserves.entry(locale.clone()).or_default() += 1;
                if a.fail_locale.as_deref() == Some(locale.as_str()) {
                    return ResponseTemplate::new(409).set_body_json(json!({"errors": [{
                        "status": "409", "code": "ENTITY_ERROR.ATTRIBUTE.INVALID",
                        "title": "An attribute value is invalid.",
                        "detail": "The screenshot dimensions are invalid."}]}));
                }
                let id = a.id("shot-");
                let attrs = &body["data"]["attributes"];
                let shot = Shot {
                    set: set.clone(),
                    file_name: attrs["fileName"].as_str().unwrap().to_owned(),
                    size: attrs["fileSize"].as_u64().unwrap(),
                    checksum: None,
                    state: "AWAITING_UPLOAD".into(),
                    received: 0,
                };
                a.shots.insert(id.clone(), shot);
                a.order.entry(set).or_default().push(id.clone());
                ResponseTemplate::new(201).set_body_json(json!({ "data": a.shot_doc(&id) }))
            }
            ("PUT", ["upload", id]) => {
                let n = req.body.len() as u64;
                if let Some(s) = a.shots.get_mut(*id) {
                    s.received += n;
                }
                let r = ResponseTemplate::new(200);
                match a.part_delay {
                    Some(d) => r.set_delay(d),
                    None => r,
                }
            }
            ("PATCH", ["v1", "appScreenshots", id]) => {
                let checksum = body["data"]["attributes"]["sourceFileChecksum"]
                    .as_str()
                    .map(str::to_owned);
                let Some(name) = a.shots.get(*id).map(|s| s.file_name.clone()) else {
                    return ResponseTemplate::new(404);
                };
                let state = if a.stuck.contains(&name) {
                    "UPLOAD_COMPLETE"
                } else if a.failing.contains(&name) {
                    "FAILED"
                } else {
                    "COMPLETE"
                };
                let s = a.shots.get_mut(*id).unwrap();
                assert_eq!(s.received, s.size, "all bytes before the commit");
                s.checksum = checksum;
                s.state = state.into();
                ok(json!({ "data": a.shot_doc(id) }))
            }
            ("GET", ["v1", "appScreenshots", id]) => match a.shots.contains_key(*id) {
                true => ok(json!({ "data": a.shot_doc(id) })),
                false => ResponseTemplate::new(404),
            },
            ("DELETE", ["v1", "appScreenshots", id]) => {
                if let Some(s) = a.shots.remove(*id) {
                    if let Some(o) = a.order.get_mut(&s.set) {
                        o.retain(|x| x != id);
                    }
                }
                ResponseTemplate::new(204)
            }
            _ => ResponseTemplate::new(404).set_body_json(json!({"errors": [{
                "status": "404", "code": "NOT_FOUND", "title": format!("no route {method} {path}")}]})),
        }
    }
}

// ----- fake Google Play ---------------------------------------------------------------

type Listing = BTreeMap<(String, String), Vec<(String, String)>>;

#[derive(Debug, Default)]
struct Play {
    /// (lang, type) → [(id, sha256)]
    live: Listing,
    edits: BTreeMap<String, Listing>,
    next: u32,
    commits: u32,
    deleted_edits: u32,
    fail_lang: Option<String>,
}

#[derive(Clone)]
struct FakePlay(Arc<Mutex<Play>>);

impl Respond for FakePlay {
    fn respond(&self, req: &Request) -> ResponseTemplate {
        let mut p = self.0.lock().unwrap();
        let path = req.url.path().replace("%3A", ":");
        let method = req.method.as_str();
        if path == "/token" {
            return ok(json!({"access_token": "tok", "expires_in": 3600, "token_type": "Bearer"}));
        }
        let upload = path.starts_with("/upload/");
        let rest = path
            .trim_start_matches("/upload")
            .trim_start_matches(&format!("/androidpublisher/v3/applications/{PACKAGE}/"));
        let seg: Vec<&str> = rest.split('/').collect();
        match (method, seg.as_slice()) {
            ("POST", ["edits"]) => {
                p.next += 1;
                let id = format!("edit-{}", p.next);
                let live = p.live.clone();
                p.edits.insert(id.clone(), live);
                ok(json!({"id": id, "expiryTimeSeconds": "9999999999"}))
            }
            ("POST", ["edits", commit]) if commit.ends_with(":commit") => {
                let id = commit.trim_end_matches(":commit");
                let Some(e) = p.edits.remove(id) else {
                    return ResponseTemplate::new(404);
                };
                p.live = e;
                p.commits += 1;
                ok(json!({ "id": id }))
            }
            ("DELETE", ["edits", id]) => {
                p.edits.remove(*id);
                p.deleted_edits += 1;
                ResponseTemplate::new(204)
            }
            (_, ["edits", edit, "listings", lang, kind]) => {
                let key = ((*lang).to_owned(), (*kind).to_owned());
                if upload && method == "POST" {
                    if p.fail_lang.as_deref() == Some(*lang) {
                        return ResponseTemplate::new(400).set_body_json(json!({"error": {
                            "code": 400, "message": "Image is too large", "status": "INVALID_ARGUMENT"}}));
                    }
                    p.next += 1;
                    let id = format!("img-{}", p.next);
                    let sha = sha256_hex(&req.body);
                    p.edits
                        .get_mut(*edit)
                        .unwrap()
                        .entry(key)
                        .or_default()
                        .push((id.clone(), sha.clone()));
                    return ok(json!({"image": {"id": id, "sha256": sha}}));
                }
                let Some(e) = p.edits.get_mut(*edit) else {
                    return ResponseTemplate::new(404);
                };
                let images: Vec<Value> = e
                    .get(&key)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(id, sha)| json!({"id": id, "sha256": sha}))
                    .collect();
                match method {
                    "GET" => ok(json!({ "images": images })),
                    "DELETE" => {
                        e.remove(&key);
                        ok(json!({ "deleted": images }))
                    }
                    _ => ResponseTemplate::new(405),
                }
            }
            _ => ResponseTemplate::new(404),
        }
    }
}

// ----- setup --------------------------------------------------------------------------

const PROJECT: &str = r##"format: arrangia/1
revision: 0
name: "Upload test"
languages:
  default: en
  available: [en, de]
strips:
- id: main
  page: {aspect: 1320:2868, count: 2, ids: [one, two]}
  backgrounds:
  - "#EEF1F6"
  elements:
  - id: t1
    kind: text
    page: 0
    anchor_y: top
    text: one.title
    box: {top: 160, left: 80, right: 80}
    align: center
  - id: t2
    kind: text
    page: 1
    anchor_y: top
    text: two.title
    box: {top: 160, left: 80, right: 80}
    align: center
targets:
- id: iphone
  strip: main
  size: [1320, 2868]
  gallery: appstore-iphone
  store: {apple: {display_type: APP_IPHONE_67}}
- id: play
  strip: main
  size: [1080, 2160]
  gallery: play-phone
  store: {google: {image_type: phoneScreenshots}}
store:
  apple: {account: apple-team, app_id: "1234567890"}
  google: {account: google-play, package_name: com.example.fernwood}
"##;

struct Env {
    _tmp: tempfile::TempDir,
    app: Arc<AppState>,
    events: Arc<RecordingSink>,
    asc: Arc<Mutex<Asc>>,
    play: Arc<Mutex<Play>>,
    _servers: (MockServer, MockServer),
    dir: std::path::PathBuf,
}

fn fast_config(asc: &str, play: &str) -> UploadConfig {
    UploadConfig {
        apple_base_url: Some(asc.to_owned()),
        google_base_url: Some(play.to_owned()),
        google_token_url: Some(format!("{play}/token")),
        options: ClientOptions {
            retry: RetryPolicy {
                max_attempts: 1,
                ..RetryPolicy::default()
            },
            poll: PollPolicy {
                initial_interval: Duration::from_millis(5),
                max_interval: Duration::from_millis(20),
                timeout: Duration::from_secs(5),
            },
            request_timeout: Duration::from_secs(10),
            ..ClientOptions::default()
        },
        confirm_timeout: Duration::from_secs(10),
        ..UploadConfig::default()
    }
}

async fn env_with(project: &str) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let events = Arc::new(RecordingSink::default());
    let app = Arc::new(
        AppState::new(
            AppPaths::in_dir(&tmp.path().join("app")),
            events.clone(),
            Arc::new(NoDialogs),
        )
        .with_secret_store(SecretStore::mock())
        .without_watcher(),
    );
    let asc_server = MockServer::start().await;
    let play_server = MockServer::start().await;
    let asc = Arc::new(Mutex::new(Asc {
        base: asc_server.uri(),
        editable: true,
        ..Asc::default()
    }));
    let play = Arc::new(Mutex::new(Play::default()));
    Mock::given(any())
        .respond_with(FakeAsc(asc.clone()))
        .mount(&asc_server)
        .await;
    Mock::given(any())
        .respond_with(FakePlay(play.clone()))
        .mount(&play_server)
        .await;
    app.upload()
        .set_config(fast_config(&asc_server.uri(), &play_server.uri()));

    let p8 = tmp.path().join("AuthKey_ABCDE12345.p8");
    std::fs::write(&p8, ec_pem()).unwrap();
    call(
        &app,
        "store_account_import",
        json!({"kind": "apple", "path": p8, "issuer_id": ISSUER, "label": "team"}),
    );
    let sa = tmp.path().join("sa.json");
    std::fs::write(
        &sa,
        json!({"type": "service_account", "project_id": "demo", "private_key_id": "k1",
               "private_key": rsa_pem(), "client_email": "up@demo.iam.gserviceaccount.com"})
        .to_string(),
    )
    .unwrap();
    call(
        &app,
        "store_account_import",
        json!({"kind": "google", "path": sa, "label": "play"}),
    );

    let dir = tmp.path().join("project");
    std::fs::create_dir_all(dir.join("i18n")).unwrap();
    std::fs::write(dir.join("project.yaml"), project).unwrap();
    std::fs::write(
        dir.join("i18n/en.yaml"),
        "one.title: First page\ntwo.title: Second page\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("i18n/de.yaml"),
        "one.title: Erste Seite\ntwo.title: Zweite Seite\n",
    )
    .unwrap();
    call(&app, "project_open", json!({ "dir": dir }));
    Env {
        _tmp: tmp,
        app,
        events,
        asc,
        play,
        _servers: (asc_server, play_server),
        dir,
    }
}

async fn env() -> Env {
    env_with(PROJECT).await
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

fn plan(app: &AppState, args: Value) -> Value {
    call(app, "upload_plan", args)
}

fn set<'a>(plan: &'a Value, target: &str, lang: &str) -> &'a Value {
    plan["stores"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|s| s["sets"].as_array().unwrap())
        .find(|s| s["target"] == target && s["lang"] == lang)
        .unwrap_or_else(|| panic!("no set {target}/{lang}"))
}

fn execute(app: &AppState, token: &Value) -> Value {
    call(app, "upload_execute", json!({"token": token, "wait": true}))["run"].clone()
}

fn plan_images(plan: &Value, target: &str, lang: &str, key: &str) -> Vec<String> {
    set(plan, target, lang)["images"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i[key].as_str().unwrap().to_owned())
        .collect()
}

// ----- tests --------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plan_local_then_remote() {
    let e = env().await;
    e.asc.lock().unwrap().seed("en-US", "APP_IPHONE_67");

    let p = plan(&e.app, json!({}));
    assert_eq!(p["blocked"], false, "{p:#}");
    assert!(p["confirm_token"].as_str().unwrap().starts_with("upl_"));
    assert_eq!(p["totals"]["upload_sets"], 4);
    assert_eq!(p["totals"]["images"], 8);
    let s = set(&p, "iphone", "de");
    assert_eq!(s["locale"], "de-DE");
    assert_eq!(s["slot"], "APP_IPHONE_67");
    assert_eq!(s["action"], "new");
    assert_eq!(s["images"].as_array().unwrap().len(), 2);
    assert_eq!(s["images"][0]["index"], 1);
    assert_eq!(p["stores"][0]["flow"], "screenshot_sets");
    assert_eq!(p["stores"][1]["flow"], "edit");
    assert!(p["stores"][0].get("remote").is_none());
    assert!(
        e.asc.lock().unwrap().seen.is_empty(),
        "local plan: no network"
    );

    let r = plan(&e.app, json!({ "remote": true }));
    assert_eq!(r["blocked"], false, "{r:#}");
    assert_eq!(
        r["stores"][0]["remote"]["version"],
        "1.0 PREPARE_FOR_SUBMISSION"
    );
    let en = set(&r, "iphone", "en");
    assert_eq!(en["action"], "replace");
    assert_eq!(en["remote_count"], 1);
    let de = set(&r, "iphone", "de");
    assert_eq!(de["action"], "new");
    assert_eq!(de["steps"][0], "create_localization");
    assert_eq!(set(&r, "play", "en")["action"], "new");
    let play = e.play.lock().unwrap();
    assert_eq!(play.deleted_edits, 1, "the remote check discards its edit");
    assert_eq!(play.commits, 0);
    drop(play);
    // The store-only filter.
    let g = plan(&e.app, json!({ "store": "google", "langs": ["de"] }));
    assert_eq!(g["stores"].as_array().unwrap().len(), 1);
    assert_eq!(g["totals"]["sets"], 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn executes_both_stores_and_then_nothing_is_left() {
    let e = env().await;
    e.asc.lock().unwrap().seed("en-US", "APP_IPHONE_67");
    let p = plan(&e.app, json!({}));
    let run = execute(&e.app, &p["confirm_token"]);
    assert_eq!(run["status"], "completed", "{run:#}");
    assert_eq!(run["images_done"], 8);

    // Apple: old screenshot gone, new ones in plan order, de-DE created.
    let asc = e.asc.lock().unwrap();
    assert_eq!(
        asc.checksums("en-US", "APP_IPHONE_67"),
        plan_images(&p, "iphone", "en", "md5")
    );
    assert_eq!(
        asc.checksums("de-DE", "APP_IPHONE_67"),
        plan_images(&p, "iphone", "de", "md5")
    );
    assert!(
        asc.seen
            .iter()
            .any(|s| s.starts_with("DELETE /v1/appScreenshots/old-"))
    );
    assert!(
        asc.seen
            .iter()
            .any(|s| s == "POST /v1/appStoreVersionLocalizations")
    );
    assert!(
        asc.seen
            .iter()
            .any(|s| s.starts_with("PATCH /v1/appScreenshotSets/")
                && s.ends_with("/relationships/appScreenshots"))
    );
    drop(asc);
    // Google: one edit, committed, images in order.
    let play = e.play.lock().unwrap();
    assert_eq!(play.commits, 1);
    let live: Vec<String> = play.live[&("en-US".to_owned(), "phoneScreenshots".to_owned())]
        .iter()
        .map(|(_, h)| h.clone())
        .collect();
    assert_eq!(live, plan_images(&p, "play", "en", "sha256"));
    drop(play);

    // Progress events and the log.
    let progress = e.events.named("upload-progress");
    assert!(progress.iter().any(|v| v["step"] == "file_done"));
    assert!(progress.iter().any(|v| v["step"] == "commit"));
    assert_eq!(e.events.named("upload-finished").len(), 1);
    let log = call(&e.app, "upload_log", json!({ "events": true }));
    assert_eq!(log["runs"][0]["status"], "completed");
    assert_eq!(log["runs"][0]["sets"].as_array().unwrap().len(), 4);
    assert!(
        log["runs"][0]["sets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["status"] == "done")
    );
    let text = log.to_string();
    assert!(
        !text.contains("PRIVATE KEY") && !text.contains("Bearer") && !text.contains("access_token"),
        "no secrets in the log"
    );

    // Nothing changed: unchanged locally and in the store.
    let again = plan(&e.app, json!({}));
    assert_eq!(again["totals"]["upload_sets"], 0, "{again:#}");
    assert_eq!(again["totals"]["unchanged_sets"], 4);
    let remote = plan(&e.app, json!({ "remote": true }));
    assert_eq!(remote["totals"]["unchanged_sets"], 4, "{remote:#}");
    assert_eq!(
        set(&remote, "iphone", "en")["reason"],
        "the store already holds these images in this order"
    );
    let forced = plan(&e.app, json!({ "force": true }));
    assert_eq!(forced["totals"]["upload_sets"], 4);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn partial_failure_then_resume_skips_completed_sets() {
    let e = env().await;
    e.asc.lock().unwrap().fail_locale = Some("de-DE".into());
    let p = plan(&e.app, json!({ "store": "apple" }));
    let run = execute(&e.app, &p["confirm_token"]);
    assert_eq!(run["status"], "partial", "{run:#}");
    let sets = run["sets"].as_array().unwrap();
    let by_lang = |l: &str| sets.iter().find(|s| s["lang"] == l).unwrap();
    assert_eq!(by_lang("en")["status"], "done");
    assert_eq!(by_lang("de")["status"], "failed");
    assert!(
        by_lang("de")["error"]
            .as_str()
            .unwrap()
            .contains("dimensions are invalid")
    );
    let en_reserves = e.asc.lock().unwrap().reserves["en-US"];

    e.asc.lock().unwrap().fail_locale = None;
    let p2 = plan(&e.app, json!({ "store": "apple" }));
    assert_eq!(set(&p2, "iphone", "en")["action"], "unchanged");
    assert_eq!(set(&p2, "iphone", "de")["action"], "new");
    let run = execute(&e.app, &p2["confirm_token"]);
    assert_eq!(run["status"], "completed", "{run:#}");
    let asc = e.asc.lock().unwrap();
    assert_eq!(
        asc.reserves["en-US"], en_reserves,
        "en was not uploaded again"
    );
    assert_eq!(
        asc.checksums("de-DE", "APP_IPHONE_67"),
        plan_images(&p2, "iphone", "de", "md5")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn google_failure_discards_the_edit() {
    let e = env().await;
    e.play.lock().unwrap().fail_lang = Some("de-DE".into());
    let p = plan(&e.app, json!({ "store": "google" }));
    let run = execute(&e.app, &p["confirm_token"]);
    assert_eq!(run["status"], "failed", "{run:#}");
    let play = e.play.lock().unwrap();
    assert_eq!(play.commits, 0);
    assert_eq!(play.deleted_edits, 1);
    assert!(play.live.is_empty(), "nothing changed in Play");
    drop(play);
    assert!(
        run["sets"][0]["error"]
            .as_str()
            .unwrap()
            .contains("nothing was changed in Google Play")
    );
    // Nothing was logged as done: the next plan uploads both again.
    let p2 = plan(&e.app, json!({ "store": "google" }));
    assert_eq!(p2["totals"]["upload_sets"], 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_stops_the_run() {
    let e = env().await;
    e.asc.lock().unwrap().part_delay = Some(Duration::from_millis(1500));
    let p = plan(&e.app, json!({ "store": "apple" }));
    let started = call(
        &e.app,
        "upload_execute",
        json!({"token": p["confirm_token"]}),
    );
    assert_eq!(started["run"]["status"], "running");
    assert_eq!(
        fail(&e.app, "upload_execute", json!({"token": "x"})).code,
        ErrorCode::Busy
    );
    std::thread::sleep(Duration::from_millis(400));
    assert_eq!(
        call(&e.app, "upload_status", json!({}))["run"]["status"],
        "running"
    );
    assert_eq!(call(&e.app, "upload_cancel", json!({}))["cancelled"], true);
    let run = arrangia_app::commands::upload_wait(&e.app, Some(Duration::from_secs(10))).unwrap();
    let run = serde_json::to_value(run).unwrap();
    assert_eq!(run["status"], "cancelled", "{run:#}");
    assert!(
        run["sets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|s| s["status"] == "cancelled")
    );
    let log = call(&e.app, "upload_log", json!({}));
    assert_eq!(log["runs"][0]["status"], "cancelled");
    // The cancelled sets are not done: the next plan uploads them.
    assert_eq!(
        plan(&e.app, json!({ "store": "apple" }))["totals"]["upload_sets"],
        2
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tokens_expire_run_once_and_follow_the_project() {
    let e = env().await;
    // Unknown.
    let err = fail(&e.app, "upload_execute", json!({"token": "upl_nope"}));
    assert_eq!(
        (err.code, reason(&err).as_str()),
        (ErrorCode::NotConfirmed, "unknown")
    );
    // Changed project.
    let p = plan(&e.app, json!({ "store": "google" }));
    call(&e.app, "project_rename", json!({"name": "Renamed"}));
    let err = fail(
        &e.app,
        "upload_execute",
        json!({"token": p["confirm_token"]}),
    );
    assert_eq!(reason(&err), "changed", "{err:?}");
    // Changed image (a translation).
    let p = plan(&e.app, json!({ "store": "google" }));
    std::fs::write(
        e.dir.join("i18n/de.yaml"),
        "one.title: Erste Seite!\ntwo.title: Zweite Seite\n",
    )
    .unwrap();
    call(&e.app, "project_open", json!({ "dir": e.dir }));
    let err = fail(
        &e.app,
        "upload_execute",
        json!({"token": p["confirm_token"]}),
    );
    assert_eq!(reason(&err), "changed", "{err:?}");
    // Used once.
    let p = plan(&e.app, json!({ "store": "google" }));
    assert_eq!(execute(&e.app, &p["confirm_token"])["status"], "completed");
    let err = fail(
        &e.app,
        "upload_execute",
        json!({"token": p["confirm_token"]}),
    );
    assert_eq!(reason(&err), "used");
    // Expired.
    let mut cfg = e.app.upload().config();
    cfg.token_ttl = Duration::ZERO;
    e.app.upload().set_config(cfg);
    let p = plan(&e.app, json!({ "store": "google", "force": true }));
    std::thread::sleep(Duration::from_millis(1100));
    let err = fail(
        &e.app,
        "upload_execute",
        json!({"token": p["confirm_token"]}),
    );
    assert_eq!(reason(&err), "expired");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rule_violations_block_execution() {
    // A device without screenshot (validation error) and no Apple account.
    let project = PROJECT
        .replace(
            "  - id: t2\n",
            "  - id: phone\n    kind: device\n    page: 0\n    at: {x: 500, y: 1400}\n    width: 600\n    model: iphone-island\n    screenshot: home\n  - id: t2\n",
        )
        .replace("account: apple-team, ", "");
    let e = env_with(&project).await;
    let p = plan(&e.app, json!({}));
    assert_eq!(p["blocked"], true);
    let s = set(&p, "iphone", "en");
    assert_eq!(s["action"], "blocked");
    assert!(
        s["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["code"] == "screenshot_missing" && x["blocking"] == true),
        "{s:#}"
    );
    assert!(
        p["stores"][0]["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["code"] == "account_not_selected")
    );
    let err = fail(
        &e.app,
        "upload_execute",
        json!({"token": p["confirm_token"]}),
    );
    assert_eq!(reason(&err), "blocked");
    assert!(e.asc.lock().unwrap().seen.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_plan_reports_missing_editable_version() {
    let e = env().await;
    e.asc.lock().unwrap().editable = false;
    let p = plan(&e.app, json!({ "store": "apple", "remote": true }));
    assert_eq!(p["blocked"], true);
    assert!(
        p["stores"][0]["problems"]
            .as_array()
            .unwrap()
            .iter()
            .any(|x| x["code"] == "no_editable_version"),
        "{p:#}"
    );
}

// ----- MCP policy ---------------------------------------------------------------------

fn bot() -> Actor {
    Actor::assistant("claude-ai", None)
}

fn verified(app: &AppState, token: &Value) -> UploadPlan {
    arrangia_app::commands::upload_verify(app, token.as_str().unwrap()).unwrap()
}

/// Answers the next confirmation request like the GUI dialog would.
fn answer_in_gui(app: Arc<AppState>, events: Arc<RecordingSink>, approved: bool) {
    let before = events.named("upload-confirm-requested").len();
    std::thread::spawn(move || {
        for _ in 0..500 {
            if let Some(req) = events.named("upload-confirm-requested").get(before) {
                assert!(
                    req["summary"]
                        .as_str()
                        .unwrap()
                        .contains("set(s) to upload")
                );
                assert_eq!(req["client"], "Claude Desktop");
                let r = dispatch(
                    &app,
                    "upload_confirm",
                    json!({"id": req["id"], "approved": approved}),
                );
                assert!(r.is_ok());
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("no confirmation request");
    });
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_gui_confirmation_approve_deny_timeout() {
    let e = env().await;
    let p = plan(&e.app, json!({ "store": "google" }));
    let token = p["confirm_token"].as_str().unwrap().to_owned();
    let pl = verified(&e.app, &p["confirm_token"]);

    // Without the gate an assistant cannot execute.
    let err = match dispatch_as(&e.app, bot(), "upload_execute", json!({"token": token})) {
        Err(e) => e,
        Ok(_) => panic!("must not run"),
    };
    assert_eq!(reason(&err), "no_confirmation");
    // Assistants cannot answer the dialog.
    let err = match dispatch_as(
        &e.app,
        bot(),
        "upload_confirm",
        json!({"id": "x", "approved": true}),
    ) {
        Err(e) => e,
        Ok(_) => panic!("must not answer"),
    };
    assert_eq!(err.code, ErrorCode::Forbidden);

    // Denied.
    answer_in_gui(e.app.clone(), e.events.clone(), false);
    let err = bot()
        .scope(|| policy::authorize_mcp(&e.app, McpHost::Gui, &token, &pl))
        .unwrap_err();
    assert_eq!(
        (err.code, reason(&err).as_str()),
        (ErrorCode::NotConfirmed, "denied")
    );
    let resolved = e.events.named("upload-confirm-resolved");
    assert_eq!(resolved.last().unwrap()["outcome"], "denied");

    // Timeout.
    let mut cfg = e.app.upload().config();
    cfg.confirm_timeout = Duration::from_millis(200);
    e.app.upload().set_config(cfg.clone());
    let err = bot()
        .scope(|| policy::authorize_mcp(&e.app, McpHost::Gui, &token, &pl))
        .unwrap_err();
    assert_eq!(reason(&err), "timeout");
    assert!(
        call(&e.app, "upload_status", json!({}))["pending_confirmations"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    // Approved → the assistant's execute runs once.
    cfg.confirm_timeout = Duration::from_secs(10);
    e.app.upload().set_config(cfg);
    answer_in_gui(e.app.clone(), e.events.clone(), true);
    bot()
        .scope(|| policy::authorize_mcp(&e.app, McpHost::Gui, &token, &pl))
        .unwrap();
    let out = dispatch_as(
        &e.app,
        bot(),
        "upload_execute",
        json!({"token": token, "wait": true}),
    )
    .unwrap();
    let Reply::Json(out) = out else { panic!() };
    assert_eq!(out["run"]["status"], "completed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_headless_uploads_are_off_by_default() {
    let e = env().await;
    let p = plan(&e.app, json!({ "store": "google" }));
    let token = p["confirm_token"].as_str().unwrap().to_owned();
    let pl = verified(&e.app, &p["confirm_token"]);
    let err = bot()
        .scope(|| policy::authorize_mcp(&e.app, McpHost::Headless, &token, &pl))
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::Forbidden);
    assert!(err.message.contains("mcp_allow_headless_upload"));

    call(
        &e.app,
        "settings_set",
        json!({"settings": {"mcp_allow_headless_upload": true}}),
    );
    bot()
        .scope(|| policy::authorize_mcp(&e.app, McpHost::Headless, &token, &pl))
        .unwrap();
    assert!(e.events.named("upload-confirm-requested").is_empty());
    let Reply::Json(out) = dispatch_as(
        &e.app,
        bot(),
        "upload_execute",
        json!({"token": token, "wait": true}),
    )
    .unwrap() else {
        panic!()
    };
    assert_eq!(out["run"]["status"], "completed");
}

// ----- Apple processing (first real upload: hang after image 2) ------------------------

fn short_budget(e: &Env, budget: Duration) {
    let mut cfg = e.app.upload().config();
    cfg.processing_budget = budget;
    e.app.upload().set_config(cfg);
}

fn log_events(app: &AppState) -> Vec<Value> {
    call(app, "upload_log", json!({"events": true, "limit": 1}))["runs"][0]["events"]
        .as_array()
        .unwrap()
        .clone()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apple_processing_does_not_block_the_run() {
    let e = env().await;
    let p = plan(&e.app, json!({ "store": "apple" }));
    let en_names = plan_images(&p, "iphone", "en", "file_name");
    e.asc.lock().unwrap().stuck = vec![en_names[0].clone()];
    short_budget(&e, Duration::from_millis(1300));
    let t0 = std::time::Instant::now();
    let run = execute(&e.app, &p["confirm_token"]);
    assert!(t0.elapsed() < Duration::from_secs(8), "no hang");
    assert_eq!(run["status"], "completed", "{run:#}");
    assert_eq!(run["images_processing"], 1);
    let sets = run["sets"].as_array().unwrap();
    let by_lang = |l: &str| sets.iter().find(|s| s["lang"] == l).unwrap().clone();
    let en = by_lang("en");
    assert_eq!(en["status"], "uploaded_processing", "{en:#}");
    assert_eq!(
        (en["files_done"].as_u64(), en["files_processing"].as_u64()),
        (Some(1), Some(1))
    );
    assert!(
        en["note"]
            .as_str()
            .unwrap()
            .contains("still processing 1 image")
    );
    assert_eq!(by_lang("de")["status"], "done");
    assert!(
        run["message"]
            .as_str()
            .unwrap()
            .contains("still processing 1 image")
    );
    // All images are in the store in plan order (the stuck one too).
    assert_eq!(
        e.asc.lock().unwrap().checksums("en-US", "APP_IPHONE_67"),
        plan_images(&p, "iphone", "en", "md5")
    );
    // Progress shows the processing step with the state and the time.
    let progress = e.events.named("upload-progress");
    let proc = progress
        .iter()
        .find(|v| v["step"] == "processing")
        .expect("processing event");
    assert_eq!(proc["file"]["state"], "UPLOAD_COMPLETE");
    assert!(proc["file"]["elapsed_secs"].is_u64());
    for step in [
        "file_started",
        "transfer",
        "file_uploaded",
        "file_done",
        "set_order",
    ] {
        assert!(progress.iter().any(|v| v["step"] == step), "missing {step}");
    }

    // The log: set + file states, transitions instead of every poll, and
    // the real time of each event (not one flush time).
    let log = call(&e.app, "upload_log", json!({"events": true, "limit": 1}));
    let run_log = &log["runs"][0];
    assert_eq!(run_log["status"], "completed");
    let en_log = run_log["sets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["lang"] == "en")
        .unwrap();
    assert_eq!(en_log["status"], "uploaded_processing");
    let statuses: Vec<&str> = en_log["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["processing", "done"]);
    let events = log_events(&e.app);
    let polls = events
        .iter()
        .filter(|v| {
            let m = v["message"].as_str().unwrap();
            m.starts_with("GET ") && m.contains("/v1/appScreenshots/")
        })
        .count();
    assert_eq!(polls, 0, "successful polls are not logged one by one");
    assert!(events.iter().any(|v| {
        v["message"]
            .as_str()
            .unwrap()
            .contains("UPLOAD_COMPLETE after")
    }));
    let at: Vec<u64> = events.iter().map(|v| v["at"].as_u64().unwrap()).collect();
    assert!(at.windows(2).all(|w| w[0] <= w[1]), "{at:?}");
    assert!(
        at.last().unwrap() > at.first().unwrap(),
        "events carry their own time: {at:?}"
    );

    // Re-plan (local): the processing set is not uploaded again.
    let p2 = plan(&e.app, json!({ "store": "apple" }));
    let en2 = set(&p2, "iphone", "en");
    assert_eq!(en2["action"], "unchanged", "{en2:#}");
    assert!(en2["reason"].as_str().unwrap().contains("still processing"));
    assert_eq!(en2["problems"][0]["code"], "processing_unconfirmed");
    // Remote, still processing: unchanged, with the count.
    let r = plan(&e.app, json!({ "store": "apple", "remote": true }));
    let en3 = set(&r, "iphone", "en");
    assert_eq!(en3["action"], "unchanged", "{en3:#}");
    assert_eq!(en3["processing"], 1);
    assert!(en3["problems"].as_array().unwrap().is_empty());
    // Apple finished: plain unchanged.
    e.asc.lock().unwrap().finish_processing();
    let r = plan(&e.app, json!({ "store": "apple", "remote": true }));
    let en4 = set(&r, "iphone", "en");
    assert_eq!(en4["action"], "unchanged");
    assert!(en4.get("processing").is_none());
    assert_eq!(r["totals"]["upload_sets"], 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apple_failed_processing_fails_the_set_with_details() {
    let e = env().await;
    let p = plan(&e.app, json!({ "store": "apple" }));
    let de_names = plan_images(&p, "iphone", "de", "file_name");
    e.asc.lock().unwrap().failing = vec![de_names[1].clone()];
    let run = execute(&e.app, &p["confirm_token"]);
    assert_eq!(run["status"], "partial", "{run:#}");
    let de = run["sets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["lang"] == "de")
        .unwrap();
    assert_eq!(de["status"], "failed");
    let err = de["error"].as_str().unwrap();
    assert!(err.contains("IMAGE_INCORRECT_DIMENSIONS"), "{err}");
    assert!(err.contains(&de_names[1]), "{err}");
    // The failed asset is deleted; the good one stays.
    assert_eq!(e.asc.lock().unwrap().ids("de-DE", "APP_IPHONE_67").len(), 1);
    let log = call(&e.app, "upload_log", json!({"limit": 1}));
    let files = log["runs"][0]["sets"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["lang"] == "de")
        .unwrap()["files"]
        .clone();
    assert!(
        files
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f["status"] == "failed" && f["error"].as_str().unwrap().contains("dimensions"))
    );
    // Next run: the good image is kept, only the failed one is uploaded.
    e.asc.lock().unwrap().failing.clear();
    let before = e.asc.lock().unwrap().reserves["de-DE"];
    let p2 = plan(&e.app, json!({ "store": "apple", "remote": true }));
    let de2 = set(&p2, "iphone", "de");
    assert_eq!(de2["action"], "replace");
    assert_eq!(de2["reusable"], 1);
    let run = execute(&e.app, &p2["confirm_token"]);
    assert_eq!(run["status"], "completed", "{run:#}");
    assert_eq!(e.asc.lock().unwrap().reserves["de-DE"], before + 1);
    assert_eq!(
        e.asc.lock().unwrap().checksums("de-DE", "APP_IPHONE_67"),
        plan_images(&p2, "iphone", "de", "md5")
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_works_while_apple_is_processing() {
    let e = env().await;
    let p = plan(&e.app, json!({ "store": "apple", "langs": ["en"] }));
    let names = plan_images(&p, "iphone", "en", "file_name");
    e.asc.lock().unwrap().stuck = names.clone();
    short_budget(&e, Duration::from_secs(60));
    call(
        &e.app,
        "upload_execute",
        json!({"token": p["confirm_token"]}),
    );
    let mut waited = 0;
    while !e
        .events
        .named("upload-progress")
        .iter()
        .any(|v| v["step"] == "processing")
    {
        std::thread::sleep(Duration::from_millis(20));
        waited += 1;
        assert!(waited < 500, "never reached processing");
    }
    let t0 = std::time::Instant::now();
    assert_eq!(call(&e.app, "upload_cancel", json!({}))["cancelled"], true);
    let run = arrangia_app::commands::upload_wait(&e.app, Some(Duration::from_secs(10))).unwrap();
    assert!(t0.elapsed() < Duration::from_secs(3), "cancel is immediate");
    let run = serde_json::to_value(run).unwrap();
    assert_eq!(run["status"], "cancelled", "{run:#}");
    // Re-plan with the remote check: the uploaded images are kept.
    e.asc.lock().unwrap().finish_processing();
    let r = plan(
        &e.app,
        json!({ "store": "apple", "langs": ["en"], "remote": true }),
    );
    assert_eq!(set(&r, "iphone", "en")["action"], "unchanged", "{r:#}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replan_keeps_images_already_in_the_store() {
    let e = env().await;
    let p = plan(&e.app, json!({ "store": "apple", "langs": ["en"] }));
    let md5 = plan_images(&p, "iphone", "en", "md5");
    // The store holds image 1 (done) and an old one; image 2 is missing.
    let kept = e
        .asc
        .lock()
        .unwrap()
        .seed_shot("en-US", "APP_IPHONE_67", &md5[0]);
    let r = plan(
        &e.app,
        json!({ "store": "apple", "langs": ["en"], "remote": true }),
    );
    let s = set(&r, "iphone", "en");
    assert_eq!(s["action"], "replace");
    assert_eq!(s["reusable"], 1);
    assert!(
        s["reason"]
            .as_str()
            .unwrap()
            .contains("already in the store")
    );
    let run = execute(&e.app, &r["confirm_token"]);
    assert_eq!(run["status"], "completed", "{run:#}");
    assert!(run["sets"][0]["note"].as_str().unwrap().contains("kept"));
    let asc = e.asc.lock().unwrap();
    assert_eq!(
        asc.reserves["en-US"], 1,
        "only the missing image is uploaded"
    );
    assert_eq!(asc.checksums("en-US", "APP_IPHONE_67"), md5);
    assert_eq!(asc.ids("en-US", "APP_IPHONE_67")[0], kept);
    drop(asc);
    // --force uploads everything again.
    let f = plan(
        &e.app,
        json!({ "store": "apple", "langs": ["en"], "remote": true, "force": true }),
    );
    assert!(set(&f, "iphone", "en").get("reusable").is_none());
    execute(&e.app, &f["confirm_token"]);
    assert_eq!(e.asc.lock().unwrap().reserves["en-US"], 3);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_running_run_is_interrupted() {
    let e = env().await;
    let log = arrangia_app::upload::log::UploadLog::open(&e.dir).unwrap();
    let old = log.start_run("upl_x", 1_000).unwrap();
    log.start_set(old, "apple", "1", "de-DE", "S", "iphone", "de", &[], 1_000)
        .unwrap();
    drop(log);
    let runs = call(&e.app, "upload_log", json!({}));
    let r = &runs["runs"][0];
    assert_eq!(r["status"], "interrupted", "{r:#}");
    assert_eq!(r["sets"][0]["status"], "interrupted");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn watchdog_stops_a_run_without_progress() {
    let e = env().await;
    e.asc.lock().unwrap().part_delay = Some(Duration::from_secs(4));
    let mut cfg = e.app.upload().config();
    cfg.stall_timeout = Duration::from_millis(400);
    cfg.heartbeat_interval = Duration::from_millis(50);
    cfg.stall_grace = Duration::from_secs(5);
    e.app.upload().set_config(cfg);
    let p = plan(&e.app, json!({ "store": "apple", "langs": ["en"] }));
    let t0 = std::time::Instant::now();
    let run = execute(&e.app, &p["confirm_token"]);
    assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    assert_eq!(run["status"], "failed", "{run:#}");
    assert!(
        run["message"].as_str().unwrap().contains("no progress"),
        "{run:#}"
    );
    let log = call(&e.app, "upload_log", json!({"events": true, "limit": 1}));
    assert_eq!(log["runs"][0]["status"], "failed");
    assert!(
        log["runs"][0]["events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["message"].as_str().unwrap().starts_with("watchdog:"))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_editable_version_is_reported_once() {
    let e = env().await;
    e.asc.lock().unwrap().editable = false;
    // Remote plan: one store-level problem with values and a link, and a
    // readiness step.
    let r = plan(&e.app, json!({ "store": "apple", "remote": true }));
    let probs = r["stores"][0]["problems"].as_array().unwrap();
    let p = probs
        .iter()
        .find(|x| x["code"] == "no_editable_version")
        .unwrap();
    assert_eq!(
        p["message"],
        "App Store Connect: No editable App Store version. Create a new version (e.g. 1.0.1) in App Store Connect, then plan again. Latest version: 1.0 (Ready for Distribution)."
    );
    assert_eq!(p["vars"]["latest"], "1.0");
    assert_eq!(p["vars"]["state"], "Ready for Distribution");
    assert_eq!(p["vars"]["suggestion"], "1.0.1");
    assert_eq!(
        p["link"],
        "https://appstoreconnect.apple.com/apps/1234567890/distribution"
    );
    let step = r["readiness"]["steps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["code"] == "no_editable_version")
        .unwrap();
    assert_eq!(step["fix"], "app_store_connect");
    assert_eq!(r["readiness"]["ready"], false);
    for s in r["stores"][0]["sets"].as_array().unwrap() {
        assert!(
            s["problems"].as_array().unwrap().is_empty(),
            "not per set: {s:#}"
        );
    }
    // Execution (local plan): fail the store once, sets skipped.
    let p = plan(&e.app, json!({ "store": "apple" }));
    let run = execute(&e.app, &p["confirm_token"]);
    assert_eq!(run["status"], "failed", "{run:#}");
    let fails = run["store_failures"].as_array().unwrap();
    assert_eq!(fails.len(), 1);
    assert_eq!(fails[0]["code"], "no_editable_version");
    for s in run["sets"].as_array().unwrap() {
        assert_eq!(s["status"], "skipped");
        assert_eq!(s["error"], "no editable App Store version");
    }
    let text = run.to_string();
    assert_eq!(
        text.matches("Latest version").count(),
        2,
        "message + summary only: {text}"
    );
}
