//! `vitrine gallery …` against the API server in-process (127.0.0.1) and a
//! fake GitHub (wiremock): list, upload with a static token from an env
//! variable, download, device-flow login into the mock keychain, logout,
//! unreachable server. No network beyond loopback.

use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use vitrine_api_server::{AppState, Config, StaticToken, serve};
use vitrine_core::Project;
use vitrine_core::template::TemplateTags;
use vitrine_core::template::extract::{SaveOptions, extract};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Env {
    _rt: tokio::runtime::Runtime,
    api: String,
    tmp: tempfile::TempDir,
}

fn start() -> Env {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let github = rt.block_on(async {
        let github = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/login/device/code"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "device_code": "dev-1", "user_code": "WDJB-MJHT",
                "verification_uri": "https://github.com/login/device",
                "expires_in": 60, "interval": 1
            })))
            .mount(&github)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"error": "authorization_pending"})),
            )
            .up_to_n_times(1)
            .with_priority(1)
            .mount(&github)
            .await;
        Mock::given(method("POST"))
            .and(path("/login/oauth/access_token"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"access_token": "gho_t"})),
            )
            .with_priority(2)
            .mount(&github)
            .await;
        Mock::given(method("GET"))
            .and(path("/user"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(json!({"login": "octocat", "id": 1})),
            )
            .mount(&github)
            .await;
        github
    });
    let config = Config {
        data_dir: tmp.path().join("server"),
        auto_approve: true,
        admin_token: Some("admin-secret".into()),
        github_url: github.uri(),
        github_api_url: github.uri(),
        upload_tokens: vec![StaticToken {
            token: "static-official".into(),
            login: "vitrinestudio-app".into(),
            github_id: None,
        }],
        ..Config::default()
    };
    let state = AppState::open(config).unwrap();
    let listener = rt
        .block_on(tokio::net::TcpListener::bind("127.0.0.1:0"))
        .unwrap();
    let addr = listener.local_addr().unwrap();
    rt.spawn(async move {
        serve(listener, state, std::future::pending())
            .await
            .unwrap();
    });
    // Keep the fake GitHub alive with the runtime.
    rt.spawn(async move {
        let _github = github;
        std::future::pending::<()>().await;
    });
    Env {
        _rt: rt,
        api: format!("http://{addr}"),
        tmp,
    }
}

fn vitrine(env: &Env, args: &[&str], vars: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_vitrine"));
    cmd.args(args)
        .current_dir(env.tmp.path())
        .env("VITRINE_KEYRING", "mock")
        .env("VITRINE_APP_DIR", env.tmp.path().join("app"))
        .env_remove("VITRINE_API_URL")
        .env("NO_COLOR", "1");
    for (k, v) in vars {
        cmd.env(k, v);
    }
    cmd.output().unwrap()
}

