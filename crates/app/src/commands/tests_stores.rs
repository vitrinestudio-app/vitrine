//! Store accounts (mock keychain) and the project store mapping.

use std::sync::Arc;

use arrangia_publish::credentials::{SecretStore, sample_p8_for_tests};
use serde_json::{Value, json};

use super::*;
use crate::dialogs::NoDialogs;
use crate::events::RecordingSink;
use crate::stores::{ConnectionError, ConnectionInfo, ConnectionTester, StoreAccount};
use crate::testutil::example_copy;

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
        Ok(_) => panic!("{name} should fail"),
    }
}

fn state(dir: &std::path::Path) -> AppState {
    AppState::new(
        AppPaths::in_dir(dir),
        Arc::new(RecordingSink::default()),
        Arc::new(NoDialogs),
    )
    .with_secret_store(SecretStore::mock())
    .without_watcher()
}

const ISSUER: &str = "57246542-96fe-1a63-e053-0824d011072a";

#[test]
fn import_list_test_and_remove_accounts() {
    let tmp = tempfile::tempdir().unwrap();
    let app = &state(&tmp.path().join("app"));
    let p8 = tmp.path().join("AuthKey_2X9R4HXF34.p8");
    std::fs::write(&p8, sample_p8_for_tests()).unwrap();
    let json_key = tmp.path().join("play.json");
    let key = sample_p8_for_tests();
    std::fs::write(
        &json_key,
        json!({
            "type": "service_account", "project_id": "demo-1", "private_key": key,
            "client_email": "uploader@demo-1.iam.gserviceaccount.com"
        })
        .to_string(),
    )
    .unwrap();

    let list = call(app, "store_accounts_list", json!({}));
    assert_eq!(list["accounts"], json!([]));
    assert_eq!(list["keychain"]["service"], "app.arrangia");
    assert_eq!(list["keychain"]["backend"], "mock");

    // Apple: key id from the file name, issuer id required.
    let e = fail(
        app,
        "store_account_import",
        json!({"kind": "apple", "path": p8}),
    );
    assert_eq!(e.code, ErrorCode::InvalidArgs);
    let a = call(
        app,
        "store_account_import",
        json!({"kind": "apple", "path": p8, "label": "My Team", "issuer_id": ISSUER}),
    );
    assert_eq!(a["id"], "apple-my-team");
    assert_eq!(a["apple"]["key_id"], "2X9R4HXF34");
    let g = call(
        app,
        "store_account_import",
        json!({"kind": "google", "path": json_key}),
    );
    assert_eq!(g["id"], "google-uploader-demo-1-iam-gserviceaccount-com");
    assert_eq!(g["google"]["project_id"], "demo-1");

    // Wrong file for the kind; nothing is added.
    let e = fail(
        app,
        "store_account_import",
        json!({"kind": "google", "path": p8}),
    );
    assert_eq!(e.code, ErrorCode::InvalidArgs);
    assert!(
        !e.message.contains("MIGHAgEA"),
        "no key material: {}",
        e.message
    );

    // The settings file holds no secrets.
    let yaml = std::fs::read_to_string(app.paths().settings_file()).unwrap();
    assert!(yaml.contains("store_accounts:"), "{yaml}");
    assert!(yaml.contains(ISSUER));
    assert!(
        !yaml.contains("PRIVATE") && !yaml.contains("MIGHAgEA"),
        "{yaml}"
    );
    let list = call(app, "store_accounts_list", json!({}));
    assert_eq!(list["accounts"].as_array().unwrap().len(), 2);
    assert!(!list.to_string().contains("MIGHAgEA"));

    // The secrets are in the (mock) keychain.
    let secrets = app.secret_store().unwrap();
    assert_eq!(
        secrets
            .get("apple-my-team", arrangia_core::model::StoreKind::Apple)
            .unwrap()
            .expose(),
        sample_p8_for_tests()
    );

    // Without store clients: not implemented, after checking the secret.
    let e = fail(app, "store_account_test", json!({"id": "apple-my-team"}));
    assert_eq!(e.code, ErrorCode::NotImplemented);
    assert_eq!(e.details, Some(json!({"secret": "ok"})));

    // settings_set cannot touch the accounts.
    let e = fail(
        app,
        "settings_set",
        json!({"settings": {"store_accounts": []}}),
    );
    assert_eq!(e.code, ErrorCode::InvalidArgs);

    // Remove deletes the keychain entry.
    call(app, "store_account_remove", json!({"id": "apple-my-team"}));
    assert!(
        secrets
            .get("apple-my-team", arrangia_core::model::StoreKind::Apple)
            .is_err()
    );
    let e = fail(app, "store_account_test", json!({"id": "apple-my-team"}));
    assert_eq!(e.code, ErrorCode::NotFound);
    assert_eq!(
        call(app, "store_accounts_list", json!({}))["accounts"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

struct FakeTester;

impl ConnectionTester for FakeTester {
    fn test(
        &self,
        account: &StoreAccount,
        _: &arrangia_publish::credentials::Secret,
    ) -> Result<ConnectionInfo, ConnectionError> {
        Ok(ConnectionInfo {
            summary: format!("hello {}", account.label),
            ..ConnectionInfo::default()
        })
    }
}

#[test]
fn connection_test_uses_the_tester() {
    let tmp = tempfile::tempdir().unwrap();
    let app = &state(&tmp.path().join("app"));
    app.set_connection_tester(Arc::new(FakeTester));
    let p8 = tmp.path().join("key.p8");
    std::fs::write(&p8, sample_p8_for_tests()).unwrap();
    let a = call(
        app,
        "store_account_import",
        json!({"kind": "apple", "path": p8, "issuer_id": ISSUER, "key_id": "ABCDE12345"}),
    );
    assert_eq!(a["label"], "Apple ABCDE12345");
    let r = call(app, "store_account_test", json!({"id": a["id"]}));
    assert_eq!(r["ok"], true);
    assert_eq!(r["summary"], "hello Apple ABCDE12345");
}

#[test]
fn project_store_get_and_patch() {
    let (tmp, dir) = example_copy();
    let app = &state(&tmp.path().join("app"));
    call(app, "project_open", json!({"dir": dir}));
    let info = call(app, "project_store_get", json!({}));
    assert_eq!(info["store"], json!({}));
    let de = info["effective"]["apple"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["lang"] == "de")
        .unwrap()
        .clone();
    assert_eq!(de["locale"], "de-DE");
    assert_eq!(de["source"], "default");
    assert!(info["locales"]["apple"].as_array().unwrap().len() >= 50);

    let r = call(
        app,
        "project_store_patch",
        json!({
            "apple": {"app_id": "1234567890", "bundle_id": " com.example.app ", "account": "nope"},
            "locales": {"apple": {"de": "de-DE", "en": "en-GB", "fr": ""}},
        }),
    );
    assert_eq!(r["applied"], true);
    let info = call(app, "project_store_get", json!({}));
    assert_eq!(info["store"]["apple"]["bundle_id"], "com.example.app");
    assert_eq!(
        info["store"]["locales"]["apple"],
        json!({"de": "de-DE", "en": "en-GB"})
    );
    assert_eq!(info["missing_accounts"][0]["problem"], "unknown");
    let yaml = std::fs::read_to_string(dir.join("project.yaml")).unwrap();
    assert!(yaml.contains("store:"), "{yaml}");

    // Unknown locale codes and bad ids are rejected; the project is unchanged.
    let e = fail(
        app,
        "project_store_patch",
        json!({"locales": {"apple": {"de": "de-XX"}}}),
    );
    assert_eq!(e.code, ErrorCode::OpRejected);
    let e = fail(
        app,
        "project_store_patch",
        json!({"google": {"package_name": "nodots"}}),
    );
    assert_eq!(e.code, ErrorCode::OpRejected);

    // Same values: no step. Undo restores.
    let r = call(
        app,
        "project_store_patch",
        json!({"apple": {"app_id": "1234567890", "bundle_id": "com.example.app", "account": "nope"}}),
    );
    assert_eq!(r["applied"], false);
    call(app, "project_store_patch", json!({"apple": null}));
    assert_eq!(
        call(app, "project_store_get", json!({}))["store"].get("apple"),
        None
    );
    call(app, "undo", json!({}));
    assert_eq!(
        call(app, "project_store_get", json!({}))["store"]["apple"]["app_id"],
        "1234567890"
    );

    // Read-only overview without secrets and locale lists.
    let c = call(app, "store_config_get", json!({}));
    assert!(c["project"]["locales"].is_null());
    assert_eq!(c["project"]["store"]["apple"]["app_id"], "1234567890");
}

fn codes(r: &Value) -> Vec<String> {
    r["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["code"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn readiness_walks_from_app_settings_to_targets() {
    let (tmp, dir) = example_copy();
    let app = &state(&tmp.path().join("app"));
    call(app, "project_open", json!({ "dir": dir }));

    // Nothing set up: app settings first, then the project.
    let r = call(app, "upload_readiness", json!({}));
    assert_eq!(r["ready"], false);
    assert_eq!(codes(&r), ["no_account", "project_not_mapped"]);
    assert_eq!(r["steps"][0]["fix"], "app_settings_stores");
    assert_eq!(r["steps"][1]["fix"], "project_settings_stores");
    assert!(r["steps"][0]["store"].is_null());

    // An Apple key on this machine: only the project step is left.
    let p8 = tmp.path().join("AuthKey_2X9R4HXF34.p8");
    std::fs::write(&p8, sample_p8_for_tests()).unwrap();
    call(
        app,
        "store_account_import",
        json!({"kind": "apple", "path": p8, "label": "Team", "issuer_id": ISSUER}),
    );
    let r = call(app, "upload_readiness", json!({"store": "apple"}));
    assert_eq!(codes(&r), ["project_not_mapped"]);
    assert_eq!(r["steps"][0]["store"], "apple");

    // Mapped without account/app → the project step names both.
    call(app, "project_store_patch", json!({"apple": {}}));
    let r = call(app, "upload_readiness", json!({"store": "apple"}));
    assert_eq!(codes(&r), ["project_not_mapped"]);
    let msg = r["steps"][0]["message"].as_str().unwrap();
    assert!(
        msg.contains("the account") && msg.contains("Apple ID"),
        "{msg}"
    );

    // An account id that is not on this machine.
    call(
        app,
        "project_store_patch",
        json!({"apple": {"account": "apple-elsewhere", "app_id": "1234567890"}}),
    );
    let r = call(app, "upload_readiness", json!({"store": "apple"}));
    assert_eq!(codes(&r), ["account_missing_for_project"]);
    assert_eq!(r["steps"][0]["fix"], "app_settings_stores");

    // Fully mapped: ready (the example has Apple slots).
    call(
        app,
        "project_store_patch",
        json!({"apple": {"account": "apple-team", "app_id": "1234567890"}}),
    );
    let r = call(app, "upload_readiness", json!({"store": "apple"}));
    assert_eq!(r["ready"], true, "{r:#}");

    // Two languages on one store language → project settings.
    call(
        app,
        "project_store_patch",
        json!({"locales": {"apple": {"fr-CA": "fr-FR"}}}),
    );
    let r = call(app, "upload_readiness", json!({"store": "apple"}));
    assert_eq!(codes(&r), ["store_locale_conflict"]);
    call(
        app,
        "project_store_patch",
        json!({"locales": {"apple": {}}}),
    );

    // Google mapped, but no Google account and no Google slot left.
    call(
        app,
        "project_store_patch",
        json!({"google": {"package_name": "com.example.app"}}),
    );
    let targets: Vec<String> = call(app, "project_get", json!({}))["project"]["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["store"]["google"].is_object())
        .map(|t| t["id"].as_str().unwrap().to_owned())
        .collect();
    assert_ne!(targets.len(), 0, "the example has Google targets");
    for t in &targets {
        call(
            app,
            "target_patch",
            json!({"id": t, "patch": {"store": null}}),
        );
    }
    let r = call(app, "upload_readiness", json!({"store": "google"}));
    assert_eq!(
        codes(&r),
        ["no_account", "project_not_mapped", "no_store_slots"],
        "{r:#}"
    );
    assert_eq!(r["steps"][2]["fix"], "targets");

    // The plan carries the same checklist.
    let plan = call(app, "upload_plan", json!({"store": "google"}));
    assert_eq!(codes(&plan["readiness"]), codes(&r));
}

#[test]
fn open_url_only_opens_help_links() {
    let tmp = tempfile::tempdir().unwrap();
    let dialogs = crate::dialogs::FixedDialogs::default();
    let opened = dialogs.opened.clone();
    let app = AppState::new(
        AppPaths::in_dir(&tmp.path().join("app")),
        Arc::new(RecordingSink::default()),
        Arc::new(dialogs),
    )
    .with_secret_store(SecretStore::mock())
    .without_watcher();
    for url in HELP_URLS {
        assert!(url.starts_with("https://"));
        call(&app, "open_url", json!({ "url": url }));
    }
    assert_eq!(opened.lock().unwrap().len(), HELP_URLS.len());
    for bad in [
        "https://example.com",
        "http://appstoreconnect.apple.com/access/integrations/api",
        "https://appstoreconnect.apple.com/access/integrations/api?x=1",
        "file:///etc/passwd",
        "https://appstoreconnect.apple.com/apps/12a/distribution",
        "https://appstoreconnect.apple.com/apps//distribution",
        "https://appstoreconnect.apple.com/apps/123/distribution/../x",
    ] {
        let e = fail(&app, "open_url", json!({ "url": bad }));
        assert_eq!(e.code, ErrorCode::Forbidden, "{bad}");
    }
    assert_eq!(opened.lock().unwrap().len(), HELP_URLS.len());
    // An app's version page in App Store Connect (no editable version).
    call(
        &app,
        "open_url",
        json!({ "url": "https://appstoreconnect.apple.com/apps/6757819650/distribution" }),
    );
    assert_eq!(opened.lock().unwrap().len(), HELP_URLS.len() + 1);
    // Without a browser (NoDialogs) the GUI is told so.
    let plain = state(&tmp.path().join("app2"));
    let e = fail(&plain, "open_url", json!({ "url": HELP_URLS[0] }));
    assert_eq!(e.code, ErrorCode::NotImplemented);
}

#[test]
fn app_picker_and_package_check_check_their_arguments() {
    assert!(is_valid_package_name("com.example.app"));
    assert!(is_valid_package_name("de.looking4cache.app_2"));
    for bad in [
        "app",
        "com..x",
        "1com.x",
        "com.example-app",
        "com.example.",
        "",
    ] {
        assert!(!is_valid_package_name(bad), "{bad}");
    }
    let tmp = tempfile::tempdir().unwrap();
    let app = &state(&tmp.path().join("app"));
    let e = fail(app, "store_apps_list", json!({"account": "nope"}));
    assert_eq!(e.code, ErrorCode::NotFound);
    let p8 = tmp.path().join("AuthKey_2X9R4HXF34.p8");
    std::fs::write(&p8, sample_p8_for_tests()).unwrap();
    call(
        app,
        "store_account_import",
        json!({"kind": "apple", "path": p8, "label": "Team", "issuer_id": ISSUER}),
    );
    // An Apple account cannot check a Play package (no network reached).
    let e = fail(
        app,
        "store_package_check",
        json!({"account": "apple-team", "package_name": "com.example.app"}),
    );
    assert_eq!(e.code, ErrorCode::InvalidArgs);
    let e = fail(
        app,
        "store_package_check",
        json!({"account": "apple-team", "package_name": "not a package"}),
    );
    assert_eq!(e.code, ErrorCode::InvalidArgs);
}
