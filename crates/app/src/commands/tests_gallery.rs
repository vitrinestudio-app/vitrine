//! Gallery commands against a fake API (wiremock, no network): list with
//! installed versions, preview image, download + safe import + install,
//! wrong package refused, unreachable server.

use std::sync::Arc;

use serde_json::{Value, json};
use vitrine_core::Project;
use vitrine_core::template::extract::{SaveOptions, extract};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::*;
use crate::dialogs::NoDialogs;
use crate::error::ErrorCode;
use crate::events::RecordingSink;

fn package(name: &str) -> (Vec<u8>, Vec<u8>) {
    let root = crate::testutil::example_fixture();
    let project = Project::load(&root).unwrap();
    let opts = SaveOptions {
        name: name.into(),
        canvases: Some(vec!["mac".into()]),
        ..SaveOptions::default()
    };
    let mut pkg = extract(&project, &root, &opts).unwrap().0;
    vitrine_render::template::add_previews(&mut pkg, 200).unwrap();
    let preview = pkg.preview("mac").unwrap().to_vec();
    (pkg.to_zip_bytes().unwrap(), preview)
}

fn app_with(tmp: &std::path::Path, url: &str) -> AppState {
    AppState::new(
        AppPaths::in_dir(&tmp.join("app")),
        Arc::new(RecordingSink::default()),
        Arc::new(NoDialogs),
    )
    .without_watcher()
    .with_gallery_url(url)
}

fn template_json(id: &str, base: &str) -> Value {
    json!({
        "id": id, "name": "Fernwood Mac", "version": "1.0.0", "license": "CC0-1.0",
        "uploader": "vitrinestudio-app", "official": true, "status": "approved",
        "canvases": [{"id": "mac", "aspect": "16:10", "pages": 3, "targets": ["mac"],
                      "preview": "previews/mac.png",
                      "preview_url": format!("{base}/v1/templates/{id}/previews/mac")}],
        "fonts": [], "size": 1, "downloads": 0, "created_at": 1, "updated_at": 1,
        "download_url": format!("{base}/v1/templates/{id}/download"),
        "a_future_field": 1
    })
}

#[test]
fn list_preview_download_install() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let server = rt.block_on(MockServer::start());
    let base = server.uri();
    let (zip, png) = package("Fernwood Mac");
    rt.block_on(async {
        Mock::given(method("GET"))
            .and(path("/v1/templates"))
            .and(query_param("q", "fern"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "templates": [template_json("fernwood-mac", &base)], "limit": 50, "offset": 0
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/templates/fernwood-mac"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(template_json("fernwood-mac", &base)),
            )
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/templates/fernwood-mac/previews/mac"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(png.clone()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/templates/fernwood-mac/download"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(zip.clone()))
            .mount(&server)
            .await;
        // A server sending another template than asked for.
        Mock::given(method("GET"))
            .and(path("/v1/templates/other-id/download"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(zip.clone()))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/v1/templates/broken/download"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"not a zip".to_vec()))
            .mount(&server)
            .await;
    });

    let tmp = tempfile::tempdir().unwrap();
    let app = app_with(tmp.path(), &base);
    let list = |app: &AppState| match dispatch(app, "gallery_list", json!({"query": "fern"})) {
        Ok(Reply::Json(v)) => v,
        other => panic!("{other:?}"),
    };
    let v = list(&app);
    assert_eq!(v["templates"][0]["id"], "fernwood-mac");
    assert_eq!(v["templates"][0]["official"], true);
    assert_eq!(v["templates"][0]["installed_version"], Value::Null);
    assert_eq!(v["has_more"], false);

    let Ok(Reply::Image(img)) = dispatch(&app, "gallery_preview", json!({"id": "fernwood-mac"}))
    else {
        panic!("no preview")
    };
    assert!(img.width > 0 && img.data.starts_with(b"\x89PNG"));

    let Ok(Reply::Json(t)) = dispatch(&app, "gallery_download", json!({"id": "fernwood-mac"}))
    else {
        panic!("download failed")
    };
    assert_eq!(t["id"], "fernwood-mac");
    assert_eq!(t["source"], "installed");
    assert_eq!(list(&app)["templates"][0]["installed_version"], "1.0.0");
    // Installed again only with `replace`.
    let e = dispatch(&app, "gallery_download", json!({"id": "fernwood-mac"})).unwrap_err();
    assert_eq!(e.code, ErrorCode::Io);
    assert!(
        dispatch(
            &app,
            "gallery_download",
            json!({"id": "fernwood-mac", "replace": true})
        )
        .is_ok()
    );

    let e = dispatch(&app, "gallery_download", json!({"id": "other-id"})).unwrap_err();
    assert_eq!(e.code, ErrorCode::Remote, "{e:?}");
    let e = dispatch(&app, "gallery_download", json!({"id": "broken"})).unwrap_err();
    assert_eq!(e.code, ErrorCode::InvalidArgs, "{e:?}");
    let e = dispatch(&app, "gallery_download", json!({"id": "missing"})).unwrap_err();
    assert_eq!(e.code, ErrorCode::NotFound, "{e:?}");
    drop(rt);
}

#[test]
fn unreachable_server_is_a_quiet_error() {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with(tmp.path(), &format!("http://127.0.0.1:{port}"));
    for (name, args) in [
        ("gallery_list", json!({})),
        ("gallery_download", json!({"id": "x"})),
        ("gallery_preview", json!({"id": "x", "canvas": "phone"})),
    ] {
        let e = dispatch(&app, name, args).unwrap_err();
        assert_eq!(e.code, ErrorCode::Unreachable, "{name}: {e:?}");
    }
}