fn text(o: &Output) -> (String, String) {
    (
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn write_package(dir: &Path) -> std::path::PathBuf {
    write_template(dir, "Fernwood Mac", "mac", true, false)
}

fn write_template(
    dir: &Path,
    name: &str,
    canvas: &str,
    tagged: bool,
    samples: bool,
) -> std::path::PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/example");
    let project = Project::load(&root).unwrap();
    let tags = if tagged {
        TemplateTags {
            style: vec!["minimal".into()],
            color: vec!["light".into()],
            use_: vec!["productivity".into()],
            ..TemplateTags::default()
        }
    } else {
        TemplateTags::default()
    };
    let opts = SaveOptions {
        name: name.into(),
        canvases: Some(vec![canvas.into()]),
        tags,
        sample_screenshots: samples,
        ..SaveOptions::default()
    };
    let (pkg, _) = extract(&project, &root, &opts).unwrap();
    let file = dir.join(format!("{}.vitrine-template", pkg.manifest.id));
    std::fs::write(&file, pkg.to_zip_bytes().unwrap()).unwrap();
    file
}

#[test]
#[allow(clippy::too_many_lines)]
fn list_upload_download_login_logout() {
    let env = start();
    let api = env.api.as_str();

    let o = vitrine(&env, &["gallery", "list", "--api", api], &[]);
    assert!(o.status.success(), "{:?}", text(&o));
    assert!(text(&o).0.contains("no templates found"));

    // Upload: no session in this process's (mock) keychain → clear error.
    let file = write_package(env.tmp.path());
    let file = file.to_str().unwrap();
    let o = vitrine(&env, &["gallery", "upload", file, "--api", api], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o).1.contains("not logged in"), "{:?}", text(&o));
    let o = vitrine(
        &env,
        &[
            "gallery",
            "upload",
            file,
            "--api",
            api,
            "--token-env",
            "VITRINE_TEST_UNSET",
        ],
        &[],
    );
    assert_eq!(o.status.code(), Some(2));
    // Static token from an environment variable.
    let o = vitrine(
        &env,
        &[
            "gallery",
            "upload",
            file,
            "--api",
            api,
            "--token-env",
            "VITRINE_TEST_TOKEN",
        ],
        &[("VITRINE_TEST_TOKEN", "static-official")],
    );
    assert!(o.status.success(), "{:?}", text(&o));
    assert!(text(&o).0.contains("uploaded (approved)"), "{:?}", text(&o));
    let o = vitrine(
        &env,
        &[
            "gallery",
            "upload",
            file,
            "--api",
            api,
            "--token-env",
            "VITRINE_TEST_TOKEN",
        ],
        &[("VITRINE_TEST_TOKEN", "wrong")],
    );
    assert_eq!(o.status.code(), Some(1));
    assert!(text(&o).0.contains("token was refused"), "{:?}", text(&o));
    assert!(
        !text(&o).0.contains("wrong") && !text(&o).1.contains("wrong"),
        "the token is never printed"
    );

    // List (via the loopback env override instead of --api), JSON.
    let o = vitrine(
        &env,
        &["gallery", "list", "--query", "fernwood"],
        &[("VITRINE_API_URL", api)],
    );
    let (out, _) = text(&o);
    assert!(
        out.contains("fernwood-mac") && out.contains("[official]"),
        "{out}"
    );
    let o = vitrine(
        &env,
        &["gallery", "list", "--api", api, "--format", "json"],
        &[],
    );
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["templates"][0]["official"], true);
    assert_eq!(v["templates"][0]["uploader"], "vitrinestudio-app");

    // Download, checked.
    let out_file = env.tmp.path().join("dl.vitrine-template");
    let o = vitrine(
        &env,
        &[
            "gallery",
            "download",
            "fernwood-mac",
            "--api",
            api,
            "-o",
            out_file.to_str().unwrap(),
        ],
        &[],
    );
    assert!(o.status.success(), "{:?}", text(&o));
    assert!(out_file.is_file());
    let o = vitrine(&env, &["gallery", "download", "nope", "--api", api], &[]);
    assert_eq!(o.status.code(), Some(2));

    // Device-flow login (fake GitHub: pending once, then confirmed).
    let o = vitrine(&env, &["gallery", "login", "--api", api], &[]);
    let (out, err) = text(&o);
    assert!(o.status.success(), "{out}{err}");
    assert!(
        err.contains("WDJB-MJHT") && err.contains("https://github.com/login/device"),
        "{err}"
    );
    assert!(out.contains("logged in as octocat"), "{out}");
    assert!(
        !out.contains("vss_") && !err.contains("vss_"),
        "the session token is never printed"
    );

    // Logout in a new process: the mock keychain is per process, so there
    // is no session to revoke.
    let o = vitrine(&env, &["gallery", "logout", "--api", api], &[]);
    assert!(o.status.success());
    assert!(text(&o).0.contains("not logged in"));
}

#[test]
fn unreachable_and_invalid_urls() {
    let env = start();
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let o = vitrine(
        &env,
        &[
            "gallery",
            "list",
            "--api",
            &format!("http://127.0.0.1:{port}"),
        ],
        &[],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o).1.contains("not reachable"), "{:?}", text(&o));
    let o = vitrine(
        &env,
        &["gallery", "list", "--api", "http://example.com"],
        &[],
    );
    assert_eq!(o.status.code(), Some(2));
    assert!(text(&o).1.contains("https"), "{:?}", text(&o));
}

#[test]
fn bulk_upload_dry_run_samples_and_admin() {
    let env = start();
    let api = env.api.as_str();
    let dir = env.tmp.path();
    let good = write_template(dir, "Bulk Mac", "mac", true, false);
    let untagged = write_template(dir, "Bulk Untagged", "mac", false, false);
    let samples = write_template(dir, "Bulk Phone", "phone", true, true);
    let broken = dir.join("broken.vitrine-template");
    std::fs::write(&broken, b"not a zip").unwrap();
    let files: Vec<&str> = [&good, &untagged, &samples, &broken]
        .iter()
        .map(|p| p.to_str().unwrap())
        .collect();

    // Dry run: all local checks, nothing sent, no login needed.
    let mut args = vec!["gallery", "upload", "--api", api, "--dry-run"];
    args.extend(&files);
    let o = vitrine(&env, &args, &[]);
    let (out, err) = text(&o);
    assert_eq!(o.status.code(), Some(1), "{out}{err}");
    assert!(out.contains("bulk-mac") && out.contains("ok"), "{out}");
    assert!(out.contains("at least 1 style tag"), "{out}");
    assert!(out.contains("--confirm-own-screenshots"), "{out}");
    assert!(out.contains("not a valid template"), "{out}");
    assert!(
        out.contains("dry run: 4 files 1 ready to upload, 3 failed"),
        "{out}"
    );
    let o = vitrine(
        &env,
        &["gallery", "list", "--api", api, "--format", "json"],
        &[],
    );
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["templates"].as_array().unwrap().len(), 0, "nothing sent");

    // Real run with confirmation: continues after errors.
    let mut args = vec![
        "gallery",
        "upload",
        "--api",
        api,
        "--confirm-own-screenshots",
        "--token-env",
        "VITRINE_TEST_TOKEN",
        "--format",
        "json",
    ];
    args.extend(&files);
    let o = vitrine(&env, &args, &[("VITRINE_TEST_TOKEN", "static-official")]);
    assert_eq!(o.status.code(), Some(1));
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["failed"], 2);
    assert_eq!(v["files"][0]["result"], "uploaded");
    assert_eq!(v["files"][2]["result"], "uploaded");
    assert!(v["files"][2]["samples"].as_u64().unwrap() > 0);

    // Filters.
    let o = vitrine(
        &env,
        &[
            "gallery",
            "list",
            "--api",
            api,
            "--devices",
            "phone",
            "--format",
            "json",
        ],
        &[],
    );
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["templates"].as_array().unwrap().len(), 1);
    assert_eq!(v["templates"][0]["id"], "bulk-phone");
    assert!(v["templates"][0]["samples"].as_u64().unwrap() > 0);
    assert!(v["templates"][0]["canvases"][0]["sprite"]["url"].is_string());

    // Admin: token only from the environment.
    let o = vitrine(&env, &["gallery", "admin", "queue", "--api", api], &[]);
    assert_eq!(o.status.code(), Some(2));
    assert!(
        text(&o).1.contains("VITRINE_API_ADMIN_TOKEN"),
        "{:?}",
        text(&o)
    );
    let admin = [("VITRINE_API_ADMIN_TOKEN", "admin-secret")];
    let o = vitrine(&env, &["gallery", "admin", "queue", "--api", api], &admin);
    assert!(o.status.success(), "{:?}", text(&o));
    assert!(text(&o).0.contains("no pending templates"));
    let o = vitrine(
        &env,
        &["gallery", "admin", "rerender", "bulk-mac", "--api", api],
        &admin,
    );
    assert!(o.status.success(), "{:?}", text(&o));
    let o = vitrine(
        &env,
        &[
            "gallery", "admin", "reject", "bulk-mac", "nope", "--note", "test", "--api", api,
        ],
        &admin,
    );
    assert_eq!(o.status.code(), Some(1));
    let (out, err) = text(&o);
    assert!(
        out.contains("ok bulk-mac reject") && err.contains("nope"),
        "{out}{err}"
    );
    assert!(!out.contains("admin-secret") && !err.contains("admin-secret"));
    let o = vitrine(
        &env,
        &[
            "gallery",
            "admin",
            "approve",
            "bulk-mac",
            "--api",
            api,
            "--admin-token-env",
            "MY_ADMIN",
        ],
        &[("MY_ADMIN", "admin-secret")],
    );
    assert!(o.status.success(), "{:?}", text(&o));
}
